//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;

#[test]
fn a_continuation_line_stays_with_its_entry() {
    let entries = split_entries("- alpha: does a\n  and more\n- beta\n- gamma: c");
    assert_eq!(
        entries,
        vec!["- alpha: does a\n  and more", "- beta", "- gamma: c"]
    );
}

#[test]
fn a_described_entry_and_a_name_only_entry_are_told_apart() {
    assert_eq!(
        entry_name("- prove-it: Adversarially review"),
        ("prove-it", true)
    );
    assert_eq!(
        entry_name("- caveman:cavecrew"),
        ("caveman:cavecrew", false)
    );
    assert_eq!(entry_name("- name-only  "), ("name-only", false));
}

#[test]
fn a_colon_inside_prose_is_not_a_name_separator() {
    assert_eq!(entry_name("- two words: text"), ("two words: text", false));
}

#[test]
fn civil_date_matches_known_days() {
    assert_eq!(civil_date(0), "1970-01-01");
    assert_eq!(civil_date(951_782_400), "2000-02-29");
    assert_eq!(civil_date(1_794_355_200), "2026-11-11");
}

#[test]
fn clean_accepts_names_and_rejects_prose() {
    assert_eq!(clean("/ritual"), Some("ritual".into()));
    assert_eq!(
        clean("caveman:cavecrew-builder"),
        Some("caveman:cavecrew-builder".into())
    );
    assert_eq!(clean("not a name"), None);
    assert_eq!(clean(""), None);
}
