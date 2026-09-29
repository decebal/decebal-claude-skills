use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::catalog::{Host, Source};
use crate::test_support::{skill, TempDir};

struct Scripted {
    replies: RefCell<VecDeque<Result<Value, TransportError>>>,
    bodies: RefCell<Vec<Value>>,
}

impl Scripted {
    fn new(replies: Vec<Result<Value, TransportError>>) -> Scripted {
        Scripted {
            replies: RefCell::new(replies.into()),
            bodies: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.bodies.borrow().len()
    }
}

impl Transport for Scripted {
    fn post(&self, body: &Value) -> Result<Value, TransportError> {
        self.bodies.borrow_mut().push(body.clone());
        self.replies
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(TransportError::Network("no scripted reply".to_owned())))
    }
}

fn answers(scores: &[f64]) -> Value {
    let mut answers = serde_json::Map::new();
    for (i, s) in scores.iter().enumerate() {
        answers.insert(
            format!("s{i}"),
            json!({"type": "score", "score": s, "probabilities": {}, "confidence": 0.9}),
        );
    }
    json!({"answers": answers, "model": "jev-test"})
}

fn opts(batch: usize) -> Options {
    Options {
        batch,
        model: "jev-latest".to_owned(),
        force: false,
        budget: RUN_BUDGET,
        backoff: [Duration::ZERO, Duration::ZERO],
        now_unix: 1_700_000_000,
    }
}

fn catalog() -> Catalog {
    Catalog {
        skills: vec![
            skill("alpha", "Alpha.", Host::Claude, Source::User),
            skill("beta", "Beta.", Host::Claude, Source::User),
            skill("alpha", "Alpha.", Host::Codex, Source::User),
            skill("gamma", "Gamma.", Host::Codex, Source::System),
        ],
        ..Catalog::default()
    }
}

fn no_save() -> impl FnMut(&Catalog) -> Result<(), String> {
    |_: &Catalog| Ok(())
}

#[test]
fn pending_merges_identical_copies_and_skips_fresh_ratings() {
    let mut c = catalog();
    let p = pending(&c, "sha", false);
    let names: Vec<&str> = p.iter().map(|p| p.state.name.as_str()).collect();
    assert_eq!(names, ["alpha", "beta", "gamma"]);
    assert_eq!(p[0].state.hosts, ["claude", "codex"]);

    let beta = c.skills[1].key.clone();
    let rating = |sha: &str| Rating {
        rating: 0.5,
        confidence: None,
        probabilities: Value::Null,
        model: String::new(),
        profile_sha: sha.to_owned(),
        rated_at: 0,
    };
    c.ratings.insert(beta.clone(), rating("sha"));
    assert_eq!(pending(&c, "sha", false).len(), 2);
    assert_eq!(
        pending(&c, "sha", true).len(),
        3,
        "--force rates everything again"
    );
    c.ratings.insert(beta, rating("older-profile"));
    assert_eq!(
        pending(&c, "sha", false).len(),
        3,
        "a changed profile makes ratings stale"
    );
}

#[test]
fn batches_are_sent_in_order_and_ratings_stored_per_key() {
    let mut c = catalog();
    let t = Scripted::new(vec![Ok(answers(&[4.0, 1.0])), Ok(answers(&[2.0]))]);
    let mut saves = 0;
    let mut save = |_: &Catalog| {
        saves += 1;
        Ok(())
    };
    let report = run(
        &mut c,
        "profile",
        &Redactor::default(),
        &opts(2),
        &t,
        None,
        &mut save,
    );
    assert_eq!(report.rated, 3);
    assert_eq!(report.batches, 2);
    assert!(report.failures.is_empty() && report.stopped.is_none());
    assert_eq!(saves, 2, "the catalog is saved after every batch");
    assert_eq!(t.calls(), 2);
    let alpha = &c.ratings[&c.skills[0].key];
    assert!((alpha.rating - 1.0).abs() < 1e-9);
    assert_eq!(alpha.model, "jev-test");
    assert_eq!(alpha.profile_sha, sha256_hex(b"profile"));
    assert_eq!(alpha.rated_at, 1_700_000_000);
    assert!((c.ratings[&c.skills[3].key].rating - 0.5).abs() < 1e-9);
    assert_eq!(
        t.bodies.borrow()[1]["state"]["skills"]["s0"]["name"],
        "gamma"
    );
}

#[test]
fn overload_is_retried_at_most_three_times() {
    let mut c = catalog();
    let busy = || Err(TransportError::Status(529, "overloaded".to_owned()));
    let t = Scripted::new(vec![
        busy(),
        Err(TransportError::Status(429, String::new())),
        Ok(answers(&[3.0, 3.0, 3.0])),
    ]);
    let report = run(
        &mut c,
        "p",
        &Redactor::default(),
        &opts(20),
        &t,
        None,
        &mut no_save(),
    );
    assert_eq!(t.calls(), 3);
    assert_eq!(report.rated, 3);

    let mut c = catalog();
    let t = Scripted::new(vec![busy(), busy(), busy(), Ok(answers(&[3.0]))]);
    let report = run(
        &mut c,
        "p",
        &Redactor::default(),
        &opts(20),
        &t,
        None,
        &mut no_save(),
    );
    assert_eq!(t.calls(), 3, "a fourth try is never made");
    assert_eq!(report.rated, 0);
    assert_eq!(report.failures, ["batch 0: HTTP 529 after 3 tries"]);
}

#[test]
fn unauthorized_stops_the_run_and_other_errors_skip_the_batch() {
    let mut c = catalog();
    let t = Scripted::new(vec![
        Err(TransportError::Status(401, String::new())),
        Ok(answers(&[1.0])),
    ]);
    let report = run(
        &mut c,
        "p",
        &Redactor::default(),
        &opts(1),
        &t,
        None,
        &mut no_save(),
    );
    assert_eq!(t.calls(), 1);
    assert!(report
        .stopped
        .as_deref()
        .is_some_and(|s| s.starts_with("HTTP 401")));

    let mut c = catalog();
    let t = Scripted::new(vec![
        Err(TransportError::Status(422, "bad question".to_owned())),
        Ok(answers(&[1.0])),
        Ok(json!({"answers": {}})),
    ]);
    let report = run(
        &mut c,
        "p",
        &Redactor::default(),
        &opts(1),
        &t,
        None,
        &mut no_save(),
    );
    assert_eq!(t.calls(), 3);
    assert_eq!(report.rated, 1);
    assert_eq!(
        report.failures,
        [
            "batch 0: HTTP 422: bad question",
            "batch 2: 1 of 1 unanswered"
        ]
    );
}

#[test]
fn an_exhausted_budget_sends_nothing() {
    let mut c = catalog();
    let t = Scripted::new(vec![Ok(answers(&[1.0]))]);
    let mut o = opts(20);
    o.budget = Duration::from_secs(1);
    let report = run(
        &mut c,
        "p",
        &Redactor::default(),
        &o,
        &t,
        None,
        &mut no_save(),
    );
    assert_eq!(t.calls(), 0);
    assert!(report.stopped.is_some());
}

#[test]
fn every_attempt_is_logged_as_request_and_response() {
    let dir = TempDir::new("jev-log");
    let mut c = catalog();
    let t = Scripted::new(vec![
        Err(TransportError::Status(429, "slow down".to_owned())),
        Ok(answers(&[1.0, 1.0, 1.0])),
    ]);
    run(
        &mut c,
        "p",
        &Redactor::default(),
        &opts(20),
        &t,
        Some(dir.path()),
        &mut no_save(),
    );
    let mut names: Vec<String> = fs::read_dir(dir.path())
        .expect("log dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "1700000000-b000-a0-request.json",
            "1700000000-b000-a0-response.json",
            "1700000000-b000-a1-request.json",
            "1700000000-b000-a1-response.json",
        ]
    );
    let first: Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("1700000000-b000-a0-response.json")).expect("read"),
    )
    .expect("json");
    assert_eq!(first, json!({"status": 429, "body": "slow down"}));
}

#[test]
fn first_request_is_the_first_batch_only() {
    let c = catalog();
    let req = first_request(&c, "p", &Redactor::default(), &opts(2)).expect("a request");
    assert_eq!(
        req["questions"].as_object().map(serde_json::Map::len),
        Some(2)
    );
}
