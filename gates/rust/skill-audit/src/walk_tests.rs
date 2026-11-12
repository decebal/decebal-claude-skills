//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("skill-audit-walk-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("a/b/c")).expect("mkdir");
    fs::write(dir.join("top.jsonl"), "").expect("write");
    fs::write(dir.join("a/one.jsonl"), "").expect("write");
    fs::write(dir.join("a/b/c/deep.jsonl"), "").expect("write");
    fs::write(dir.join("a/skip.md"), "").expect("write");
    dir
}

fn jsonl(p: &Path) -> bool {
    p.extension().is_some_and(|x| x == "jsonl")
}

#[test]
fn finds_matching_files_at_every_depth_within_the_bound() {
    let dir = scratch("all");
    let found = files(&dir, 8, &jsonl);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found.iter().all(|p| jsonl(p)));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn stops_at_the_depth_bound() {
    let dir = scratch("depth");
    let found = files(&dir, 1, &jsonl);
    assert_eq!(found, vec![dir.join("a/one.jsonl"), dir.join("top.jsonl")]);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_root_is_empty_not_an_error() {
    assert!(files(Path::new("/nonexistent/skill-audit"), 8, &jsonl).is_empty());
}
