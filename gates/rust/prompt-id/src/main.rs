//! prompt-id — allocate, archive and audit prompt numbers across every worktree.
//!
//! Subcommands:
//!   store             print the resolved store path
//!   next              print the next free number, zero-padded
//!   alloc [--dir] <slug>
//!                     atomically create `<store>/NNN-<slug>.md` (or, with
//!                     `--dir`, the meta-prompt directory) and print its path
//!   archive <n|name>  move a finished prompt into `completed/` and make it tracked
//!   shipped           report root prompts whose reserved branch has already merged
//!   audit [--since n] report duplicated numbers and unused runs
//!
//! `alloc` is the one that matters: it CREATES the file with `create_new`, so
//! two sessions racing from different worktrees cannot be handed the same
//! number — the loser's create fails and it takes the next one. Printing a
//! number for the caller to use later would reintroduce the very race this
//! crate exists to remove.

/// Print a line, treating a closed pipe as a normal end rather than a panic.
///
/// `audit` is meant to be read through `head`/`grep`, and Rust's `println!`
/// panics when the reader goes away — which turns "I looked at the first three
/// lines" into a stack trace.
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let mut out = std::io::stdout().lock();
        if writeln!(out, $($arg)*).is_err() {
            return ExitCode::SUCCESS;
        }
    }};
}

mod archive;
mod branch;
mod shipped;
mod store;

#[cfg(test)]
mod store_tests;

#[cfg(test)]
mod branch_tests;

#[cfg(test)]
mod archive_tests;

#[cfg(test)]
mod shipped_tests;

use std::io::ErrorKind;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => return fail(&format!("cannot read the working directory: {e}")),
    };
    let root = match store::resolve(&cwd) {
        Ok(path) => path,
        Err(e) => return fail(&e),
    };

    match args.first().map(String::as_str) {
        Some("store") => {
            outln!("{}", root.display());
            ExitCode::SUCCESS
        }
        Some("next") => {
            // Steps past numbers a concurrent session has claimed but not yet
            // written a file for, so the number reported is one a caller could
            // actually get.
            let entries = store::entries(&root);
            let mut candidate = store::next_free(&entries, 1);
            while store::is_taken(&root, &entries, candidate) {
                candidate += 1;
            }
            outln!("{candidate:03}");
            ExitCode::SUCCESS
        }
        Some("alloc") => {
            let as_dir = args.iter().any(|a| a == "--dir");
            match args.iter().skip(1).find(|a| !a.starts_with("--")) {
                Some(slug) => alloc(&root, slug, as_dir),
                None => fail("alloc needs a slug: prompt-id alloc a-thing-that-works"),
            }
        }
        Some("archive") => match args.get(1) {
            Some(which) => run_archive(&root, which),
            None => fail("archive needs a number or name: prompt-id archive 208"),
        },
        Some("shipped") => run_shipped(&root, number_flag(&args, "--limit").unwrap_or(200)),
        Some("audit") => audit(&root, number_flag(&args, "--since").unwrap_or(0)),
        _ => fail("usage: prompt-id <store|next|alloc [--dir] <slug>|archive <n>|shipped|audit>"),
    }
}

/// The value of `--flag n`, or of `--flag=n`.
fn number_flag(args: &[String], flag: &str) -> Option<u32> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(rest) = arg.strip_prefix(flag) {
            let value = rest.strip_prefix('=').map(str::to_string);
            return match value {
                Some(v) => v.parse().ok(),
                None if rest.is_empty() => iter.next().and_then(|v| v.parse().ok()),
                None => None,
            };
        }
    }
    None
}

/// Claim the next free number by creating the file, stepping past any number a
/// concurrent session wins first.
///
/// `as_dir` makes it a meta-prompt's stage DIRECTORY instead. That path exists so
/// meta-prompts claim their number through the same lock as everything else —
/// the alternative, reading `next` and creating the directory afterwards, is the
/// read-then-write race this crate was built to remove.
fn alloc(root: &Path, slug: &str, as_dir: bool) -> ExitCode {
    let slug = slug.trim_matches('-');
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return fail("slug must be lowercase words separated by hyphens");
    }
    if let Err(e) = std::fs::create_dir_all(root) {
        return fail(&format!("cannot create {}: {e}", root.display()));
    }

    let mut candidate = store::next_free(&store::entries(root), 1);
    // Bounded so a permissions fault cannot spin: 200 consecutive losses would
    // mean 200 sessions allocating at once, which is not a real state.
    for _ in 0..200 {
        // Claim the NUMBER first. The file name cannot be the claim: two
        // sessions picking the same number under different slugs would both
        // succeed at creating their own path.
        match store::claim(root, candidate) {
            Ok(false) => {
                candidate += 1;
                continue;
            }
            Err(e) => return fail(&format!("cannot claim {candidate:03}: {e}")),
            Ok(true) => {}
        }
        let name = if as_dir {
            format!("{candidate:03}-{slug}")
        } else {
            format!("{candidate:03}-{slug}.md")
        };
        let path = root.join(name);
        let created = if as_dir {
            std::fs::create_dir(&path)
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map(|_| ())
        };
        match created {
            Ok(()) => {
                outln!("{}", path.display());
                return ExitCode::SUCCESS;
            }
            // The number was ours but this exact name already exists — the same
            // prompt asked for twice. Keep the name, hand back what is there.
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                outln!("{}", path.display());
                return ExitCode::SUCCESS;
            }
            Err(e) => return fail(&format!("cannot create {}: {e}", path.display())),
        }
    }
    fail("could not claim a number after 200 attempts")
}

fn run_archive(root: &Path, which: &str) -> ExitCode {
    match archive::archive(root, which) {
        archive::Outcome::Archived(path) => {
            outln!("{}", path.display());
            ExitCode::SUCCESS
        }
        archive::Outcome::AlreadyArchived(names) => {
            for name in names {
                outln!("already archived: completed/{name}");
            }
            ExitCode::SUCCESS
        }
        archive::Outcome::Ambiguous(names) => fail(&format!(
            "{which} matches {} prompts — name one exactly: {}",
            names.len(),
            names.join(", ")
        )),
        archive::Outcome::NotFound => fail(&format!("no prompt at the store root matches {which}")),
        archive::Outcome::Failed(message) => fail(&message),
    }
}

/// Report the root prompts whose branch already merged, and separately the ones
/// that reserve no branch at all.
///
/// A prompt with no branch is unanswerable rather than wrong, so it is listed
/// and does NOT fail the check — otherwise the check is red forever and gets
/// ignored, which is the state `audit` was already in.
fn run_shipped(root: &Path, limit: u32) -> ExitCode {
    let Some(repo) = root.parent() else {
        return fail(&format!("{} has no parent repo", root.display()));
    };
    let entries = store::entries(root);
    let reserved = shipped::reserved(root, &entries);
    let merged = match shipped::merged(repo, limit) {
        Ok(list) => list,
        Err(e) => return fail(&format!("gh pr list failed: {e}")),
    };

    outln!("store: {}", root.display());
    outln!(
        "checked: {} prompts at the store root against {} merged pull requests",
        reserved.len(),
        merged.len()
    );

    let mut shipped_now = Vec::new();
    let mut branchless = Vec::new();
    for prompt in &reserved {
        match &prompt.branch {
            None => branchless.push(prompt.file_name.as_str()),
            Some(name) => {
                if let Some(pr) = shipped::find(&merged, name) {
                    shipped_now.push((prompt.file_name.as_str(), name.as_str(), pr));
                }
            }
        }
    }

    if shipped_now.is_empty() {
        outln!("shipped but unarchived: none");
    } else {
        outln!("shipped but unarchived: {}", shipped_now.len());
        for (file, name, pr) in &shipped_now {
            outln!(
                "  {file}  {name}  PR #{} merged {}",
                pr.number,
                shipped::day(&pr.merged_at)
            );
        }
        outln!("archive each with: prompt-id archive <number>");
    }

    if !branchless.is_empty() {
        outln!("no branch declared: {}", branchless.len());
        for file in &branchless {
            outln!("  {file}");
        }
    }

    if shipped_now.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Say what the store's numbering looks like, and exit non-zero when a number is
/// shared — so this can serve as a gate rather than only a report.
///
/// `since` is what makes the gate usable: 55 numbers were already shared before
/// the allocator existed, so an unfloored audit is permanently red and can be
/// wired to nothing. `--since <n>` fails only on duplicates at or above `n`.
fn audit(root: &Path, since: u32) -> ExitCode {
    let entries = store::entries(root);
    let distinct: std::collections::BTreeSet<u32> = entries.iter().map(|e| e.number).collect();
    outln!("store: {}", root.display());
    let archived = entries.iter().filter(|e| e.archived).count();
    outln!(
        "prompts: {} ({} active, {} archived) | distinct numbers: {}",
        entries.len(),
        entries.len() - archived,
        archived,
        distinct.len()
    );

    let gaps = store::gaps(&entries);
    if gaps.is_empty() {
        outln!("unused numbers: none");
    } else {
        let rendered: Vec<String> = gaps
            .iter()
            .map(|(from, to)| {
                if from == to {
                    format!("{from:03}")
                } else {
                    format!("{from:03}-{to:03}")
                }
            })
            .collect();
        outln!("unused numbers: {}", rendered.join(", "));
    }

    let duplicates = store::duplicates(&entries);
    if duplicates.is_empty() {
        outln!("duplicated numbers: none");
        return ExitCode::SUCCESS;
    }
    let (failing, tolerated): (Vec<_>, Vec<_>) =
        duplicates.iter().partition(|(number, _)| **number >= since);
    outln!(
        "duplicated numbers: {} ({} at or above {since:03})",
        duplicates.len(),
        failing.len()
    );
    for (number, files) in &failing {
        outln!("  {number:03}: {}", files.join(", "));
    }
    if since > 0 && !tolerated.is_empty() {
        outln!("below {since:03}, not counted: {}", tolerated.len());
    }
    if failing.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("prompt-id: {message}");
    ExitCode::FAILURE
}
