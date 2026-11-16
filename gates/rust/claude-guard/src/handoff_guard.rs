//! `Stop` hook: the agent may not end its turn by handing the user a command
//! to run. A `! <command>` line, or an inline `` `! cmd` ``, in the last
//! assistant message blocks the stop once, with the reason sent back to the
//! model: run it, or ask through AskUserQuestion. Approval belongs to the user,
//! execution to the agent.
//!
//! `stop_hook_active` is set when the model is already continuing because of a
//! Stop hook; the second stop is allowed, so a quoted example cannot loop.

use std::io::{BufRead, BufReader};

use serde_json::{json, Value};

use crate::hook;

pub fn run(payload: &Value) -> ! {
    if payload.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
        hook::allow();
    }
    let text = match payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
    {
        Some(t) => t.to_owned(),
        None => {
            let path = hook::str_at(payload, &["transcript_path"]);
            match std::fs::File::open(path) {
                Ok(f) => last_assistant_text(BufReader::new(f)),
                Err(_) => hook::allow(),
            }
        }
    };
    match handoff(&text) {
        Some(line) => {
            let out = json!({ "decision": "block", "reason": reason(&line) });
            println!("{out}");
            std::process::exit(0)
        }
        None => hook::allow(),
    }
}

/// The text of the last assistant message in a transcript, its text parts joined.
pub(crate) fn last_assistant_text(reader: impl BufRead) -> String {
    let mut last = String::new();
    for line in reader.lines().map_while(Result::ok) {
        if !line.contains("\"assistant\"") {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let parts: Vec<&str> = entry
            .pointer("/message/content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|c| c.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|c| c.get("text").and_then(Value::as_str))
            .collect();
        if !parts.is_empty() {
            last = parts.join("\n");
        }
    }
    last
}

/// The first line that hands the user a command, if any.
pub(crate) fn handoff(text: &str) -> Option<String> {
    for line in text.lines() {
        let t = line
            .trim_start()
            .trim_start_matches(['>', '-', '*'])
            .trim_start();
        let bang_line =
            t.strip_prefix('!').is_some_and(|rest| {
                rest.starts_with(' ')
                    && rest.trim().chars().next().is_some_and(|c| {
                        c.is_ascii_alphanumeric() || c == '/' || c == '.' || c == '~'
                    })
            });
        let inline = line.contains("`! ");
        if bang_line || inline {
            return Some(line.trim().to_owned());
        }
    }
    None
}

pub(crate) fn reason(line: &str) -> String {
    format!(
        "Your reply hands the user a command to run: `{line}`. Run it yourself. If it needs the user's approval, ask with AskUserQuestion and then run it, or let the guard's permission prompt ask. If you were only quoting one, say it without the `!` prefix."
    )
}

#[cfg(test)]
#[path = "handoff_guard_tests.rs"]
mod handoff_guard_tests;
