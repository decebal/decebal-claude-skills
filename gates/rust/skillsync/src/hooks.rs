//! Guard hook entries: read from the templates in `configs/`, merged into a home
//! directory's hook files. An entry already present under another spelling of the
//! same guard (an absolute path to the binary, say) counts as present, so a rerun
//! finds what an earlier install or a hand edit wrote and changes nothing.

use crate::repo::{read_json, Repo};
use crate::report::Report;
use serde_json::{json, Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CLAUDE_TEMPLATE: &str = "settings.json";
pub const CODEX_TEMPLATE: &str = "codex-hooks.json";

/// A guard hook as a template declares it.
pub struct Guard {
    pub event: String,
    pub matcher: String,
    pub command: String,
    pub hook: Value,
}

/// Every hook whose executable name ends in `-guard`, in template order.
pub fn guards(template: &Value) -> Vec<Guard> {
    let mut out = Vec::new();
    let Some(events) = template.get("hooks").and_then(Value::as_object) else {
        return out;
    };
    for (event, groups) in events {
        for group in groups.as_array().into_iter().flatten() {
            let matcher = group.get("matcher").and_then(Value::as_str).unwrap_or("");
            for hook in group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(command) = hook.get("command").and_then(Value::as_str) else {
                    continue;
                };
                if signature(command).0.ends_with("-guard") {
                    out.push(Guard {
                        event: event.clone(),
                        matcher: matcher.to_string(),
                        command: command.to_string(),
                        hook: hook.clone(),
                    });
                }
            }
        }
    }
    out
}

/// The executable's file name and the subcommand. `/x/.cargo/bin/codex-guard infra-guard`
/// and `codex-guard infra-guard` share a signature.
pub fn signature(command: &str) -> (&str, &str) {
    let mut words = command.split_whitespace();
    let exe = words.next().unwrap_or("").rsplit('/').next().unwrap_or("");
    (exe, words.next().unwrap_or(""))
}

/// The event and matcher of the group holding the guard `command` names, if any.
pub fn locate(hooks: &Map<String, Value>, command: &str) -> Option<(String, String)> {
    let wanted = signature(command);
    for (event, groups) in hooks {
        for group in groups.as_array().into_iter().flatten() {
            let found = group
                .get("hooks")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|h| h.get("command").and_then(Value::as_str))
                .any(|c| signature(c) == wanted);
            if found {
                let matcher = group.get("matcher").and_then(Value::as_str).unwrap_or("");
                return Some((event.clone(), matcher.to_string()));
            }
        }
    }
    None
}

/// Add each guard whose command is absent. Returns one line per added entry.
pub fn merge(doc: &mut Value, guards: &[Guard]) -> Result<Vec<String>, String> {
    let root = doc
        .as_object_mut()
        .ok_or("the file's root is not a JSON object")?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("\"hooks\" is not a JSON object")?;

    let mut actions = Vec::new();
    for guard in guards {
        if locate(hooks, &guard.command).is_some() {
            continue;
        }
        let groups = hooks
            .entry(guard.event.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| format!("hooks.{} is not an array", guard.event))?;
        let same_matcher = groups.iter_mut().find(|group| {
            group.get("matcher").and_then(Value::as_str).unwrap_or("") == guard.matcher
        });
        match same_matcher {
            Some(group) => {
                group
                    .as_object_mut()
                    .ok_or("a hook group is not a JSON object")?
                    .entry("hooks")
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .ok_or("a group's \"hooks\" is not an array")?
                    .push(guard.hook.clone());
            }
            None => {
                let mut group = Map::new();
                if !guard.matcher.is_empty() {
                    group.insert("matcher".into(), json!(guard.matcher));
                }
                group.insert("hooks".into(), json!([guard.hook.clone()]));
                groups.push(Value::Object(group));
            }
        }
        actions.push(format!(
            "{} [{}] {}",
            guard.event, guard.matcher, guard.command
        ));
    }
    Ok(actions)
}

/// A hook file whose merged text is ready to write.
struct Planned {
    path: PathBuf,
    existed: bool,
    text: String,
}

/// Merge the templates into `<home>/.claude/settings.json` and, when `~/.codex`
/// exists, `<home>/.codex/hooks.json`. Every file is merged in memory before any is
/// written, so a malformed file stops the run with nothing changed. A changed file
/// is backed up first.
pub fn install(home: &Path, repo: &Repo, dry_run: bool, report: &mut Report) -> Result<(), String> {
    let targets = [
        (CLAUDE_TEMPLATE, ".claude/settings.json", false),
        (CODEX_TEMPLATE, ".codex/hooks.json", true),
    ];
    let mut planned = Vec::new();
    for (template, rel, needs_codex_home) in targets {
        if needs_codex_home && !home.join(".codex").is_dir() {
            report.notice(format!("skip {rel}: no ~/.codex in this home"));
            continue;
        }
        let path = home.join(rel);
        let guards = guards(&repo.template(template)?);
        let existed = path.is_file();
        let mut doc = if existed {
            read_json(&path)?
        } else {
            json!({})
        };
        let actions = merge(&mut doc, &guards)?;
        if actions.is_empty() {
            report.notice(format!("{rel}: guard hooks already present"));
            continue;
        }
        for action in &actions {
            report.action(format!("{rel}: {action}"));
        }
        let mut text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
        text.push('\n');
        planned.push(Planned {
            path,
            existed,
            text,
        });
    }
    if dry_run {
        return Ok(());
    }
    for file in planned {
        let path = if file.existed {
            fs::canonicalize(&file.path)
                .map_err(|e| format!("cannot resolve {}: {e}", file.path.display()))?
        } else {
            file.path.clone()
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        if file.existed {
            let backup = backup_path(&path);
            fs::copy(&path, &backup)
                .map_err(|e| format!("cannot back up {}: {e}", path.display()))?;
            report.action(format!("backup: {}", backup.display()));
        }
        write_replacing(&path, &file.text)?;
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("{}-{}", d.as_secs(), d.subsec_nanos()))
        .unwrap_or_default();
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    path.with_file_name(format!("{name}.skillsync-{stamp}.bak"))
}

/// Write beside `path` and rename over it. `path` must already be the resolved
/// target of any symlink, or the rename replaces the link.
fn write_replacing(path: &Path, text: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(
        "{name}.skillsync-{}-{nanos}.tmp",
        std::process::id()
    ));
    fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("cannot replace {}: {e}", path.display())
    })
}
