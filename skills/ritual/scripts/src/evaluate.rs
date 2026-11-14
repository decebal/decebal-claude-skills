use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::cases::{self, Case};

/// The skill directories a candidate may change, relative to the repository root.
pub const SKILL_DIRS: [&str; 2] = ["skills/ritual", "skills/ritual-full"];
/// Inside a mutable skill directory but never prose: the evaluator's own source.
pub const PROTECTED_DIRS: [&str; 1] = ["skills/ritual/scripts"];

pub const OBJECTIVE: &str = "open_feedback_cases";
pub const DIAGNOSTIC: &str = "total_words";
pub const GATES: [&str; 5] = [
    "holdout_not_regressed",
    "word_budget",
    "invariants_kept",
    "links_resolve",
    "frontmatter_valid",
];
/// Rules no candidate may remove, one `<skill>/<file>: <phrase>` per line. It sits under
/// `PROTECTED_DIRS`, so a candidate cannot edit the list to pass the gate.
pub const INVARIANTS: &str = "skills/ritual/scripts/invariants.txt";

/// Frozen inputs, passed as literal evaluator args in the contract.
#[derive(Debug, Clone)]
pub struct Frozen {
    pub dev: PathBuf,
    pub holdout: PathBuf,
    pub holdout_sha256: String,
    pub holdout_max_open: usize,
    pub max_words: usize,
}

#[derive(Debug, Deserialize)]
struct Request {
    evaluator_id: String,
    run_id: String,
    baseline_commit: String,
    evaluated_commit: String,
    candidate_worktree: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Score {
    pub open_dev: usize,
    pub total_words: usize,
    pub holdout_not_regressed: bool,
    pub word_budget: bool,
    pub invariants_kept: bool,
    pub links_resolve: bool,
    pub frontmatter_valid: bool,
}

/// Parsed invariants as `(target relative to the repository root, phrase)`, or `None`
/// when the file is missing, empty or has a malformed line.
pub fn invariants(root: &Path) -> Option<Vec<(String, String)>> {
    let text = fs::read_to_string(root.join(INVARIANTS)).ok()?;
    let mut out = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (target, phrase) = line.split_once(": ")?;
        crate::feedback::validate_reference(target).ok()?;
        out.push((format!("skills/{target}"), phrase.trim().to_string()));
    }
    (!out.is_empty()).then_some(out)
}

pub fn invariants_kept(root: &Path) -> bool {
    invariants(root).is_some_and(|list| {
        list.iter().all(|(target, phrase)| {
            fs::read_to_string(root.join(target))
                .is_ok_and(|t| cases::normalize(&t).contains(&cases::normalize(phrase)))
        })
    })
}

/// Every prose file in the skill directories, sorted, excluding the protected source tree.
pub fn prose_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for dir in SKILL_DIRS {
        collect_md(root, &root.join(dir), &mut files);
    }
    files.sort();
    files
}

fn collect_md(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if PROTECTED_DIRS.iter().any(|p| rel.starts_with(p)) {
            continue;
        }
        if path.is_dir() {
            collect_md(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

pub fn total_words(root: &Path) -> usize {
    prose_files(root)
        .iter()
        .filter_map(|p| fs::read_to_string(p).ok())
        .map(|t| t.split_whitespace().count())
        .sum()
}

/// Relative Markdown link targets in `text`, anchors stripped.
pub fn relative_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else { break };
        let target = rest[..end].trim();
        rest = &rest[end..];
        let external = target.is_empty()
            || target.starts_with('#')
            || target.contains("://")
            || target.starts_with("mailto:");
        if !external {
            let path = target.split('#').next().unwrap_or(target);
            links.push(path.to_string());
        }
    }
    links
}

pub fn links_resolve(root: &Path) -> bool {
    prose_files(root).iter().all(|file| {
        let Ok(text) = fs::read_to_string(file) else {
            return false;
        };
        let dir = file.parent().unwrap_or(root);
        relative_links(&text)
            .iter()
            .all(|link| dir.join(link).exists())
    })
}

pub fn frontmatter_valid(root: &Path) -> bool {
    SKILL_DIRS.iter().all(|dir| {
        let name = dir.rsplit('/').next().unwrap_or(dir);
        let Ok(text) = fs::read_to_string(root.join(dir).join("SKILL.md")) else {
            return false;
        };
        let Some(body) = text.strip_prefix("---\n") else {
            return false;
        };
        let Some(end) = body.find("\n---") else {
            return false;
        };
        let front = &body[..end];
        front.lines().any(|l| l == format!("name: {name}"))
            && front
                .lines()
                .any(|l| l.starts_with("description: ") && l.len() > "description: ".len())
    })
}

pub fn score(
    root: &Path,
    dev: &[Case],
    holdout: &[Case],
    frozen: &Frozen,
    holdout_intact: bool,
) -> Score {
    let total_words = total_words(root);
    Score {
        open_dev: cases::open_cases(root, dev).len(),
        total_words,
        holdout_not_regressed: holdout_intact
            && cases::open_cases(root, holdout).len() <= frozen.holdout_max_open,
        word_budget: total_words <= frozen.max_words,
        invariants_kept: invariants_kept(root),
        links_resolve: links_resolve(root),
        frontmatter_valid: frontmatter_valid(root),
    }
}

fn gate(name: &str, passed: bool) -> Value {
    json!({"kind": "hard_gate", "name": name, "outcome": {"passed": passed, "detail": null}})
}

/// The JSONL v1 success line. Gate details stay null so `verify` compares exactly and
/// no holdout count leaks to the mutator.
pub fn success_line(req_ids: [&str; 4], score: &Score) -> String {
    let [evaluator_id, run_id, baseline_commit, evaluated_commit] = req_ids;
    let measurements = vec![
        gate(GATES[0], score.holdout_not_regressed),
        gate(GATES[1], score.word_budget),
        gate(GATES[2], score.invariants_kept),
        gate(GATES[3], score.links_resolve),
        gate(GATES[4], score.frontmatter_valid),
        json!({"kind": "numeric", "name": OBJECTIVE, "metric_kind": "objective",
               "direction": "minimize", "value": score.open_dev as f64}),
        json!({"kind": "numeric", "name": DIAGNOSTIC, "metric_kind": "diagnostic",
               "direction": "minimize", "value": score.total_words as f64}),
    ];
    json!({"protocol_version": 1, "result": {"status": "success", "output": {
        "evaluator_id": evaluator_id,
        "run_id": run_id,
        "baseline_commit": baseline_commit,
        "evaluated_commit": evaluated_commit,
        "measurements": measurements,
        "observations": [],
        "artifacts": [],
        "warnings": [],
    }}})
    .to_string()
}

pub fn failure_line(detail: &str) -> String {
    json!({"protocol_version": 1, "result": {"status": "failure",
        "failure": {"class": "reported", "detail": detail}}})
    .to_string()
}

/// Runs one evaluation from the request on stdin and returns the line for stdout.
/// A broken input is a reported failure; a bad candidate is a failed gate.
pub fn run(frozen: &Frozen) -> String {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        return failure_line(&format!("read stdin: {e}"));
    }
    let Some(line) = input.lines().find(|l| !l.trim().is_empty()) else {
        return failure_line("empty request");
    };
    let req: Request = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => return failure_line(&format!("request: {e}")),
    };
    let dev = match cases::read_jsonl(&frozen.dev) {
        Ok(c) => c,
        Err(e) => return failure_line(&e),
    };
    let holdout_bytes = fs::read(&frozen.holdout).unwrap_or_default();
    let holdout_intact = cases::sha256_hex(&holdout_bytes) == frozen.holdout_sha256;
    let holdout = if holdout_intact {
        cases::read_jsonl(&frozen.holdout).unwrap_or_default()
    } else {
        Vec::new()
    };
    let score = score(
        &req.candidate_worktree,
        &dev,
        &holdout,
        frozen,
        holdout_intact,
    );
    success_line(
        [
            &req.evaluator_id,
            &req.run_id,
            &req.baseline_commit,
            &req.evaluated_commit,
        ],
        &score,
    )
}
