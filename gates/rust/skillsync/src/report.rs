//! What a command found and did, rendered as text or as one JSON object.

use serde_json::json;
use std::fmt::Display;
use std::process::ExitCode;

#[derive(Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub notices: Vec<String>,
    pub actions: Vec<String>,
}

pub struct Finding {
    pub kind: &'static str,
    pub path: String,
    pub detail: String,
}

impl Report {
    pub fn finding(&mut self, kind: &'static str, path: impl Display, detail: impl Display) {
        self.findings.push(Finding {
            kind,
            path: path.to_string(),
            detail: detail.to_string(),
        });
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.notices.push(text.into());
    }

    pub fn action(&mut self, text: impl Into<String>) {
        self.actions.push(text.into());
    }
}

pub fn finish(command: &str, report: &Report, as_json: bool) -> ExitCode {
    let ok = report.findings.is_empty();
    if as_json {
        let findings: Vec<_> = report
            .findings
            .iter()
            .map(|f| json!({"kind": f.kind, "path": f.path, "detail": f.detail}))
            .collect();
        let out = json!({
            "command": command,
            "ok": ok,
            "findings": findings,
            "actions": report.actions,
            "notices": report.notices,
        });
        println!("{out}");
    } else {
        for notice in &report.notices {
            println!("note: {notice}");
        }
        for action in &report.actions {
            println!("+ {action}");
        }
        for f in &report.findings {
            println!("{}: {} — {}", f.kind, f.path, f.detail);
        }
        if ok {
            println!("ok: {command}");
        } else {
            println!("{}: {} finding(s)", command, report.findings.len());
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
