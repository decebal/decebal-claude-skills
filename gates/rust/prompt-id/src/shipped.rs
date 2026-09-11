//! Which prompts at the store root reserve a branch that has already merged.
//!
//! That is the exact signal that caught two prompts sitting unarchived for days
//! after shipping. One `gh` call for the whole store: a call per prompt turns a
//! cheap check into a rate-limited one, and the matching is local anyway.

use crate::branch;
use crate::store::Entry;
use std::path::Path;
use std::process::Command;

/// A merged pull request, reduced to what matching needs.
pub struct Merged {
    pub number: String,
    pub branch: String,
    pub merged_at: String,
}

/// One prompt at the store root and the branch it reserves.
pub struct Reserved {
    pub file_name: String,
    pub branch: Option<String>,
}

/// The prompt text a branch is declared in.
///
/// A meta-prompt is a directory of stages, so its declaration is inside; the
/// stage carrying the same number is the one that names the branch.
pub fn body(root: &Path, entry: &Entry) -> Option<String> {
    let path = root.join(&entry.file_name);
    if !entry.is_dir {
        return std::fs::read_to_string(path).ok();
    }
    let mut stages: Vec<_> = std::fs::read_dir(&path)
        .ok()?
        .flatten()
        .map(|i| i.path())
        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
        .collect();
    stages.sort();
    let mut joined = String::new();
    for stage in stages {
        if let Ok(text) = std::fs::read_to_string(&stage) {
            joined.push_str(&text);
            joined.push('\n');
        }
    }
    (!joined.is_empty()).then_some(joined)
}

pub fn reserved(root: &Path, entries: &[Entry]) -> Vec<Reserved> {
    entries
        .iter()
        .filter(|e| !e.archived)
        .map(|e| Reserved {
            file_name: e.file_name.clone(),
            branch: body(root, e).as_deref().and_then(branch::reserved),
        })
        .collect()
}

/// Every merged pull request, newest first, in one call.
///
/// `--jq` is `gh`'s own gojq, so the JSON never has to be parsed here and this
/// crate keeps its zero dependencies.
pub fn merged(repo: &Path, limit: u32) -> Result<Vec<Merged>, String> {
    let out = Command::new("gh")
        .current_dir(repo)
        .args([
            "pr",
            "list",
            "--state",
            "merged",
            "--limit",
            &limit.to_string(),
            "--json",
            "number,headRefName,mergedAt",
            "--jq",
            r#".[] | [.number, .headRefName, .mergedAt] | @tsv"#,
        ])
        .output()
        .map_err(|e| format!("could not run gh: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(parse(&String::from_utf8_lossy(&out.stdout)))
}

/// Rows of `number<TAB>branch<TAB>merged_at`, skipping anything malformed.
pub fn parse(tsv: &str) -> Vec<Merged> {
    tsv.lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let number = cols.next()?.trim();
            let branch = cols.next()?.trim();
            let merged_at = cols.next().unwrap_or("").trim();
            (!number.is_empty() && !branch.is_empty()).then(|| Merged {
                number: number.to_string(),
                branch: branch.to_string(),
                merged_at: merged_at.to_string(),
            })
        })
        .collect()
}

/// The merged PR a branch belongs to, newest wins.
pub fn find<'a>(merged: &'a [Merged], branch: &str) -> Option<&'a Merged> {
    merged.iter().find(|m| m.branch == branch)
}

/// A merge date as the day alone — the time of day answers nothing here.
pub fn day(merged_at: &str) -> &str {
    merged_at.split('T').next().unwrap_or(merged_at)
}
