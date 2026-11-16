//! What counts as handing the user a command, and what does not.

use super::{handoff, last_assistant_text, reason};

#[test]
fn a_bang_command_line_is_a_handoff() {
    for text in [
        "Run this:\n\n```\n! gh pr create -R o/r --base develop\n```",
        "! git -C ~/x switch -c docs/VM-1-thing",
        "  ! kubectl --context c get ns",
        "> ! make deploy",
        "- ! ./scripts/release.sh",
        "Run `! gh pr create --fill` to open it.",
    ] {
        assert!(handoff(text).is_some(), "missed: {text}");
    }
}

#[test]
fn prose_and_ordinary_bangs_are_not() {
    for text in [
        "Done! Pushed and verified.",
        "!important note",
        "if x != y { return }",
        "The guard says `!` is banned.",
        "Approve with AskUserQuestion, then I run it.",
        "",
    ] {
        assert!(handoff(text).is_none(), "false positive: {text}");
    }
}

#[test]
fn the_last_assistant_message_is_the_one_read() {
    let lines = [
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"! old command"}]}}"#,
        r#"{"type":"user","message":{"content":"ok"}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#,
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"All done."}]}}"#,
    ]
    .join("\n");
    assert_eq!(last_assistant_text(lines.as_bytes()), "All done.");
}

/// Every guard's reason text, in source: none may tell the agent the user runs
/// a command. That instruction is how a deny turned into a copy-paste handoff.
#[test]
fn no_guard_reason_hands_the_user_a_command() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.clone()];
    let mut offenders = Vec::new();
    while let Some(dir) = files.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().map(Result::unwrap) {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                files.push(path);
                continue;
            }
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") || name == "handoff_guard.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                if line.contains("`! ")
                    || line.contains("run it themselves")
                    || line.contains("they run `")
                {
                    offenders.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}

#[test]
fn the_reason_tells_the_model_to_run_it_not_the_user() {
    let r = reason("! gh pr create");
    assert!(r.contains("Run it yourself") && r.contains("AskUserQuestion"));
    assert!(!r.contains("user runs") && !r.contains("themselves"));
}
