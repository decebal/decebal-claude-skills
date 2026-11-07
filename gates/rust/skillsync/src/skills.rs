//! `skillsync skills install`: symlink repo skills into `~/.claude/skills` and
//! `~/.agents/skills`. Anything already at a target path that is not a symlink to
//! this repo is a conflict, and a conflict leaves the whole install undone. An I/O
//! error partway through removes the links this run created and reports how many.

use crate::repo::Repo;
use crate::report::Report;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const TARGETS: [&str; 2] = [".claude/skills", ".agents/skills"];

pub fn install(home: &Path, repo: &Repo, dry_run: bool, report: &mut Report) -> Result<(), String> {
    let mut links: Vec<(PathBuf, PathBuf)> = Vec::new();
    for rel in TARGETS {
        let dir = home.join(rel);
        for skill in repo.skills()? {
            let link = dir.join(&skill.name);
            let target = fs::canonicalize(&skill.dir)
                .map_err(|e| format!("cannot resolve {}: {e}", skill.dir.display()))?;
            match fs::symlink_metadata(&link) {
                Err(e) if e.kind() == ErrorKind::NotFound => links.push((link, target)),
                Err(e) => report.finding("conflict", link.display(), e),
                Ok(meta) if meta.file_type().is_symlink() => {
                    if fs::canonicalize(&link).ok().as_deref() == Some(target.as_path()) {
                        report.notice(format!("{}: already linked", link.display()));
                    } else {
                        let elsewhere = fs::read_link(&link)
                            .map(|t| t.display().to_string())
                            .unwrap_or_default();
                        report.finding(
                            "conflict",
                            link.display(),
                            format!("symlink to {elsewhere}, not this repo; left in place"),
                        );
                    }
                }
                Ok(_) => report.finding(
                    "conflict",
                    link.display(),
                    "exists and is not a symlink; left in place",
                ),
            }
        }
    }

    if !report.findings.is_empty() {
        report.notice("nothing installed; resolve the conflicts above and rerun");
        return Ok(());
    }
    for (link, target) in &links {
        report.action(format!("link {} -> {}", link.display(), target.display()));
    }
    if dry_run {
        return Ok(());
    }
    let mut created = Vec::new();
    if let Err(e) = apply(&links, &mut created) {
        for link in &created {
            let _ = fs::remove_file(link);
        }
        return Err(format!(
            "{e}; removed {} link(s) this run made",
            created.len()
        ));
    }
    Ok(())
}

fn apply(links: &[(PathBuf, PathBuf)], created: &mut Vec<PathBuf>) -> Result<(), String> {
    for (link, target) in links {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        symlink(target, link).map_err(|e| format!("cannot link {}: {e}", link.display()))?;
        created.push(link.clone());
    }
    Ok(())
}
