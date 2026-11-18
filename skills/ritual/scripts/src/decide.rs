use std::path::Path;
use std::time::Duration;

use serde_json::{Map, Value, json};

pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const QUESTION_ID: &str = "decision";
/// At or above: act without telling anyone.
pub const ACT_MIN: f64 = 0.80;
/// At or above: act, and name the runner-up in the step report. Below: ask the user.
pub const REPORT_MIN: f64 = 0.50;

#[derive(Debug, PartialEq)]
pub enum Policy {
    Act,
    ActAndReport,
    Ask,
}

impl Policy {
    pub fn of(confidence: f64) -> Self {
        if confidence >= ACT_MIN {
            Policy::Act
        } else if confidence >= REPORT_MIN {
            Policy::ActAndReport
        } else {
            Policy::Ask
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Policy::Act => "act",
            Policy::ActAndReport => "act-and-report",
            Policy::Ask => "ask",
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Decision {
    pub choice: String,
    pub confidence: f64,
    /// Every option, most probable first.
    pub probabilities: Vec<(String, f64)>,
    pub input_tokens: Option<u64>,
}

impl Decision {
    pub fn policy(&self) -> Policy {
        Policy::of(self.confidence)
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "decide: {} {} confidence={:.2}",
            self.choice,
            self.policy().as_str(),
            self.confidence
        );
        if let Some((name, p)) = self.probabilities.iter().find(|(n, _)| *n != self.choice) {
            out.push_str(&format!(" · runner-up {name} p={p:.2}"));
        }
        if let Some(t) = self.input_tokens {
            out.push_str(&format!(" · {t} input tokens"));
        }
        out
    }

    pub fn to_json(&self) -> Value {
        json!({
            "choice": self.choice,
            "confidence": self.confidence,
            "policy": self.policy().as_str(),
            "probabilities": self.probabilities.iter().map(|(n, p)| json!({"option": n, "p": p})).collect::<Vec<_>>(),
            "input_tokens": self.input_tokens,
        })
    }
}

/// Parses `name=description` options. A bare `name` is judged by its name alone.
pub fn parse_options(raw: &[String]) -> Result<Vec<(String, String)>, String> {
    let options: Vec<(String, String)> = raw
        .iter()
        .map(|o| match o.split_once('=') {
            Some((n, d)) => (n.trim().to_string(), d.trim().to_string()),
            None => (o.trim().to_string(), String::new()),
        })
        .collect();
    if options.len() < 2 {
        return Err("a decision needs at least two --option values".into());
    }
    if options.len() > 255 {
        return Err("TypeSafe accepts at most 255 options".into());
    }
    let mut names: Vec<&str> = options.iter().map(|(n, _)| n.as_str()).collect();
    if names.iter().any(|n| n.is_empty()) {
        return Err("an option has an empty name".into());
    }
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return Err("option names must be unique".into());
    }
    Ok(options)
}

/// JSON state stays structured so the question can reference its fields; anything
/// else is sent as text.
pub fn parse_state(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

pub fn request_body(
    model: &str,
    question: &str,
    options: &[(String, String)],
    state: Value,
) -> Value {
    let criteria: Map<String, Value> = options
        .iter()
        .map(|(n, d)| {
            let desc = if d.is_empty() {
                Value::Null
            } else {
                Value::String(d.clone())
            };
            (n.clone(), desc)
        })
        .collect();
    json!({
        "model": model,
        "state": state,
        "questions": {
            QUESTION_ID: {
                "type": "choice",
                "instructions": question,
                "criteria": criteria,
            }
        }
    })
}

pub fn parse_response(resp: &Value) -> Result<Decision, String> {
    let answer = resp
        .get("answers")
        .and_then(|a| a.get(QUESTION_ID))
        .ok_or_else(|| format!("response has no answers.{QUESTION_ID}: {resp}"))?;
    let choice = answer
        .get("choice")
        .and_then(Value::as_str)
        .ok_or("answer has no choice")?
        .to_string();
    let confidence = answer
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or("answer has no confidence")?;
    if !(0.0..=1.0).contains(&confidence) {
        return Err(format!("confidence {confidence} is outside 0..=1"));
    }
    let mut probabilities: Vec<(String, f64)> = answer
        .get("probabilities")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_f64().map(|p| (k.clone(), p)))
                .collect()
        })
        .unwrap_or_default();
    probabilities.sort_by(|a, b| b.1.total_cmp(&a.1));
    let input_tokens = resp
        .get("usage")
        .and_then(|u| u.get("input_tokens"))
        .and_then(Value::as_u64);
    Ok(Decision {
        choice,
        confidence,
        probabilities,
        input_tokens,
    })
}

/// `env_key` (the caller passes `$TYPESAFE_API_KEY`), else `~/.config/typesafe/key`.
pub fn api_key(env_key: Option<String>, home: &Path) -> Result<String, String> {
    if let Some(k) = env_key
        && !k.trim().is_empty()
    {
        return Ok(k.trim().to_string());
    }
    let path = home.join(".config").join("typesafe").join("key");
    let key = std::fs::read_to_string(&path)
        .map_err(|e| format!("no TYPESAFE_API_KEY and {}: {e}", path.display()))?;
    let key = key.trim();
    if key.is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    Ok(key.to_string())
}

pub fn call(key: &str, body: &Value, timeout: Duration) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut resp = agent
        .post(ENDPOINT)
        .header("Authorization", &format!("Bearer {key}"))
        .send_json(body)
        .map_err(|e| format!("TypeSafe: {e}"))?;
    resp.body_mut()
        .read_json::<Value>()
        .map_err(|e| format!("TypeSafe response: {e}"))
}
