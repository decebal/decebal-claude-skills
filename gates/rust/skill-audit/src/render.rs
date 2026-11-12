//! `skill-audit render <joined.tsv> <template.html> <out.html> [--disable p1,p2] [--keep p3,p4]`:
//! replaces `<!--ROWS-->` in the template with one table row per joined entry
//! and a rule-based proposal. Plugin rows take the plugin-level decision the
//! caller passes; the tool holds no opinion about any particular plugin.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Default, PartialEq)]
pub struct Plugins {
    pub disable: Vec<String>,
    pub keep: Vec<String>,
}

impl Plugins {
    /// `None` on an unknown flag or a flag with no value.
    pub fn from_args(args: &[String]) -> Option<Self> {
        let mut out = Self::default();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let list: Vec<String> = it
                .next()?
                .split(',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            match flag.as_str() {
                "--disable" => out.disable.extend(list),
                "--keep" => out.keep.extend(list),
                _ => return None,
            }
        }
        Some(out)
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn proposal(
    kind: &str,
    set: &str,
    bytes: usize,
    since: u64,
    all: u64,
    protected: bool,
    plugins: &Plugins,
) -> String {
    if let Some(p) = set.strip_prefix("plugin:") {
        return if plugins.disable.iter().any(|d| d == p) {
            format!("disable plugin {p}")
        } else if plugins.keep.iter().any(|k| k == p) {
            "keep (plugin in use)".into()
        } else {
            "plugin: no decision passed".into()
        };
    }
    match (kind, set) {
        (_, "builtin") => return "built-in; not removable here".into(),
        ("mcp", _) if since == 0 && all == 0 => return "disconnect server / connector".into(),
        ("mcp", _) => return "keep".into(),
        _ => {}
    }
    if since > 0 {
        return "keep".into();
    }
    if protected {
        return "keep (protected)".into();
    }
    if all > 0 {
        return "keep (seasonal: used before window)".into();
    }
    if kind == "agent" {
        return "archive agent file".into();
    }
    if bytes > 60 {
        "disable-model-invocation or archive".into()
    } else {
        "archive (name-only entry)".into()
    }
}

pub fn run(joined: &Path, template: &Path, out: &Path, plugins: &Plugins) -> io::Result<()> {
    let tsv = fs::read_to_string(joined)?;
    let mut rows = String::new();
    let mut totals: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    for line in tsv.lines().skip(1) {
        let c: Vec<&str> = line.split('\t').collect();
        if c.len() < 10 {
            continue;
        }
        let bytes: usize = c[3].parse().unwrap_or(0);
        let since: u64 = c[5].parse().unwrap_or(0);
        let all: u64 = c[6].parse().unwrap_or(0);
        let t = totals
            .entry((c[0].to_owned(), c[2].to_owned()))
            .or_insert((0, 0));
        t.0 += 1;
        t.1 += bytes;
        let prop = proposal(c[0], c[2], bytes, since, all, c[9] != "-", plugins);
        let keep = prop.starts_with("keep")
            || prop.starts_with("built-in")
            || prop.starts_with("plugin: no");
        let cls = if keep { "" } else { " class=\"cut\"" };
        rows.push_str(&format!(
            "<tr{cls}><td>{}</td><td><code>{}</code></td><td>{}</td><td class=\"n\">{bytes}</td><td class=\"n\">{}</td><td class=\"n\">{since}</td><td class=\"n\">{all}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            c[0],
            esc(c[1]),
            esc(c[2]),
            c[4],
            c[7],
            esc(c[8]),
            esc(c[9]),
            esc(&prop)
        ));
    }
    for ((kind, set), (n, b)) in &totals {
        eprintln!(
            "total\t{kind}\t{set}\tentries {n}\tbytes {b}\ttokens~{}",
            b.div_ceil(4)
        );
    }
    let html = fs::read_to_string(template)?.replace("<!--ROWS-->", &rows);
    fs::write(out, html)
}

#[cfg(test)]
#[path = "render_tests.rs"]
mod render_tests;
