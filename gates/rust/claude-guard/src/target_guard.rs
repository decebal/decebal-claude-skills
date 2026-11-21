//! `PreToolUse` on Bash: a Cargo target directory may not be created inside
//! another one.
//!
//! `CARGO_TARGET_DIR=<dir>` or `--target-dir <dir>` pointing under an existing
//! build directory (one holding the `CACHEDIR.TAG` Cargo writes) makes a nested
//! build tree. `target-gc` treats the outer directory as one unit, so it keeps
//! the nested tree for as long as anything runs in the project, and the nested
//! tree never expires on its own: an agent's `target/target-p2-screens_a3`
//! sat at 48 GiB by `du` inside a `target/` held by a live `dx serve`.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::hook;

pub fn run(payload: &Value) -> ! {
    let command = hook::str_at(payload, &["tool_input", "command"]);
    let cwd = Path::new(hook::str_at(payload, &["cwd"]));
    for dir in target_dirs(command) {
        let resolved = resolve(cwd, &dir);
        if let Some(outer) = nested_in(&resolved, |p| p.join("CACHEDIR.TAG").is_file()) {
            hook::deny(&reason(&resolved, &outer));
        }
    }
    hook::allow()
}

/// Every target directory the command names, through the environment or the
/// flag, as written.
pub(crate) fn target_dirs(command: &str) -> Vec<String> {
    let words: Vec<String> = command
        .split_whitespace()
        .map(|w| w.trim_matches(|c| c == '"' || c == '\''))
        .map(str::to_owned)
        .collect();
    let mut dirs = Vec::new();
    for (i, word) in words.iter().enumerate() {
        let assigned = word
            .strip_prefix("CARGO_TARGET_DIR=")
            .or_else(|| word.strip_prefix("--target-dir="));
        if let Some(dir) = assigned {
            dirs.push(dir.trim_matches(|c| c == '"' || c == '\'').to_owned());
        } else if word == "--target-dir" {
            if let Some(next) = words.get(i + 1) {
                dirs.push(next.clone());
            }
        }
    }
    dirs.retain(|d| !d.is_empty());
    dirs
}

/// `dir` made absolute against `cwd`, with `~` expanded and `.`/`..` folded.
pub(crate) fn resolve(cwd: &Path, dir: &str) -> PathBuf {
    let raw = match dir.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(dir)),
        None => PathBuf::from(dir),
    };
    let joined = if raw.is_absolute() {
        raw
    } else {
        cwd.join(raw)
    };
    let mut out = PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The nearest proper ancestor of `dir` that is a Cargo build directory.
pub(crate) fn nested_in(dir: &Path, is_build_dir: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    dir.ancestors()
        .skip(1)
        .find(|p| is_build_dir(p))
        .map(Path::to_path_buf)
}

pub(crate) fn reason(dir: &Path, outer: &Path) -> String {
    format!(
        "{} is inside the build directory {}; target-gc cannot expire it there. Use a target dir outside any existing target/.",
        dir.display(),
        outer.display()
    )
}

#[cfg(test)]
#[path = "target_guard_tests.rs"]
mod target_guard_tests;
