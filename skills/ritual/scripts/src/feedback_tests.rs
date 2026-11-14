use crate::feedback::{Check, Feedback, Kind, Skill, Step, validate_reference};

fn sample() -> Feedback {
    Feedback {
        schema: 1,
        repo: "longhand".into(),
        epic: "t-1".into(),
        step: Step::Build,
        kind: Kind::Friction,
        skill: Skill::Ritual,
        agent: "claude".into(),
        note: "build.md never says to run the tests before closing a bead".into(),
        reference: Some("references/build.md".into()),
        check: Some(Check {
            must: Some("run the tests before closing".into()),
            must_not: None,
        }),
    }
}

#[test]
fn a_valid_feedback_names_its_event_and_entity() {
    let fb = sample();
    assert_eq!(fb.validate(), Ok(()));
    assert_eq!(fb.event_type(), "ritual.feedback.friction");
    assert_eq!(fb.entity_id(), "longhand/t-1");
}

#[test]
fn a_check_without_a_reference_is_refused() {
    let mut fb = sample();
    fb.reference = None;
    assert!(fb.validate().unwrap_err().contains("--reference"));
}

#[test]
fn a_reference_into_the_tools_source_is_refused() {
    let mut fb = sample();
    fb.reference = Some("scripts/README.md".into());
    assert!(fb.validate().unwrap_err().contains("scripts/"));
}

#[test]
fn an_empty_check_is_refused() {
    let mut fb = sample();
    fb.check = Some(Check::default());
    assert!(fb.validate().unwrap_err().contains("--must"));
}

#[test]
fn an_overlong_phrase_is_refused() {
    let mut fb = sample();
    fb.check = Some(Check {
        must: Some("x".repeat(201)),
        must_not: None,
    });
    assert!(fb.validate().is_err());
}

#[test]
fn references_stay_inside_the_skill_directory() {
    assert!(validate_reference("SKILL.md").is_ok());
    assert!(validate_reference("references/spec.md").is_ok());
    for bad in [
        "../other/SKILL.md",
        "/etc/passwd.md",
        "references/spec.txt",
        "",
        "./SKILL.md",
    ] {
        assert!(validate_reference(bad).is_err(), "{bad} should be refused");
    }
}

#[test]
fn feedback_round_trips_through_json() {
    let fb = sample();
    let json = serde_json::to_value(&fb).unwrap();
    assert_eq!(json["skill"], "ritual");
    assert_eq!(json["kind"], "friction");
    assert_eq!(serde_json::from_value::<Feedback>(json).unwrap(), fb);
}
