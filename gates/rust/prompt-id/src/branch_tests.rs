//! What branch extraction must survive.

use super::branch::reserved;

#[test]
fn the_name_may_sit_on_the_next_line() {
    // Six of twenty root prompts wrapped exactly like this. A line-at-a-time
    // reader calls every one of them "no branch declared".
    let text = "Trunk-based development: feature `a-thing-that-works`, branch\n\
                `fix/a-thing-that-works`, branched fresh off `origin/main`.";
    assert_eq!(reserved(text).as_deref(), Some("fix/a-thing-that-works"));
}

#[test]
fn prose_using_the_word_is_not_a_declaration() {
    // `branch of the parser` is followed by a backtick span further along the
    // paragraph. Only separators may sit between the word and the name.
    let text = "so the second branch of the parser is reached, which calls\n\
                `normalise_input` before the request goes out.\n\n\
                Trunk-based development: feature `x`, branch `fix/real-one`.";
    assert_eq!(reserved(text).as_deref(), Some("fix/real-one"));
}

#[test]
fn branched_and_branches_are_different_words() {
    assert_eq!(reserved("branched fresh off `origin/main`."), None);
    assert_eq!(reserved("two branches `feat/a` apart").as_deref(), None);
}

#[test]
fn the_first_declaration_wins_over_a_later_aside() {
    let text = "Trunk-based: feature `the-feature`, branch\n\
                `fix/the-feature`. Ask first.\n\n\
                **In-flight work to stay clear of.** Another branch, `fix/something-else`\n\
                (prompt 195), is changing the same module.";
    assert_eq!(reserved(text).as_deref(), Some("fix/the-feature"));
}

#[test]
fn a_name_without_a_type_prefix_is_skipped_and_the_search_continues() {
    let text = "branch `main` is not one. Later: branch `feat/the-real-thing`.";
    assert_eq!(reserved(text).as_deref(), Some("feat/the-real-thing"));
}

#[test]
fn a_capitalised_word_still_counts() {
    let text = "Create branch `fix/startup-auth` (or the closest equivalent).";
    assert_eq!(reserved(text).as_deref(), Some("fix/startup-auth"));
}

#[test]
fn a_prompt_that_names_no_branch_reports_none() {
    assert_eq!(reserved("Investigate the failure and report back.\n"), None);
}
