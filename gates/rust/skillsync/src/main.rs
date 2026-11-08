//! skillsync — keep a home directory's skills and guard hooks in step with the repo.
//!
//! ```text
//! skillsync check                 # read-only: skills, guard hooks, prod tokens, cn lint
//! skillsync hooks install         # merge configs/ guard hooks into ~/.claude and ~/.codex
//! skillsync skills install        # symlink repo skills into ~/.claude/skills and ~/.agents/skills
//! ```
//!
//! Flags: `--home <dir>` (default `$HOME`), `--repo <dir>` (default: nearest
//! ancestor of the working directory holding `skills/` and `configs/`),
//! `--cn <bin>` (default `cn`), `--dry-run`, `--json`, `--strict` (check: every
//! uninstalled repo skill is a finding).
//!
//! Exit 0 when clean, 1 on findings or conflicts, 2 on usage or I/O errors.

mod check;
mod cn;
mod hooks;
mod repo;
mod report;
mod skills;

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: skillsync <check | hooks install | skills install> \
[--home <dir>] [--repo <dir>] [--cn <bin>] [--dry-run] [--json] [--strict]";

struct Opts {
    home: Option<PathBuf>,
    repo: Option<PathBuf>,
    cn: String,
    json: bool,
    dry_run: bool,
    strict: bool,
    help: bool,
}

fn main() -> ExitCode {
    match run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("skillsync: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let (words, opts) = parse(args)?;
    if opts.help {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let command = match words.as_slice() {
        ["check"] => "check",
        ["hooks", "install"] => "hooks install",
        ["skills", "install"] => "skills install",
        _ => return Err(USAGE.into()),
    };

    let home = match &opts.home {
        Some(home) => home.clone(),
        None => std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("HOME is not set; pass --home")?,
    };
    let repo = repo::Repo::locate(opts.repo.as_deref())?;
    let mut report = report::Report::default();

    match command {
        "check" => check::run(&home, &repo, &opts.cn, opts.strict, &mut report)?,
        "hooks install" => hooks::install(&home, &repo, opts.dry_run, &mut report)?,
        _ => skills::install(&home, &repo, opts.dry_run, &mut report)?,
    }
    Ok(report::finish(command, &report, opts.json))
}

fn parse(args: &[String]) -> Result<(Vec<String>, Opts), String> {
    let mut words = Vec::new();
    let mut opts = Opts {
        home: None,
        repo: None,
        cn: "cn".into(),
        json: false,
        dry_run: false,
        strict: false,
        help: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arg.as_str(), None),
        };
        match flag {
            "--home" => opts.home = Some(PathBuf::from(value(flag, inline, &mut iter)?)),
            "--repo" => opts.repo = Some(PathBuf::from(value(flag, inline, &mut iter)?)),
            "--cn" => opts.cn = value(flag, inline, &mut iter)?,
            "--json" => opts.json = true,
            "--dry-run" => opts.dry_run = true,
            "--strict" => opts.strict = true,
            "-h" | "--help" => opts.help = true,
            other if other.starts_with('-') => return Err(format!("unknown flag {other}")),
            _ => words.push(arg.clone()),
        }
    }
    Ok((words, opts))
}

fn value(
    flag: &str,
    inline: Option<String>,
    iter: &mut std::slice::Iter<'_, String>,
) -> Result<String, String> {
    inline
        .or_else(|| iter.next().cloned())
        .ok_or_else(|| format!("{flag} needs a value"))
}
