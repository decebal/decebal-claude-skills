//! What the shipped check must get right without touching the network.

use super::shipped::{day, find, parse};

#[test]
fn rows_become_merges_and_malformed_lines_are_dropped() {
    let tsv = "1155\tfix/the-first-thing\t2026-09-08T15:32:33Z\n\
               1154\tfeat/the-second-thing\t2026-09-08T15:34:47Z\n\
               \n\
               nonsense-without-tabs\n";
    let merged = parse(tsv);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].number, "1155");
    assert_eq!(merged[1].branch, "feat/the-second-thing");
}

#[test]
fn a_branch_matches_exactly_and_never_by_prefix() {
    // A shorter name must not claim a longer one's merge — reporting a live
    // prompt as shipped is how finished work gets re-run.
    let merged = parse("900\tfix/the-long-name\t2026-09-01T10:00:00Z\n");
    assert!(find(&merged, "fix/the-long-name").is_some());
    assert!(find(&merged, "fix/the-long").is_none());
    assert!(find(&merged, "fix/the-long-name-more").is_none());
}

#[test]
fn the_newest_merge_of_a_reused_branch_wins() {
    // gh lists newest first, and a branch name gets reused.
    let merged = parse(
        "901\tfix/same-name\t2026-09-05T10:00:00Z\n\
         700\tfix/same-name\t2026-04-01T10:00:00Z\n",
    );
    assert_eq!(find(&merged, "fix/same-name").unwrap().number, "901");
}

#[test]
fn a_merge_date_reads_as_a_day() {
    assert_eq!(day("2026-09-08T15:32:33Z"), "2026-09-08");
    assert_eq!(day(""), "");
}
