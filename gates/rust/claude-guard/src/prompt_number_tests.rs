//! What the prompt-number guard must deny, and everything it must not.

use super::{parse, reason, refusal};
use std::path::{Path, PathBuf};

fn scratch(tag: &str) -> PathBuf {
    let store = std::env::temp_dir()
        .join(format!("claude-guard-prompts-{tag}-{}", std::process::id()))
        .join(".prompts");
    let _ = std::fs::remove_dir_all(store.parent().unwrap());
    std::fs::create_dir_all(&store).unwrap();
    store
}

fn claim(store: &Path, number: &str) {
    std::fs::create_dir_all(store.join(".numbers")).unwrap();
    std::fs::write(store.join(".numbers").join(number), "").unwrap();
}

#[test]
fn a_numbered_prompt_at_the_store_root_is_recognised() {
    let found = parse("/repo/.prompts/208-a-numbered-prompt.md").unwrap();
    assert_eq!(found.number, "208");
    assert_eq!(found.store, Path::new("/repo/.prompts"));
}

#[test]
fn a_short_number_resolves_to_the_padded_claim() {
    assert_eq!(parse("/repo/.prompts/8-early.md").unwrap().number, "008");
    assert_eq!(parse("/repo/.prompts/0208-x.md").unwrap().number, "208");
}

#[test]
fn paths_this_guard_has_no_opinion_about() {
    // An archive move, a stage inside a directory whose number was claimed when
    // the directory was made, an unnumbered note, a file outside any store, and
    // digits running into the title.
    for path in [
        "/repo/.prompts/completed/208-a-numbered-prompt.md",
        "/repo/.prompts/214-mobile-blockers/214-mobile-blockers.md",
        "/repo/.prompts/notes.md",
        "/repo/docs/208-a-numbered-prompt.md",
        "/repo/.prompts/208notahyphen.md",
        "/repo/.prompts/208-a-numbered-prompt.txt",
    ] {
        assert!(parse(path).is_none(), "{path} should not be claimed ground");
    }
}

#[test]
fn creating_an_unclaimed_number_is_refused() {
    // The exact call that caused the damage: a bare Write to a path nobody
    // allocated, in a session that never invoked the command carrying the rule.
    let store = scratch("unclaimed");
    claim(&store, "207");
    let path = store.join("208-a-numbered-prompt.md");

    let message = refusal(path.to_str().unwrap()).expect("208 was never claimed");
    assert!(message.contains("208"));
    assert!(message.contains("prompt-id alloc"));
}

#[test]
fn a_claimed_number_is_allowed() {
    let store = scratch("claimed");
    claim(&store, "208");
    let path = store.join("208-a-numbered-prompt.md");
    assert!(refusal(path.to_str().unwrap()).is_none());
}

#[test]
fn editing_a_prompt_that_already_exists_is_allowed() {
    // Every prompt written before the allocator existed is unclaimed. Denying
    // edits to those would block ordinary work to prevent nothing — the damage
    // is issuing a number, not revising a file.
    let store = scratch("existing");
    claim(&store, "207");
    let path = store.join("208-a-numbered-prompt.md");
    std::fs::write(&path, "already here").unwrap();
    assert!(refusal(path.to_str().unwrap()).is_none());
}

#[test]
fn a_store_with_no_claims_directory_is_left_alone() {
    // A repo that never adopted the allocator has no `.numbers/`. Denying every
    // prompt write there makes the guard hostile in exactly the repos it has
    // nothing to say about.
    let store = scratch("unadopted");
    let path = store.join("001-first-ever.md");
    assert!(refusal(path.to_str().unwrap()).is_none());
}

#[test]
fn the_refusal_names_only_the_store_and_the_tool() {
    // The repo carrying this guard is public. The message may name `.prompts`
    // and `prompt-id`, and nothing else.
    let message = reason("208");
    assert!(message.contains(".prompts/.numbers/208"));
    assert!(message.contains("prompt-id alloc"));
    assert!(!message.contains("/Users"), "no machine path may leak");
    assert!(!message.to_lowercase().contains("github.com"));
}
