//! Where each host loads skills from, and the skills found there.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::catalog::{listing_cost, rating_key, Host, Skill, Source};
use crate::frontmatter::{self, Frontmatter};

const MAX_SKILL_BYTES: u64 = 1 << 20;

static PLUGIN_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*\[\s*plugins\s*\.\s*(?:"([^"]+)"|([A-Za-z0-9_@.-]+))\s*\]\s*(?:#.*)?$"#)
        .expect("valid regex")
});
static ANY_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\[").expect("valid regex"));
static ENABLED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*enabled\s*=\s*(true|false)\b").expect("valid regex"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    pub dir: PathBuf,
    pub host: Host,
    pub source: Source,
    pub plugin: Option<String>,
}

impl Root {
    fn new(dir: PathBuf, host: Host, source: Source, plugin: Option<String>) -> Root {
        Root {
            dir,
            host,
            source,
            plugin,
        }
    }
}

/// Every skill root under `home`, in the order entries are listed.
pub fn roots(home: &Path) -> Vec<Root> {
    let mut out = vec![Root::new(
        home.join(".claude/skills"),
        Host::Claude,
        Source::User,
        None,
    )];
    out.push(Root::new(
        home.join(".claude/commands"),
        Host::Claude,
        Source::Command,
        None,
    ));
    let settings = read_json(&home.join(".claude/settings.json"));
    let installed = read_json(&home.join(".claude/plugins/installed_plugins.json"));
    for (plugin, path) in claude_plugins(&settings, &installed) {
        out.push(Root::new(
            path.join("skills"),
            Host::Claude,
            Source::Plugin,
            Some(plugin.clone()),
        ));
        out.push(Root::new(
            path.join("commands"),
            Host::Claude,
            Source::Command,
            Some(plugin),
        ));
    }
    out.push(Root::new(
        home.join(".agents/skills"),
        Host::Codex,
        Source::User,
        None,
    ));
    out.push(Root::new(
        home.join(".codex/skills"),
        Host::Codex,
        Source::User,
        None,
    ));
    out.push(Root::new(
        home.join(".codex/skills/.system"),
        Host::Codex,
        Source::System,
        None,
    ));
    let config = fs::read_to_string(home.join(".codex/config.toml")).unwrap_or_default();
    for id in codex_plugins(&config) {
        let Some((name, market)) = id.split_once('@') else {
            continue;
        };
        let base = home.join(".codex/plugins/cache").join(market).join(name);
        if let Some(version) = newest_subdir(&base) {
            out.push(Root::new(
                version.join("skills"),
                Host::Codex,
                Source::Plugin,
                Some(name.to_owned()),
            ));
        }
    }
    out
}

pub fn discover(home: &Path) -> Vec<Skill> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for root in roots(home) {
        let entries = if root.source == Source::Command {
            command_files(&root.dir)
        } else {
            skill_files(&root.dir)
        };
        for (dir_name, path) in entries {
            let real = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !seen.insert((root.host, real)) {
                continue;
            }
            if let Some(text) = read_bounded(&path) {
                let mut fm = frontmatter::parse(&text);
                if root.source == Source::Command && fm.description.is_none() {
                    fm.description = first_prose_line(&text);
                }
                out.push(build_skill(&fm, &dir_name, &path, &root));
            }
        }
    }
    out
}

/// `<dir>/<name>.md` command files: Claude Code lists each one beside the
/// skills, by file name.
pub fn command_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let is_md = path.extension().is_some_and(|x| x == "md") && path.is_file();
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            (is_md && !stem.starts_with('.')).then_some((stem, path))
        })
        .collect();
    out.sort();
    out
}

/// A command with no `description` is listed by its first line of prose.
fn first_prose_line(text: &str) -> Option<String> {
    let body = match text.strip_prefix("---") {
        Some(rest) => rest.split_once("\n---").map_or(text, |(_, b)| b),
        None => text,
    };
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("---"))
        .map(str::to_owned)
}

/// `<dir>/<name>/SKILL.md` for each visible subdirectory, symlinks followed.
pub fn skill_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let file = e.path().join("SKILL.md");
            (!name.starts_with('.') && file.is_file()).then_some((name, file))
        })
        .collect();
    out.sort();
    out
}

pub fn build_skill(fm: &Frontmatter, dir_name: &str, path: &Path, root: &Root) -> Skill {
    let name = fm.name.clone().unwrap_or_else(|| dir_name.to_owned());
    let description = fm.description.clone().unwrap_or_default();
    let display = match &root.plugin {
        Some(p) => format!("{p}:{name}"),
        None => name.clone(),
    };
    Skill {
        cost: listing_cost(root.host, &display, &description),
        key: rating_key(&name, &description),
        name,
        description,
        dir_name: dir_name.to_owned(),
        path: path.to_string_lossy().into_owned(),
        host: root.host,
        source: root.source,
        plugin: root.plugin.clone(),
        controllable: root.source == Source::User,
        tier: root.source.default_tier().to_owned(),
    }
}

/// Enabled Claude Code plugins and their install paths: `(plugin name, path)`.
pub fn claude_plugins(settings: &Value, installed: &Value) -> Vec<(String, PathBuf)> {
    let Some(enabled) = settings.get("enabledPlugins").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, on) in enabled {
        if on.as_bool() != Some(true) {
            continue;
        }
        let Some(installs) = installed
            .get("plugins")
            .and_then(|p| p.get(id))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let pick = installs
            .iter()
            .find(|i| i.get("scope").and_then(Value::as_str) == Some("user"))
            .or_else(|| installs.first());
        let Some(path) = pick
            .and_then(|i| i.get("installPath"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let name = id.split('@').next().unwrap_or(id);
        out.push((name.to_owned(), PathBuf::from(path)));
    }
    out
}

/// `plugin@marketplace` ids of `[plugins."…"]` tables in a Codex `config.toml`
/// whose `enabled` is not `false`.
pub fn codex_plugins(config: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Option<(String, bool)> = None;
    let mut close = |current: &mut Option<(String, bool)>| {
        if let Some((id, true)) = current.take() {
            out.push(id);
        }
    };
    for line in config.lines() {
        if let Some(c) = PLUGIN_HEADER.captures(line) {
            close(&mut current);
            let id = c.get(1).or_else(|| c.get(2)).map_or("", |m| m.as_str());
            current = Some((id.to_owned(), true));
        } else if ANY_HEADER.is_match(line) {
            close(&mut current);
        } else if let (Some(cur), Some(c)) = (current.as_mut(), ENABLED.captures(line)) {
            cur.1 = &c[1] == "true";
        }
    }
    close(&mut current);
    out
}

fn newest_subdir(base: &Path) -> Option<PathBuf> {
    fs::read_dir(base)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

fn read_json(path: &Path) -> Value {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

fn read_bounded(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_SKILL_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
#[path = "scan_tests.rs"]
mod tests;
