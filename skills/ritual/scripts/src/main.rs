mod cases;
mod evaluate;
mod feedback;
mod proc;
mod schedule;
mod store;
mod weekly;

#[cfg(test)]
mod cases_tests;
#[cfg(test)]
mod evaluate_tests;
#[cfg(test)]
mod feedback_tests;
#[cfg(test)]
mod proc_tests;
#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod weekly_tests;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Parser, Subcommand};

use feedback::{Check, Feedback, Kind, Skill, Step};

#[derive(Parser)]
#[command(
    name = "ritual-feedback",
    version,
    about = "Ritual run feedback in AllSource, scored weekly by autoresearch"
)]
struct Cli {
    /// Store directory. Default: $RITUAL_FEEDBACK_DIR, else ~/.ritual/feedback.
    #[arg(long, global = true)]
    store: Option<PathBuf>,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Append one feedback event for a flow.
    Record {
        /// The flow's Chronis epic id.
        #[arg(long)]
        epic: String,
        #[arg(long, value_enum)]
        step: Step,
        #[arg(long, value_enum)]
        kind: Kind,
        /// What happened, in one sentence a later reader can act on.
        #[arg(long)]
        note: String,
        /// Repository name. Default: the main checkout's directory name.
        #[arg(long)]
        repo: Option<String>,
        #[arg(long, value_enum, default_value = "ritual")]
        skill: Skill,
        /// Agent that ran the step. Default: codex under Codex, else claude.
        #[arg(long)]
        agent: Option<String>,
        /// Skill file the feedback is about, relative to the skill directory.
        #[arg(long)]
        reference: Option<String>,
        /// Phrase the reference must contain once the feedback is addressed.
        #[arg(long)]
        must: Option<String>,
        /// Phrase the reference must no longer contain.
        #[arg(long)]
        must_not: Option<String>,
    },
    /// Print recorded feedback, oldest first.
    List {
        #[arg(long)]
        epic: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Freeze feedback cases (events with a check) as dev and holdout JSONL.
    Export {
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// autoresearch JSONL v1 evaluator: one request on stdin, one result on stdout.
    Evaluate {
        #[arg(long)]
        dev: PathBuf,
        #[arg(long)]
        holdout: PathBuf,
        #[arg(long)]
        holdout_sha256: String,
        #[arg(long)]
        holdout_max_open: usize,
        #[arg(long)]
        max_words: usize,
    },
    /// Advance the weekly loop by one bounded step.
    Weekly(WeeklyArgs),
    /// Install the weekly loop as a macOS LaunchAgent that ticks every 15 minutes.
    Schedule {
        #[command(flatten)]
        weekly: WeeklyArgs,
        /// Print the plist instead of installing it.
        #[arg(long)]
        print: bool,
    },
}

#[derive(clap::Args, Clone)]
struct WeeklyArgs {
    /// Where the loop keeps its clone, cases, logs and reports. Default: ~/.ritual/weekly.
    #[arg(long)]
    workdir: Option<PathBuf>,
    #[arg(
        long,
        default_value = "https://github.com/decebal/decebal-claude-skills.git"
    )]
    repo_url: String,
    #[arg(long, default_value = "main")]
    branch: String,
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=20))]
    candidates: u32,
    /// Local weekday a run starts on, 0 = Sunday.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..=6))]
    start_weekday: u32,
    /// Local hour a run starts at.
    #[arg(long, default_value_t = 7, value_parser = clap::value_parser!(u32).range(0..=23))]
    start_hour: u32,
    /// Absolute path to autoresearch. Default: found on PATH.
    #[arg(long)]
    autoresearch: Option<PathBuf>,
    /// Absolute path to the claude CLI that writes each candidate. Default: found on PATH.
    #[arg(long)]
    claude: Option<PathBuf>,
    #[arg(long)]
    claude_model: Option<String>,
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".into())
}

fn which(name: &str) -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("{name} not found on PATH; pass --{name}"))
}

fn main_checkout_name() -> Result<String, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err("not in a git repository; pass --repo".into());
    }
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    common
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| "cannot name the repository; pass --repo".into())
}

/// The contract embeds these paths and the evaluator runs with the candidate worktree
/// as its cwd, so a relative path would resolve against the wrong directory.
fn absolute(path: PathBuf) -> Result<PathBuf, String> {
    std::path::absolute(&path).map_err(|e| format!("resolve {}: {e}", path.display()))
}

fn weekly_options(args: &WeeklyArgs, store: PathBuf) -> Result<weekly::Options, String> {
    let workdir = match &args.workdir {
        Some(w) => w.clone(),
        None => home()?.join(".ritual").join("weekly"),
    };
    Ok(weekly::Options {
        workdir: absolute(workdir)?,
        store: absolute(store)?,
        repo_url: args.repo_url.clone(),
        branch: args.branch.clone(),
        candidates: args.candidates,
        evaluator: std::env::current_exe().map_err(|e| e.to_string())?,
        autoresearch: absolute(match &args.autoresearch {
            Some(p) => p.clone(),
            None => which("autoresearch")?,
        })?,
        claude: absolute(match &args.claude {
            Some(p) => p.clone(),
            None => which("claude")?,
        })?,
        claude_model: args.claude_model.clone(),
        start_weekday: args.start_weekday,
        start_hour: args.start_hour,
    })
}

fn run(cli: Cli) -> Result<String, String> {
    // Resolved per command: the evaluator runs with an empty environment and no HOME.
    let store = || match &cli.store {
        Some(s) => Ok(s.clone()),
        None => store::default_dir(),
    };
    match cli.command {
        Cmd::Record {
            epic,
            step,
            kind,
            note,
            repo,
            skill,
            agent,
            reference,
            must,
            must_not,
        } => {
            let check = (must.is_some() || must_not.is_some()).then_some(Check { must, must_not });
            let agent = agent.unwrap_or_else(|| {
                let codex = std::env::vars().any(|(k, _)| k.starts_with("CODEX_"));
                if codex { "codex" } else { "claude" }.to_string()
            });
            let fb = Feedback {
                schema: feedback::SCHEMA,
                repo: match repo {
                    Some(r) => r,
                    None => main_checkout_name()?,
                },
                epic,
                step,
                kind,
                skill,
                agent,
                note,
                reference,
                check,
            };
            store::append(&store()?, &fb)?;
            Ok(format!(
                "ok:recorded:{}:{}",
                fb.event_type(),
                fb.entity_id()
            ))
        }
        Cmd::List { epic, json } => {
            let (records, unreadable) = store::read_all(&store()?)?;
            let records: Vec<_> = records
                .into_iter()
                .filter(|r| epic.as_deref().is_none_or(|e| r.feedback.epic == e))
                .collect();
            if json {
                return serde_json::to_string_pretty(&records).map_err(|e| e.to_string());
            }
            let mut out = records
                .iter()
                .map(|r| {
                    let f = &r.feedback;
                    format!(
                        "{} {} {}/{} {} {}: {}",
                        r.timestamp.format("%Y-%m-%d %H:%M"),
                        f.kind.as_str(),
                        f.repo,
                        f.epic,
                        f.skill.dir(),
                        f.step,
                        f.note
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            out.push_str(&format!(
                "\n{} events, {unreadable} unreadable",
                records.len()
            ));
            Ok(out)
        }
        Cmd::Export { out_dir } => {
            let (records, unreadable) = store::read_all(&store()?)?;
            let manifest = cases::export(&records, unreadable, &out_dir, |_| true)?;
            serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())
        }
        Cmd::Evaluate {
            dev,
            holdout,
            holdout_sha256,
            holdout_max_open,
            max_words,
        } => Ok(evaluate::run(&evaluate::Frozen {
            dev,
            holdout,
            holdout_sha256,
            holdout_max_open,
            max_words,
        })),
        Cmd::Weekly(args) => weekly::step(&weekly_options(&args, store()?)?),
        Cmd::Schedule {
            weekly: args,
            print,
        } => {
            let opts = weekly_options(&args, store()?)?;
            let mut argv = vec![
                opts.evaluator.display().to_string(),
                "--store".into(),
                opts.store.display().to_string(),
                "weekly".into(),
                "--workdir".into(),
                opts.workdir.display().to_string(),
                "--repo-url".into(),
                opts.repo_url.clone(),
                "--branch".into(),
                opts.branch.clone(),
                "--candidates".into(),
                opts.candidates.to_string(),
                "--start-weekday".into(),
                opts.start_weekday.to_string(),
                "--start-hour".into(),
                opts.start_hour.to_string(),
                "--autoresearch".into(),
                opts.autoresearch.display().to_string(),
                "--claude".into(),
                opts.claude.display().to_string(),
            ];
            if let Some(model) = &opts.claude_model {
                argv.extend(["--claude-model".into(), model.clone()]);
            }
            let home = home()?;
            let path_env = std::env::var("PATH").unwrap_or_default();
            let log = opts.workdir.join("logs").join("launchd.log");
            let body = schedule::plist(&argv, &path_env, &home, &log);
            if print {
                return Ok(body);
            }
            std::fs::create_dir_all(opts.workdir.join("logs")).map_err(|e| e.to_string())?;
            let path = schedule::install(&body, &home)?;
            Ok(format!(
                "ok:scheduled:{}:{}",
                schedule::LABEL,
                path.display()
            ))
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ritual-feedback: {e}");
            ExitCode::FAILURE
        }
    }
}
