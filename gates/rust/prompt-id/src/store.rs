//! Where the prompts live, and what numbers are already taken.
//!
//! The store is the MAIN checkout's `.prompts/`, never the caller's own
//! directory: `.prompts/` is gitignored, so each worktree owns a private copy
//! and a number allocated against one of those copies says nothing about the
//! others.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One prompt the store knows about.
pub struct Entry {
    pub number: u32,
    pub file_name: String,
    pub archived: bool,
    /// A meta-prompt: a numbered DIRECTORY of stages rather than a single file.
    pub is_dir: bool,
}

/// The repo's single prompt store, resolved from the main checkout.
///
/// `git rev-parse --git-common-dir` answers with the MAIN repository's `.git`
/// from inside any worktree — that is the whole reason it is used here rather
/// than `--show-toplevel`, which answers with the worktree the caller is in.
pub fn resolve(cwd: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err("not inside a git repository".to_string());
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let git_dir = if Path::new(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        cwd.join(raw)
    };
    let root = git_dir
        .parent()
        .ok_or_else(|| format!("git dir has no parent: {}", git_dir.display()))?;
    Ok(root.join(".prompts"))
}

/// Every numbered prompt in the store, active and archived alike.
///
/// Archived prompts count: a number that has shipped must never be handed out
/// again, or two different pieces of work answer to `/run-prompt 184`.
///
/// A numbered DIRECTORY counts too. `create-meta-prompts` writes a whole
/// `NNN-<topic>-<purpose>/` stage directory, and while only `*.md` was scanned
/// those numbers were invisible: with `214-a-meta-prompt/` on disk
/// and 213 the highest file, `next` answered `214` — a number already in use.
pub fn entries(store: &Path) -> Vec<Entry> {
    let mut found = Vec::new();
    for (dir, archived) in [
        (store.to_path_buf(), false),
        (store.join("completed"), true),
    ] {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let name = item.file_name().to_string_lossy().to_string();
            let Some(number) = leading_number(&name) else {
                continue;
            };
            let is_dir = item.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_dir && item.path().extension() != Some(OsStr::new("md")) {
                continue;
            }
            found.push(Entry {
                number,
                file_name: name,
                archived,
                is_dir,
            });
        }
    }
    found.sort_by(|a, b| a.number.cmp(&b.number).then(a.file_name.cmp(&b.file_name)));
    found
}

/// The number a file name leads with, or `None` when it carries no prefix.
pub fn leading_number(file_name: &str) -> Option<u32> {
    let digits: String = file_name.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || !file_name[digits.len()..].starts_with('-') {
        return None;
    }
    digits.parse().ok()
}

/// One past the highest number any prompt has used, never below `floor`.
///
/// MONOTONIC, deliberately. Filling the store's holes would hand new work a
/// number below everything already written — `001` while 246 prompts exist —
/// so `190` would stop meaning "after 185", which is the only thing a reader
/// gets from a number. The one hole worth closing is the 186..503 jump a
/// session made by hand to dodge the collisions; that is closed once, by
/// renaming those three files, not by making every future number ambiguous.
pub fn next_free(entries: &[Entry], floor: u32) -> u32 {
    let highest = entries.iter().map(|e| e.number).max().unwrap_or(0);
    highest.saturating_add(1).max(floor.max(1))
}

/// Where a number is claimed, independently of what the prompt is called.
pub fn claims_dir(store: &Path) -> PathBuf {
    store.join(".numbers")
}

/// Take `number` for this caller, or report that somebody else already has it.
///
/// The claim is a file named for the NUMBER ALONE. Claiming by creating
/// `NNN-<slug>.md` looks atomic and is not: two sessions allocating the same
/// number under different slugs write different paths, so both `create_new`
/// calls succeed and both believe they own 189 — which is the exact collision
/// this crate exists to prevent, reproduced at a smaller scale.
pub fn claim(store: &Path, number: u32) -> std::io::Result<bool> {
    let dir = claims_dir(store);
    std::fs::create_dir_all(&dir)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(format!("{number:03}")))
    {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(e) => Err(e),
    }
}

/// Whether a number is spoken for, by a file or by a live claim.
pub fn is_taken(store: &Path, entries: &[Entry], number: u32) -> bool {
    entries.iter().any(|e| e.number == number)
        || claims_dir(store).join(format!("{number:03}")).exists()
}

/// Numbers used by more than one prompt, with the files that share them.
pub fn duplicates(entries: &[Entry]) -> BTreeMap<u32, Vec<String>> {
    let mut by_number: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for entry in entries {
        by_number
            .entry(entry.number)
            .or_default()
            .push(entry.file_name.clone());
    }
    by_number.retain(|_, files| files.len() > 1);
    by_number
}

/// Runs of unused numbers below the highest one in use, as inclusive ranges.
pub fn gaps(entries: &[Entry]) -> Vec<(u32, u32)> {
    let used: std::collections::BTreeSet<u32> = entries.iter().map(|e| e.number).collect();
    let Some(&max) = used.iter().next_back() else {
        return Vec::new();
    };
    let mut runs = Vec::new();
    let mut start: Option<u32> = None;
    for n in 1..=max {
        match (used.contains(&n), start) {
            (false, None) => start = Some(n),
            (true, Some(from)) => {
                runs.push((from, n - 1));
                start = None;
            }
            _ => {}
        }
    }
    runs
}
