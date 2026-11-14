use std::fs;
use std::path::PathBuf;

use chrono::{Datelike, Local, TimeZone, Timelike, Utc};

use crate::cases::Case;
use crate::evaluate::{Frozen, GATES, OBJECTIVE};
use crate::feedback::{Check, Skill, Step};
use crate::schedule;
use crate::weekly::{self, Options, Phase, Row, State, StatusEntry};

fn options() -> Options {
    Options {
        workdir: PathBuf::from("/w"),
        store: PathBuf::from("/s"),
        repo_url: "https://example.com/skills.git".into(),
        branch: "main".into(),
        candidates: 3,
        evaluator: PathBuf::from("/bin/ritual-feedback"),
        autoresearch: PathBuf::from("/bin/autoresearch"),
        claude: PathBuf::from("/bin/claude"),
        claude_model: None,
        start_weekday: 0,
        start_hour: 7,
    }
}

fn row(disposition: &str) -> Row {
    Row {
        index: Some(1),
        case_id: "c1".into(),
        hypothesis: "tighten build.md | one rule".into(),
        status: "evaluated".into(),
        commit: Some("abc".into()),
        decision: Some(serde_json::json!({ "disposition": disposition })),
        finalization: None,
    }
}

#[test]
fn a_keep_read_back_from_its_finalization_still_counts_as_kept() {
    let report = serde_json::json!({
        "command": "run",
        "status": "finalized",
        "index": 2,
        "finalization": {"outcome": "kept", "commit": "abc"},
    });
    let pending = weekly::Pending {
        case_id: "c2".into(),
        hypothesis: "h".into(),
    };
    let row = weekly::row_from(&report, Some(pending));
    assert_eq!(row.case_id, "c2");
    assert!(row.decision.is_none());
    assert!(row.kept());
    assert!(!row_discarded().kept());
}

fn row_discarded() -> Row {
    Row {
        decision: Some(serde_json::json!({ "disposition": "discard" })),
        finalization: Some(serde_json::json!({ "outcome": "discarded" })),
        ..row("discard")
    }
}

#[test]
fn the_abandon_hypothesis_names_the_gate_it_fails() {
    assert!(weekly::ABANDON_HYPOTHESIS.contains("frontmatter_valid"));
}

#[test]
fn the_contract_freezes_the_evaluator_inputs_and_scope() {
    let frozen = Frozen {
        dev: PathBuf::from("/w/cases/2026-11-15/dev.jsonl"),
        holdout: PathBuf::from("/w/cases/2026-11-15/holdout.jsonl"),
        holdout_sha256: "abc".into(),
        holdout_max_open: 2,
        max_words: 9000,
    };
    let toml = weekly::contract(&options(), &frozen, "2026-11-15");
    assert!(toml.contains(&format!("name = \"{OBJECTIVE}\"")));
    assert!(toml.contains("mutable_paths = [\"skills/ritual\", \"skills/ritual-full\"]"));
    assert!(toml.contains("protected_paths = [\"skills/ritual/scripts\"]"));
    assert!(toml.contains("program = \"/bin/ritual-feedback\""));
    assert!(toml.contains("\"--holdout-max-open\", \"2\", \"--max-words\", \"9000\""));
    assert!(toml.contains("max_candidates = 3"));
    for gate in GATES {
        assert!(toml.contains(&format!("\"{gate}\"")));
    }
}

#[test]
fn the_program_brief_forbids_touching_the_evaluator() {
    assert!(weekly::program().contains("never `skills/ritual/scripts/`"));
}

#[test]
fn the_mutator_is_told_the_check_is_a_literal_phrase() {
    let case = Case {
        id: "c1".into(),
        recorded_at: String::new(),
        skill: Skill::RitualFull,
        step: Step::Review,
        reference: "SKILL.md".into(),
        check: Check {
            must: Some("pin each reviewer to the strongest model".into()),
            must_not: Some("small default".into()),
        },
        note: "reviewer ran on the small model".into(),
    };
    let kept = [(
        "skills/ritual/SKILL.md".to_string(),
        "Never cut beads for an unapproved design".to_string(),
    )];
    let prompt = weekly::mutator_prompt(&case, 2, &kept);
    assert!(prompt.contains("word for word and unbroken"));
    assert!(
        prompt.contains("- `skills/ritual/SKILL.md`: \"Never cut beads for an unapproved design\"")
    );
    assert!(prompt.contains("never drop a rule to make room"));
    assert!(prompt.contains("\"pin each reviewer to the strongest model\""));
    assert!(prompt.contains("`skills/ritual-full/SKILL.md` must no longer contain"));
    assert!(prompt.contains("2 feedback cases are open"));
}

#[test]
fn the_next_start_is_the_configured_local_slot_strictly_after() {
    let sat = Local.with_ymd_and_hms(2026, 11, 14, 22, 0, 0).unwrap();
    let next = weekly::next_start(sat, 0, 7);
    assert_eq!(next.weekday().num_days_from_sunday(), 0);
    assert_eq!((next.day(), next.hour(), next.minute()), (15, 7, 0));
    let sunday_at_seven = next;
    assert_eq!(weekly::next_start(sunday_at_seven, 0, 7).day(), 22);
    let sunday_morning = Local.with_ymd_and_hms(2026, 11, 15, 6, 59, 0).unwrap();
    assert_eq!(weekly::next_start(sunday_morning, 0, 7).day(), 15);
}

#[test]
fn a_finished_loop_waits_for_the_slot_and_catches_up_after_sleep() {
    let started = Local.with_ymd_and_hms(2026, 11, 15, 7, 5, 0).unwrap();
    let state = State {
        phase: Phase::Reported,
        started_at: Some(started.with_timezone(&Utc)),
        ..State::default()
    };
    let midweek = Local.with_ymd_and_hms(2026, 11, 18, 12, 0, 0).unwrap();
    assert!(!weekly::due(&state, midweek, 0, 7));
    let woke_monday = Local.with_ymd_and_hms(2026, 11, 23, 9, 0, 0).unwrap();
    assert!(weekly::due(&state, woke_monday, 0, 7));
    assert!(weekly::due(&State::default(), midweek, 0, 7));
}

#[test]
fn every_way_out_of_candidates_leads_to_the_report() {
    let opts = options();
    let mut state = State {
        phase: Phase::Running,
        ..State::default()
    };
    assert!(weekly::candidates_left(&state, &opts));
    state.rows = vec![row("keep"), row("discard"), row("keep")];
    assert!(!weekly::candidates_left(&state, &opts), "row budget");
    state.rows.clear();
    state.attempts = 6;
    assert!(
        !weekly::candidates_left(&state, &opts),
        "failed attempts count"
    );
    state.attempts = 0;
    state.halt = Some("every dev case is closed".into());
    assert!(!weekly::candidates_left(&state, &opts), "halt");
}

#[test]
fn a_corrupt_state_file_is_refused_not_reset() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(weekly::load_state(dir.path()).unwrap().phase, Phase::Idle);
    fs::write(dir.path().join("state.json"), "{\"run\": \"2026-11-").unwrap();
    assert!(
        weekly::load_state(dir.path())
            .unwrap_err()
            .contains("unreadable")
    );
}

#[test]
fn status_z_parsing_handles_renames_and_odd_paths() {
    let raw = " M skills/ritual/SKILL.md\0?? notes with space.md\0R  skills/ritual/new.md\0skills/ritual/old.md\0 M README.md\0";
    let entries = weekly::parse_status_z(raw);
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[1].path, "notes with space.md");
    assert_eq!(entries[2].from.as_deref(), Some("skills/ritual/old.md"));
    let scoped: Vec<bool> = entries.iter().map(weekly::in_scope).collect();
    assert_eq!(scoped, [true, false, false, false]);
}

#[test]
fn the_evaluators_own_source_is_out_of_scope_for_candidates() {
    let entry = StatusEntry {
        code: " M".into(),
        path: "skills/ritual/scripts/notes.md".into(),
        from: None,
    };
    assert!(!weekly::in_scope(&entry));
}

#[test]
fn the_ledger_lists_every_candidate_and_never_claims_a_merge() {
    let state = State {
        run: "2026-11-15".into(),
        phase: Phase::Running,
        base_commit: Some("base".into()),
        attempts: 2,
        halt: Some("every dev case is closed".into()),
        rows: vec![row("keep")],
        ..State::default()
    };
    let ledger = weekly::ledger(&state, "run-1", "matched", &options());
    assert!(ledger.contains("nothing is merged"));
    assert!(ledger.contains("2 mutator attempts, 1 evaluated candidates"));
    assert!(ledger.contains("stopped because: every dev case is closed"));
    assert!(ledger.contains("| 1 | `c1` | evaluated | abc |"));
    assert!(ledger.contains("tighten build.md / one rule"));
}

#[test]
fn the_plist_ticks_on_an_interval() {
    let argv = vec!["/bin/ritual-feedback".to_string(), "weekly".to_string()];
    let body = schedule::plist(
        &argv,
        "/usr/bin:/bin",
        &PathBuf::from("/Users/me"),
        &PathBuf::from("/w/logs/launchd.log"),
    );
    assert!(body.contains("<key>StartInterval</key>\n  <integer>900</integer>"));
    assert!(!body.contains("StartCalendarInterval"));
    assert!(body.contains(&format!("<string>{}</string>", schedule::LABEL)));
    assert!(body.contains("<string>weekly</string>"));
}
