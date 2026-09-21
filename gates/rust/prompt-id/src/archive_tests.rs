//! What archiving must guarantee, pinned against a real gitignored store.

use super::archive::{archive, matches, tracked, Outcome};
use super::store::Entry;
use std::path::{Path, PathBuf};
use std::process::Command;

fn entry(number: u32, file_name: &str, archived: bool) -> Entry {
    Entry {
        number,
        file_name: file_name.to_string(),
        archived,
        is_dir: false,
    }
}

#[test]
fn a_bare_number_matches_the_number_and_text_matches_the_name() {
    let entries = vec![
        entry(208, "208-the-first-thing.md", false),
        entry(209, "209-a-later-thing.md", false),
        entry(208, "208-the-second-thing.md", true),
    ];
    assert_eq!(matches(&entries, "208", false).len(), 1);
    assert_eq!(matches(&entries, "208", true).len(), 1);
    assert_eq!(matches(&entries, "a-later-thing", false).len(), 1);
    assert_eq!(matches(&entries, "nothing-like-this", false).len(), 0);
}

/// A store inside a throwaway repo where `.prompts/` is ignored, which is the
/// only configuration this code ever runs in.
fn scratch(tag: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("prompt-id-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(repo.join(".prompts")).unwrap();
    std::fs::write(repo.join(".gitignore"), ".prompts/\n").unwrap();
    run(&repo, &["init", "-q"]);
    repo
}

fn run(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?} failed");
}

#[test]
fn an_untracked_prompt_is_moved_and_force_added() {
    // The 240-of-255 case: `git mv` refuses, and a bare `mv` would record a
    // deletion and leave the new path ignored — the prompt vanishing from the
    // repo instead of moving.
    let repo = scratch("untracked");
    let store = repo.join(".prompts");
    std::fs::write(
        store.join("208-the-first-thing.md"),
        "branch `fix/the-first-thing`",
    )
    .unwrap();

    let to = match archive(&store, "208") {
        Outcome::Archived(p) => p,
        _ => panic!("expected the prompt to archive"),
    };
    assert_eq!(to, store.join("completed").join("208-the-first-thing.md"));
    assert!(to.exists());
    assert!(!store.join("208-the-first-thing.md").exists());
    assert!(tracked(&repo, &to), "the archive must be visible to git");

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_tracked_prompt_moves_through_git_and_stays_tracked() {
    let repo = scratch("tracked");
    let store = repo.join(".prompts");
    let from = store.join("184-an-older-thing.md");
    std::fs::write(&from, "branch `fix/an-older-thing`").unwrap();
    run(&repo, &["add", "-f", &from.to_string_lossy()]);

    let to = match archive(&store, "184") {
        Outcome::Archived(p) => p,
        other => panic!("expected an archive, got {}", describe(&other)),
    };
    assert!(tracked(&repo, &to));
    assert!(!from.exists());

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn a_number_two_prompts_share_is_refused_rather_than_guessed() {
    // 55 numbers are shared in the live store, so picking the first match would
    // archive the wrong prompt roughly half the time it mattered.
    let repo = scratch("ambiguous");
    let store = repo.join(".prompts");
    std::fs::write(store.join("208-the-first-thing.md"), "x").unwrap();
    std::fs::write(store.join("208-the-second-thing.md"), "x").unwrap();

    match archive(&store, "208") {
        Outcome::Ambiguous(names) => assert_eq!(names.len(), 2),
        other => panic!("expected ambiguity, got {}", describe(&other)),
    }
    assert!(
        store.join("208-the-first-thing.md").exists(),
        "nothing moved"
    );

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn archiving_twice_reports_the_archive_rather_than_failing() {
    let repo = scratch("idempotent");
    let store = repo.join(".prompts");
    std::fs::write(store.join("200-a-third-thing.md"), "x").unwrap();

    assert!(matches!(archive(&store, "200"), Outcome::Archived(_)));
    match archive(&store, "200") {
        Outcome::AlreadyArchived(names) => assert_eq!(names, vec!["200-a-third-thing.md"]),
        other => panic!("expected already-archived, got {}", describe(&other)),
    }

    let _ = std::fs::remove_dir_all(&repo);
}

#[test]
fn an_unknown_prompt_is_not_found() {
    let repo = scratch("missing");
    let store = repo.join(".prompts");
    assert!(matches!(archive(&store, "999"), Outcome::NotFound));
    let _ = std::fs::remove_dir_all(&repo);
}

fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Archived(p) => format!("archived {}", p.display()),
        Outcome::AlreadyArchived(n) => format!("already archived {n:?}"),
        Outcome::Ambiguous(n) => format!("ambiguous {n:?}"),
        Outcome::NotFound => "not found".to_string(),
        Outcome::Failed(m) => format!("failed: {m}"),
    }
}
