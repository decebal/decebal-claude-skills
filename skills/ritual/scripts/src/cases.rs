use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::feedback::{Check, Skill, Step};
use crate::store::Record;

/// Share of cases, newest first, held out from the mutator and scored only as a gate.
const HOLDOUT_SHARE: f64 = 0.3;

/// A feedback event that carries a check, frozen for one weekly run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub recorded_at: String,
    pub skill: Skill,
    pub step: Step,
    pub reference: String,
    pub check: Check,
    pub note: String,
}

impl Case {
    /// The file the check reads, relative to the repository root.
    pub fn target(&self) -> String {
        format!("skills/{}/{}", self.skill.dir(), self.reference)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub dev_count: usize,
    pub holdout_count: usize,
    pub dev_sha256: String,
    pub holdout_sha256: String,
    pub records_read: usize,
    pub records_unreadable: usize,
    /// Cases whose target file does not exist where they would be scored; no candidate
    /// can close them, since the mutator may not create files.
    pub dropped_missing_target: usize,
}

pub fn cases_from(records: &[Record]) -> Vec<Case> {
    records
        .iter()
        .filter_map(|r| {
            let check = r.feedback.check.clone()?;
            Some(Case {
                id: r.id.clone(),
                recorded_at: r.timestamp.to_rfc3339(),
                skill: r.feedback.skill,
                step: r.feedback.step,
                reference: r.feedback.reference.clone()?,
                check,
                note: r.feedback.note.clone(),
            })
        })
        .collect()
}

/// Splits cases already in record order: the newest `HOLDOUT_SHARE` become the holdout.
pub fn split(cases: Vec<Case>) -> (Vec<Case>, Vec<Case>) {
    let holdout = (cases.len() as f64 * HOLDOUT_SHARE).floor() as usize;
    let mut dev = cases;
    let held = dev.split_off(dev.len() - holdout);
    (dev, held)
}

pub fn to_jsonl(cases: &[Case]) -> String {
    cases
        .iter()
        .map(|c| serde_json::to_string(c).expect("case serializes") + "\n")
        .collect()
}

pub fn read_jsonl(path: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn export(
    records: &[Record],
    unreadable: usize,
    out_dir: &Path,
    target_exists: impl Fn(&Case) -> bool,
) -> Result<ExportManifest, String> {
    let all = cases_from(records);
    let total = all.len();
    let scorable: Vec<Case> = all.into_iter().filter(|c| target_exists(c)).collect();
    let dropped_missing_target = total - scorable.len();
    let (dev, holdout) = split(scorable);
    fs::create_dir_all(out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;
    let dev_text = to_jsonl(&dev);
    let holdout_text = to_jsonl(&holdout);
    let manifest = ExportManifest {
        dev_count: dev.len(),
        holdout_count: holdout.len(),
        dev_sha256: sha256_hex(dev_text.as_bytes()),
        holdout_sha256: sha256_hex(holdout_text.as_bytes()),
        records_read: records.len(),
        records_unreadable: unreadable,
        dropped_missing_target,
    };
    for (name, body) in [
        ("dev.jsonl", dev_text),
        ("holdout.jsonl", holdout_text),
        (
            "manifest.json",
            serde_json::to_string_pretty(&manifest).expect("manifest serializes") + "\n",
        ),
    ] {
        let path = out_dir.join(name);
        fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    }
    Ok(manifest)
}

pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// A case is open when its target file is missing, lacks the `must` phrase, or
/// contains the `must_not` phrase. Matching ignores case and whitespace runs.
pub fn is_open(root: &Path, case: &Case) -> bool {
    let Ok(text) = fs::read_to_string(root.join(case.target())) else {
        return true;
    };
    let text = normalize(&text);
    let missing = case
        .check
        .must
        .as_deref()
        .is_some_and(|m| !text.contains(&normalize(m)));
    let present = case
        .check
        .must_not
        .as_deref()
        .is_some_and(|m| text.contains(&normalize(m)));
    missing || present
}

pub fn open_cases<'a>(root: &Path, cases: &'a [Case]) -> Vec<&'a Case> {
    cases.iter().filter(|c| is_open(root, c)).collect()
}
