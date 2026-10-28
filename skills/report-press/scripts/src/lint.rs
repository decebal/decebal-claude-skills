//! `report-press check`: the story rules and the diagram limits that the
//! vendored Markdy 1.5.7 was measured against.

use std::collections::{HashMap, HashSet};

use crate::markup::TAGS;
use crate::story::{figures, Figure, Kind, Section, Story};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    pub level: Level,
    pub msg: String,
}

/// Node kinds rendered without a warning by the vendored bundle.
pub const VERIFIED_KINDS: [&str; 20] = [
    "start",
    "state",
    "decision",
    "end",
    "step",
    "user",
    "client",
    "service",
    "database",
    "security",
    "runtime",
    "repo",
    "policy",
    "container",
    "desktop",
    "branch",
    "configmap",
    "pipeline",
    "release",
    "hub",
];

/// Above this many ranks a left-to-right diagram no longer fits a 1,180 px page
/// at 0.75 scale, so it renders as a sideways-scrolling strip.
pub const MAX_RANKS: usize = 4;

const BANNED: [&str; 18] = [
    "delve",
    "tapestry",
    "landscape",
    "testament",
    "pivotal",
    "vibrant",
    "seamless",
    "robust",
    "leverage",
    "utilize",
    "facilitate",
    "crucial",
    "comprehensive",
    "moreover",
    "additionally",
    "furthermore",
    "notably",
    "importantly",
];

const TIME_UNITS: [&str; 8] = [
    "hour", "hours", "day", "days", "week", "weeks", "sprint", "sprints",
];

fn err(line: usize, msg: impl Into<String>) -> Finding {
    Finding {
        line,
        level: Level::Error,
        msg: msg.into(),
    }
}

fn warn(line: usize, msg: impl Into<String>) -> Finding {
    Finding {
        line,
        level: Level::Warn,
        msg: msg.into(),
    }
}

pub fn check(story: &Story) -> Vec<Finding> {
    let mut out = Vec::new();
    for section in &story.sections {
        let figs = figures(section);
        if matches!(section.kind, Kind::Lead | Kind::Story) && figs.is_empty() {
            out.push(err(
                section.line,
                "every lead and story section carries a ```markdy figure (state or C4 container)",
            ));
        }
        for fig in &figs {
            out.extend(check_figure(fig));
        }
        out.extend(check_prose(section));
        if section.kind == Kind::Story && !section.body.contains("{{") {
            out.push(warn(
                section.line,
                "no {{source label}} in this section: label each claim observed / inferred / not checked",
            ));
        }
    }
    match story.sections.last() {
        Some(last) if last.kind == Kind::NotChecked => {}
        _ => out.push(err(
            story.sections.last().map_or(1, |s| s.line),
            "the last section is `## What I have not checked {.notchecked}`",
        )),
    }
    let notchecked = story
        .sections
        .iter()
        .filter(|s| s.kind == Kind::NotChecked)
        .count();
    if notchecked > 1 {
        out.push(err(1, "only one {.notchecked} section"));
    }
    out.sort_by_key(|f| f.line);
    out
}

pub fn check_figure(fig: &Figure) -> Vec<Finding> {
    let mut out = Vec::new();
    let at = |i: usize| fig.line + 1 + i;
    if fig.caption.is_empty() {
        out.push(err(
            fig.line,
            "a figure needs a caption: ```markdy <caption>",
        ));
    }
    if fig.caption.contains('`') {
        out.push(err(
            fig.line,
            "a ``` fence's info string cannot hold a backtick, so the caption cannot use `code`",
        ));
    }
    if fig.code.contains("</") {
        out.push(err(
            fig.line,
            "`</` inside a figure would close the page's <script>",
        ));
    }
    let lines: Vec<&str> = fig.code.lines().collect();
    let Some((first_i, first)) = lines.iter().enumerate().find(|(_, l)| !l.trim().is_empty())
    else {
        out.push(err(fig.line, "empty figure"));
        return out;
    };
    if !first.trim_start().starts_with("scene") {
        out.push(err(at(first_i), "a figure starts with a `scene` line"));
        return out;
    }
    let ty = first
        .split_whitespace()
        .find_map(|t| t.strip_prefix("type="))
        .unwrap_or("architecture");
    if !matches!(ty, "state" | "architecture") {
        out.push(warn(
            at(first_i),
            format!("scene type `{ty}`: reports use `state` (state changes) or `architecture` (C4 containers)"),
        ));
    }

    let mut nodes: HashSet<&str> = HashSet::new();
    let mut groups: HashSet<&str> = HashSet::new();
    let mut edges: Vec<(&str, &str)> = Vec::new();
    let mut lr = true;
    for (i, line) in lines.iter().enumerate().skip(first_i + 1) {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let toks: Vec<&str> = line.split_whitespace().collect();
        let Some(&head) = toks.first() else { continue };
        if !indented {
            match head {
                "layout" => lr = toks.get(1) == Some(&"LR"),
                "beat" => {}
                "group" => {
                    if let Some(id) = toks.get(1) {
                        groups.insert(id);
                    }
                }
                kind => match toks.get(1) {
                    Some(id) => {
                        if !VERIFIED_KINDS.contains(&kind) {
                            out.push(warn(
                                at(i),
                                format!("node kind `{kind}` was not verified against Markdy 1.5.7; known good: {}", VERIFIED_KINDS.join(" ")),
                            ));
                        }
                        nodes.insert(id);
                    }
                    None => out.push(err(at(i), format!("`{kind}` needs an id"))),
                },
            }
        } else if let [a, op @ ("->" | "~>" | "<-"), b, ..] = toks.as_slice() {
            for id in [a, b] {
                if !nodes.contains(id) {
                    out.push(err(
                        at(i),
                        format!("edge names `{id}`, which is not a node"),
                    ));
                }
            }
            if *op != "<-" {
                edges.push((a, b));
            }
        } else if let [cue, target, ..] = toks.as_slice() {
            if matches!(*cue, "glow" | "focus" | "frame" | "show" | "hide")
                && !target.starts_with('$')
                && !nodes.contains(target)
                && !groups.contains(target)
            {
                out.push(err(
                    at(i),
                    format!("`{cue}` targets `{target}`, which is not a node or group"),
                ));
            }
        }
    }
    let ranks = longest_chain(&edges);
    if lr && ranks > MAX_RANKS {
        out.push(warn(
            fig.line,
            format!("{ranks} ranks left-to-right; above {MAX_RANKS} the figure scrolls sideways. Merge states or split the figure"),
        ));
    }
    out
}

/// Nodes on the longest forward path; a cycle is cut where it closes.
pub fn longest_chain(edges: &[(&str, &str)]) -> usize {
    let mut next: HashMap<&str, Vec<&str>> = HashMap::new();
    for (a, b) in edges {
        next.entry(a).or_default().push(b);
    }
    fn walk<'a>(
        n: &'a str,
        next: &HashMap<&'a str, Vec<&'a str>>,
        seen: &mut Vec<&'a str>,
        memo: &mut HashMap<&'a str, usize>,
    ) -> usize {
        if let Some(&d) = memo.get(n) {
            return d;
        }
        seen.push(n);
        let mut best = 0;
        for &m in next.get(n).map(Vec::as_slice).unwrap_or(&[]) {
            if !seen.contains(&m) {
                best = best.max(walk(m, next, seen, memo));
            }
        }
        seen.pop();
        memo.insert(n, best + 1);
        best + 1
    }
    let mut memo = HashMap::new();
    edges
        .iter()
        .map(|(a, _)| walk(a, &next, &mut Vec::new(), &mut memo))
        .max()
        .unwrap_or(0)
}

fn check_prose(section: &Section) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut fence = false;
    let mut words = 0usize;
    let mut dashes = 0usize;
    for (i, line) in section.body.lines().enumerate() {
        let n = section.body_line + i;
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        dashes += line.matches('—').count();
        let toks: Vec<String> = line
            .split(|c: char| !c.is_alphanumeric() && c != '-')
            .filter(|t| !t.is_empty())
            .map(str::to_lowercase)
            .collect();
        words += toks.len();
        for t in &toks {
            if BANNED.contains(&t.as_str()) {
                out.push(warn(
                    n,
                    format!("`{t}` reads as filler; say the concrete thing"),
                ));
            }
        }
        for pair in toks.windows(2) {
            if pair[0].chars().all(|c| c.is_ascii_digit()) && TIME_UNITS.contains(&pair[1].as_str())
            {
                out.push(warn(
                    n,
                    format!(
                        "`{} {}` looks like a time estimate; use rock / sand / water",
                        pair[0], pair[1]
                    ),
                ));
            }
        }
        for tag in tags_in(line) {
            if !TAGS.contains(&tag) {
                out.push(err(
                    n,
                    format!("unknown tag `[[{tag}]]`; known: {}", TAGS.join(" ")),
                ));
            }
        }
        if line.matches("{{").count() != line.matches("}}").count() {
            out.push(err(n, "unbalanced {{ }} source label"));
        }
    }
    if words >= 50 && dashes * 100 > words * 2 {
        out.push(warn(
            section.line,
            format!("{dashes} em dashes in {words} words; keep to 2 per 100"),
        ));
    }
    out
}

fn tags_in(line: &str) -> Vec<&str> {
    line.split("[[")
        .skip(1)
        .filter_map(|s| s.split_once("]]").map(|(t, _)| t))
        .collect()
}
