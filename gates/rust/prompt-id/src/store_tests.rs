//! What the numbering must guarantee, pinned.

use super::store::{duplicates, entries, gaps, leading_number, next_free, Entry};

fn entry(number: u32, file_name: &str, archived: bool) -> Entry {
    Entry {
        number,
        file_name: file_name.to_string(),
        archived,
        is_dir: false,
    }
}

#[test]
fn a_number_is_only_a_number_when_a_hyphen_follows_it() {
    assert_eq!(leading_number("184-a-thing-that-works.md"), Some(184));
    assert_eq!(leading_number("006-early.md"), Some(6));
    // Not prefixes: a bare word, and digits that run into the title.
    assert_eq!(leading_number("notes.md"), None);
    assert_eq!(leading_number("184notahyphen.md"), None);
}

#[test]
fn archived_numbers_are_taken_too() {
    // The failure this prevents: handing out a number that has already shipped,
    // so two different pieces of work answer to `/run-prompt 184`.
    let entries = vec![entry(184, "184-a-thing.md", true)];
    assert_eq!(next_free(&entries, 1), 185);
}

#[test]
fn allocation_is_monotonic_and_never_reaches_back_into_a_hole() {
    // A store with holes at 1..184 and at 187..503. New work must still land
    // above everything written, or its number stops saying when it was written.
    let entries = vec![
        entry(185, "185-a.md", true),
        entry(186, "186-b.md", true),
        entry(504, "504-c.md", false),
    ];
    assert_eq!(next_free(&entries, 1), 505);
    // A floor only ever raises the answer; it cannot pull it back down.
    assert_eq!(next_free(&entries, 600), 600);
}

#[test]
fn duplicates_name_every_file_that_shares_a_number() {
    let entries = vec![
        entry(75, "075-one-prompt.md", true),
        entry(75, "075-another-prompt.md", true),
        entry(76, "076-alone.md", true),
    ];
    let found = duplicates(&entries);
    assert_eq!(found.len(), 1);
    assert_eq!(found[&75].len(), 2);
    assert!(!found.contains_key(&76));
}

#[test]
fn gaps_are_reported_as_inclusive_runs() {
    let entries = vec![
        entry(1, "001-a.md", true),
        entry(4, "004-b.md", true),
        entry(9, "009-c.md", false),
    ];
    assert_eq!(gaps(&entries), vec![(2, 3), (5, 8)]);
}

#[test]
fn an_empty_store_has_no_gaps_and_starts_at_one() {
    let entries: Vec<Entry> = Vec::new();
    assert_eq!(gaps(&entries), Vec::new());
    assert_eq!(next_free(&entries, 1), 1);
}

#[test]
fn a_numbered_directory_holds_its_number_as_firmly_as_a_file() {
    // `create-meta-prompts` writes a stage DIRECTORY. While only `*.md` was
    // scanned, `214-a-meta-prompt/` was invisible and `next`
    // answered 214 — a number already in use.
    let store = std::env::temp_dir().join(format!("prompt-id-dirs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    std::fs::create_dir_all(store.join("214-a-meta-prompt")).unwrap();
    std::fs::create_dir_all(store.join("completed")).unwrap();
    std::fs::create_dir_all(store.join("reports")).unwrap();
    std::fs::write(store.join("213-a-single-file-prompt.md"), "x").unwrap();
    std::fs::write(store.join("notes.txt"), "x").unwrap();

    let found = entries(&store);
    assert_eq!(
        found.len(),
        2,
        "the .txt and the unnumbered dirs are not prompts"
    );
    assert!(found.iter().any(|e| e.number == 214 && e.is_dir));
    assert_eq!(next_free(&found, 1), 215);

    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn a_number_can_only_be_claimed_once_whatever_the_prompt_is_called() {
    // The bug this pins, found by racing three allocations: claiming by creating
    // `NNN-<slug>.md` is atomic per PATH, so two sessions with different slugs
    // both "won" 189 and the store gained a duplicate — the exact defect this
    // crate exists to remove.
    let store = std::env::temp_dir().join(format!("prompt-id-claim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);

    assert!(
        super::store::claim(&store, 189).unwrap(),
        "first caller takes it"
    );
    assert!(
        !super::store::claim(&store, 189).unwrap(),
        "second caller is refused"
    );
    assert!(
        super::store::claim(&store, 190).unwrap(),
        "a different number is free"
    );

    let entries: Vec<Entry> = Vec::new();
    assert!(
        super::store::is_taken(&store, &entries, 189),
        "a live claim counts as taken"
    );
    assert!(
        !super::store::is_taken(&store, &entries, 191),
        "an unclaimed number does not"
    );

    let _ = std::fs::remove_dir_all(&store);
}
