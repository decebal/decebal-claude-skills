//! Recursive file listing on `std::fs`.

use std::fs;
use std::path::{Path, PathBuf};

/// Every file under `root`, at most `max_depth` directories down, for which
/// `keep` holds. Symlinks are followed (skill folders are often links), so the
/// depth bound is also what stops a link cycle.
pub fn files(root: &Path, max_depth: usize, keep: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect(root, max_depth, keep, &mut out);
    out.sort();
    out
}

fn collect(dir: &Path, depth: usize, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            if depth > 0 {
                collect(&path, depth - 1, keep, out);
            }
        } else if meta.is_file() && keep(&path) {
            out.push(path);
        }
    }
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod walk_tests;
