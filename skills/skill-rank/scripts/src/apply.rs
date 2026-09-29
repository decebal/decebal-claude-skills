//! Which skills stay fully listed on a host, and the settings edits that say so.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{json, Value};

use crate::catalog::{Catalog, Host, Skill, Source, PINNED};
use crate::fmt;

pub const BEGIN: &str = "# BEGIN skill-rank (managed; edit via skill-rank apply)";
pub const END: &str = "# END skill-rank";
pub const CHARS_PER_TOKEN: u64 = 4;
pub const DEFAULT_CONTEXT_TOKENS: u64 = 200_000;
/// Codex caps a configured `[skills] max_context_tokens` at this.
pub const CODEX_MAX_CONTEXT_TOKENS: u64 = 10_000;

static SKILLS_CONFIG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\[\[\s*skills\s*\.\s*config\s*\]\]").expect("valid regex"));
/// The only lines this tool writes inside its block.
static BLOCK_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?:#.*|\[\[skills\.config\]\]|name = "(?:[^"\\]|\\.)*"|enabled = false)?\s*$"#,
    )
    .expect("valid regex")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    On,
    NameOnly,
    Disabled,
    UserOnly,
    Plugin,
    System,
    Command,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::On => "on",
            Tier::NameOnly => "name-only",
            Tier::Disabled => "disabled",
            Tier::UserOnly => "user-only",
            Tier::Plugin => "plugin",
            Tier::System => "system",
            Tier::Command => "command",
        }
    }

    /// A Claude Code `skillOverrides` value.
    pub fn from_override(value: &str) -> Tier {
        match value {
            "name-only" => Tier::NameOnly,
            "off" => Tier::Disabled,
            "user-invocable-only" => Tier::UserOnly,
            _ => Tier::On,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub name: String,
    pub rating: Option<f64>,
    pub uses: u64,
    pub cost: u64,
    pub tier: Tier,
    pub running: u64,
    pub user_set: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub host: Host,
    pub budget: u64,
    pub fixed_cost: u64,
    pub reserved: u64,
    pub total: u64,
    pub fixed: Vec<Row>,
    pub rows: Vec<Row>,
}

impl Plan {
    pub fn demoted(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| !r.user_set && matches!(r.tier, Tier::NameOnly | Tier::Disabled))
            .map(|r| r.name.clone())
            .collect()
    }

    pub fn tier_of(&self, name: &str) -> Option<Tier> {
        self.rows.iter().find(|r| r.name == name).map(|r| r.tier)
    }
}

/// Claude Code lists skills in 1% of the context window, Codex in 2%.
pub fn budget_chars(host: Host, explicit: Option<u64>, context_tokens: u64) -> u64 {
    explicit.unwrap_or_else(|| {
        let percent = match host {
            Host::Claude => 1,
            Host::Codex => 2,
        };
        context_tokens * CHARS_PER_TOKEN * percent / 100
    })
}

struct Group {
    name: String,
    cost: u64,
    name_cost: u64,
    rating: Option<f64>,
    uses: u64,
}

fn total_use(catalog: &Catalog, skill: &Skill) -> u64 {
    let (claude, codex) = catalog.uses(skill);
    claude + codex
}

fn rank(a: &Group, b: &Group) -> Ordering {
    (b.name == PINNED)
        .cmp(&(a.name == PINNED))
        .then_with(|| b.rating.is_some().cmp(&a.rating.is_some()))
        .then_with(|| b.rating.unwrap_or(0.0).total_cmp(&a.rating.unwrap_or(0.0)))
        .then_with(|| b.uses.cmp(&a.uses))
        .then_with(|| a.name.cmp(&b.name))
}

/// Names of Codex system and plugin skills: a `name` selector cannot switch off
/// a user skill with one of these names without switching them off too.
fn names_switched_by_name(catalog: &Catalog, host: Host) -> BTreeSet<&str> {
    if host != Host::Codex {
        return BTreeSet::new();
    }
    catalog
        .skills
        .iter()
        .filter(|s| s.host == host && !s.controllable)
        .map(|s| s.name.as_str())
        .collect()
}

/// Non-controllable entries are charged first. On Claude a demoted skill keeps
/// its name in the listing, so every controllable name is reserved before any
/// description is admitted. Controllable skills then stay on, best first, until
/// the next one would not fit; `user` holds tiers the user set by hand, which
/// the plan charges but never changes. On Codex a skill is switched off by
/// name, so a user skill sharing its name with a system or plugin skill stays
/// on: switching it off would switch that one off too.
pub fn plan(catalog: &Catalog, host: Host, budget: u64, user: &BTreeMap<String, Tier>) -> Plan {
    let mut fixed = Vec::new();
    let mut running = 0;
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    let shared = names_switched_by_name(catalog, host);
    for s in catalog.skills.iter().filter(|s| s.host == host) {
        if !s.controllable {
            running += s.cost;
            fixed.push(Row {
                name: s.display_name(),
                rating: catalog.rating_of(s),
                uses: total_use(catalog, s),
                cost: s.cost,
                tier: match s.source {
                    Source::System => Tier::System,
                    Source::Command => Tier::Command,
                    Source::Plugin | Source::User => Tier::Plugin,
                },
                running,
                user_set: false,
            });
            continue;
        }
        let g = groups.entry(s.name.clone()).or_insert_with(|| Group {
            name: s.name.clone(),
            cost: 0,
            name_cost: 0,
            rating: None,
            uses: 0,
        });
        g.cost += s.cost;
        if host == Host::Claude {
            g.name_cost += s.display_name().chars().count() as u64;
        }
        g.rating = match (g.rating, catalog.rating_of(s)) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        g.uses = g.uses.max(total_use(catalog, s));
    }
    let fixed_cost = running;
    let mut groups: Vec<Group> = groups.into_values().collect();
    groups.sort_by(rank);

    let listed = |g: &Group| {
        user.get(&g.name)
            .is_none_or(|t| matches!(t, Tier::On | Tier::NameOnly))
    };
    let reserved: u64 = groups
        .iter()
        .filter(|g| listed(g))
        .map(|g| g.name_cost)
        .sum();
    running += reserved;
    running += groups
        .iter()
        .filter(|g| user.get(&g.name) == Some(&Tier::On))
        .map(|g| g.cost - g.name_cost)
        .sum::<u64>();

    let demoted = if host == Host::Claude {
        Tier::NameOnly
    } else {
        Tier::Disabled
    };
    let mut open = true;
    let mut rows = Vec::new();
    for g in groups {
        let extra = g.cost - g.name_cost;
        let (tier, user_set) = if let Some(&tier) = user.get(&g.name) {
            (tier, true)
        } else if g.name == PINNED
            || shared.contains(g.name.as_str())
            || (open && running + extra <= budget)
        {
            running += extra;
            (Tier::On, false)
        } else {
            open = false;
            (demoted, false)
        };
        rows.push(Row {
            name: g.name,
            rating: g.rating,
            uses: g.uses,
            cost: g.cost,
            tier,
            running,
            user_set,
        });
    }
    Plan {
        host,
        budget,
        fixed_cost,
        reserved,
        total: running,
        fixed,
        rows,
    }
}

pub fn render_plan(plan: &Plan) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} listing budget: {} chars",
        plan.host.as_str(),
        fmt::thousands(plan.budget)
    );
    let _ = writeln!(
        out,
        "  plugin, system and command entries: {} chars in {} entries",
        fmt::thousands(plan.fixed_cost),
        plan.fixed.len()
    );
    if plan.host == Host::Claude {
        let _ = writeln!(
            out,
            "  not counted: Claude Code's bundled skills and the current project's .claude/skills and commands"
        );
    }
    if plan.reserved > 0 {
        let _ = writeln!(
            out,
            "  names of controllable skills: {} chars (a name-only skill still lists its name)",
            fmt::thousands(plan.reserved)
        );
    }
    let tiers = [
        Tier::On,
        Tier::NameOnly,
        Tier::Disabled,
        Tier::UserOnly,
        Tier::Plugin,
        Tier::System,
        Tier::Command,
    ];
    for tier in tiers {
        let rows: Vec<&Row> = plan
            .rows
            .iter()
            .chain(&plan.fixed)
            .filter(|r| r.tier == tier)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n{} ({})", tier.as_str(), rows.len());
        let _ = writeln!(
            out,
            "  {:>6}  {:>6}  {:>8}  name",
            "rating", "cost", "running"
        );
        for r in rows {
            let _ = writeln!(
                out,
                "  {:>6}  {:>6}  {:>8}  {}{}",
                fmt::rating(r.rating),
                fmt::thousands(r.cost),
                fmt::thousands(r.running),
                r.name,
                if r.user_set {
                    "  (set by you, kept)"
                } else {
                    ""
                }
            );
        }
    }
    let _ = writeln!(
        out,
        "\ntotal {} of {} chars",
        fmt::thousands(plan.total),
        fmt::thousands(plan.budget)
    );
    if plan.total > plan.budget {
        let _ = writeln!(
            out,
            "over budget: plugin and system skills, reserved names and skill-rank already exceed it"
        );
    }
    out
}

/// `skillOverrides` entries the user set: every entry except a `name-only`
/// this tool wrote.
pub fn claude_user_overrides(settings: &Value, managed: &[String]) -> BTreeMap<String, Tier> {
    let mut out = BTreeMap::new();
    let Some(map) = settings.get("skillOverrides").and_then(Value::as_object) else {
        return out;
    };
    for (name, value) in map {
        let value = value.as_str().unwrap_or("");
        let ours = value == "name-only" && managed.iter().any(|m| m == name);
        if !ours {
            out.insert(name.clone(), Tier::from_override(value));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClaudeMerge {
    pub settings: Value,
    pub managed: Vec<String>,
    pub changed: bool,
}

/// Writes `name-only` for each demoted skill and removes this tool's entry for
/// each promoted one. User entries and every other key keep their value and order.
pub fn merge_claude(
    mut settings: Value,
    plan: &Plan,
    managed_before: &[String],
) -> Result<ClaudeMerge, String> {
    let Some(root) = settings.as_object_mut() else {
        return Err("settings.json is not a JSON object".to_owned());
    };
    let had_key = root.contains_key("skillOverrides");
    let before = root
        .get("skillOverrides")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut overrides = before.clone();
    let mut managed: BTreeSet<String> = managed_before.iter().cloned().collect();
    for row in &plan.rows {
        if row.user_set {
            managed.remove(&row.name);
            continue;
        }
        if row.tier == Tier::NameOnly {
            overrides.insert(row.name.clone(), json!("name-only"));
            managed.insert(row.name.clone());
        } else if managed.remove(&row.name) {
            overrides.shift_remove(&row.name);
        }
    }
    let names: BTreeSet<&str> = plan.rows.iter().map(|r| r.name.as_str()).collect();
    let gone: Vec<String> = managed
        .iter()
        .filter(|m| !names.contains(m.as_str()))
        .cloned()
        .collect();
    for name in gone {
        managed.remove(&name);
        if overrides.get(&name).and_then(Value::as_str) == Some("name-only") {
            overrides.shift_remove(&name);
        }
    }
    let changed = overrides != before;
    if changed && (had_key || !overrides.is_empty()) {
        root.insert("skillOverrides".to_owned(), Value::Object(overrides));
    }
    Ok(ClaudeMerge {
        settings,
        managed: managed.into_iter().collect(),
        changed,
    })
}

pub fn settings_text(settings: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(settings)
        .map(|t| t + "\n")
        .map_err(|e| e.to_string())
}

/// `skills.config` set outside the managed block, in any TOML form: a
/// `[[skills.config]]` header (named by line), or an inline or dotted `config`
/// key under `skills`, which only a parse finds.
pub fn foreign_skill_config(config: &str) -> Vec<String> {
    let lines: Vec<&str> = config.lines().collect();
    let block = block_range(&lines);
    let mut found: Vec<String> = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            !block.is_some_and(|(b, e)| (b..=e).contains(i)) && SKILLS_CONFIG.is_match(l)
        })
        .map(|(i, l)| format!("  line {}: {}", i + 1, l.trim()))
        .collect();
    if found.is_empty() {
        let outside: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| !block.is_some_and(|(b, e)| (b..=e).contains(i)))
            .map(|(_, l)| *l)
            .collect();
        let has_config = outside
            .join("\n")
            .parse::<toml::Table>()
            .ok()
            .and_then(|t| t.get("skills").and_then(toml::Value::as_table).cloned())
            .is_some_and(|skills| skills.contains_key("config"));
        if has_config {
            found.push("  skills.config is set as an inline array or dotted key".to_owned());
        }
    }
    found
}

/// `[skills] max_context_tokens` from a Codex `config.toml`, capped as Codex caps it.
pub fn codex_max_context_tokens(config: &str) -> Option<u64> {
    let table = config.parse::<toml::Table>().ok()?;
    let tokens = table
        .get("skills")?
        .as_table()?
        .get("max_context_tokens")?
        .as_integer()?;
    u64::try_from(tokens)
        .ok()
        .filter(|t| *t > 0)
        .map(|t| t.min(CODEX_MAX_CONTEXT_TOKENS))
}

fn block_range(lines: &[&str]) -> Option<(usize, usize)> {
    let b = lines.iter().position(|l| l.trim_end() == BEGIN)?;
    let e = lines[b..].iter().position(|l| l.trim_end() == END)?;
    Some((b, b + e))
}

fn toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn render_block(disabled: &[String]) -> Vec<String> {
    if disabled.is_empty() {
        return Vec::new();
    }
    let mut out = vec![BEGIN.to_owned()];
    for (i, name) in disabled.iter().enumerate() {
        if i > 0 {
            out.push(String::new());
        }
        out.push("[[skills.config]]".to_owned());
        out.push(format!("name = {}", toml_string(name)));
        out.push("enabled = false".to_owned());
    }
    out.push(END.to_owned());
    out
}

/// Replaces the managed block (or appends one) listing `disabled`. Refuses when
/// the file does not parse, when `skills.config` is set outside the block, when
/// the block holds a line this tool would not write, or when the result would
/// not parse.
pub fn merge_codex(config: &str, disabled: &[String]) -> Result<String, String> {
    if let Err(e) = config.parse::<toml::Table>() {
        return Err(format!(
            "refusing to write config.toml: it does not parse as TOML, and Codex cannot load it either:\n{e}"
        ));
    }
    let lines: Vec<&str> = config.lines().collect();
    let has_begin = lines.iter().any(|l| l.trim_end() == BEGIN);
    let range = block_range(&lines);
    if has_begin && range.is_none() {
        return Err(format!(
            "config.toml has the line \"{BEGIN}\" but no \"{END}\"; fix it by hand"
        ));
    }
    let foreign = foreign_skill_config(config);
    if !foreign.is_empty() {
        return Err(format!(
            "refusing to write config.toml: it sets skills.config outside the skill-rank block:\n{}\n\
             delete those entries or move them between the skill-rank lines, then run apply again",
            foreign.join("\n")
        ));
    }
    if let Some((b, e)) = range {
        let alien: Vec<String> = lines[b + 1..e]
            .iter()
            .enumerate()
            .filter(|(_, l)| !BLOCK_LINE.is_match(l))
            .map(|(i, l)| format!("  line {}: {}", b + 2 + i, l.trim()))
            .collect();
        if !alien.is_empty() {
            return Err(format!(
                "refusing to write config.toml: the skill-rank block holds lines skill-rank did not write, \
                 which the next apply would delete:\n{}\nmove them outside the block, then run apply again",
                alien.join("\n")
            ));
        }
    }
    let block = render_block(disabled);
    let mut out: Vec<String> = Vec::new();
    if let Some((b, e)) = range {
        out.extend(lines[..b].iter().map(|l| (*l).to_owned()));
        out.extend(block);
        out.extend(lines[e + 1..].iter().map(|l| (*l).to_owned()));
    } else {
        out.extend(lines.iter().map(|l| (*l).to_owned()));
        if !block.is_empty() {
            if out.last().is_some_and(|l| !l.trim().is_empty()) {
                out.push(String::new());
            }
            out.extend(block);
        }
    }
    let mut text = out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    let parsed = text.parse::<toml::Table>().map_err(|e| {
        format!("refusing to write config.toml: the edited file would not parse, so nothing was written:\n{e}")
    })?;
    let entries = parsed
        .get("skills")
        .and_then(toml::Value::as_table)
        .and_then(|s| s.get("config"))
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let stray = entries.iter().any(|entry| {
        entry
            .as_table()
            .is_none_or(|t| t.keys().any(|k| k != "name" && k != "enabled"))
    });
    if stray {
        return Err(
            "refusing to write config.toml: a key after the skill-rank block would join its last \
             [[skills.config]] entry, which Codex rejects; give the lines after the block a table \
             header, or move the block to the end of the file"
                .to_owned(),
        );
    }
    Ok(text)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
