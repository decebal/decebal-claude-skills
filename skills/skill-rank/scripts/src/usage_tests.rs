use serde_json::json;

use super::*;
use crate::catalog::Use;
use crate::test_support::{write, TempDir};

fn counts(m: &UsageMap) -> Vec<(String, u64)> {
    m.iter().map(|(k, u)| (k.clone(), u.count)).collect()
}

fn owned(pairs: &[(&str, u64)]) -> Vec<(String, u64)> {
    pairs.iter().map(|(k, n)| ((*k).to_owned(), *n)).collect()
}

fn unlimited() -> ScanBudget {
    ScanBudget::new(Duration::from_mins(1), u64::MAX)
}

#[test]
fn invoked_keeps_the_plugin_namespace_and_drops_the_slash() {
    assert_eq!(invoked("vercel:nextjs"), "vercel:nextjs");
    assert_eq!(invoked("/tdd"), "tdd");
    assert_eq!(invoked("plain"), "plain");
}

#[test]
fn claude_skill_tool_calls_count_under_the_invoked_name() {
    let line = json!({
        "type": "assistant",
        "timestamp": "2026-09-01T10:00:00.000Z",
        "message": {"content": [
            {"type": "text", "text": "loading"},
            {"type": "tool_use", "id": "toolu_1", "name": "Skill", "input": {"skill": "vercel:nextjs", "args": "x"}},
            {"type": "tool_use", "id": "toolu_2", "name": "Skill", "input": {"skill": "nextjs"}},
            {"type": "tool_use", "name": "Bash", "input": {"command": "echo \"Skill\""}}
        ]}
    })
    .to_string();
    let mut m = UsageMap::new();
    let mut seen = HashSet::new();
    claude_line(&line, &mut m, &mut seen);
    assert_eq!(counts(&m), owned(&[("nextjs", 1), ("vercel:nextjs", 1)]));
    assert_eq!(
        m["nextjs"].last_seen.as_deref(),
        Some("2026-09-01T10:00:00.000Z")
    );
    claude_line(&line, &mut m, &mut seen);
    assert_eq!(
        counts(&m),
        owned(&[("nextjs", 1), ("vercel:nextjs", 1)]),
        "a repeated tool_use id is counted once"
    );
}

#[test]
fn claude_typed_slash_commands_count_from_user_text() {
    let typed = json!({
        "type": "user",
        "timestamp": "2026-09-02T08:00:00.000Z",
        "message": {"role": "user", "content": "<command-message>tdd</command-message>\n<command-name>/tdd</command-name>\n<command-args></command-args>"}
    })
    .to_string();
    let in_array = json!({
        "type": "user",
        "message": {"content": [{"type": "text", "text": "<command-name>/caveman:caveman</command-name>"}]}
    })
    .to_string();
    let mut m = UsageMap::new();
    let mut seen = HashSet::new();
    claude_line(&typed, &mut m, &mut seen);
    claude_line(&in_array, &mut m, &mut seen);
    assert_eq!(counts(&m), owned(&[("caveman:caveman", 1), ("tdd", 1)]));
}

#[test]
fn claude_lines_that_only_mention_the_markers_are_ignored() {
    let tool_docs = json!({
        "type": "attachment",
        "content": "If a `<command-name>` block is already present, the \"Skill\" is loaded"
    })
    .to_string();
    let meta = json!({
        "type": "user",
        "isMeta": true,
        "message": {"content": "<command-name>/tdd</command-name>"}
    })
    .to_string();
    let prose = json!({
        "type": "user",
        "message": {"content": "a `<command-name>` block and the \"Skill\" tool"}
    })
    .to_string();
    let mut m = UsageMap::new();
    let mut seen = HashSet::new();
    for line in [tool_docs, meta, prose, "not json \"Skill\"".to_owned()] {
        claude_line(&line, &mut m, &mut seen);
    }
    assert!(m.is_empty(), "{m:?}");
}

#[test]
fn codex_tool_calls_that_read_a_skill_file_count_once_per_skill() {
    let function_call = json!({
        "timestamp": "2026-09-03T00:00:00.000Z",
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "arguments": json!({"cmd": "sed -n 1,200p /home/u/.codex/skills/proofshot/SKILL.md && wc -l /home/u/.codex/skills/proofshot/SKILL.md"}).to_string()
        }
    })
    .to_string();
    let custom = json!({
        "timestamp": "2026-09-04T00:00:00.000Z",
        "type": "response_item",
        "payload": {
            "type": "custom_tool_call",
            "name": "exec",
            "input": "const r = await tools.exec_command({cmd: \"cat /home/u/.agents/skills/seo-audit/SKILL.md; cat /opt/p/proofshot/SKILL.md\"});"
        }
    })
    .to_string();
    let mut m = UsageMap::new();
    codex_line(&function_call, &mut m);
    codex_line(&custom, &mut m);
    assert_eq!(counts(&m), owned(&[("proofshot", 2), ("seo-audit", 1)]));
    assert_eq!(
        m["proofshot"].last_seen.as_deref(),
        Some("2026-09-04T00:00:00.000Z")
    );
}

#[test]
fn codex_skill_listings_and_messages_are_not_use() {
    let listing = json!({
        "type": "response_item",
        "payload": {"type": "message", "content": [{"type": "input_text", "text": "- bolder: ... (file: r0/bolder/SKILL.md)"}]}
    })
    .to_string();
    let output = json!({
        "type": "response_item",
        "payload": {"type": "function_call_output", "output": "/home/u/.codex/skills/proofshot/SKILL.md"}
    })
    .to_string();
    let event = json!({"type": "event_msg", "payload": {"type": "function_call", "arguments": "skills/x/SKILL.md"}}).to_string();
    let mut m = UsageMap::new();
    for line in [listing, output, event] {
        codex_line(&line, &mut m);
    }
    assert!(m.is_empty(), "{m:?}");
}

#[test]
fn bump_keeps_the_latest_timestamp() {
    let mut u = Use::default();
    u.bump(Some("2026-09-02T00:00:00Z"));
    u.bump(Some("2026-09-01T00:00:00Z"));
    u.bump(None);
    assert_eq!(u.count, 3);
    assert_eq!(u.last_seen.as_deref(), Some("2026-09-02T00:00:00Z"));
}

#[test]
fn for_each_line_skips_an_oversized_line_and_reads_the_rest() {
    let dir = TempDir::new("lines");
    let path = dir.path().join("t.jsonl");
    let big = "x".repeat(MAX_LINE + 10);
    write(&path, &format!("first\n{big}\nlast"));
    let mut seen = Vec::new();
    for_each_line(&path, |l| seen.push(l.to_owned()));
    assert_eq!(seen, vec!["first", "last"]);
}

#[test]
fn scans_read_only_recent_files_under_the_given_home() {
    let home = TempDir::new("usage-home");
    let h = home.path();
    let skill_call = json!({
        "type": "assistant",
        "message": {"content": [{"type": "tool_use", "name": "Skill", "input": {"skill": "tdd"}}]}
    })
    .to_string();
    write(
        &h.join(".claude/projects/p1/session.jsonl"),
        &format!("{skill_call}\n{skill_call}\n"),
    );
    write(&h.join(".claude/projects/p1/notes.txt"), &skill_call);
    write(
        &h.join(".claude/projects/p1/s1/subagents/agent-a.jsonl"),
        &format!("{skill_call}\n"),
    );
    let rollout = json!({
        "type": "response_item",
        "payload": {"type": "function_call", "arguments": "{\"cmd\":\"cat /s/rust-perf/SKILL.md\"}"}
    })
    .to_string();
    write(
        &h.join(".codex/sessions/2026/09/03/rollout-a.jsonl"),
        &rollout,
    );
    write(&h.join(".codex/sessions/2026/09/03/other.jsonl"), &rollout);

    assert_eq!(
        counts(&scan_claude(h, SystemTime::UNIX_EPOCH, &mut unlimited())),
        owned(&[("tdd", 3)]),
        "subagent transcripts count too"
    );
    assert_eq!(
        counts(&scan_codex(h, SystemTime::UNIX_EPOCH, &mut unlimited())),
        owned(&[("rust-perf", 1)])
    );

    let future = SystemTime::now() + Duration::from_hours(1);
    assert!(scan_claude(h, future, &mut unlimited()).is_empty());
    assert!(scan_codex(h, future, &mut unlimited()).is_empty());

    let mut tight = ScanBudget::new(Duration::from_mins(1), 1);
    assert!(scan_claude(h, SystemTime::UNIX_EPOCH, &mut tight).is_empty());
    assert!(
        tight.truncated,
        "a byte budget smaller than one file stops the scan"
    );
}
