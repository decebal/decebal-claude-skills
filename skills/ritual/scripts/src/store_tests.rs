use crate::feedback::{Check, Feedback, Kind, Skill, Step};
use crate::store;

fn feedback(epic: &str, kind: Kind) -> Feedback {
    Feedback {
        schema: 1,
        repo: "repo".into(),
        epic: epic.into(),
        step: Step::Review,
        kind,
        skill: Skill::RitualFull,
        agent: "claude".into(),
        note: format!("{epic} {}", kind.as_str()),
        reference: Some("SKILL.md".into()),
        check: Some(Check {
            must: Some("fresh context".into()),
            must_not: None,
        }),
    }
}

#[test]
fn events_survive_a_reopen_and_read_back_in_order() {
    let dir = tempfile::tempdir().unwrap();
    store::append(dir.path(), &feedback("e1", Kind::Friction)).unwrap();
    store::append(dir.path(), &feedback("e2", Kind::Praise)).unwrap();
    let (records, unreadable) = store::read_all(dir.path()).unwrap();
    assert_eq!(unreadable, 0);
    let epics: Vec<&str> = records.iter().map(|r| r.feedback.epic.as_str()).collect();
    assert_eq!(epics, ["e1", "e2"]);
    assert_eq!(records[1].feedback, feedback("e2", Kind::Praise));
}

#[test]
fn an_invalid_event_is_refused_before_it_reaches_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut bad = feedback("e1", Kind::Drift);
    bad.note = "  ".into();
    assert!(store::append(dir.path(), &bad).is_err());
    assert_eq!(store::read_all(dir.path()).unwrap().0.len(), 0);
}
