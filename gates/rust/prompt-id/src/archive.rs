//! Move a finished prompt into `completed/` and make the move stick in git.
//!
//! Documented as three manual steps, this was honoured 15 times out of 255: the
//! store's archive is 94% invisible to every other clone of the repo. The steps
//! are all here because none of them is optional — a bare `mv` records a
//! DELETION and leaves the new path ignored, so the prompt disappears from the
//! repo instead of moving, and nothing says so until somebody looks.

use crate::store::{self, Entry};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The live prompts an argument names.
///
/// A bare number matches the number; anything else matches a substring of the
/// name — the two ways `/run-prompt` already resolves one.
pub fn matches<'a>(entries: &'a [Entry], arg: &str, archived: bool) -> Vec<&'a Entry> {
    let number = (!arg.is_empty() && arg.chars().all(|c| c.is_ascii_digit()))
        .then(|| arg.parse::<u32>().ok())
        .flatten();
    entries
        .iter()
        .filter(|e| e.archived == archived)
        .filter(|e| match number {
            Some(n) => e.number == n,
            None => e.file_name.contains(arg),
        })
        .collect()
}

/// The outcome of one archive attempt, so the caller reports rather than prints
/// from four places.
pub enum Outcome {
    Archived(PathBuf),
    AlreadyArchived(Vec<String>),
    Ambiguous(Vec<String>),
    NotFound,
    Failed(String),
}

pub fn archive(root: &Path, arg: &str) -> Outcome {
    let entries = store::entries(root);
    let live = matches(&entries, arg, false);
    if live.is_empty() {
        let done = matches(&entries, arg, true);
        return if done.is_empty() {
            Outcome::NotFound
        } else {
            Outcome::AlreadyArchived(done.iter().map(|e| e.file_name.clone()).collect())
        };
    }
    if live.len() > 1 {
        return Outcome::Ambiguous(live.iter().map(|e| e.file_name.clone()).collect());
    }

    let name = live[0].file_name.clone();
    let from = root.join(&name);
    let to = root.join("completed").join(&name);
    if to.exists() {
        return Outcome::Failed(format!(
            "{} already exists — archiving would clobber it",
            to.display()
        ));
    }
    let Some(repo) = root.parent() else {
        return Outcome::Failed(format!("{} has no parent repo", root.display()));
    };
    if let Err(e) = std::fs::create_dir_all(root.join("completed")) {
        return Outcome::Failed(format!("cannot create completed/: {e}"));
    }

    // `git mv` first: it keeps the rename in the index for a prompt that IS
    // tracked. It fails with "not under version control" for the ignored
    // majority, and that is the ordinary path, not an error.
    if !git(
        repo,
        &["mv", &from.to_string_lossy(), &to.to_string_lossy()],
    )
    .0
    {
        if let Err(e) = std::fs::rename(&from, &to) {
            return Outcome::Failed(format!("cannot move {}: {e}", from.display()));
        }
    }
    // `-f` is load-bearing: `.prompts/` is gitignored, usually by the user's
    // GLOBAL ignore file, so a plain `git add` is a silent no-op.
    let (added, add_err) = git(repo, &["add", "-f", &to.to_string_lossy()]);
    if !added {
        return Outcome::Failed(format!("git add -f refused {}: {add_err}", to.display()));
    }

    if !tracked(repo, &to) {
        return Outcome::Failed(format!(
            "{} moved but is not tracked — the archive would be invisible to every other clone",
            to.display()
        ));
    }
    if from.exists() {
        return Outcome::Failed(format!("{} is still at the store root", from.display()));
    }
    Outcome::Archived(to)
}

/// Whether git has the path in its index, which is what `run-prompt`'s own
/// verification line checks.
pub fn tracked(repo: &Path, path: &Path) -> bool {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("ls-files")
        .arg("--")
        .arg(path)
        .output();
    matches!(out, Ok(o) if o.status.success() && !o.stdout.is_empty())
}

fn git(repo: &Path, args: &[&str]) -> (bool, String) {
    match Command::new("git").arg("-C").arg(repo).args(args).output() {
        Ok(out) => (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}
