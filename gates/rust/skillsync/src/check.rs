//! `skillsync check`: a read-only comparison of the repo against a home directory.

use crate::cn;
use crate::hooks::{self, CLAUDE_TEMPLATE, CODEX_TEMPLATE};
use crate::repo::{self, Repo, Skill};
use crate::report::Report;
use serde_json::Value;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Skill roots to compare, and whether a missing root counts as a finding.
const SKILL_ROOTS: [(&str, bool); 3] = [
    (".claude/skills", true),
    (".agents/skills", true),
    (".codex/skills", false),
];

/// `strict` reports each uninstalled repo skill as a finding. Without it, a home
/// that holds a curated subset gets one notice per root, and only copies, drift,
/// dangling links and links elsewhere count as findings.
pub fn run(
    home: &Path,
    repo: &Repo,
    cn_bin: &str,
    strict: bool,
    report: &mut Report,
) -> Result<(), String> {
    skills(home, repo, strict, report)?;
    guard_hooks(home, repo, report)?;
    prod_tokens(home, repo, report);
    cn::lint(cn_bin, repo, report)
}

fn skills(home: &Path, repo: &Repo, strict: bool, report: &mut Report) -> Result<(), String> {
    let skills = repo.skills()?;
    for (rel, required) in SKILL_ROOTS {
        let root = home.join(rel);
        if !root.is_dir() && !required {
            report.notice(format!("skip {rel}: not present"));
            continue;
        }
        let mut missing = 0;
        for skill in &skills {
            let link = root.join(&skill.name);
            if !installed(&link) {
                missing += 1;
                if strict {
                    report.finding("missing", link.display(), "not installed");
                }
                continue;
            }
            check_skill(&link, skill, report);
        }
        if missing > 0 && !strict {
            report.notice(format!(
                "{missing} of {} repo skills not in {}; `skillsync skills install` adds them, --strict reports each",
                skills.len(),
                root.display()
            ));
        }
    }
    Ok(())
}

fn installed(link: &Path) -> bool {
    !matches!(fs::symlink_metadata(link), Err(e) if e.kind() == ErrorKind::NotFound)
}

fn check_skill(link: &Path, skill: &Skill, report: &mut Report) {
    let meta = match fs::symlink_metadata(link) {
        Ok(meta) => meta,
        Err(e) => {
            report.finding("unreadable", link.display(), e);
            return;
        }
    };

    if meta.file_type().is_symlink() {
        let Ok(resolved) = fs::canonicalize(link) else {
            let target = fs::read_link(link)
                .map(|t| t.display().to_string())
                .unwrap_or_default();
            report.finding(
                "dangling",
                link.display(),
                format!("symlink to {target}, which does not exist"),
            );
            return;
        };
        let repo_dir = fs::canonicalize(&skill.dir).unwrap_or_else(|_| skill.dir.clone());
        if resolved != repo_dir {
            report.finding(
                "elsewhere",
                link.display(),
                format!("symlink to {}, not this repo", resolved.display()),
            );
            return;
        }
    } else if meta.is_dir() {
        report.finding(
            "copy",
            link.display(),
            "a copy, not a symlink into this repo",
        );
    } else {
        report.finding(
            "not-a-directory",
            link.display(),
            "expected a skill directory",
        );
        return;
    }
    drift(link, skill, report);
}

fn drift(link: &Path, skill: &Skill, report: &mut Report) {
    let installed = link.join("SKILL.md");
    let source = skill.dir.join("SKILL.md");
    match (repo::sha256(&installed), repo::sha256(&source)) {
        (Ok(a), Ok(b)) if a != b => report.finding(
            "drift",
            installed.display(),
            format!("sha256 {} differs from repo {}", &a[..12], &b[..12]),
        ),
        (Err(e), _) => report.finding("drift", installed.display(), e),
        _ => {}
    }
}

fn guard_hooks(home: &Path, repo: &Repo, report: &mut Report) -> Result<(), String> {
    let targets = [
        (CLAUDE_TEMPLATE, ".claude/settings.json", false),
        (CODEX_TEMPLATE, ".codex/hooks.json", true),
    ];
    for (template, rel, needs_codex_home) in targets {
        if needs_codex_home && !home.join(".codex").is_dir() {
            report.notice(format!("skip {rel}: no ~/.codex in this home"));
            continue;
        }
        let guards = hooks::guards(&repo.template(template)?);
        let path = home.join(rel);
        let doc: Value = match repo::read_json(&path) {
            Ok(doc) => doc,
            Err(e) => {
                report.finding("hooks-file", path.display(), e);
                continue;
            }
        };
        let hook_map = doc.get("hooks").and_then(Value::as_object);
        for guard in guards {
            let expected = format!("{} [{}] {}", guard.event, guard.matcher, guard.command);
            match hook_map.and_then(|h| hooks::locate(h, &guard.command)) {
                None => report.finding("hook-missing", path.display(), expected),
                Some((event, matcher)) if event != guard.event || matcher != guard.matcher => {
                    report.finding(
                        "hook-matcher",
                        path.display(),
                        format!(
                            "{} is under {event} [{matcher}], template says {expected}",
                            guard.command
                        ),
                    );
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

fn prod_tokens(home: &Path, repo: &Repo, report: &mut Report) {
    let live = home.join(".claude/prod-guard-tokens.txt");
    let template = repo.root.join("configs/prod-guard-tokens.txt");
    if !live.is_file() {
        report
            .notice("no ~/.claude/prod-guard-tokens.txt; infra-guard runs its literal tiers only");
        return;
    }
    match (fs::read_to_string(&live), fs::read_to_string(&template)) {
        (Ok(a), Ok(b)) if a == b => report.finding(
            "prod-token-template",
            live.display(),
            "still the repo template; replace the placeholder tokens",
        ),
        (Err(e), _) => report.finding("unreadable", live.display(), e),
        _ => {}
    }
}
