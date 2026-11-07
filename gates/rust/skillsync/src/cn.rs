//! Lint the `cn` commands that SKILL.md files document against `cn --help`.
//!
//! Only code spans and fenced blocks are read, so prose such as "cn create" in a
//! trigger list is not treated as a command. Every child is bounded by a deadline
//! and reaped: killed, then waited on, when it does not answer in time.

use crate::repo::Repo;
use crate::report::Report;
use std::collections::BTreeMap;
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DEADLINE: Duration = Duration::from_secs(10);
/// Commands whose first word is a group, so the second word is the subcommand.
const GROUPS: [&str; 2] = ["task", "dep"];

pub fn lint(cn_bin: &str, repo: &Repo, report: &mut Report) -> Result<(), String> {
    match run_bounded(cn_bin, &["--version"]) {
        Err(_) => {
            report.notice(format!("{cn_bin} not found; cn lint skipped"));
            return Ok(());
        }
        Ok(None) => {
            report.finding("cn-timeout", cn_bin, "`--version` did not answer in time");
            return Ok(());
        }
        Ok(Some(_)) => {}
    }

    let mut seen: BTreeMap<Vec<String>, String> = BTreeMap::new();
    for skill in repo.skills()? {
        let file = skill.dir.join("SKILL.md");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        for path in mentions(&text) {
            seen.entry(path)
                .or_insert_with(|| file.display().to_string());
        }
    }

    for (path, file) in &seen {
        let shown = format!("cn {}", path.join(" "));
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        match run_bounded(cn_bin, &args) {
            Ok(Some(true)) => {}
            Ok(Some(false)) => report.finding(
                "cn-unknown",
                file,
                format!("`{shown} --help` exits non-zero"),
            ),
            Ok(None) => report.finding(
                "cn-timeout",
                file,
                format!("`{shown} --help` did not answer in time"),
            ),
            Err(e) => report.finding("cn-unknown", file, format!("`{shown}`: {e}")),
        }
    }
    Ok(())
}

/// `Ok(Some(success))` once the child exits, `Ok(None)` if it was killed at the
/// deadline, `Err` if it could not be spawned.
fn run_bounded(bin: &str, args: &[&str]) -> std::io::Result<Option<bool>> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + DEADLINE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(Some(status.success())),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Command paths named in the code spans and fenced blocks of one SKILL.md.
pub fn mentions(text: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            scan(line, &mut out);
        } else {
            for (i, span) in line.split('`').enumerate() {
                if i % 2 == 1 {
                    scan(span, &mut out);
                }
            }
        }
    }
    out
}

fn scan(segment: &str, out: &mut Vec<Vec<String>>) {
    for (pos, _) in segment.match_indices("cn ") {
        let glued = segment[..pos]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || "-_./".contains(c));
        if glued {
            continue;
        }
        let mut words = segment[pos + 3..].split_whitespace();
        let Some(first) = words.next().filter(|w| is_word(w)) else {
            continue;
        };
        let mut path = vec![first.to_string()];
        if GROUPS.contains(&first) {
            if let Some(second) = words.next().filter(|w| is_word(w)) {
                path.push(second.to_string());
            }
        }
        out.push(path);
    }
}

fn is_word(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_lowercase())
        && word.chars().all(|c| c.is_ascii_lowercase() || c == '-')
}
