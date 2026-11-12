//! `skill-audit join <listing.tsv> <usage.tsv> <repo> <home>`: one row per
//! listing entry with its set, cost, usage, referrers and protection reason.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::walk;

const BUILTIN_AGENTS: &[&str] = &[
    "claude",
    "claude-code-guide",
    "Explore",
    "general-purpose",
    "Plan",
    "statusline-setup",
    "web-fetch",
];

/// Slash commands appear both in transcripts and in history.jsonl, so the two
/// are combined with `max`, never summed.
#[derive(Default)]
struct Usage {
    tool: (u64, u64),
    slash: (u64, u64),
    hist: (u64, u64),
    last: String,
}

impl Usage {
    fn since(&self) -> u64 {
        self.tool.0 + self.slash.0.max(self.hist.0)
    }
    fn all(&self) -> u64 {
        self.tool.1 + self.slash.1.max(self.hist.1)
    }
}

fn read_tsv(path: &Path) -> Vec<Vec<String>> {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .skip(1)
        .map(|l| l.split('\t').map(str::to_owned).collect())
        .collect()
}

fn norm_server(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == ':' || c == '.' || c == ' ' {
                '_'
            } else {
                c
            }
        })
        .collect()
}

fn md_files(root: &Path, only_skill_md: bool) -> Vec<PathBuf> {
    walk::files(root, 8, &|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy();
        if only_skill_md {
            n == "SKILL.md"
        } else {
            n.ends_with(".md")
        }
    })
}

/// Mention forms that name a skill rather than use the word in prose.
fn mentions(text: &str, name: &str) -> bool {
    let bare = name.rsplit(':').next().unwrap_or(name);
    [
        format!("`{bare}`"),
        format!("/{bare} "),
        format!("/{bare}`"),
        format!("/{bare})"),
        format!("{bare} skill"),
        format!("skills/{bare}"),
        format!("Skill({bare}"),
        format!("\"{bare}\""),
        format!("`/{bare}"),
    ]
    .iter()
    .any(|p| text.contains(p.as_str()))
}

fn set_of(kind: &str, name: &str, repo: &Path, home: &Path) -> String {
    match kind {
        "agent" => match name.split_once(':') {
            Some((p, _)) => format!("plugin:{p}"),
            None if BUILTIN_AGENTS.contains(&name) => "builtin".into(),
            None if home
                .join(".claude/agents")
                .join(format!("{name}.md"))
                .exists() =>
            {
                "personal".into()
            }
            None => "other".into(),
        },
        "mcp" => match name.strip_prefix("plugin:") {
            Some(rest) => format!("plugin:{}", rest.split(':').next().unwrap_or(rest)),
            None => "mcp-config".into(),
        },
        _ => match name.split_once(':') {
            Some((p, _)) => format!("plugin:{p}"),
            None if repo.join(".claude/skills").join(name).exists() => "repo".into(),
            None if repo
                .join(".claude/commands")
                .join(format!("{name}.md"))
                .exists() =>
            {
                "repo-command".into()
            }
            None if home.join(".claude/skills").join(name).exists() => "personal".into(),
            None if home
                .join(".claude/commands")
                .join(format!("{name}.md"))
                .exists() =>
            {
                "personal-command".into()
            }
            None => "builtin".into(),
        },
    }
}

fn source_path(set: &str, name: &str, repo: &Path, home: &Path) -> Option<PathBuf> {
    match set {
        "repo" => Some(repo.join(".claude/skills").join(name)),
        "repo-command" => Some(repo.join(".claude/commands").join(format!("{name}.md"))),
        "personal" => Some(home.join(".claude/skills").join(name)),
        "personal-command" => Some(home.join(".claude/commands").join(format!("{name}.md"))),
        _ => None,
    }
}

fn newest_mtime(path: &Path) -> Option<SystemTime> {
    let own = fs::metadata(path).ok()?.modified().ok();
    walk::files(path, 3, &|_| true)
        .iter()
        .filter_map(|p| fs::metadata(p).ok()?.modified().ok())
        .chain(own)
        .max()
}

pub fn run(listing: &Path, usage: &Path, repo: &Path, home: &Path) {
    let mut use_map: HashMap<(String, String), Usage> = HashMap::new();
    for r in read_tsv(usage) {
        if r.len() < 6 || r[0] == "kind" {
            continue;
        }
        let (kind, name) = (r[0].as_str(), r[1].clone());
        let key = match kind {
            "slash" | "history" => ("skill".to_owned(), name),
            k => (k.to_owned(), name),
        };
        let u = use_map.entry(key).or_default();
        let counts = (
            r[2].parse::<u64>().unwrap_or(0),
            r[3].parse::<u64>().unwrap_or(0),
        );
        let slot = match kind {
            "slash" => &mut u.slash,
            "history" => &mut u.hist,
            _ => &mut u.tool,
        };
        slot.0 += counts.0;
        slot.1 += counts.1;
        if r[5] > u.last {
            u.last = r[5].clone();
        }
    }

    let mut protect_docs: Vec<(String, String)> = Vec::new();
    let mut add = |p: &Path, label: String| {
        if let Ok(t) = fs::read_to_string(p) {
            protect_docs.push((label, t));
        }
    };
    add(
        &home.join(".claude/CLAUDE.md"),
        "~/.claude/CLAUDE.md".into(),
    );
    add(&repo.join("CLAUDE.md"), "repo CLAUDE.md".into());
    for dir in [home.join(".claude/rules"), repo.join(".claude/rules")] {
        for p in md_files(&dir, false) {
            add(
                &p,
                format!(
                    "rule {}",
                    p.file_name().unwrap_or_default().to_string_lossy()
                ),
            );
        }
    }
    for p in fs::read_dir(home.join(".claude/projects"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path().join("memory/MEMORY.md"))
        .filter(|p| p.exists())
    {
        let proj = p
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        add(
            &p,
            format!("MEMORY.md ({})", proj.rsplit('-').next().unwrap_or(&proj)),
        );
    }

    let mut ref_docs: Vec<(String, String)> = Vec::new();
    let mut ref_roots: Vec<(PathBuf, bool)> = vec![
        (repo.join(".claude/skills"), true),
        (home.join(".claude/skills"), true),
        (home.join(".claude/plugins/cache"), true),
        (repo.join(".claude/commands"), false),
        (home.join(".claude/commands"), false),
    ];
    ref_roots.push((home.join(".claude/hooks"), false));
    for (root, skill_only) in ref_roots {
        for p in md_files(&root, skill_only) {
            if let Ok(t) = fs::read_to_string(&p) {
                let owner = if skill_only {
                    p.parent()
                        .and_then(Path::file_name)
                        .map(|s| s.to_string_lossy().into_owned())
                } else {
                    p.file_stem().map(|s| s.to_string_lossy().into_owned())
                };
                ref_docs.push((owner.unwrap_or_default(), t));
            }
        }
    }
    for hook_cfg in [
        home.join(".claude/settings.json"),
        repo.join(".claude/settings.json"),
    ] {
        if let Ok(t) = fs::read_to_string(&hook_cfg) {
            ref_docs.push(("settings.json".into(), t));
        }
    }

    let cutoff = SystemTime::now() - Duration::from_secs(30 * 86_400);
    let mut plugin_roll: BTreeMap<String, (usize, usize, u64, u64, String)> = BTreeMap::new();
    println!(
        "kind\tname\tset\tbytes\tapprox_tokens\tuses_60d\tuses_all\tlast_used\tused_by\tprotected"
    );
    for r in read_tsv(listing) {
        if r.len() < 4 {
            continue;
        }
        let (kind, name, bytes) = (
            r[0].as_str(),
            r[1].as_str(),
            r[2].parse::<usize>().unwrap_or(0),
        );
        let set = set_of(kind, name, repo, home);
        let ukey = match kind {
            "mcp" => ("mcp".to_owned(), norm_server(name)),
            k => (k.to_owned(), name.to_owned()),
        };
        let u = use_map.get(&ukey);
        let (since, all, last) = u.map_or((0, 0, String::new()), |u| {
            (u.since(), u.all(), u.last.clone())
        });
        let bare = name.rsplit(':').next().unwrap_or(name);
        let mut refs: Vec<String> = ref_docs
            .iter()
            .filter(|(owner, t)| owner != bare && mentions(t, name))
            .map(|(o, _)| o.clone())
            .collect();
        refs.sort();
        refs.dedup();
        let used_by = if refs.is_empty() {
            "-".into()
        } else {
            let more = if refs.len() > 4 {
                format!(" +{}", refs.len() - 4)
            } else {
                String::new()
            };
            format!(
                "{}{more}",
                refs.iter().take(4).cloned().collect::<Vec<_>>().join(",")
            )
        };
        let mut why: Vec<String> = protect_docs
            .iter()
            .filter(|(_, t)| mentions(t, name))
            .map(|(l, _)| l.clone())
            .collect();
        why.dedup();
        if let Some(src) = source_path(&set, name, repo, home) {
            if newest_mtime(&src).is_some_and(|m| m > cutoff) {
                why.push("modified <30d".into());
            }
        }
        let protected = if why.is_empty() {
            "-".into()
        } else {
            why.into_iter().take(3).collect::<Vec<_>>().join("; ")
        };
        println!(
            "{kind}\t{name}\t{set}\t{bytes}\t{}\t{since}\t{all}\t{}\t{used_by}\t{protected}",
            bytes.div_ceil(4),
            if last.is_empty() { "-" } else { &last }
        );
        if let Some(p) = set.strip_prefix("plugin:") {
            let e = plugin_roll
                .entry(p.to_owned())
                .or_insert((0, 0, 0, 0, String::new()));
            e.0 += bytes;
            e.1 += 1;
            e.2 += since;
            e.3 += all;
            if last > e.4 {
                e.4 = last;
            }
        }
    }
    for (p, (bytes, n, since, all, last)) in &plugin_roll {
        let mcp = use_map
            .iter()
            .filter(|((k, s), _)| k == "mcp" && s.starts_with(&format!("plugin_{p}_")))
            .fold((0, 0), |a, (_, u)| (a.0 + u.since(), a.1 + u.all()));
        eprintln!("plugin\t{p}\tentries {n}\tbytes {bytes}\ttokens~{}\tuses_60d {since}\tuses_all {all}\tplugin_mcp_60d {}\tplugin_mcp_all {}\tlast {}",
            bytes.div_ceil(4), mcp.0, mcp.1, if last.is_empty() { "-" } else { last });
    }
}
