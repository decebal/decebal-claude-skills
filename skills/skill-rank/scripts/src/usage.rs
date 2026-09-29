//! Recent skill use, read from Claude Code transcripts and Codex rollouts.

use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::hash::BuildHasher;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant, SystemTime};

use regex::Regex;
use serde_json::Value;

use crate::catalog::UsageMap;

/// A longer line is skipped unread: skill calls are small, tool results are not.
const MAX_LINE: usize = 8 << 20;
const MAX_FILE: u64 = 1 << 30;
const MAX_DEPTH: usize = 5;
/// Project dir → session → `subagents` → `workflows` → run → transcript.
const CLAUDE_DEPTH: usize = 5;
pub const SCAN_TIME: Duration = Duration::from_mins(2);
pub const SCAN_BYTES: u64 = 4 << 30;

static COMMAND: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<command-name>/?([A-Za-z0-9][A-Za-z0-9_.:-]*)</command-name>")
        .expect("valid regex")
});
static SKILL_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z0-9_.-]+)/SKILL\.md").expect("valid regex"));

/// One limit on time and bytes for a whole usage scan. Files are read newest
/// first, so a scan cut short loses the oldest use.
#[derive(Debug)]
pub struct ScanBudget {
    deadline: Instant,
    bytes_left: u64,
    pub truncated: bool,
}

impl ScanBudget {
    pub fn new(time: Duration, bytes: u64) -> ScanBudget {
        ScanBudget {
            deadline: Instant::now() + time,
            bytes_left: bytes,
            truncated: false,
        }
    }

    /// Whether `len` more bytes may be read; marks the scan truncated when not.
    fn take(&mut self, len: u64) -> bool {
        if self.truncated || Instant::now() >= self.deadline || len > self.bytes_left {
            self.truncated = true;
            return false;
        }
        self.bytes_left -= len;
        true
    }
}

/// The name a use is counted under: the invoked name without a leading `/`,
/// keeping a `plugin:` namespace so a plugin skill never counts toward a user
/// skill of the same name.
pub fn invoked(name: &str) -> &str {
    name.trim().trim_start_matches('/')
}

/// One Claude Code transcript line: `Skill` tool calls and typed slash commands.
/// `seen` holds `tool_use` ids already counted, since a subagent transcript can
/// repeat its parent's messages.
pub fn claude_line<S: BuildHasher>(line: &str, out: &mut UsageMap, seen: &mut HashSet<String, S>) {
    if !line.contains("\"Skill\"") && !line.contains("<command-name>") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let ts = v.get("timestamp").and_then(Value::as_str);
    let content = v.pointer("/message/content");
    match v.get("type").and_then(Value::as_str) {
        Some("assistant") => {
            for item in content.and_then(Value::as_array).into_iter().flatten() {
                if item.get("type").and_then(Value::as_str) != Some("tool_use")
                    || item.get("name").and_then(Value::as_str) != Some("Skill")
                {
                    continue;
                }
                if let Some(id) = item.get("id").and_then(Value::as_str) {
                    if !seen.insert(id.to_owned()) {
                        continue;
                    }
                }
                if let Some(skill) = item.pointer("/input/skill").and_then(Value::as_str) {
                    out.entry(invoked(skill).to_owned()).or_default().bump(ts);
                }
            }
        }
        Some("user") if v.get("isMeta").and_then(Value::as_bool) != Some(true) => {
            for text in user_texts(content) {
                for c in COMMAND.captures_iter(text) {
                    out.entry(invoked(&c[1]).to_owned()).or_default().bump(ts);
                }
            }
        }
        _ => {}
    }
}

fn user_texts(content: Option<&Value>) -> Vec<&str> {
    match content {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(items)) => items
            .iter()
            .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect(),
        _ => Vec::new(),
    }
}

/// One Codex rollout line: a tool call that reads `.../<skill dir>/SKILL.md`.
pub fn codex_line(line: &str, out: &mut UsageMap) {
    if !line.contains("SKILL.md") {
        return;
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if v.get("type").and_then(Value::as_str) != Some("response_item") {
        return;
    }
    let payload = v.get("payload");
    let text = match payload.and_then(|p| p.get("type")).and_then(Value::as_str) {
        Some("function_call") => payload.and_then(|p| p.get("arguments")),
        Some("custom_tool_call") => payload.and_then(|p| p.get("input")),
        _ => None,
    };
    let Some(text) = text.and_then(Value::as_str) else {
        return;
    };
    let ts = v.get("timestamp").and_then(Value::as_str);
    let dirs: BTreeSet<&str> = SKILL_PATH
        .captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect();
    for dir in dirs {
        out.entry(dir.to_owned()).or_default().bump(ts);
    }
}

fn is_jsonl(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
}

pub fn scan_claude(home: &Path, since: SystemTime, budget: &mut ScanBudget) -> UsageMap {
    let mut out = UsageMap::new();
    let mut seen = HashSet::new();
    let projects = home.join(".claude/projects");
    for (file, len) in newest_first(files(&projects, CLAUDE_DEPTH + 1, since, is_jsonl)) {
        if !budget.take(len) {
            break;
        }
        for_each_line(&file, |line| claude_line(line, &mut out, &mut seen));
    }
    out
}

pub fn scan_codex(home: &Path, since: SystemTime, budget: &mut ScanBudget) -> UsageMap {
    let mut out = UsageMap::new();
    let sessions = home.join(".codex/sessions");
    let rollouts = files(&sessions, MAX_DEPTH, since, |n| {
        n.starts_with("rollout-") && is_jsonl(n)
    });
    for (file, len) in newest_first(rollouts) {
        if !budget.take(len) {
            break;
        }
        for_each_line(&file, |line| codex_line(line, &mut out));
    }
    out
}

/// `(path, bytes to read)`, most recently modified first.
fn newest_first(paths: Vec<PathBuf>) -> Vec<(PathBuf, u64)> {
    let mut with_meta: Vec<(PathBuf, u64, Option<SystemTime>)> = paths
        .into_iter()
        .map(|p| {
            let meta = fs::metadata(&p).ok();
            let len = meta.as_ref().map_or(0, |m| m.len().min(MAX_FILE));
            let modified = meta.and_then(|m| m.modified().ok());
            (p, len, modified)
        })
        .collect();
    with_meta.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    with_meta.into_iter().map(|(p, len, _)| (p, len)).collect()
}

/// Files under `dir` (up to `depth` levels) whose name passes `keep` and whose
/// mtime is at or after `since`.
pub fn files(
    dir: &Path,
    depth: usize,
    since: SystemTime,
    keep: impl Fn(&str) -> bool + Copy,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            if depth > 1 {
                out.extend(files(&path, depth - 1, since, keep));
            }
            continue;
        }
        let name = entry.file_name();
        let recent = meta.modified().is_ok_and(|m| m >= since);
        if recent && keep(&name.to_string_lossy()) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Calls `f` for each UTF-8 line of at most `MAX_LINE` bytes, reading at most
/// `MAX_FILE` bytes; longer lines are skipped without being buffered.
pub fn for_each_line(path: &Path, mut f: impl FnMut(&str)) {
    let Ok(file) = File::open(path) else { return };
    let mut reader = BufReader::with_capacity(1 << 16, file.take(MAX_FILE));
    let mut buf: Vec<u8> = Vec::new();
    let mut skipping = false;
    loop {
        let (used, complete) = {
            let Ok(chunk) = reader.fill_buf() else { return };
            if chunk.is_empty() {
                if !skipping && !buf.is_empty() {
                    emit(&buf, &mut f);
                }
                return;
            }
            let newline = chunk.iter().position(|&b| b == b'\n');
            let take = newline.unwrap_or(chunk.len());
            if !skipping {
                if buf.len() + take > MAX_LINE {
                    skipping = true;
                    buf.clear();
                } else {
                    buf.extend_from_slice(&chunk[..take]);
                }
            }
            (newline.map_or(take, |i| i + 1), newline.is_some())
        };
        reader.consume(used);
        if complete {
            if !skipping {
                emit(&buf, &mut f);
            }
            buf.clear();
            skipping = false;
        }
    }
}

fn emit(buf: &[u8], f: &mut impl FnMut(&str)) {
    if let Ok(line) = std::str::from_utf8(buf) {
        f(line);
    }
}

#[cfg(test)]
#[path = "usage_tests.rs"]
mod tests;
