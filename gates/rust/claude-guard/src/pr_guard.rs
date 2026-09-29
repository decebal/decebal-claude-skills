//! PreToolUse, Bash — one session keeps at most one open PR per GitHub repo.
//!
//! While a PR this session created is still open in a repo, `gh pr create` /
//! `gh pr new` and branch creation in that repo are denied: the change belongs on
//! the open PR's branch. A second PR is the user's call, made in their own words
//! and run by them with `!`.
//!
//! The session's PRs come from its own transcript. Claude Code records a PR it
//! created as `toolUseResult.gitOperation.pr` with `action: "created"`; a result
//! without that record is matched by joining a PR-creation `tool_use` to its
//! `tool_result` and reading `/pull/<n>` URLs from the text.
//!
//! Fails open: no transcript, no GitHub remote, no `gh`, or a lookup past the
//! deadline all allow. A guard that blocked PRs whenever `gh` was slow would be
//! routed around, and then it guards nothing.

use crate::hook;
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// One budget for every child the guard runs, shared across all lookups.
const DEADLINE: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// `gh pr create` / `gh pr new`, with the repo named by `-R`/`--repo`/`GH_REPO`.
    CreatePr { repo: Option<String> },
    /// A new local branch, in the repo at `dir` (`git -C`) or the session's cwd.
    /// `force` is `-C`/`-B`/`--force-create`/`branch -f`, which only resets a
    /// branch that already exists.
    CreateBranch {
        name: String,
        dir: Option<String>,
        force: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenPr {
    pub number: u64,
    pub title: String,
    pub head: String,
}

/// The facts the guard reads from outside the process.
pub(crate) trait Lookup {
    /// `owner/repo` of the `origin` remote of the repo at `dir`, lowercased.
    fn origin_repo(&self, dir: &str, deadline: Instant) -> Option<String>;
    /// The user's open PRs in `repo`; empty when they cannot be read.
    fn open_prs(&self, repo: &str, deadline: Instant) -> Vec<OpenPr>;
    /// Whether `refs/heads/<name>` exists in the repo at `dir`.
    fn branch_exists(&self, dir: &str, name: &str, deadline: Instant) -> bool;
}

pub fn run(payload: &Value) -> ! {
    match decide(payload, &Live, Instant::now() + DEADLINE) {
        Some(reason) => hook::deny(&reason),
        None => hook::allow(),
    }
}

/// The deny reason, or `None` to allow.
pub(crate) fn decide(payload: &Value, lookup: &dyn Lookup, deadline: Instant) -> Option<String> {
    if hook::str_at(payload, &["tool_name"]) != "Bash" {
        return None;
    }
    let trigger = classify(hook::str_at(payload, &["tool_input", "command"]))?;
    let transcript = hook::str_at(payload, &["transcript_path"]);
    if transcript.is_empty() {
        return None;
    }
    let file = std::fs::File::open(transcript).ok()?;
    let created = session_prs(BufReader::new(file));
    if created.is_empty() {
        return None;
    }

    let cwd = hook::str_at(payload, &["cwd"]);
    let repo = match &trigger {
        Trigger::CreatePr { repo: Some(repo) } => normalize_repo(repo)?,
        Trigger::CreatePr { repo: None } => lookup.origin_repo(cwd, deadline)?,
        Trigger::CreateBranch { dir, .. } => {
            lookup.origin_repo(dir.as_deref().unwrap_or(cwd), deadline)?
        }
    };
    if !created.iter().any(|(pr_repo, _)| *pr_repo == repo) {
        return None;
    }
    if let Trigger::CreateBranch {
        name,
        dir,
        force: true,
    } = &trigger
    {
        if lookup.branch_exists(dir.as_deref().unwrap_or(cwd), name, deadline) {
            return None;
        }
    }

    let open = lookup.open_prs(&repo, deadline);
    let newest_open = created
        .iter()
        .rev()
        .filter(|(pr_repo, _)| *pr_repo == repo)
        .find_map(|(_, number)| open.iter().find(|pr| pr.number == *number))?;
    Some(reason(&repo, newest_open, &trigger))
}

pub(crate) fn reason(repo: &str, open: &OpenPr, trigger: &Trigger) -> String {
    let OpenPr {
        number,
        title,
        head,
    } = open;
    let facts = format!(
        "This session already has an open PR in {repo}: #{number} \"{title}\" (branch {head})."
    );
    match trigger {
        Trigger::CreatePr { .. } => format!(
            "{facts} Commit this change on {head} instead: a fix that PR needs to go green, a follow-up on the same surface, and work found along the way all belong on it. A second open PR from one session needs the user's own words; if they ask for one, they run it themselves with `! gh pr create ...`."
        ),
        Trigger::CreateBranch { name, .. } => format!(
            "{facts} Commit on {head} instead of creating branch {name}; start a new branch once #{number} merges, or when the user asks for one in their own words (they run `! git switch -c {name}` themselves)."
        ),
    }
}

/// The trigger in `command`, if any segment of it is a PR or branch creation.
/// Heredoc bodies are data, so they are removed before the command is split.
pub(crate) fn classify(command: &str) -> Option<Trigger> {
    segments(&strip_heredoc_bodies(command))
        .iter()
        .find_map(|segment| classify_segment(segment))
}

/// `owner/repo` from a `-R`/`--repo`/`GH_REPO` value, lowercased. Accepts the
/// `[HOST/]OWNER/REPO` and URL forms `gh` accepts.
pub(crate) fn normalize_repo(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
        .unwrap_or(value);
    let value = value.strip_prefix("github.com/").unwrap_or(value);
    let value = value.trim_end_matches('/');
    let value = value.strip_suffix(".git").unwrap_or(value);
    let (owner, repo) = value.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
        .then(|| format!("{owner}/{repo}").to_lowercase())
}

/// `command` with every heredoc body removed, keeping the line that opens it.
///
/// Quoting on the opening line is not tracked, so a `<<WORD` inside a quoted
/// string also starts a body. That hides lines from the guard, which errs
/// toward allowing.
pub(crate) fn strip_heredoc_bodies(command: &str) -> Cow<'_, str> {
    if !command.contains("<<") {
        return Cow::Borrowed(command);
    }
    let mut out = String::with_capacity(command.len());
    let mut pending: Vec<(String, bool)> = Vec::new();
    for line in command.split_inclusive('\n') {
        if let Some((delimiter, strip_tabs)) = pending.first() {
            let body = line.trim_end_matches(['\n', '\r']);
            let body = if *strip_tabs {
                body.trim_start_matches('\t')
            } else {
                body
            };
            if body == delimiter {
                pending.remove(0);
            }
            continue;
        }
        out.push_str(line);
        pending.extend(heredoc_delimiters(line));
    }
    Cow::Owned(out)
}

/// `(delimiter, strip_leading_tabs)` for each `<<WORD` / `<<-WORD` on `line`.
/// `<<<` is a here-string, which has no body.
fn heredoc_delimiters(line: &str) -> Vec<(String, bool)> {
    let chars: Vec<char> = line.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i + 1 < chars.len() {
        let opens = chars[i] == '<'
            && chars[i + 1] == '<'
            && (i == 0 || chars[i - 1] != '<')
            && chars.get(i + 2) != Some(&'<');
        if !opens {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        let strip_tabs = chars.get(j) == Some(&'-');
        if strip_tabs {
            j += 1;
        }
        while chars.get(j).is_some_and(|c| *c == ' ' || *c == '\t') {
            j += 1;
        }
        if chars.get(j).is_some_and(|c| *c == '\'' || *c == '"') {
            j += 1;
        }
        let start = j;
        while chars
            .get(j)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        {
            j += 1;
        }
        if j > start {
            found.push((chars[start..j].iter().collect(), strip_tabs));
        }
        i = j.max(i + 2);
    }
    found
}

fn classify_segment(tokens: &[String]) -> Option<Trigger> {
    let (env, argv) = strip_wrappers(tokens);
    match argv.first().map(String::as_str) {
        Some("gh") => classify_gh(&argv[1..], &env),
        Some("git") => classify_git(&argv[1..]),
        _ => None,
    }
}

/// Leading `VAR=value` assignments and the `env`, `command`, `rtk` and
/// `rtk proxy` wrappers come off; the assignments are kept for `GH_REPO`.
fn strip_wrappers(tokens: &[String]) -> (Vec<(String, String)>, Vec<String>) {
    let mut env = Vec::new();
    let mut rest = tokens;
    loop {
        match rest.first().map(String::as_str) {
            Some(token) if assignment(token).is_some() => {
                if let Some(pair) = assignment(token) {
                    env.push(pair);
                }
                rest = &rest[1..];
            }
            Some("env") => {
                rest = &rest[1..];
                while rest.first().is_some_and(|t| t.starts_with('-')) {
                    rest = &rest[1..];
                }
            }
            Some("command") => rest = &rest[1..],
            Some("rtk") => {
                rest = &rest[1..];
                if rest.first().map(String::as_str) == Some("proxy") {
                    rest = &rest[1..];
                }
            }
            _ => break,
        }
    }
    (env, rest.to_vec())
}

fn assignment(token: &str) -> Option<(String, String)> {
    let (name, value) = token.split_once('=')?;
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    valid.then(|| (name.to_string(), value.to_string()))
}

fn classify_gh(args: &[String], env: &[(String, String)]) -> Option<Trigger> {
    let mut repo = env
        .iter()
        .find(|(name, _)| name == "GH_REPO")
        .map(|(_, value)| value.clone());
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-R" || arg == "--repo" {
            repo = args.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--repo=") {
            repo = Some(value.to_string());
        } else if let Some(value) = arg.strip_prefix("-R").filter(|v| !v.is_empty()) {
            repo = Some(value.to_string());
        } else if !arg.starts_with('-') {
            words.push(arg);
        }
        i += 1;
    }
    let help = args.iter().any(|a| a == "-h" || a == "--help");
    let creates = words.first() == Some(&"pr") && matches!(words.get(1), Some(&"create" | &"new"));
    (creates && !help).then_some(Trigger::CreatePr { repo })
}

fn classify_git(args: &[String]) -> Option<Trigger> {
    let mut dir = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-C" => {
                dir = args.get(i + 1).cloned();
                i += 2;
            }
            "-c" => i += 2,
            arg if arg.starts_with('-') => i += 1,
            _ => break,
        }
    }
    let subcommand = args.get(i)?.as_str();
    let rest = &args[i + 1..];
    let (name, force) = match subcommand {
        "switch" => create_or_reset(rest, &["-c", "--create"], &["-C", "--force-create"]),
        "checkout" => create_or_reset(rest, &["-b"], &["-B"]),
        "worktree" if rest.first().map(String::as_str) == Some("add") => {
            create_or_reset(&rest[1..], &["-b"], &["-B"])
        }
        "branch" => new_branch_name(rest).map(|name| {
            let force = rest.iter().any(|a| a == "-f" || a == "--force");
            (name, force)
        }),
        _ => None,
    }?;
    Some(Trigger::CreateBranch { name, dir, force })
}

/// The branch a create flag names, and whether the flag was the resetting form.
fn create_or_reset(args: &[String], create: &[&str], reset: &[&str]) -> Option<(String, bool)> {
    value_after(args, create)
        .map(|name| (name, false))
        .or_else(|| value_after(args, reset).map(|name| (name, true)))
}

fn value_after(args: &[String], flags: &[&str]) -> Option<String> {
    args.iter().enumerate().find_map(|(i, arg)| {
        if flags.contains(&arg.as_str()) {
            return args.get(i + 1).cloned();
        }
        flags.iter().find_map(|flag| {
            let long = flag.starts_with("--");
            let value = arg.strip_prefix(&format!("{flag}="))?;
            long.then(|| value.to_string())
        })
    })
}

/// `git branch <name> [<start>]` creates; listing, deleting, renaming and
/// upstream edits do not.
fn new_branch_name(args: &[String]) -> Option<String> {
    const NOT_CREATING: &[&str] = &[
        "-d",
        "-D",
        "--delete",
        "-m",
        "-M",
        "--move",
        "-l",
        "--list",
        "-a",
        "--all",
        "-r",
        "--remotes",
        "-v",
        "-vv",
        "--verbose",
        "--show-current",
        "--contains",
        "--no-contains",
        "--merged",
        "--no-merged",
        "--points-at",
        "-u",
        "--set-upstream-to",
        "--unset-upstream",
        "--edit-description",
    ];
    let listing = args.iter().any(|arg| {
        let flag = arg.split_once('=').map_or(arg.as_str(), |(flag, _)| flag);
        let combined = flag.len() > 2 && flag.starts_with('-') && !flag.starts_with("--");
        let short_listing = combined
            && flag[1..]
                .chars()
                .any(|c| NOT_CREATING.contains(&format!("-{c}").as_str()));
        NOT_CREATING.contains(&flag)
            || short_listing
            || flag.starts_with("--format")
            || flag.starts_with("--sort")
    });
    if listing {
        return None;
    }
    args.iter().find(|arg| !arg.starts_with('-')).cloned()
}

/// Shell words, split into segments at unquoted `;`, `&&`, `||`, `|` and newlines.
pub(crate) fn segments(command: &str) -> Vec<Vec<String>> {
    let mut segments = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut chars = command.chars().peekable();

    let end_word = |word: &mut String, words: &mut Vec<String>| {
        if !word.is_empty() {
            words.push(std::mem::take(word));
        }
    };

    while let Some(ch) = chars.next() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some('"') if ch == '\\' => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            Some(_) => word.push(ch),
            None => match ch {
                '\'' | '"' => quote = Some(ch),
                '\\' => {
                    if let Some(next) = chars.next() {
                        word.push(next);
                    }
                }
                ' ' | '\t' => end_word(&mut word, &mut words),
                ';' | '\n' | '|' | '&' => {
                    if matches!(ch, '|' | '&') && chars.peek() == Some(&ch) {
                        chars.next();
                    }
                    end_word(&mut word, &mut words);
                    if !words.is_empty() {
                        segments.push(std::mem::take(&mut words));
                    }
                }
                _ => word.push(ch),
            },
        }
    }
    end_word(&mut word, &mut words);
    if !words.is_empty() {
        segments.push(words);
    }
    segments
}

/// Every `(repo, number)` this session created, in creation order, repo lowercased.
pub(crate) fn session_prs(reader: impl BufRead) -> Vec<(String, u64)> {
    let mut created: Vec<(String, u64)> = Vec::new();
    let mut seen: HashSet<(String, u64)> = HashSet::new();
    let mut pending: HashSet<String> = HashSet::new();
    let mut record = |repo: String, number: u64| {
        if seen.insert((repo.clone(), number)) {
            created.push((repo, number));
        }
    };

    for line in reader.lines().map_while(Result::ok) {
        let relevant = (line.contains("\"gitOperation\"") && line.contains("\"created\""))
            || line.contains("gh pr create")
            || line.contains("gh pr new")
            || (!pending.is_empty() && line.contains("/pull/"));
        if !relevant {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let content = entry
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array);

        if entry.get("type").and_then(Value::as_str) == Some("assistant") {
            for item in content.into_iter().flatten() {
                let is_bash_call = item.get("type").and_then(Value::as_str) == Some("tool_use")
                    && item.get("name").and_then(Value::as_str) == Some("Bash");
                let command = item
                    .get("input")
                    .and_then(|i| i.get("command"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if is_bash_call && matches!(classify(command), Some(Trigger::CreatePr { .. })) {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        pending.insert(id.to_string());
                    }
                }
            }
            continue;
        }

        let pr_op = entry
            .get("toolUseResult")
            .filter(|r| r.is_object())
            .and_then(|r| r.get("gitOperation"))
            .and_then(|op| op.get("pr"));
        if let Some(pr) = pr_op {
            let url = pr.get("url").and_then(Value::as_str).unwrap_or("");
            let created_here = pr.get("action").and_then(Value::as_str) == Some("created");
            if let (true, Some((repo, number))) = (created_here, pull_urls(url).into_iter().next())
            {
                record(repo, number);
            }
        }

        for item in content.into_iter().flatten() {
            if item.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }
            let Some(id) = item.get("tool_use_id").and_then(Value::as_str) else {
                continue;
            };
            if !pending.remove(id) {
                continue;
            }
            for (repo, number) in pull_urls(&result_text(item.get("content"))) {
                record(repo, number);
            }
        }
    }
    created
}

fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Every `github.com/<owner>/<repo>/pull/<n>` in `text`, repo lowercased.
pub(crate) fn pull_urls(text: &str) -> Vec<(String, u64)> {
    static PULL: OnceLock<Regex> = OnceLock::new();
    let pull = PULL.get_or_init(|| {
        Regex::new(r"github\.com/([A-Za-z0-9._-]+)/([A-Za-z0-9._-]+)/pull/(\d+)")
            .expect("valid regex")
    });
    pull.captures_iter(text)
        .filter_map(|c| {
            let number = c[3].parse().ok()?;
            Some((format!("{}/{}", &c[1], &c[2]).to_lowercase(), number))
        })
        .collect()
}

/// `owner/repo` from an https or ssh GitHub remote URL, lowercased.
pub(crate) fn github_repo(url: &str) -> Option<String> {
    let url = url.trim();
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    (!owner.is_empty() && !repo.is_empty() && !repo.contains('/'))
        .then(|| format!("{owner}/{repo}").to_lowercase())
}

/// The real lookups: `git` and `gh`, each run as a child this process owns.
struct Live;

impl Lookup for Live {
    fn origin_repo(&self, dir: &str, deadline: Instant) -> Option<String> {
        let dir = if dir.is_empty() { "." } else { dir };
        let out = run_bounded(
            Command::new("git").args(["-C", dir, "remote", "get-url", "origin"]),
            deadline,
        )?;
        github_repo(&out)
    }

    fn branch_exists(&self, dir: &str, name: &str, deadline: Instant) -> bool {
        let dir = if dir.is_empty() { "." } else { dir };
        let reference = format!("refs/heads/{name}");
        run_bounded(
            Command::new("git").args(["-C", dir, "rev-parse", "--verify", "--quiet", &reference]),
            deadline,
        )
        .is_some()
    }

    fn open_prs(&self, repo: &str, deadline: Instant) -> Vec<OpenPr> {
        let out = run_bounded(
            Command::new("gh").args([
                "pr",
                "list",
                "--repo",
                repo,
                "--state",
                "open",
                "--author",
                "@me",
                "--limit",
                "200",
                "--json",
                "number,headRefName,title",
            ]),
            deadline,
        );
        let listed: Vec<Value> = out
            .and_then(|out| serde_json::from_str(&out).ok())
            .unwrap_or_default();
        listed
            .iter()
            .filter_map(|pr| {
                let text = |key: &str| {
                    pr.get(key)
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string()
                };
                Some(OpenPr {
                    number: pr.get("number").and_then(Value::as_u64)?,
                    title: text("title"),
                    head: text("headRefName"),
                })
            })
            .collect()
    }
}

/// Stdout of a successful run, or `None` on failure or at the deadline. The
/// child is killed THEN reaped on the deadline, so none outlives the hook.
///
/// Stdout drains on a reader thread while this thread polls the child, so an
/// output larger than the pipe buffer cannot stall the child until the deadline.
/// After a kill the reader is not joined: a grandchild can still hold the pipe,
/// and the hook process exits right after.
pub(crate) fn run_bounded(command: &mut Command, deadline: Instant) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        stdout.read_to_string(&mut out).ok().map(|_| out)
    });
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return reader.join().ok().flatten(),
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
#[path = "pr_guard_tests.rs"]
mod pr_guard_tests;
