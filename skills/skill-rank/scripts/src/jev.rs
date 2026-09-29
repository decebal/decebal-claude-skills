//! The `TypeSafe` System One request that rates a batch of skills, and its answer.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use regex::{Regex, RegexBuilder};
use serde_json::{json, Map, Value};

pub const URL: &str = "https://api.typesafe.ai/v1/systemone";
pub const DEFAULT_MODEL: &str = "jev-latest";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const DESCRIPTION_LIMIT: usize = 700;

pub const CRITERIA: [&str; 5] = [
    "Unrelated: nothing in the profile's work calls for this skill, and it has not been used.",
    "Marginal: it touches the profile's work only at the edges, or duplicates a more specific skill.",
    "Occasional: it helps with a kind of task the profile's work involves now and then.",
    "Regular: it fits a recurring part of the profile's work.",
    "Core: the profile's main, frequent work needs it, or it is used often.",
];

/// The model never sees question ids, so the text names the state path itself.
pub fn instructions(id: &str) -> String {
    format!(
        "Rate how much keeping the skill `skills.{id}` ready in an AI coding agent's context would help \
         with the work described in `profile`. Judge the skill's purpose against that work. Its recent_use \
         counts are evidence of need; zero use is weak evidence against it, not proof."
    )
}

/// Replaces every case-insensitive occurrence of each term with `[redacted]`.
#[derive(Debug, Default)]
pub struct Redactor {
    pattern: Option<Regex>,
}

impl Redactor {
    /// Fails when the terms do not compile into one pattern: sending text
    /// unredacted because a pattern failed to build would defeat the list.
    pub fn new(terms: &[String]) -> Result<Redactor, String> {
        let mut terms: Vec<&str> = terms
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        terms.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        terms.dedup();
        if terms.is_empty() {
            return Ok(Redactor::default());
        }
        let alternation = terms
            .iter()
            .map(|t| regex::escape(t))
            .collect::<Vec<_>>()
            .join("|");
        let pattern = RegexBuilder::new(&alternation)
            .case_insensitive(true)
            .build()
            .map_err(|e| {
                format!("redact.txt does not build into a pattern, so nothing was sent: {e}")
            })?;
        Ok(Redactor {
            pattern: Some(pattern),
        })
    }

    /// One term per line; blank lines are ignored.
    pub fn from_text(text: &str) -> Result<Redactor, String> {
        let terms: Vec<String> = text.lines().map(str::to_owned).collect();
        Redactor::new(&terms)
    }

    pub fn apply(&self, s: &str) -> String {
        match &self.pattern {
            Some(re) => re.replace_all(s, "[redacted]").into_owned(),
            None => s.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillState {
    pub name: String,
    pub description: String,
    pub claude_use: u64,
    pub codex_use: u64,
    pub hosts: Vec<String>,
}

pub fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn question_id(i: usize) -> String {
    format!("s{i}")
}

pub fn build_request(
    model: &str,
    profile: &str,
    skills: &[SkillState],
    redact: &Redactor,
) -> Value {
    let mut state_skills = Map::new();
    let mut questions = Map::new();
    for (i, s) in skills.iter().enumerate() {
        let id = question_id(i);
        state_skills.insert(
            id.clone(),
            json!({
                "name": redact.apply(&s.name),
                "description": truncate_chars(&redact.apply(&s.description), DESCRIPTION_LIMIT),
                "recent_use": {"claude": s.claude_use, "codex": s.codex_use},
                "hosts": s.hosts,
            }),
        );
        questions.insert(
            id.clone(),
            json!({
                "type": "score",
                "instructions": instructions(&id),
                "criteria": CRITERIA,
            }),
        );
    }
    json!({
        "model": model,
        "state": {"profile": redact.apply(profile), "skills": state_skills},
        "questions": questions,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub rating: f64,
    pub confidence: Option<f64>,
    pub probabilities: Value,
}

/// Answers by batch index, normalised to 0..=1, and the model that gave them.
pub fn parse_response(resp: &Value, count: usize) -> (BTreeMap<usize, Answer>, String) {
    let top = (CRITERIA.len() - 1) as f64;
    let mut out = BTreeMap::new();
    for i in 0..count {
        let Some(a) = resp.get("answers").and_then(|a| a.get(question_id(i))) else {
            continue;
        };
        let Some(score) = a.get("score").and_then(Value::as_f64) else {
            continue;
        };
        out.insert(
            i,
            Answer {
                rating: (score / top).clamp(0.0, 1.0),
                confidence: a.get("confidence").and_then(Value::as_f64),
                probabilities: a.get("probabilities").cloned().unwrap_or(Value::Null),
            },
        );
    }
    let model = resp
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    (out, model)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    Status(u16, String),
    Network(String),
}

pub trait Transport {
    fn post(&self, body: &Value) -> Result<Value, TransportError>;
}

pub struct HttpTransport {
    agent: ureq::Agent,
    key: String,
}

impl HttpTransport {
    pub fn new(key: String) -> HttpTransport {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(REQUEST_TIMEOUT)
            .build();
        HttpTransport { agent, key }
    }
}

impl Transport for HttpTransport {
    fn post(&self, body: &Value) -> Result<Value, TransportError> {
        let result = self
            .agent
            .post(URL)
            .set("Authorization", &format!("Bearer {}", self.key))
            .send_json(body.clone());
        match result {
            Ok(resp) => resp
                .into_json()
                .map_err(|e| TransportError::Network(format!("response did not parse: {e}"))),
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Err(TransportError::Status(code, truncate_chars(&text, 2000)))
            }
            Err(ureq::Error::Transport(t)) => Err(TransportError::Network(t.to_string())),
        }
    }
}

/// `TYPESAFE_API_KEY`, else `~/.config/typesafe/key`, trimmed.
pub fn api_key(home: &Path) -> Option<String> {
    if let Ok(k) = std::env::var("TYPESAFE_API_KEY") {
        if !k.trim().is_empty() {
            return Some(k.trim().to_owned());
        }
    }
    let k = fs::read_to_string(home.join(".config/typesafe/key")).ok()?;
    let k = k.trim();
    (!k.is_empty()).then(|| k.to_owned())
}

#[cfg(test)]
#[path = "jev_tests.rs"]
mod tests;
