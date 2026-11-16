use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::proc;

pub const SCHEMA: u32 = 1;
const RUN_BUDGET: Duration = Duration::from_secs(90);

/// A check that spawns a process, so its result is cached against the binary's fingerprint.
struct Probe {
    id: &'static str,
    bin: &'static str,
    args: &'static [&'static str],
    install: &'static str,
}

const CN: Probe = Probe {
    id: "cn",
    bin: "cn",
    args: &["--help"],
    install: "cargo install chronis, then cn init in the repo",
};
const ALLSTACK: Probe = Probe {
    id: "allstack",
    bin: "allstack",
    args: &["doctor"],
    install: "clone all-source-os/allstack, run cargo install --path . --locked in it",
};
const GRAPHIFY: Probe = Probe {
    id: "graphify",
    bin: "graphify",
    args: &["--help"],
    install: "uv tool install graphifyy then graphify install",
};
const COS: Probe = Probe {
    id: "cos",
    bin: "cos",
    args: &["ritual", "check", "--help"],
    install: "clone decebal/chief-of-staff-v2, run task setup in it",
};

pub const PROBES: [&str; 4] = [CN.id, ALLSTACK.id, GRAPHIFY.id, COS.id];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A stat or file read, run every time.
    Live,
    Cached,
    Checked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    pub fingerprint: Fingerprint,
    pub result: Status,
    pub note: Option<String>,
    pub install: String,
    #[serde(default)]
    pub stale: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    pub schema: u32,
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub name: String,
    pub hard: bool,
    pub status: Status,
    pub source: Source,
    pub note: Option<String>,
    pub install: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub mode: &'static str,
    pub cache: PathBuf,
    pub rows: Vec<Row>,
    pub cached: usize,
    pub checked: usize,
}

impl Report {
    /// True when every hard row is ok; soft rows never fail a preflight.
    pub fn ok(&self) -> bool {
        self.rows.iter().all(|r| !r.hard || r.status == Status::Ok)
    }

    pub fn line(&self) -> String {
        let mut parts: Vec<String> = self
            .rows
            .iter()
            .map(|r| {
                let status = match r.status {
                    Status::Ok => "ok",
                    Status::Missing => "missing",
                };
                let detail: Vec<&str> = [r.note.as_deref(), r.install.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                if detail.is_empty() {
                    format!("{} {status}", r.name)
                } else {
                    format!("{} {status} ({})", r.name, detail.join(": "))
                }
            })
            .collect();
        parts.push(format!("{} cached, {} checked", self.cached, self.checked));
        format!("preflight: {}", parts.join(" · "))
    }
}

pub struct Options {
    pub full: bool,
    pub recheck: bool,
    pub stale: Option<String>,
}

/// Everything the preflight reads from the machine, so tests can point it at a fixture.
pub struct Env {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub path: Vec<PathBuf>,
    pub cache: PathBuf,
}

pub struct Ran {
    pub code: Option<i32>,
    pub stdout: String,
    pub timed_out: bool,
    pub error: Option<String>,
}

pub trait Runner {
    fn run(&mut self, label: &str, program: &Path, args: &[&str], deadline: Instant) -> Ran;
}

pub struct ProcRunner {
    pub log_dir: PathBuf,
}

impl Runner for ProcRunner {
    fn run(&mut self, label: &str, program: &Path, args: &[&str], deadline: Instant) -> Ran {
        let mut cmd = Command::new(program);
        cmd.args(args);
        match proc::run(cmd, &self.log_dir, label, deadline) {
            Ok(done) => Ran {
                code: done.code(),
                stdout: done.stdout,
                timed_out: done.timed_out,
                error: None,
            },
            Err(e) => Ran {
                code: None,
                stdout: String::new(),
                timed_out: false,
                error: Some(e),
            },
        }
    }
}

/// The first executable `name` on `path`, canonicalized so a retargeted symlink changes
/// the fingerprint.
pub fn lookup(path: &[PathBuf], name: &str) -> Option<PathBuf> {
    path.iter().map(|d| d.join(name)).find_map(|p| {
        let meta = fs::metadata(&p).ok()?;
        (meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .then(|| p.canonicalize().unwrap_or(p))
    })
}

pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = fs::metadata(path).ok()?;
    let mtime_ns = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos()
        .try_into()
        .ok()?;
    Some(Fingerprint {
        path: path.to_path_buf(),
        size: meta.len(),
        mtime_ns,
    })
}

/// The current worktree's top and the main checkout, found by reading `.git`, not by git.
pub fn repo_dirs(cwd: &Path) -> Option<(PathBuf, PathBuf)> {
    for dir in cwd.ancestors() {
        let dot = dir.join(".git");
        if dot.is_dir() {
            return Some((dir.to_path_buf(), dir.to_path_buf()));
        }
        if dot.is_file() {
            let text = fs::read_to_string(&dot).ok()?;
            let gitdir = dir.join(text.trim().strip_prefix("gitdir:")?.trim());
            let main = match fs::read_to_string(gitdir.join("commondir")) {
                Ok(common) => {
                    let common = gitdir.join(common.trim());
                    let common = common.canonicalize().unwrap_or(common);
                    common.parent()?.to_path_buf()
                }
                Err(_) => dir.to_path_buf(),
            };
            return Some((dir.to_path_buf(), main));
        }
    }
    None
}

pub fn load(path: &Path) -> Cache {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<Cache>(&t).ok())
        .filter(|c| c.schema == SCHEMA)
        .unwrap_or(Cache {
            schema: SCHEMA,
            entries: BTreeMap::new(),
        })
}

fn save(path: &Path, cache: &Cache) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(cache).map_err(|e| e.to_string())?;
    fs::write(&tmp, body).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

struct Ctx<'a> {
    env: &'a Env,
    cache: Cache,
    recheck: bool,
    deadline: Instant,
    runner: &'a mut dyn Runner,
    cached: usize,
    checked: usize,
}

impl Ctx<'_> {
    fn probe(&mut self, probe: &Probe, hard: bool) -> Row {
        let row = |status, source, note: Option<String>| Row {
            name: probe.id.into(),
            hard,
            status,
            source,
            install: (status == Status::Missing).then(|| probe.install.to_string()),
            note,
        };
        let Some(fp) = lookup(&self.env.path, probe.bin).and_then(|p| fingerprint(&p)) else {
            self.cache.entries.remove(probe.id);
            return row(Status::Missing, Source::Live, Some("not on PATH".into()));
        };
        // A missing result is never reused: its fix may leave this binary untouched.
        if let Some(e) = self.cache.entries.get(probe.id)
            && !self.recheck
            && !e.stale
            && e.result == Status::Ok
            && e.fingerprint == fp
        {
            self.cached += 1;
            return row(e.result, Source::Cached, e.note.clone());
        }
        self.checked += 1;
        let ran = self
            .runner
            .run(probe.id, &fp.path, probe.args, self.deadline);
        if ran.timed_out {
            self.cache.entries.remove(probe.id);
            return row(Status::Missing, Source::Checked, Some("timed out".into()));
        }
        let (status, note) = judge(probe, &ran);
        self.cache.entries.insert(
            probe.id.into(),
            Entry {
                fingerprint: fp,
                result: status,
                note: note.clone(),
                install: probe.install.into(),
                stale: false,
            },
        );
        row(status, Source::Checked, note)
    }
}

fn judge(probe: &Probe, ran: &Ran) -> (Status, Option<String>) {
    if let Some(e) = &ran.error {
        return (Status::Missing, Some(e.clone()));
    }
    match ran.code {
        Some(0) => {}
        Some(126) => return (Status::Missing, Some("exit 126, bad interpreter".into())),
        Some(n) => return (Status::Missing, Some(format!("exit {n}"))),
        None => return (Status::Missing, Some("killed by a signal".into())),
    }
    if probe.id != ALLSTACK.id {
        return (Status::Ok, None);
    }
    let compact: String = ran.stdout.split_whitespace().collect();
    if !compact.contains("\"executables_ready\":true") {
        return (
            Status::Missing,
            Some("doctor did not report executables_ready: true".into()),
        );
    }
    let note = compact
        .contains("\"setup_ready\":false")
        .then(|| "setup_ready: false".to_string());
    (Status::Ok, note)
}

fn live(name: &str, hard: bool, missing: Option<(String, String)>) -> Row {
    let (status, note, install) = match missing {
        None => (Status::Ok, None, None),
        Some((note, install)) => (Status::Missing, Some(note), Some(install)),
    };
    Row {
        name: name.into(),
        hard,
        status,
        source: Source::Live,
        note,
        install,
    }
}

fn skill_row(home: &Path, name: &str, skills: &[&str], hard: bool) -> Row {
    let absent: Vec<&str> = skills
        .iter()
        .copied()
        .filter(|s| {
            !home
                .join(".claude/skills")
                .join(s)
                .join("SKILL.md")
                .is_file()
        })
        .collect();
    live(
        name,
        hard,
        (!absent.is_empty()).then(|| {
            (
                absent.join(", "),
                "cp -R skills/<name> ~/.claude/skills/<name>".into(),
            )
        }),
    )
}

/// Adds the repo-state half of the cn row: `.chronis/` in the main checkout.
fn chronis(mut row: Row, repo: Option<&(PathBuf, PathBuf)>) -> Row {
    if row.status == Status::Missing {
        return row;
    }
    let problem = match repo {
        None => Some((
            "not in a git repository".to_string(),
            "run preflight from the repo".to_string(),
        )),
        Some((_, main)) if !main.join(".chronis").is_dir() => Some((
            format!("no .chronis/ in {}", main.display()),
            "cn init in the main checkout".to_string(),
        )),
        Some(_) => None,
    };
    if let Some((note, install)) = problem {
        row.status = Status::Missing;
        row.source = Source::Live;
        row.note = Some(note);
        row.install = Some(install);
    }
    row
}

fn claude_guard(env: &Env) -> Row {
    let install = "cargo install --path gates/rust/claude-guard, then skillsync hooks install";
    if lookup(&env.path, "claude-guard").is_none() {
        return live(
            "claude-guard",
            true,
            Some(("not on PATH".into(), install.into())),
        );
    }
    let settings = fs::read_to_string(env.home.join(".claude/settings.json")).unwrap_or_default();
    let unregistered: Vec<&str> = ["claude-guard infra-guard", "claude-guard bash-hygiene"]
        .into_iter()
        .filter(|h| !settings.contains(h))
        .collect();
    live(
        "claude-guard",
        true,
        (!unregistered.is_empty()).then(|| {
            (
                format!("settings.json lacks {}", unregistered.join(", ")),
                "skillsync hooks install".into(),
            )
        }),
    )
}

pub fn run(opts: &Options, env: &Env, runner: &mut dyn Runner) -> Result<Report, String> {
    let mut cache = load(&env.cache);
    if let Some(tool) = &opts.stale {
        if !PROBES.contains(&tool.as_str()) {
            return Err(format!(
                "--stale {tool}: cached checks are {}",
                PROBES.join(", ")
            ));
        }
        if let Some(e) = cache.entries.get_mut(tool) {
            e.stale = true;
        }
    }
    let mut ctx = Ctx {
        env,
        cache,
        recheck: opts.recheck,
        deadline: Instant::now() + RUN_BUDGET,
        runner,
        cached: 0,
        checked: 0,
    };
    let repo = repo_dirs(&env.cwd);
    let mut rows = vec![chronis(ctx.probe(&CN, true), repo.as_ref())];
    if opts.full {
        rows.push(ctx.probe(&ALLSTACK, true));
        rows.push(skill_row(
            &env.home,
            "semantic-isa",
            &["semantic-isa"],
            true,
        ));
        let mut graphify = ctx.probe(&GRAPHIFY, true);
        let has_graph = repo.as_ref().is_some_and(|(top, main)| {
            top.join("graphify-out").is_dir() || main.join("graphify-out").is_dir()
        });
        if graphify.status == Status::Ok && !has_graph {
            graphify.note = Some("no graphify-out/ in the repo: build it".into());
        }
        rows.push(graphify);
        rows.push(claude_guard(env));
        rows.push(skill_row(
            &env.home,
            "skills",
            &["pr-proof", "systematic-debugging", "forge-prove-it"],
            true,
        ));
        rows.push(live("feedback", true, None));
        rows.push(ctx.probe(&COS, false));
    } else {
        rows.push(live("feedback", true, None));
        rows.push(ctx.probe(&ALLSTACK, false));
        rows.push(skill_row(
            &env.home,
            "forge-prove-it",
            &["forge-prove-it"],
            false,
        ));
    }
    save(&env.cache, &ctx.cache)?;
    Ok(Report {
        mode: if opts.full { "full" } else { "lean" },
        cache: env.cache.clone(),
        rows,
        cached: ctx.cached,
        checked: ctx.checked,
    })
}
