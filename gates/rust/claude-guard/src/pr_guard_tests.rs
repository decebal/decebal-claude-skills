//! What the one-open-PR guard must deny, and everything it must leave alone.

use super::{
    classify, decide, github_repo, pull_urls, reason, run_bounded, session_prs, Lookup, OpenPr,
    Trigger,
};
use serde_json::json;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// A PR creation as Claude Code records it: the tool_use, then its result
/// carrying `gitOperation`.
fn created_with_git_operation(id: &str, repo: &str, number: u64) -> String {
    let url = format!("https://github.com/{repo}/pull/{number}");
    let call = json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": id, "name": "Bash",
         "input": {"command": format!("gh pr create --repo {repo} --title \"t\" --body-file b.md")}}
    ]}});
    let result = json!({"type": "user",
        "message": {"content": [{"type": "tool_result", "tool_use_id": id, "content": format!("ok created #{number} {url}")}]},
        "toolUseResult": {"stdout": format!("ok created #{number} {url}"), "stderr": "",
            "gitOperation": {"pr": {"number": number, "url": url, "action": "created"}}}});
    format!("{call}\n{result}\n")
}

/// A PR creation with no `gitOperation` record, its result text an array of blocks.
fn created_without_record(id: &str, repo: &str, number: u64) -> String {
    let call = json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": id, "name": "Bash", "input": {"command": "rtk proxy gh pr create --fill"}}
    ]}});
    let result = json!({"type": "user",
        "message": {"content": [{"type": "tool_result", "tool_use_id": id,
            "content": [{"type": "text", "text": format!("https://github.com/{repo}/pull/{number}\n")}]}]},
        "toolUseResult": "a plain string result"});
    format!("{call}\n{result}\n")
}

struct Fake {
    origin: Option<&'static str>,
    open: Vec<(&'static str, u64, &'static str)>,
}

impl Lookup for Fake {
    fn origin_repo(&self, _dir: &str, _deadline: Instant) -> Option<String> {
        self.origin.map(str::to_string)
    }
    fn open_prs(&self, repo: &str, _deadline: Instant) -> Vec<OpenPr> {
        self.open
            .iter()
            .filter(|(r, _, _)| *r == repo)
            .map(|(_, n, head)| OpenPr {
                number: *n,
                title: "the open one".into(),
                head: (*head).into(),
            })
            .collect()
    }
    fn branch_exists(&self, _dir: &str, name: &str, _deadline: Instant) -> bool {
        matches!(name, "main" | "fix/first")
    }
}

fn transcript(tag: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "claude-guard-pr-{tag}-{}.jsonl",
        std::process::id()
    ));
    std::fs::write(&path, body).unwrap();
    path
}

fn payload(command: &str, transcript: &Path) -> serde_json::Value {
    json!({"tool_name": "Bash", "tool_input": {"command": command},
           "transcript_path": transcript.to_str().unwrap(), "cwd": "/work"})
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[test]
fn a_second_pr_while_the_first_is_open_is_denied() {
    let path = transcript(
        "second",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    let why = decide(
        &payload("gh pr create --title Second", &path),
        &fake,
        deadline(),
    )
    .expect("denied");
    assert!(why.contains("#12"), "{why}");
    assert!(why.contains("fix/first"), "{why}");
    assert!(why.contains("! gh pr create"), "{why}");
}

#[test]
fn a_second_pr_after_the_first_merged_is_allowed() {
    let path = transcript(
        "merged",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![],
    };
    assert!(decide(
        &payload("gh pr create --title Next", &path),
        &fake,
        deadline()
    )
    .is_none());
}

#[test]
fn an_open_pr_in_another_repo_does_not_block_this_one() {
    let path = transcript(
        "other-repo",
        &created_with_git_operation("t1", "owner/other", 3),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/other", 3, "x")],
    };
    assert!(decide(&payload("gh pr create", &path), &fake, deadline()).is_none());
}

#[test]
fn a_pr_the_session_did_not_create_does_not_count() {
    // Another session's open PR is invisible here: it is not in this transcript.
    let path = transcript("foreign", "");
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 99, "someone-else")],
    };
    assert!(decide(&payload("gh pr create", &path), &fake, deadline()).is_none());
}

#[test]
fn the_repo_named_on_the_command_is_the_one_checked() {
    let path = transcript(
        "repo-flag",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/unrelated"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    assert!(decide(
        &payload("gh pr create -R Owner/Repo", &path),
        &fake,
        deadline()
    )
    .is_some());
    assert!(decide(
        &payload("gh pr create --repo=owner/repo", &path),
        &fake,
        deadline()
    )
    .is_some());
    assert!(decide(
        &payload("GH_REPO=owner/repo gh pr create", &path),
        &fake,
        deadline()
    )
    .is_some());
}

#[test]
fn a_result_without_a_git_operation_record_still_counts() {
    let path = transcript("no-record", &created_without_record("t9", "owner/repo", 41));
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 41, "feat/x")],
    };
    assert!(decide(&payload("rtk gh pr new", &path), &fake, deadline()).is_some());
}

#[test]
fn every_branch_creation_form_is_denied_while_a_pr_is_open() {
    let path = transcript(
        "branches",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    for command in [
        "git switch -c feat/next",
        "git switch --create feat/next",
        "git -C /work/repo switch -C feat/next",
        "git checkout -b feat/next",
        "git checkout -B feat/next origin/main",
        "git worktree add -b feat/next ../wt origin/main",
        "git worktree add ../wt -b feat/next",
        "git branch feat/next origin/main",
        "git -c core.x=1 branch feat/next",
    ] {
        let why = decide(&payload(command, &path), &fake, deadline());
        let why = why.unwrap_or_else(|| panic!("{command} should be denied"));
        assert!(why.contains("feat/next"), "{command}: {why}");
        assert!(why.contains("fix/first"), "{command}: {why}");
    }
}

#[test]
fn commands_that_create_nothing_are_allowed_even_with_an_open_pr() {
    let path = transcript(
        "harmless",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    for command in [
        "gh pr create --help",
        "gh pr list --search \"gh pr create\"",
        "gh pr view 12",
        "git commit -m \"gh pr create next time\"",
        "grep -rn \"git switch -c\" docs",
        "echo gh pr create",
        "git branch",
        "git branch -d old",
        "git branch -D old",
        "git branch -m old new",
        "git branch --show-current",
        "git branch -vv",
        "git branch --merged main",
        "git branch -u origin/main",
        "git switch main",
        "git checkout main",
        "git checkout -- file.txt",
        "git worktree add --detach ../wt origin/main",
        "git worktree remove ../wt",
    ] {
        assert!(
            decide(&payload(command, &path), &fake, deadline()).is_none(),
            "{command} should be allowed"
        );
    }
}

#[test]
fn a_help_request_is_not_a_creation() {
    assert_eq!(classify("gh pr create --help"), None);
    assert_eq!(classify("gh pr new -h"), None);
}

#[test]
fn wrappers_come_off_before_classifying() {
    for command in [
        "rtk gh pr create",
        "rtk proxy gh pr create",
        "env GH_TOKEN=x gh pr create",
        "env -i gh pr create",
        "command gh pr create",
        "A=1 B=2 gh pr create",
    ] {
        assert!(
            matches!(classify(command), Some(Trigger::CreatePr { .. })),
            "{command}"
        );
    }
}

#[test]
fn a_chained_command_is_checked_segment_by_segment() {
    assert!(matches!(
        classify("git add a && git commit -m x && gh pr create --fill"),
        Some(Trigger::CreatePr { .. })
    ));
    assert!(matches!(
        classify("git fetch; git switch -c feat/y origin/main"),
        Some(Trigger::CreateBranch { .. })
    ));
}

#[test]
fn a_quoted_mention_is_not_a_command() {
    assert_eq!(classify("git commit -m 'run gh pr create later'"), None);
    assert_eq!(classify("echo \"git switch -c x\""), None);
}

#[test]
fn missing_or_unreadable_inputs_allow() {
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    let no_transcript =
        json!({"tool_name": "Bash", "tool_input": {"command": "gh pr create"}, "cwd": "/w"});
    assert!(decide(&no_transcript, &fake, deadline()).is_none());

    let missing = PathBuf::from("/nonexistent/claude-guard/transcript.jsonl");
    assert!(decide(&payload("gh pr create", &missing), &fake, deadline()).is_none());

    let garbled = transcript("garbled", "{not json\n\u{0}\u{1}\n");
    assert!(decide(&payload("gh pr create", &garbled), &fake, deadline()).is_none());

    let path = transcript(
        "no-remote",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let no_remote = Fake {
        origin: None,
        open: vec![("owner/repo", 12, "fix/first")],
    };
    assert!(decide(&payload("git switch -c x", &path), &no_remote, deadline()).is_none());

    let other_tool = json!({"tool_name": "Edit", "tool_input": {"command": "gh pr create"}});
    assert!(decide(&other_tool, &fake, deadline()).is_none());
}

#[test]
fn the_newest_open_pr_is_named() {
    let body = [
        created_with_git_operation("t1", "owner/repo", 10),
        created_with_git_operation("t2", "owner/repo", 11),
    ]
    .concat();
    let path = transcript("newest", &body);
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 10, "old"), ("owner/repo", 11, "new")],
    };
    let why = decide(&payload("gh pr create", &path), &fake, deadline()).unwrap();
    assert!(why.contains("#11"), "{why}");
}

#[test]
fn session_prs_reads_both_record_shapes_and_skips_viewed_prs() {
    let viewed = json!({"type": "user", "toolUseResult": {"gitOperation":
        {"pr": {"number": 7, "url": "https://github.com/owner/repo/pull/7", "action": "viewed"}}}});
    let body = [
        created_with_git_operation("t1", "Owner/Repo", 5),
        created_without_record("t2", "owner/other", 6),
        format!("{viewed}\n"),
    ]
    .concat();
    let created = session_prs(Cursor::new(body));
    assert_eq!(
        created,
        vec![
            ("owner/repo".to_string(), 5),
            ("owner/other".to_string(), 6)
        ]
    );
}

#[test]
fn a_pull_url_in_an_unrelated_result_does_not_count() {
    // A `gh pr view` or a grep that prints a PR URL is not a creation.
    let call = json!({"type": "assistant", "message": {"content": [
        {"type": "tool_use", "id": "v1", "name": "Bash", "input": {"command": "gh pr view 5"}}]}});
    let result = json!({"type": "user", "message": {"content": [
        {"type": "tool_result", "tool_use_id": "v1", "content": "https://github.com/owner/repo/pull/5"}]}});
    assert!(session_prs(Cursor::new(format!("{call}\n{result}\n"))).is_empty());
}

#[test]
fn remote_urls_resolve_to_owner_and_repo() {
    for url in [
        "git@github.com:Owner/Repo.git",
        "https://github.com/owner/repo",
        "https://github.com/owner/repo.git\n",
        "ssh://git@github.com/owner/repo.git",
    ] {
        assert_eq!(github_repo(url).as_deref(), Some("owner/repo"), "{url}");
    }
    assert_eq!(github_repo("https://gitlab.com/owner/repo"), None);
    assert_eq!(
        pull_urls("ok created #3 https://github.com/O/R/pull/3"),
        vec![("o/r".into(), 3)]
    );
}

#[test]
fn the_reason_carries_the_way_out() {
    let open = OpenPr {
        number: 12,
        title: "fix it".into(),
        head: "fix/first".into(),
    };
    let pr = reason("owner/repo", &open, &Trigger::CreatePr { repo: None });
    assert!(pr.contains("! gh pr create"));
    assert!(pr.contains("the user's own words"));
    let branch = reason(
        "owner/repo",
        &open,
        &Trigger::CreateBranch {
            name: "feat/y".into(),
            dir: None,
            force: false,
        },
    );
    assert!(branch.contains("once #12 merges"));
}

#[test]
fn resetting_a_branch_that_exists_is_not_a_creation() {
    let path = transcript("reset", &created_with_git_operation("t1", "owner/repo", 12));
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    for command in [
        "git branch -f main origin/main",
        "git switch -C main origin/main",
        "git checkout -B fix/first origin/fix/first",
        "git branch -dr origin/old",
    ] {
        assert!(
            decide(&payload(command, &path), &fake, deadline()).is_none(),
            "{command} should be allowed"
        );
    }
    for command in ["git branch -f feat/new", "git switch -C feat/new"] {
        assert!(
            decide(&payload(command, &path), &fake, deadline()).is_some(),
            "{command} creates a branch and should be denied"
        );
    }
}

#[test]
fn a_heredoc_body_is_data() {
    let path = transcript(
        "heredoc",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/repo"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    let body = "cat > /tmp/pr.md <<'EOF'\ngit switch -c feat/y\ngh pr create --fill\nEOF";
    assert_eq!(classify(body), None);
    assert!(decide(&payload(body, &path), &fake, deadline()).is_none());
    let after = "cat <<-EOF\n\tgh pr create\n\tEOF\ngit switch -c feat/y";
    assert!(decide(&payload(after, &path), &fake, deadline()).is_some());
}

#[test]
fn a_repo_flag_in_url_or_attached_form_is_normalised() {
    let path = transcript(
        "normalise",
        &created_with_git_operation("t1", "owner/repo", 12),
    );
    let fake = Fake {
        origin: Some("owner/unrelated"),
        open: vec![("owner/repo", 12, "fix/first")],
    };
    for command in [
        "gh pr create -R github.com/Owner/Repo",
        "gh pr create -R https://github.com/owner/repo.git",
        "gh pr create -Rowner/repo",
    ] {
        assert!(
            decide(&payload(command, &path), &fake, deadline()).is_some(),
            "{command}"
        );
    }
}

#[test]
fn a_hung_child_is_killed_at_the_deadline() {
    // test-hang-allow: a real `sleep` child, bounded by the 300 ms deadline under test.
    let started = Instant::now();
    let out = run_bounded(
        Command::new("sleep").arg("30"),
        Instant::now() + Duration::from_millis(300),
    );
    assert!(out.is_none());
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "took {:?}",
        started.elapsed()
    );
}

#[test]
fn a_finished_child_returns_its_stdout() {
    let out = run_bounded(
        Command::new("echo").arg("hello"),
        Instant::now() + Duration::from_secs(5),
    );
    assert_eq!(out.as_deref(), Some("hello\n"));
}
