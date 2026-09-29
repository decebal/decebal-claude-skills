use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

/// Always kept on: it is how every demoted skill stays findable.
pub const PINNED: &str = "skill-rank";

/// Claude Code caps each listed description at this many characters.
pub const CLAUDE_DESCRIPTION_CAP: usize = 1536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Host {
    Claude,
    Codex,
}

impl Host {
    pub const ALL: [Host; 2] = [Host::Claude, Host::Codex];

    pub fn as_str(self) -> &'static str {
        match self {
            Host::Claude => "claude",
            Host::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Host> {
        match s {
            "claude" => Some(Host::Claude),
            "codex" => Some(Host::Codex),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    User,
    Plugin,
    System,
    /// A Claude Code command file (`commands/*.md`), listed beside skills.
    Command,
}

impl Source {
    pub const ALL: [Source; 4] = [
        Source::User,
        Source::Plugin,
        Source::System,
        Source::Command,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Source::User => "user",
            Source::Plugin => "plugin",
            Source::System => "system",
            Source::Command => "command",
        }
    }

    pub fn parse(s: &str) -> Option<Source> {
        match s {
            "user" => Some(Source::User),
            "plugin" => Some(Source::Plugin),
            "system" => Some(Source::System),
            "command" => Some(Source::Command),
            _ => None,
        }
    }

    pub fn default_tier(self) -> &'static str {
        match self {
            Source::User => "on",
            Source::Plugin => "plugin",
            Source::System => "system",
            Source::Command => "command",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub dir_name: String,
    pub path: String,
    pub host: Host,
    pub source: Source,
    pub plugin: Option<String>,
    pub controllable: bool,
    pub cost: u64,
    pub key: String,
    pub tier: String,
}

impl Skill {
    /// The name the host lists: plugin skills are namespaced `plugin:name`.
    pub fn display_name(&self) -> String {
        match &self.plugin {
            Some(p) => format!("{p}:{}", self.name),
            None => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Use {
    pub count: u64,
    pub last_seen: Option<String>,
}

impl Use {
    pub fn bump(&mut self, timestamp: Option<&str>) {
        self.count += 1;
        if let Some(ts) = timestamp {
            if self.last_seen.as_deref().is_none_or(|seen| ts > seen) {
                self.last_seen = Some(ts.to_owned());
            }
        }
    }
}

pub type UsageMap = BTreeMap<String, Use>;

#[derive(Debug, Clone, PartialEq)]
pub struct Rating {
    pub rating: f64,
    pub confidence: Option<f64>,
    pub probabilities: Value,
    pub model: String,
    pub profile_sha: String,
    pub rated_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalog {
    pub scanned_at: u64,
    pub days: u64,
    pub skills: Vec<Skill>,
    pub usage: BTreeMap<Host, UsageMap>,
    pub ratings: BTreeMap<String, Rating>,
    pub managed: BTreeMap<Host, Vec<String>>,
    /// The sha of the current profile.md. Not saved: set after loading, so a
    /// rating made against another profile reads as missing.
    pub current_profile: Option<String>,
    /// Set when the usage scan hit its time or byte limit.
    pub usage_truncated: bool,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

pub fn rating_key(name: &str, description: &str) -> String {
    sha256_hex(format!("{name}\n{description}").as_bytes())
}

/// Listing cost in characters, as the host would render the entry.
pub fn listing_cost(host: Host, display_name: &str, description: &str) -> u64 {
    let desc = description.chars().count();
    let desc = match host {
        Host::Claude => desc.min(CLAUDE_DESCRIPTION_CAP),
        Host::Codex => desc,
    };
    (display_name.chars().count() + desc) as u64
}

impl Catalog {
    pub fn load(path: &Path) -> Result<Option<Catalog>, String> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        };
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| format!("{} does not parse: {e}", path.display()))?;
        Ok(Some(Catalog::from_value(&value)))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        let mut text = serde_json::to_string_pretty(&self.to_value()).map_err(|e| e.to_string())?;
        text.push('\n');
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        fs::rename(&tmp, path).map_err(|e| format!("write {}: {e}", path.display()))
    }

    /// Recent use of a skill on each host: `(claude, codex)`. Claude counts are
    /// keyed by the invoked name, so a plugin skill is looked up as `plugin:name`.
    pub fn uses(&self, skill: &Skill) -> (u64, u64) {
        let count = |host: Host, first: &str, second: &str| {
            self.usage.get(&host).map_or(0, |m| {
                m.get(first)
                    .or_else(|| m.get(second))
                    .map_or(0, |u| u.count)
            })
        };
        let claude = match &skill.plugin {
            Some(_) => {
                let display = skill.display_name();
                count(Host::Claude, &display, &display)
            }
            None => count(Host::Claude, &skill.name, &skill.dir_name),
        };
        (claude, count(Host::Codex, &skill.dir_name, &skill.name))
    }

    pub fn rating_of(&self, skill: &Skill) -> Option<f64> {
        self.ratings
            .get(&skill.key)
            .filter(|r| {
                self.current_profile
                    .as_deref()
                    .is_none_or(|sha| r.profile_sha == sha)
            })
            .map(|r| r.rating)
    }

    /// Ratings kept for the current skills but made against another profile.md.
    pub fn stale_ratings(&self) -> usize {
        let Some(sha) = self.current_profile.as_deref() else {
            return 0;
        };
        let keys: std::collections::BTreeSet<&str> =
            self.skills.iter().map(|s| s.key.as_str()).collect();
        self.ratings
            .iter()
            .filter(|(k, r)| keys.contains(k.as_str()) && r.profile_sha != sha)
            .count()
    }

    pub fn managed_for(&self, host: Host) -> &[String] {
        self.managed.get(&host).map_or(&[], Vec::as_slice)
    }

    pub fn to_value(&self) -> Value {
        let skills: Vec<Value> = self.skills.iter().map(skill_to_value).collect();
        let usage: Map<String, Value> = self
            .usage
            .iter()
            .map(|(host, m)| {
                let entries: Map<String, Value> = m
                    .iter()
                    .map(|(name, u)| {
                        (
                            name.clone(),
                            json!({"count": u.count, "last_seen": u.last_seen}),
                        )
                    })
                    .collect();
                (host.as_str().to_owned(), Value::Object(entries))
            })
            .collect();
        let ratings: Map<String, Value> = self
            .ratings
            .iter()
            .map(|(key, r)| {
                (
                    key.clone(),
                    json!({
                        "rating": r.rating,
                        "confidence": r.confidence,
                        "probabilities": r.probabilities,
                        "model": r.model,
                        "profile_sha": r.profile_sha,
                        "rated_at": r.rated_at,
                    }),
                )
            })
            .collect();
        let managed: Map<String, Value> = self
            .managed
            .iter()
            .map(|(host, names)| (host.as_str().to_owned(), json!(names)))
            .collect();
        json!({
            "version": 1,
            "scanned_at": self.scanned_at,
            "days": self.days,
            "skills": skills,
            "usage": usage,
            "ratings": ratings,
            "managed": managed,
        })
    }

    pub fn from_value(v: &Value) -> Catalog {
        let skills = v
            .get("skills")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(skill_from_value).collect())
            .unwrap_or_default();
        let mut usage = BTreeMap::new();
        if let Some(obj) = v.get("usage").and_then(Value::as_object) {
            for (host, entries) in obj {
                let Some(host) = Host::parse(host) else {
                    continue;
                };
                let mut m = UsageMap::new();
                for (name, u) in entries.as_object().into_iter().flatten() {
                    m.insert(
                        name.clone(),
                        Use {
                            count: u.get("count").and_then(Value::as_u64).unwrap_or(0),
                            last_seen: u
                                .get("last_seen")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        },
                    );
                }
                usage.insert(host, m);
            }
        }
        let mut ratings = BTreeMap::new();
        for (key, r) in v
            .get("ratings")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let Some(rating) = r.get("rating").and_then(Value::as_f64) else {
                continue;
            };
            ratings.insert(
                key.clone(),
                Rating {
                    rating,
                    confidence: r.get("confidence").and_then(Value::as_f64),
                    probabilities: r.get("probabilities").cloned().unwrap_or(Value::Null),
                    model: str_field(r, "model"),
                    profile_sha: str_field(r, "profile_sha"),
                    rated_at: r.get("rated_at").and_then(Value::as_u64).unwrap_or(0),
                },
            );
        }
        let mut managed = BTreeMap::new();
        for (host, names) in v
            .get("managed")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            let Some(host) = Host::parse(host) else {
                continue;
            };
            let names = names
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            managed.insert(host, names);
        }
        Catalog {
            scanned_at: v.get("scanned_at").and_then(Value::as_u64).unwrap_or(0),
            days: v.get("days").and_then(Value::as_u64).unwrap_or(0),
            skills,
            usage,
            ratings,
            managed,
            current_profile: None,
            usage_truncated: false,
        }
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

fn skill_to_value(s: &Skill) -> Value {
    json!({
        "name": s.name,
        "description": s.description,
        "dir_name": s.dir_name,
        "path": s.path,
        "host": s.host.as_str(),
        "source": s.source.as_str(),
        "plugin": s.plugin,
        "controllable": s.controllable,
        "cost": s.cost,
        "key": s.key,
        "tier": s.tier,
    })
}

fn skill_from_value(v: &Value) -> Option<Skill> {
    let source = Source::parse(v.get("source")?.as_str()?)?;
    Some(Skill {
        name: str_field(v, "name"),
        description: str_field(v, "description"),
        dir_name: str_field(v, "dir_name"),
        path: str_field(v, "path"),
        host: Host::parse(v.get("host")?.as_str()?)?,
        source,
        plugin: v.get("plugin").and_then(Value::as_str).map(str::to_owned),
        controllable: v
            .get("controllable")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        cost: v.get("cost").and_then(Value::as_u64).unwrap_or(0),
        key: str_field(v, "key"),
        tier: v
            .get("tier")
            .and_then(Value::as_str)
            .unwrap_or(source.default_tier())
            .to_owned(),
    })
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
