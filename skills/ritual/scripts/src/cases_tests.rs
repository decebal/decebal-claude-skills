use std::fs;

use chrono::{TimeZone, Utc};

use crate::cases::{self, Case};
use crate::feedback::{Check, Feedback, Kind, Skill, Step};
use crate::store::Record;

fn record(i: u32, check: Option<Check>) -> Record {
    Record {
        id: format!("id-{i:02}"),
        timestamp: Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, i).unwrap(),
        feedback: Feedback {
            schema: 1,
            repo: "r".into(),
            epic: "e".into(),
            step: Step::Spec,
            kind: Kind::Correction,
            skill: Skill::Ritual,
            agent: "claude".into(),
            note: format!("note {i}"),
            reference: check.as_ref().map(|_| "SKILL.md".into()),
            check,
        },
    }
}

fn must(p: &str) -> Option<Check> {
    Some(Check {
        must: Some(p.into()),
        must_not: None,
    })
}

#[test]
fn only_events_with_a_check_become_cases() {
    let records = vec![record(1, None), record(2, must("a")), record(3, None)];
    let cases = cases::cases_from(&records);
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].id, "id-02");
    assert_eq!(cases[0].target(), "skills/ritual/SKILL.md");
}

#[test]
fn the_newest_thirty_percent_are_held_out() {
    let records: Vec<_> = (0..10).map(|i| record(i, must("a"))).collect();
    let (dev, holdout) = cases::split(cases::cases_from(&records));
    assert_eq!(dev.len(), 7);
    assert_eq!(
        holdout.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["id-07", "id-08", "id-09"]
    );
}

#[test]
fn three_or_fewer_cases_hold_nothing_out() {
    let records: Vec<_> = (0..3).map(|i| record(i, must("a"))).collect();
    let (dev, holdout) = cases::split(cases::cases_from(&records));
    assert_eq!((dev.len(), holdout.len()), (3, 0));
}

#[test]
fn export_writes_hashes_that_match_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let records: Vec<_> = (0..5).map(|i| record(i, must("a"))).collect();
    let manifest = cases::export(&records, 2, dir.path(), |_| true).unwrap();
    assert_eq!((manifest.dev_count, manifest.holdout_count), (4, 1));
    assert_eq!(manifest.records_unreadable, 2);
    let holdout = fs::read(dir.path().join("holdout.jsonl")).unwrap();
    assert_eq!(cases::sha256_hex(&holdout), manifest.holdout_sha256);
    assert_eq!(
        cases::read_jsonl(&dir.path().join("dev.jsonl"))
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn export_drops_cases_whose_target_file_is_missing_before_splitting() {
    let dir = tempfile::tempdir().unwrap();
    let records: Vec<_> = (0..4).map(|i| record(i, must("a"))).collect();
    let manifest = cases::export(&records, 0, dir.path(), |c| c.id != "id-01").unwrap();
    assert_eq!(manifest.dropped_missing_target, 1);
    assert_eq!((manifest.dev_count, manifest.holdout_count), (3, 0));
    let dev = cases::read_jsonl(&dir.path().join("dev.jsonl")).unwrap();
    assert!(dev.iter().all(|c| c.id != "id-01"));
}

fn case(check: Check) -> Case {
    Case {
        id: "c".into(),
        recorded_at: String::new(),
        skill: Skill::Ritual,
        step: Step::Build,
        reference: "references/build.md".into(),
        check,
        note: String::new(),
    }
}

#[test]
fn a_case_closes_when_the_text_says_it_ignoring_case_and_spacing() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("skills/ritual/references/build.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "Always RUN the\n  tests before   closing a bead.").unwrap();
    let c = case(Check {
        must: Some("run the tests before closing".into()),
        must_not: None,
    });
    assert!(!cases::is_open(root.path(), &c));
    let banned = case(Check {
        must: None,
        must_not: Some("tests before closing".into()),
    });
    assert!(cases::is_open(root.path(), &banned));
}

#[test]
fn a_missing_target_file_leaves_the_case_open() {
    let root = tempfile::tempdir().unwrap();
    let c = case(Check {
        must: None,
        must_not: Some("anything".into()),
    });
    assert!(cases::is_open(root.path(), &c));
}
