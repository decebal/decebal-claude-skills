//! One bounded step of the weekly loop per call. A LaunchAgent calls it every few
//! minutes; a call does nothing unless a run is in progress or the next start time has
//! passed. A started run is carried by the state file until it is reported, whatever
//! happens between calls: a failed call, a sleeping Mac, a week boundary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Days, Local, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cases::{self, Case};
use crate::evaluate::{self, DIAGNOSTIC, GATES, OBJECTIVE, PROTECTED_DIRS, SKILL_DIRS};
use crate::proc;
use crate::store;

const STEP_BUDGET: Duration = Duration::from_secs(280);
/// Leaves room inside `STEP_BUDGET` for prepare, confine and a submit whose evaluator
/// may take its full contract timeout.
const MUTATOR_BUDGET: Duration = Duration::from_secs(180);
const EVALUATOR_TIMEOUT_SECONDS: u32 = 30;
/// Consecutive failed calls before a phase gives up: setup becomes Failed, a running
/// run goes to its report.
const MAX_FAILURES: u32 = 3;

#[derive(Debug, Clone)]
pub struct Options {
    pub workdir: PathBuf,
    pub store: PathBuf,
    pub repo_url: String,
    pub branch: String,
    pub candidates: u32,
    pub evaluator: PathBuf,
    pub autoresearch: PathBuf,
    pub claude: PathBuf,
    pub claude_model: Option<String>,
    /// 0 = Sunday, local time.
    pub start_weekday: u32,
    pub start_hour: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Running,
    Reported,
    Skipped,
    Failed,
}

impl Phase {
    fn terminal(&self) -> bool {
        matches!(self, Phase::Reported | Phase::Skipped | Phase::Failed)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub index: Option<u32>,
    pub case_id: String,
    pub hypothesis: String,
    pub status: String,
    pub commit: Option<String>,
    pub decision: Option<Value>,
    #[serde(default)]
    pub finalization: Option<Value>,
}

impl Row {
    /// A candidate read back after an interrupted call may carry only its finalization.
    pub fn kept(&self) -> bool {
        self.decision
            .as_ref()
            .is_some_and(|d| d["disposition"] == "keep")
            || self
                .finalization
                .as_ref()
                .is_some_and(|f| f["outcome"] == "kept")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pending {
    pub case_id: String,
    pub hypothesis: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// Local date the run started, naming its cases, logs and reports.
    pub run: String,
    pub phase: Phase,
    pub started_at: Option<DateTime<Utc>>,
    pub run_id: Option<String>,
    pub base_commit: Option<String>,
    pub cases_dir: Option<PathBuf>,
    pub evaluator_sha256: Option<String>,
    /// Case ids already given to the mutator, whether or not the attempt produced a row.
    pub tried: Vec<String>,
    pub attempts: u32,
    pub failures: u32,
    /// A submitted candidate whose result has not been read back yet.
    pub pending: Option<Pending>,
    /// Why no further candidate will be prepared, once that is known.
    pub halt: Option<String>,
    /// Dev case ids open on the current best, when known. Checked before preparing a
    /// candidate: a prepared candidate the loop will not fill blocks `stop` and `verify`.
    #[serde(default)]
    pub open: Option<Vec<String>>,
    pub rows: Vec<Row>,
    pub note: Option<String>,
}

/// The first `weekday`/`hour` local start strictly after `after`.
pub fn next_start(after: DateTime<Local>, weekday: u32, hour: u32) -> DateTime<Local> {
    let time = NaiveTime::from_hms_opt(hour, 0, 0).expect("hour validated to 0..=23");
    (0..=7)
        .filter_map(|d| after.date_naive().checked_add_days(Days::new(d)))
        .filter(|day| day.weekday().num_days_from_sunday() == weekday)
        .filter_map(|day| Local.from_local_datetime(&day.and_time(time)).earliest())
        .find(|start| *start > after)
        .unwrap_or_else(|| after + chrono::Duration::days(7))
}

/// A finished (or never started) loop starts again once the scheduled time after its
/// last start has passed; a Mac that slept through it starts on the first call after.
pub fn due(state: &State, now: DateTime<Local>, weekday: u32, hour: u32) -> bool {
    match state.started_at {
        None => true,
        Some(last) => now >= next_start(last.with_timezone(&Local), weekday, hour),
    }
}

fn state_path(workdir: &Path) -> PathBuf {
    workdir.join("state.json")
}

/// Missing means never run. Unreadable is an error: guessing would start a second run.
pub fn load_state(workdir: &Path) -> Result<State, String> {
    let path = state_path(workdir);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| format!("{} is unreadable ({e}); fix or remove it", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn save_state(workdir: &Path, state: &State) -> Result<(), String> {
    let path = state_path(workdir);
    let tmp = workdir.join("state.json.tmp");
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())? + "\n";
    fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename to {}: {e}", path.display()))
}

/// Advances the loop by at most one step and returns a one-line summary.
pub fn step(opts: &Options) -> Result<String, String> {
    fs::create_dir_all(&opts.workdir)
        .map_err(|e| format!("create {}: {e}", opts.workdir.display()))?;
    let Some(_lock) = store::exclusive(&opts.workdir.join("weekly.lock"), Duration::ZERO)? else {
        return Ok("weekly: another call is running; nothing to do".into());
    };
    proc::reap_stale(&mutator_pid_file(opts), "claude");
    let deadline = Instant::now() + STEP_BUDGET;
    let now = Local::now();
    let mut state = load_state(&opts.workdir)?;
    if state.phase.terminal() || state.started_at.is_none() {
        if !due(&state, now, opts.start_weekday, opts.start_hour) {
            let last = state.started_at.map(|t| t.with_timezone(&Local));
            let next = last.map(|l| next_start(l, opts.start_weekday, opts.start_hour));
            return Ok(format!(
                "weekly: last run {} {:?}; next start {}",
                state.run,
                state.phase,
                next.map(|n| n.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default()
            ));
        }
        state = State {
            run: now.format("%Y-%m-%d").to_string(),
            started_at: Some(now.with_timezone(&Utc)),
            ..State::default()
        };
    }
    let result = match state.phase {
        Phase::Idle => setup(opts, &mut state, deadline),
        Phase::Running if candidates_left(&state, opts) => candidate(opts, &mut state, deadline),
        Phase::Running => finish(opts, &mut state, deadline),
        Phase::Reported | Phase::Skipped | Phase::Failed => unreachable!("reset above"),
    };
    match &result {
        Ok(_) => state.failures = 0,
        Err(e) => {
            state.note = Some(e.clone());
            state.failures += 1;
            if state.failures >= MAX_FAILURES {
                match (&state.phase, &state.halt) {
                    (Phase::Running, None) => {
                        state.halt = Some(format!("{MAX_FAILURES} calls in a row failed"));
                        state.failures = 0;
                    }
                    (Phase::Idle | Phase::Running, _) => state.phase = Phase::Failed,
                    _ => {}
                }
            }
        }
    }
    save_state(&opts.workdir, &state)?;
    result
}

/// Bounded by rows, by attempts (failed ones included) and by an explicit halt, so a
/// run always reaches `finish`.
pub fn candidates_left(state: &State, opts: &Options) -> bool {
    state.halt.is_none()
        && (state.rows.len() as u32) < opts.candidates
        && state.attempts < opts.candidates * 2
}

fn repo_dir(opts: &Options) -> PathBuf {
    opts.workdir.join("repo")
}

fn mutator_pid_file(opts: &Options) -> PathBuf {
    opts.workdir.join("mutator.pgid")
}

fn logs(opts: &Options, run: &str) -> PathBuf {
    opts.workdir.join("logs").join(run)
}

fn git(
    opts: &Options,
    run: &str,
    label: &str,
    args: &[&str],
    deadline: Instant,
) -> Result<String, String> {
    git_in(&repo_dir(opts), opts, run, label, args, deadline)
}

fn git_in(
    dir: &Path,
    opts: &Options,
    run: &str,
    label: &str,
    args: &[&str],
    deadline: Instant,
) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    let done = proc::run(cmd, &logs(opts, run), label, deadline)?;
    if done.success() {
        Ok(done.stdout)
    } else {
        Err(format!("git {}: {}", args.join(" "), done.stderr.trim()))
    }
}

fn autoresearch(
    opts: &Options,
    run: &str,
    label: &str,
    args: &[&str],
    deadline: Instant,
) -> Result<Value, String> {
    let mut cmd = Command::new(&opts.autoresearch);
    cmd.arg("--repository")
        .arg(repo_dir(opts))
        .arg("--json")
        .args(args);
    let done = proc::run(cmd, &logs(opts, run), label, deadline)?;
    if done.timed_out {
        return Err(format!("autoresearch {label}: timed out"));
    }
    match serde_json::from_str::<Value>(done.stdout.trim()) {
        Ok(v) if done.success() => Ok(v),
        _ => Err(format!(
            "autoresearch {label} exited {:?}: {}{}",
            done.code(),
            done.stderr.trim(),
            done.stdout.trim()
        )),
    }
}

fn setup(opts: &Options, state: &mut State, deadline: Instant) -> Result<String, String> {
    let run = state.run.clone();
    let repo = repo_dir(opts);
    if !repo.join(".git").exists() {
        let mut cmd = Command::new("git");
        cmd.args(["clone", "--quiet", &opts.repo_url]).arg(&repo);
        let done = proc::run(cmd, &logs(opts, &run), "clone", deadline)?;
        if !done.success() {
            return Err(format!(
                "git clone {}: {}",
                opts.repo_url,
                done.stderr.trim()
            ));
        }
    }
    // Local to the dedicated clone; autoresearch's candidate commits use it too, and a
    // LaunchAgent has no pinentry to sign with.
    for (key, value) in [
        ("commit.gpgsign", "false"),
        ("user.name", "ritual-feedback"),
        ("user.email", "ritual-feedback@localhost"),
    ] {
        git(opts, &run, "config", &["config", key, value], deadline)?;
    }
    let remote_ref = format!("origin/{}", opts.branch);
    git(
        opts,
        &run,
        "fetch",
        &["fetch", "--quiet", "origin", &opts.branch],
        deadline,
    )?;
    git(
        opts,
        &run,
        "checkout",
        &["checkout", "--quiet", "--force", "--detach", &remote_ref],
        deadline,
    )?;

    let cases_dir = opts.workdir.join("cases").join(&run);
    let (records, unreadable) = store::read_all(&opts.store)?;
    let manifest = cases::export(&records, unreadable, &cases_dir, |c| {
        repo.join(c.target()).is_file()
    })?;
    state.cases_dir = Some(cases_dir.clone());
    if manifest.dev_count == 0 {
        state.phase = Phase::Skipped;
        return Ok(format!(
            "weekly {run}: {} feedback events, 0 dev cases ({} dropped: target file missing); skipped",
            records.len(),
            manifest.dropped_missing_target
        ));
    }

    let dev = cases::read_jsonl(&cases_dir.join("dev.jsonl"))?;
    let holdout = cases::read_jsonl(&cases_dir.join("holdout.jsonl"))?;
    let frozen = evaluate::Frozen {
        dev: cases_dir.join("dev.jsonl"),
        holdout: cases_dir.join("holdout.jsonl"),
        holdout_sha256: manifest.holdout_sha256.clone(),
        holdout_max_open: cases::open_cases(&repo, &holdout).len(),
        max_words: evaluate::total_words(&repo),
    };
    let baseline = evaluate::score(&repo, &dev, &holdout, &frozen, true);
    if !(baseline.links_resolve && baseline.frontmatter_valid && baseline.invariants_kept) {
        state.phase = Phase::Failed;
        return Err(format!("{remote_ref} fails its own gates: {baseline:?}"));
    }
    if baseline.open_dev == 0 {
        state.phase = Phase::Skipped;
        return Ok(format!(
            "weekly {run}: all {} dev cases already addressed; skipped",
            dev.len()
        ));
    }

    let evaluator_bytes = fs::read(&opts.evaluator).map_err(|e| format!("read evaluator: {e}"))?;
    state.evaluator_sha256 = Some(cases::sha256_hex(&evaluator_bytes));
    fs::write(
        repo.join("autoresearch.toml"),
        contract(opts, &frozen, &run),
    )
    .map_err(|e| e.to_string())?;
    fs::write(repo.join("program.md"), program()).map_err(|e| e.to_string())?;
    let ignore = repo.join(".gitignore");
    let mut ignored = fs::read_to_string(&ignore).unwrap_or_default();
    if !ignored.lines().any(|l| l.trim() == ".autoresearch/") {
        if !ignored.is_empty() && !ignored.ends_with('\n') {
            ignored.push('\n');
        }
        ignored.push_str(".autoresearch/\n");
        fs::write(&ignore, ignored).map_err(|e| e.to_string())?;
    }
    git(
        opts,
        &run,
        "add",
        &["add", "autoresearch.toml", "program.md", ".gitignore"],
        deadline,
    )?;
    git(
        opts,
        &run,
        "commit",
        &[
            "commit",
            "--quiet",
            "-m",
            &format!("chore(autoresearch): ritual feedback contract for {run}"),
        ],
        deadline,
    )?;

    let doctor = autoresearch(opts, &run, "doctor", &["doctor"], deadline)?;
    if doctor["ready"] != Value::Bool(true) {
        return Err(format!("autoresearch doctor not ready: {doctor}"));
    }
    let report = autoresearch(opts, &run, "baseline", &["baseline"], deadline)?;
    let run_id = report["run_id"]
        .as_str()
        .ok_or("baseline printed no run_id")?
        .to_string();
    state.run_id = Some(run_id.clone());
    state.base_commit = report["base_commit"].as_str().map(str::to_string);
    if report["snapshot"].is_null() || report.to_string().contains("\"passed\":false") {
        state.phase = Phase::Failed;
        return Err(format!(
            "baseline {run_id} did not capture with every gate passing"
        ));
    }
    state.open = Some(open_ids(&repo, &dev));
    state.phase = Phase::Running;
    Ok(format!(
        "weekly {run}: baseline {run_id}, {} open of {} dev cases, {} holdout, {} dropped",
        baseline.open_dev,
        dev.len(),
        holdout.len(),
        manifest.dropped_missing_target
    ))
}

pub fn contract(opts: &Options, frozen: &evaluate::Frozen, run: &str) -> String {
    let quote = |p: &Path| format!("{:?}", p.display().to_string());
    let list = |items: &[&str]| {
        items
            .iter()
            .map(|i| format!("{i:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        r#"schema_version = 1

[experiment]
name = "ritual skills against recorded feedback, {run}"

[experiment.objective]
name = "{OBJECTIVE}"
direction = "minimize"

[experiment.budget]
max_candidates = {candidates}
max_failures = {candidates}
wall_clock_seconds = 86400

[scope]
mutable_paths = [{mutable}]
protected_paths = [{protected}]

[agent]
program = "git"
args = ["status", "--short"]
timeout_seconds = 600

[[evaluators]]
id = "ritual_feedback"
hard_gates = [{gates}]

[evaluators.command]
program = {evaluator}
args = ["evaluate", "--dev", {dev}, "--holdout", {holdout}, "--holdout-sha256", "{sha}", "--holdout-max-open", "{max_open}", "--max-words", "{max_words}"]
timeout_seconds = {EVALUATOR_TIMEOUT_SECONDS}

[[evaluators.metrics]]
name = "{OBJECTIVE}"
kind = "objective"
direction = "minimize"

[[evaluators.metrics]]
name = "{DIAGNOSTIC}"
kind = "diagnostic"
direction = "minimize"

[authority]
allow = []
"#,
        candidates = opts.candidates,
        mutable = list(&SKILL_DIRS),
        protected = list(&PROTECTED_DIRS),
        gates = list(&GATES),
        evaluator = quote(&opts.evaluator),
        dev = quote(&frozen.dev),
        holdout = quote(&frozen.holdout),
        sha = frozen.holdout_sha256,
        max_open = frozen.holdout_max_open,
        max_words = frozen.max_words,
    )
}

pub fn program() -> &'static str {
    "# Mutator brief\n\n\
     Change only Markdown under `skills/ritual/` and `skills/ritual-full/`, never \
     `skills/ritual/scripts/`, `autoresearch.toml` or this file. One candidate addresses \
     one recorded feedback case. Keep the total word count at or below the baseline: \
     replace or tighten text rather than append, and never drop a rule to make room. \
     Every phrase in `skills/ritual/scripts/invariants.txt` stays word for word in its \
     file. Keep every relative link resolving and both SKILL.md frontmatter blocks intact.\n"
}

pub fn mutator_prompt(case: &Case, open: usize, invariants: &[(String, String)]) -> String {
    let kept = invariants
        .iter()
        .map(|(target, phrase)| format!("- `{target}`: \"{phrase}\"\n"))
        .collect::<String>();
    let mut check = String::new();
    if let Some(m) = &case.check.must {
        check.push_str(&format!(
            "- `{}` must contain this exact phrase, word for word and unbroken (case and \
             line breaks do not matter, nothing may be inserted inside it): \"{m}\"\n",
            case.target()
        ));
    }
    if let Some(m) = &case.check.must_not {
        check.push_str(&format!(
            "- `{}` must no longer contain the phrase: \"{m}\"\n",
            case.target()
        ));
    }
    format!(
        "You are improving the `ritual` skill from feedback recorded on real runs. \
         {open} feedback cases are open; address exactly this one.\n\n\
         Feedback ({step} step): {note}\n\n\
         What the text has to change:\n{check}\n\
         These rules must survive word for word in these files; never delete or reword \
         them to make room:\n{kept}\n\
         Rules: read `program.md` first. Edit only Markdown under skills/ritual/ and \
         skills/ritual-full/ (never skills/ritual/scripts/). Do not grow the total word count: \
         tighten wording or merge redundant sentences instead of adding a paragraph, and \
         never drop a rule to make room. Keep every \
         relative link valid and both SKILL.md frontmatter blocks intact. Do not commit, do \
         not create new files, do not run builds. Write the rule so a future agent following \
         the skill would not hit this feedback again; restating the phrase without that \
         meaning is not a fix.\n",
        step = case.step,
        note = case.note,
    )
}

/// One entry of `git status --porcelain=v1 -z`: status code, path, and the source path
/// of a rename or copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    pub code: String,
    pub path: String,
    pub from: Option<String>,
}

pub fn parse_status_z(raw: &str) -> Vec<StatusEntry> {
    let mut fields = raw.split('\0').filter(|f| !f.is_empty());
    let mut out = Vec::new();
    while let Some(field) = fields.next() {
        if field.len() < 4 {
            continue;
        }
        let code = field[..2].to_string();
        let path = field[3..].to_string();
        let from = if code.contains('R') || code.contains('C') {
            fields.next().map(str::to_string)
        } else {
            None
        };
        out.push(StatusEntry { code, path, from });
    }
    out
}

/// A tracked Markdown file under a mutable skill directory and outside the protected tree.
pub fn in_scope(entry: &StatusEntry) -> bool {
    let p = &entry.path;
    entry.code != "??"
        && entry.from.is_none()
        && p.ends_with(".md")
        && SKILL_DIRS.iter().any(|d| p.starts_with(&format!("{d}/")))
        && !PROTECTED_DIRS
            .iter()
            .any(|d| p.starts_with(&format!("{d}/")))
}

/// Reverts anything the mutator changed outside the mutable skill prose, so the CLI
/// never sees an out-of-scope path and strands the candidate.
fn confine(worktree: &Path, opts: &Options, run: &str, deadline: Instant) -> Result<usize, String> {
    let raw = git_in(
        worktree,
        opts,
        run,
        "candidate-status",
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        deadline,
    )?;
    let mut in_scope_count = 0;
    for entry in parse_status_z(&raw) {
        if in_scope(&entry) {
            in_scope_count += 1;
            continue;
        }
        if entry.code == "??" {
            fs::remove_file(worktree.join(&entry.path))
                .map_err(|e| format!("remove out-of-scope {}: {e}", entry.path))?;
            continue;
        }
        let mut paths = vec![entry.path.as_str()];
        if let Some(from) = &entry.from {
            paths.push(from);
        }
        for path in paths {
            let restore = git_in(
                worktree,
                opts,
                run,
                "candidate-restore",
                &["checkout", "HEAD", "--", path],
                deadline,
            );
            if restore.is_err() {
                let _ = fs::remove_file(worktree.join(path));
            }
        }
    }
    let left = parse_status_z(&git_in(
        worktree,
        opts,
        run,
        "candidate-recheck",
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        deadline,
    )?);
    if let Some(stray) = left.iter().find(|e| !in_scope(e)) {
        return Err(format!(
            "could not revert out-of-scope change {}",
            stray.path
        ));
    }
    Ok(in_scope_count)
}

fn open_ids(root: &Path, dev: &[Case]) -> Vec<String> {
    cases::open_cases(root, dev)
        .into_iter()
        .map(|c| c.id.clone())
        .collect()
}

pub fn row_from(report: &Value, pending: Option<Pending>) -> Row {
    let pending = pending.unwrap_or(Pending {
        case_id: "unknown".into(),
        hypothesis: "resumed after an interrupted call".into(),
    });
    let field = |name: &str| (!report[name].is_null()).then(|| report[name].clone());
    Row {
        index: report["index"].as_u64().map(|i| i as u32),
        case_id: pending.case_id,
        hypothesis: pending.hypothesis,
        status: report["status"].as_str().unwrap_or("unknown").to_string(),
        commit: report["commit"].as_str().map(str::to_string),
        decision: field("decision"),
        finalization: field("finalization"),
    }
}

/// Clears a prepared worktree back to its incumbent commit.
fn reset_worktree(
    worktree: &Path,
    opts: &Options,
    run: &str,
    deadline: Instant,
) -> Result<(), String> {
    git_in(
        worktree,
        opts,
        run,
        "candidate-reset",
        &["checkout", "HEAD", "--", "."],
        deadline,
    )?;
    git_in(
        worktree,
        opts,
        run,
        "candidate-clean",
        &["clean", "-fd"],
        deadline,
    )?;
    Ok(())
}

pub const ABANDON_HYPOTHESIS: &str = "abandon: close a candidate the loop will not fill with a deliberately failed \
     frontmatter_valid gate, so the run can stop and verify";

/// Submits a candidate certain to be discarded (it renames ritual-full in its
/// frontmatter), because autoresearch can neither stop nor verify a run with a prepared
/// candidate and offers no way to drop one.
fn abandon(
    worktree: &Path,
    opts: &Options,
    state: &mut State,
    run_id: &str,
    deadline: Instant,
) -> Result<String, String> {
    let run = state.run.clone();
    reset_worktree(worktree, opts, &run, deadline)?;
    let skill = worktree.join("skills/ritual-full/SKILL.md");
    let text = fs::read_to_string(&skill).map_err(|e| format!("read {}: {e}", skill.display()))?;
    let broken = text.replacen("name: ritual-full\n", "name: ritual-full-abandoned\n", 1);
    if broken == text {
        return Err("cannot abandon: ritual-full frontmatter has no name line".into());
    }
    fs::write(&skill, broken).map_err(|e| e.to_string())?;
    state.pending = Some(Pending {
        case_id: "-".into(),
        hypothesis: ABANDON_HYPOTHESIS.into(),
    });
    save_state(&opts.workdir, state)?;
    let report = autoresearch(
        opts,
        &run,
        "run-abandon",
        &[
            "run",
            "--run-id",
            run_id,
            "--hypothesis",
            ABANDON_HYPOTHESIS,
        ],
        deadline,
    )?;
    let row = row_from(&report, state.pending.take());
    let summary = format!("weekly {run}: closed prepared candidate {:?}", row.index);
    state.rows.push(row);
    Ok(summary)
}

fn submit(
    worktree: &Path,
    opts: &Options,
    state: &mut State,
    run_id: &str,
    pending: Pending,
    deadline: Instant,
) -> Result<Row, String> {
    let run = state.run.clone();
    let cases_dir = state.cases_dir.clone().ok_or("no cases dir")?;
    let open_after = open_ids(worktree, &cases::read_jsonl(&cases_dir.join("dev.jsonl"))?);
    let hypothesis = pending.hypothesis.clone();
    state.pending = Some(pending);
    save_state(&opts.workdir, state)?;
    let evaluated = autoresearch(
        opts,
        &run,
        "run-submit",
        &["run", "--run-id", run_id, "--hypothesis", &hypothesis],
        deadline,
    )?;
    let row = row_from(&evaluated, state.pending.take());
    if row.kept() {
        state.open = Some(open_after);
    }
    Ok(row)
}

fn candidate(opts: &Options, state: &mut State, deadline: Instant) -> Result<String, String> {
    let run = state.run.clone();
    let run_id = state.run_id.clone().ok_or("running without a run id")?;
    if let Some(open) = &state.open {
        if open.is_empty() {
            state.halt = Some("every dev case is closed".into());
            return Ok(format!("weekly {run}: every dev case is closed"));
        }
        if open.iter().all(|id| state.tried.contains(id)) {
            state.halt = Some(format!("all {} open cases were tried", open.len()));
            return Ok(format!(
                "weekly {run}: all {} open cases were tried",
                open.len()
            ));
        }
    }
    let prepared = autoresearch(
        opts,
        &run,
        "run-prepare",
        &["run", "--run-id", &run_id],
        deadline,
    )?;
    match prepared["status"].as_str() {
        Some("awaiting_mutation") => {}
        Some("evaluated" | "finalized") => {
            let row = row_from(&prepared, state.pending.take());
            if row.kept() {
                state.open = None;
            }
            let summary = format!(
                "weekly {run}: read back interrupted candidate {:?}",
                row.index
            );
            state.rows.push(row);
            return Ok(summary);
        }
        Some("stopped") => {
            state.halt = Some(format!(
                "autoresearch stopped the run: {}",
                prepared["stop_reason"]
                    .as_str()
                    .unwrap_or("no reason given")
            ));
            return Ok(format!("weekly {run}: run {run_id} stopped"));
        }
        other => return Err(format!("run {run_id}: unexpected status {other:?}")),
    }
    let worktree = PathBuf::from(
        prepared["worktree"]
            .as_str()
            .ok_or("run printed no worktree")?,
    );
    if let Some(pending) = state.pending.clone() {
        // The last submit was refused before autoresearch committed it; the edit is
        // still in the worktree, so it is resubmitted rather than stacked on.
        if confine(&worktree, opts, &run, deadline)? > 0 {
            let row = submit(&worktree, opts, state, &run_id, pending, deadline)?;
            let summary = format!("weekly {run}: resubmitted candidate → {}", row.status);
            state.rows.push(row);
            return Ok(summary);
        }
        state.pending = None;
    }
    reset_worktree(&worktree, opts, &run, deadline)?;
    let cases_dir = state.cases_dir.clone().ok_or("no cases dir")?;
    let dev = cases::read_jsonl(&cases_dir.join("dev.jsonl"))?;
    let open = cases::open_cases(&worktree, &dev);
    state.open = Some(open.iter().map(|c| c.id.clone()).collect());
    let Some(pick) = open.iter().find(|c| !state.tried.contains(&c.id)).copied() else {
        state.halt = Some(format!("{} open cases, all tried", open.len()));
        return Ok(format!(
            "weekly {run}: {} open cases, all tried",
            open.len()
        ));
    };
    state.tried.push(pick.id.clone());
    state.attempts += 1;
    save_state(&opts.workdir, state)?;

    let mut cmd = Command::new(&opts.claude);
    cmd.current_dir(&worktree)
        .arg("-p")
        .arg(mutator_prompt(
            pick,
            open.len(),
            &evaluate::invariants(&worktree).unwrap_or_default(),
        ))
        // --restricted confines file tools to the worktree, so the holdout outside it
        // stays unread; no MCP servers, no shell, no prompts that nobody can answer.
        .args([
            "--restricted",
            "--strict-mcp-config",
            "--tools",
            "Read,Edit,Glob,Grep",
            "--permission-mode",
            "acceptEdits",
            "--permission-prompts",
            "none",
            "--no-session-persistence",
        ]);
    if let Some(model) = &opts.claude_model {
        cmd.args(["--model", model]);
    }
    let mutator_deadline = (Instant::now() + MUTATOR_BUDGET).min(deadline);
    let label = format!("mutator-{}", state.attempts);
    let done = proc::run_tracked(
        cmd,
        &logs(opts, &run),
        &label,
        mutator_deadline,
        &mutator_pid_file(opts),
    )?;
    let changed = confine(&worktree, opts, &run, deadline)?;
    if changed == 0 {
        return Err(format!(
            "mutator made no in-scope change for case {} (exit {:?}, timed out {}); \
             the next call tries another case",
            pick.id,
            done.code(),
            done.timed_out
        ));
    }

    let hypothesis = format!(
        "Address {} feedback case {} by changing {}: {}",
        pick.step,
        pick.id,
        pick.target(),
        pick.note.chars().take(160).collect::<String>()
    );
    let pending = Pending {
        case_id: pick.id.clone(),
        hypothesis,
    };
    let row = submit(&worktree, opts, state, &run_id, pending, deadline)?;
    let summary = format!(
        "weekly {run}: candidate {} for case {} → {} {}",
        state.rows.len() + 1,
        pick.id,
        row.status,
        row.decision
            .as_ref()
            .map(Value::to_string)
            .unwrap_or_default()
    );
    state.rows.push(row);
    Ok(summary)
}

/// A candidate is mid-flight when the run should end: read back an interrupted one, or
/// abandon one that was prepared and never filled. The report follows on the next call.
fn close_prepared(
    opts: &Options,
    state: &mut State,
    run_id: &str,
    deadline: Instant,
) -> Result<String, String> {
    let run = state.run.clone();
    let prepared = autoresearch(
        opts,
        &run,
        "run-close",
        &["run", "--run-id", run_id],
        deadline,
    )?;
    match prepared["status"].as_str() {
        Some("awaiting_mutation") => {
            let worktree = PathBuf::from(
                prepared["worktree"]
                    .as_str()
                    .ok_or("run printed no worktree")?,
            );
            abandon(&worktree, opts, state, run_id, deadline)
        }
        Some("evaluated" | "finalized") => {
            let row = row_from(&prepared, state.pending.take());
            let summary = format!("weekly {run}: read back candidate {:?}", row.index);
            state.rows.push(row);
            Ok(summary)
        }
        other => Ok(format!("weekly {run}: run {run_id} is {other:?}")),
    }
}

fn finish(opts: &Options, state: &mut State, deadline: Instant) -> Result<String, String> {
    let run = state.run.clone();
    let run_id = state.run_id.clone().ok_or("finishing without a run id")?;
    let status = autoresearch(
        opts,
        &run,
        "status",
        &["status", "--run-id", &run_id],
        deadline,
    )?;
    match status["recovery_action"].as_str() {
        Some("evaluate_candidate" | "finalize_candidate") => {
            return close_prepared(opts, state, &run_id, deadline);
        }
        Some("prepare_candidate") => {
            autoresearch(opts, &run, "stop", &["stop", "--run-id", &run_id], deadline)?;
        }
        _ => {}
    }
    let kept = state.rows.iter().any(Row::kept);
    let verify = if kept {
        match autoresearch(
            opts,
            &run,
            "verify",
            &["verify", "--run-id", &run_id],
            deadline,
        ) {
            Ok(v) => v["status"].as_str().unwrap_or("unknown").to_string(),
            Err(e) => format!("failed: {e}"),
        }
    } else {
        "skipped: no kept candidate".to_string()
    };
    let mut cmd = Command::new(&opts.autoresearch);
    cmd.arg("--repository")
        .arg(repo_dir(opts))
        .args(["report", "--run-id", &run_id, "--html"]);
    let html = proc::run(cmd, &logs(opts, &run), "report-html", deadline)?;
    if !html.success() || html.stdout.trim().is_empty() {
        return Err(format!(
            "autoresearch report --html exited {:?} (timed out {}): {}",
            html.code(),
            html.timed_out,
            html.stderr.trim()
        ));
    }
    let reports = opts.workdir.join("reports");
    fs::create_dir_all(&reports).map_err(|e| e.to_string())?;
    let html_path = reports.join(format!("{run}.html"));
    fs::write(&html_path, &html.stdout).map_err(|e| e.to_string())?;
    let md_path = reports.join(format!("{run}.md"));
    fs::write(&md_path, ledger(state, &run_id, &verify, opts)).map_err(|e| e.to_string())?;
    state.phase = Phase::Reported;
    Ok(format!(
        "weekly {run}: run {run_id} reported; verify {verify}; ledger {}; board {}",
        md_path.display(),
        html_path.display()
    ))
}

pub fn ledger(state: &State, run_id: &str, verify: &str, opts: &Options) -> String {
    let mut out = format!(
        "# Ritual feedback loop, {run}\n\n\
         Run `{run_id}`, base `{base}`, evaluator sha256 `{sha}`, verify: {verify}.\n\
         {attempts} mutator attempts, {rows} evaluated candidates; stopped because: {halt}.\n\
         Kept candidates are on branch `autoresearch/{run_id}` in `{repo}`; nothing is merged.\n\
         Review with `git -C {repo} diff {base} autoresearch/{run_id} -- skills/` and port \
         any change you keep to the Codex mirror by hand.\n\n\
         | # | Case | Status | Commit | Decision | Hypothesis |\n|---|---|---|---|---|---|\n",
        run = state.run,
        base = state.base_commit.as_deref().unwrap_or("?"),
        sha = state.evaluator_sha256.as_deref().unwrap_or("?"),
        attempts = state.attempts,
        rows = state.rows.len(),
        halt = state.halt.as_deref().unwrap_or("candidate budget reached"),
        repo = repo_dir(opts).display(),
    );
    for (i, row) in state.rows.iter().enumerate() {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            row.index.map_or((i + 1) as u64, u64::from),
            row.case_id,
            row.status,
            row.commit.as_deref().unwrap_or(""),
            row.decision
                .as_ref()
                .map(Value::to_string)
                .unwrap_or_default()
                .replace('|', "/"),
            row.hypothesis.replace('|', "/"),
        ));
    }
    out
}
