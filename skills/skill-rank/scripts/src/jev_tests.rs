use serde_json::json;

use super::*;

fn state(name: &str, description: &str, claude: u64, codex: u64, hosts: &[&str]) -> SkillState {
    SkillState {
        name: name.to_owned(),
        description: description.to_owned(),
        claude_use: claude,
        codex_use: codex,
        hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
    }
}

#[test]
fn request_has_the_exact_system_one_shape() {
    let skills = [
        state("tdd", "Test first.", 3, 0, &["claude"]),
        state("pdf", "Read PDFs.", 0, 2, &["claude", "codex"]),
    ];
    let body = build_request(
        "jev-latest",
        "I build Rust CLIs.",
        &skills,
        &Redactor::default(),
    );
    let expected = json!({
        "model": "jev-latest",
        "state": {
            "profile": "I build Rust CLIs.",
            "skills": {
                "s0": {"name": "tdd", "description": "Test first.", "recent_use": {"claude": 3, "codex": 0}, "hosts": ["claude"]},
                "s1": {"name": "pdf", "description": "Read PDFs.", "recent_use": {"claude": 0, "codex": 2}, "hosts": ["claude", "codex"]}
            }
        },
        "questions": {
            "s0": {"type": "score", "instructions": instructions("s0"), "criteria": CRITERIA},
            "s1": {"type": "score", "instructions": instructions("s1"), "criteria": CRITERIA}
        }
    });
    assert_eq!(body, expected);
    let keys: Vec<&String> = body["state"]["skills"]
        .as_object()
        .expect("skills")
        .keys()
        .collect();
    assert_eq!(keys, ["s0", "s1"], "state keeps batch order");
}

#[test]
fn question_text_names_its_state_path_because_ids_are_not_sent() {
    assert_eq!(
        instructions("s7"),
        "Rate how much keeping the skill `skills.s7` ready in an AI coding agent's context would help with \
         the work described in `profile`. Judge the skill's purpose against that work. Its recent_use counts \
         are evidence of need; zero use is weak evidence against it, not proof."
    );
    assert_eq!(CRITERIA.len(), 5);
    for c in CRITERIA {
        assert!(
            !c.contains("s0") && !c.contains("skills."),
            "criteria stay id-free: {c}"
        );
    }
}

#[test]
fn redaction_covers_profile_name_and_description_case_insensitively() {
    let redact = Redactor::from_text("Acme Corp\n\n  secret-proj  \n").unwrap();
    let skills = [state(
        "secret-proj-deploy",
        "Deploys ACME CORP apps.",
        0,
        0,
        &["codex"],
    )];
    let body = build_request(
        "jev-latest",
        "I work at acme corp on Secret-Proj.",
        &skills,
        &redact,
    );
    assert_eq!(
        body["state"]["profile"],
        "I work at [redacted] on [redacted]."
    );
    assert_eq!(body["state"]["skills"]["s0"]["name"], "[redacted]-deploy");
    assert_eq!(
        body["state"]["skills"]["s0"]["description"],
        "Deploys [redacted] apps."
    );
    let text = body.to_string().to_lowercase();
    assert!(!text.contains("acme") && !text.contains("secret-proj"));
}

#[test]
fn redaction_escapes_pattern_characters() {
    let redact = Redactor::new(&["c++".to_owned(), "a.b".to_owned()]).unwrap();
    assert_eq!(
        redact.apply("c++ and axb and a.b"),
        "[redacted] and axb and [redacted]"
    );
    assert_eq!(Redactor::default().apply("unchanged"), "unchanged");
}

#[test]
fn description_is_cut_to_the_limit_after_redaction() {
    let long = format!("{}{}", "é".repeat(DESCRIPTION_LIMIT), "tail");
    let body = build_request(
        "m",
        "p",
        &[state("n", &long, 0, 0, &[])],
        &Redactor::default(),
    );
    let sent = body["state"]["skills"]["s0"]["description"]
        .as_str()
        .expect("description");
    assert_eq!(sent.chars().count(), DESCRIPTION_LIMIT);
    assert!(!sent.contains("tail"));
}

#[test]
fn response_scores_are_normalised_by_the_top_level() {
    let resp = json!({
        "answers": {
            "s0": {"type": "score", "score": 3.0, "legend": {}, "probabilities": {"3": 0.9, "4": 0.1}, "confidence": 0.8},
            "s1": {"type": "score", "score": 0.4},
            "s2": {"type": "score", "score": 9}
        },
        "model": "jev-2026-09",
        "usage": {"input_tokens": 1234}
    });
    let (answers, model) = parse_response(&resp, 4);
    assert_eq!(model, "jev-2026-09");
    assert_eq!(answers.len(), 3, "the missing s3 is left unrated");
    assert!((answers[&0].rating - 0.75).abs() < 1e-9);
    assert_eq!(answers[&0].confidence, Some(0.8));
    assert_eq!(answers[&0].probabilities, json!({"3": 0.9, "4": 0.1}));
    assert!((answers[&1].rating - 0.1).abs() < 1e-9);
    assert_eq!(answers[&1].confidence, None);
    assert!(
        (answers[&2].rating - 1.0).abs() < 1e-9,
        "out-of-range scores clamp"
    );
}

#[test]
fn unrelated_response_parses_to_nothing() {
    let (answers, model) = parse_response(&json!({"error": "overloaded"}), 2);
    assert!(answers.is_empty());
    assert_eq!(model, "");
}
