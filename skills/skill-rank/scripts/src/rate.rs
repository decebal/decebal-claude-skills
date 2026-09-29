//! Batching unrated skills into Jev requests, with retries inside one run budget.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::catalog::{sha256_hex, Catalog, Rating};
use crate::jev::{
    build_request, parse_response, truncate_chars, Redactor, SkillState, Transport, TransportError,
    REQUEST_TIMEOUT,
};

/// Kept under the five-minute ceiling on any command, with room to save.
pub const RUN_BUDGET: Duration = Duration::from_secs(280);
pub const MAX_TRIES: usize = 3;

pub struct Options {
    pub batch: usize,
    pub model: String,
    pub force: bool,
    pub budget: Duration,
    pub backoff: [Duration; 2],
    pub now_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub key: String,
    pub state: SkillState,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub pending: usize,
    pub rated: usize,
    pub batches: usize,
    pub failures: Vec<String>,
    pub stopped: Option<String>,
}

/// One entry per rating key that has no rating for this profile (every key
/// with `force`), in catalog order.
pub fn pending(catalog: &Catalog, profile_sha: &str, force: bool) -> Vec<Pending> {
    let mut order: Vec<String> = Vec::new();
    let mut by_key: BTreeMap<String, SkillState> = BTreeMap::new();
    for skill in &catalog.skills {
        let fresh = catalog
            .ratings
            .get(&skill.key)
            .is_some_and(|r| r.profile_sha == profile_sha);
        if fresh && !force {
            continue;
        }
        let (claude, codex) = catalog.uses(skill);
        let entry = by_key.entry(skill.key.clone()).or_insert_with(|| {
            order.push(skill.key.clone());
            SkillState {
                name: skill.name.clone(),
                description: skill.description.clone(),
                claude_use: 0,
                codex_use: 0,
                hosts: Vec::new(),
            }
        });
        entry.claude_use = entry.claude_use.max(claude);
        entry.codex_use = entry.codex_use.max(codex);
        let host = skill.host.as_str().to_owned();
        if !entry.hosts.contains(&host) {
            entry.hosts.push(host);
            entry.hosts.sort();
        }
    }
    order
        .into_iter()
        .filter_map(|key| by_key.remove(&key).map(|state| Pending { key, state }))
        .collect()
}

pub fn first_request(
    catalog: &Catalog,
    profile: &str,
    redact: &Redactor,
    opts: &Options,
) -> Option<Value> {
    let todo = pending(catalog, &sha256_hex(profile.as_bytes()), opts.force);
    let states: Vec<SkillState> = todo
        .iter()
        .take(opts.batch.max(1))
        .map(|p| p.state.clone())
        .collect();
    (!states.is_empty()).then(|| build_request(&opts.model, profile, &states, redact))
}

pub fn run(
    catalog: &mut Catalog,
    profile: &str,
    redact: &Redactor,
    opts: &Options,
    transport: &dyn Transport,
    log_dir: Option<&Path>,
    save: &mut dyn FnMut(&Catalog) -> Result<(), String>,
) -> Report {
    let profile_sha = sha256_hex(profile.as_bytes());
    let todo = pending(catalog, &profile_sha, opts.force);
    let started = Instant::now();
    let mut report = Report {
        pending: todo.len(),
        ..Report::default()
    };
    for (b, chunk) in todo.chunks(opts.batch.max(1)).enumerate() {
        if started.elapsed() + REQUEST_TIMEOUT > opts.budget {
            report.stopped = Some("run budget reached; the rest stay unrated".to_owned());
            break;
        }
        let states: Vec<SkillState> = chunk.iter().map(|p| p.state.clone()).collect();
        let body = build_request(&opts.model, profile, &states, redact);
        match send(&body, b, opts, transport, log_dir, started) {
            Sent::Answer(resp) => {
                let (answers, model) = parse_response(&resp, chunk.len());
                if answers.len() < chunk.len() {
                    report.failures.push(format!(
                        "batch {b}: {} of {} unanswered",
                        chunk.len() - answers.len(),
                        chunk.len()
                    ));
                }
                for (i, a) in answers {
                    catalog.ratings.insert(
                        chunk[i].key.clone(),
                        Rating {
                            rating: a.rating,
                            confidence: a.confidence,
                            probabilities: a.probabilities,
                            model: model.clone(),
                            profile_sha: profile_sha.clone(),
                            rated_at: opts.now_unix,
                        },
                    );
                    report.rated += 1;
                }
                report.batches += 1;
                if let Err(e) = save(catalog) {
                    report.stopped = Some(e);
                    break;
                }
            }
            Sent::Fail(msg) => report.failures.push(msg),
            Sent::Stop(msg) => {
                report.stopped = Some(msg);
                break;
            }
        }
    }
    report
}

enum Sent {
    Answer(Value),
    Fail(String),
    Stop(String),
}

fn send(
    body: &Value,
    batch: usize,
    opts: &Options,
    transport: &dyn Transport,
    log_dir: Option<&Path>,
    started: Instant,
) -> Sent {
    for attempt in 0..MAX_TRIES {
        let result = transport.post(body);
        log(log_dir, opts.now_unix, batch, attempt, body, &result);
        match result {
            Ok(v) => return Sent::Answer(v),
            Err(TransportError::Status(401, _)) => {
                return Sent::Stop(
                    "HTTP 401: the TypeSafe key was refused; set TYPESAFE_API_KEY or ~/.config/typesafe/key"
                        .to_owned(),
                )
            }
            Err(TransportError::Status(code @ (429 | 529), _)) => {
                let wait = opts.backoff.get(attempt).copied().unwrap_or_default();
                let last = attempt + 1 == MAX_TRIES;
                if last || started.elapsed() + wait + REQUEST_TIMEOUT > opts.budget {
                    return Sent::Fail(format!("batch {batch}: HTTP {code} after {} tries", attempt + 1));
                }
                std::thread::sleep(wait);
            }
            Err(TransportError::Status(code, text)) => {
                return Sent::Fail(format!("batch {batch}: HTTP {code}: {}", truncate_chars(&text, 300)))
            }
            Err(TransportError::Network(e)) => return Sent::Fail(format!("batch {batch}: {e}")),
        }
    }
    Sent::Fail(format!("batch {batch}: no answer"))
}

fn log(
    dir: Option<&Path>,
    stamp: u64,
    batch: usize,
    attempt: usize,
    body: &Value,
    result: &Result<Value, TransportError>,
) {
    let Some(dir) = dir else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let response = match result {
        Ok(v) => v.clone(),
        Err(TransportError::Status(code, text)) => json!({"status": code, "body": text}),
        Err(TransportError::Network(e)) => json!({"error": e}),
    };
    let base = format!("{stamp}-b{batch:03}-a{attempt}");
    for (suffix, value) in [("request", body), ("response", &response)] {
        let text = serde_json::to_string_pretty(value).unwrap_or_default();
        let _ = fs::write(dir.join(format!("{base}-{suffix}.json")), text + "\n");
    }
}

#[cfg(test)]
#[path = "rate_tests.rs"]
mod tests;
