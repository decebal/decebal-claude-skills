use clap::Parser;
use serde_json::{Value, json};

use crate::Cli;
use crate::decide::{Policy, api_key, parse_options, parse_response, parse_state, request_body};

fn opts(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

#[test]
fn options_split_on_the_first_equals_sign() {
    let parsed = parse_options(&opts(&["lean=the lean ritual: a=b", "full"])).unwrap();
    assert_eq!(
        parsed,
        vec![
            ("lean".into(), "the lean ritual: a=b".into()),
            ("full".into(), String::new())
        ]
    );
}

#[test]
fn fewer_than_two_options_is_refused() {
    assert!(parse_options(&opts(&["only"])).is_err());
}

#[test]
fn duplicate_or_empty_option_names_are_refused() {
    assert!(parse_options(&opts(&["a=x", "a=y"])).is_err());
    assert!(parse_options(&opts(&["=x", "b"])).is_err());
}

#[test]
fn json_state_stays_structured_and_text_stays_text() {
    assert_eq!(parse_state(r#"{"files": 3}"#), json!({"files": 3}));
    assert_eq!(
        parse_state("add a flag"),
        Value::String("add a flag".into())
    );
}

#[test]
fn the_request_is_one_choice_question_with_null_for_bare_options() {
    let body = request_body(
        "jev-latest",
        "Which tier?",
        &parse_options(&opts(&["lean=small", "full"])).unwrap(),
        json!({"files": 3}),
    );
    assert_eq!(
        body,
        json!({
            "model": "jev-latest",
            "state": {"files": 3},
            "questions": {"decision": {
                "type": "choice",
                "instructions": "Which tier?",
                "criteria": {"lean": "small", "full": null}
            }}
        })
    );
}

fn response(choice: &str, confidence: f64) -> Value {
    json!({
        "model": "jev-latest",
        "answers": {"decision": {
            "type": "choice",
            "choice": choice,
            "confidence": confidence,
            "probabilities": {"none": 0.05, "lean": 0.85, "full": 0.10}
        }},
        "usage": {"input_tokens": 120, "output_tokens": 1}
    })
}

#[test]
fn a_response_parses_with_probabilities_most_likely_first() {
    let d = parse_response(&response("lean", 0.86)).unwrap();
    assert_eq!(d.choice, "lean");
    assert_eq!(d.probabilities[0].0, "lean");
    assert_eq!(d.probabilities[1].0, "full");
    assert_eq!(d.input_tokens, Some(120));
    assert_eq!(
        d.render(),
        "decide: lean act confidence=0.86 · runner-up full p=0.10 · 120 input tokens"
    );
}

#[test]
fn a_response_without_the_answer_is_an_error() {
    assert!(parse_response(&json!({"answers": {}})).is_err());
}

#[test]
fn a_response_without_confidence_or_out_of_range_is_an_error() {
    let mut r = response("lean", 0.86);
    r["answers"]["decision"]
        .as_object_mut()
        .unwrap()
        .remove("confidence");
    assert!(parse_response(&r).is_err());
    assert!(parse_response(&response("lean", 86.0)).is_err());
    assert!(parse_response(&response("lean", -0.1)).is_err());
}

#[test]
fn non_object_probabilities_leave_no_runner_up() {
    let mut r = response("lean", 0.9);
    r["answers"]["decision"]["probabilities"] = json!([0.9, 0.1]);
    r.as_object_mut().unwrap().remove("usage");
    let d = parse_response(&r).unwrap();
    assert!(d.probabilities.is_empty());
    assert_eq!(d.render(), "decide: lean act confidence=0.90");
}

#[test]
fn json_output_carries_the_policy_and_every_option() {
    let j = parse_response(&response("lean", 0.6)).unwrap().to_json();
    assert_eq!(j["choice"], "lean");
    assert_eq!(j["policy"], "act-and-report");
    assert_eq!(j["probabilities"].as_array().unwrap().len(), 3);
    assert_eq!(j["probabilities"][0]["option"], "lean");
    assert_eq!(j["input_tokens"], 120);
}

#[test]
fn more_than_255_options_is_refused() {
    let many: Vec<String> = (0..256).map(|i| format!("o{i}")).collect();
    assert!(parse_options(&many).is_err());
    assert!(parse_options(&many[..255]).is_ok());
}

#[test]
fn the_env_key_wins_and_the_file_is_the_fallback() {
    let home = tempfile::tempdir().unwrap();
    assert!(api_key(None, home.path()).is_err());
    let dir = home.path().join(".config").join("typesafe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("key"), "  \n").unwrap();
    assert!(api_key(None, home.path()).is_err());
    std::fs::write(dir.join("key"), "file-key\n").unwrap();
    assert_eq!(api_key(None, home.path()).unwrap(), "file-key");
    assert_eq!(api_key(Some(" ".into()), home.path()).unwrap(), "file-key");
    assert_eq!(
        api_key(Some("env-key".into()), home.path()).unwrap(),
        "env-key"
    );
}

#[test]
fn a_missing_key_error_names_the_path_not_a_key() {
    let home = tempfile::tempdir().unwrap();
    let err = api_key(None, home.path()).unwrap_err();
    assert!(err.contains(".config/typesafe/key"));
}

#[test]
fn decide_takes_exactly_one_of_state_and_state_file() {
    let base = [
        "ritual-feedback",
        "decide",
        "--question",
        "q?",
        "--option",
        "a",
        "--option",
        "b",
    ];
    let with = |extra: &[&str]| {
        let mut argv: Vec<&str> = base.to_vec();
        argv.extend_from_slice(extra);
        Cli::try_parse_from(argv)
    };
    assert!(with(&[]).is_err());
    assert!(with(&["--state", "s", "--state-file", "f"]).is_err());
    assert!(with(&["--state", "s"]).is_ok());
    assert!(with(&["--state-file", "f"]).is_ok());
    assert!(with(&["--state", "s", "--timeout-secs", "61"]).is_err());
}

#[test]
fn policy_thresholds_are_inclusive_at_their_floor() {
    assert_eq!(Policy::of(0.80), Policy::Act);
    assert_eq!(Policy::of(0.79), Policy::ActAndReport);
    assert_eq!(Policy::of(0.50), Policy::ActAndReport);
    assert_eq!(Policy::of(0.49), Policy::Ask);
}
