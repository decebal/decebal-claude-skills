//! report-press: a story in Markdown → one newspaper-style HTML page with
//! animated Markdy figures, self-contained so it opens from `file://`.
//!
//!     report-press check story.md
//!     report-press build story.md [-o out.html]
//!
//! `build` refuses a story that `check` reports errors for.

mod brand;
mod lint;
mod markup;
mod render;
mod story;

#[cfg(test)]
mod lint_tests;
#[cfg(test)]
mod markup_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod story_tests;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lint::{Finding, Level};

const USAGE: &str = "report-press: newspaper-style HTML reports from Markdown stories

usage:
  report-press check <story.md>
  report-press build <story.md> [-o <out.html>]   (default: story path with .html)
  report-press --version
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 2 => run(Path::new(&args[1]), None),
        Some("build") => match args.as_slice() {
            [_, input] => {
                let input = Path::new(input);
                run(input, Some(input.with_extension("html")))
            }
            [_, input, flag, out] if flag == "-o" => {
                run(Path::new(input), Some(PathBuf::from(out)))
            }
            _ => usage(),
        },
        Some("--version") => {
            println!("report-press {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("-h" | "--help") => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprint!("{USAGE}");
    ExitCode::from(2)
}

fn run(input: &Path, out: Option<PathBuf>) -> ExitCode {
    let src = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: cannot read: {e}", input.display());
            return ExitCode::from(2);
        }
    };
    let story = match story::parse(&src) {
        Ok(s) => s,
        Err(problems) => {
            for p in problems {
                eprintln!("{}:{}: error: {}", input.display(), p.line, p.msg);
            }
            return ExitCode::FAILURE;
        }
    };
    let findings = lint::check(&story);
    report(input, &findings);
    let errors = findings.iter().filter(|f| f.level == Level::Error).count();
    if errors > 0 {
        eprintln!("{}: {errors} error(s); nothing written", input.display());
        return ExitCode::FAILURE;
    }
    if let Some(out) = out {
        if let Err(e) = std::fs::write(&out, render::page(&story)) {
            eprintln!("{}: cannot write: {e}", out.display());
            return ExitCode::from(2);
        }
        let figures: usize = story.sections.iter().map(|s| story::figures(s).len()).sum();
        println!(
            "wrote {} ({} sections, {figures} figures)",
            out.display(),
            story.sections.len()
        );
    } else {
        println!("{}: ok", input.display());
    }
    ExitCode::SUCCESS
}

fn report(input: &Path, findings: &[Finding]) {
    for f in findings {
        let level = match f.level {
            Level::Error => "error",
            Level::Warn => "warn",
        };
        eprintln!("{}:{}: {level}: {}", input.display(), f.line, f.msg);
    }
}
