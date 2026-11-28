//! Wrapper-chain resolution: a blocked command hidden one layer down is still a
//! blocked command.
//!
//! FOLLOWS project-local text scripts reached by static resolution, in command
//! position only —
//! `[cd <dir> &&] bash|sh|source|. <path>`, `./<path>.sh`, `<dir>/<path>.sh`,
//! `make [-C <dir>] <target>`, `$(MAKE) <target>`, and
//! `[cd <dir> &&] bun|npm|pnpm run <script>` with the `--cwd`/`--prefix`/`-C`
//! forms, plus `bash -c`/`eval` bodies, `| bash`, `bash < <path>` and
//! `find -exec`. A path given to `grep`/`cat` or inside a quoted argument is
//! data and is not followed.
//!
//! Inside a followed script, each executed command is classified on its own.
//! Printed text (`echo`, `printf`), comments and heredoc bodies not fed to a
//! shell are not commands. Classifying a whole file at once would let one
//! line's `git push`, another's `--force` and a third's `main` add up to a deny.
//!
//! `-C` and `--cwd` matter more than they look. Any convention that prefers them
//! over `cd <dir> && …` routes every command through the flag form, so a
//! resolver blind to it is blind to the common case.
//!
//! STOPS at installed binaries, anything outside the repo, `node_modules`,
//! `dist`, `build`, non-text files, and anything already visited. Bounded by
//! depth, file count and file size.
//!
//! Known limits, both covered by the literal tier and the trust cache: a script
//! referencing a sibling by a path relative to a `cd`'d subdir is not resolved
//! (the `cd <dir> && bash <path>` form itself is), and a dynamic command naming
//! no infra tool is a blind spot — a force-push whose branch is a variable
//! included.

use super::classify::{cached, classify, Verdict, INFRA_TOOL_RE, MUTATE_RE, READONLY_RE};
use super::shell::{self, Segment};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One thing a chain reaches.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ref {
    Script(String),
    /// `(dir, target)` — the dir may be empty.
    Make(String, String),
    /// `(dir, script name)` — the dir may be empty.
    Node(String, String),
}

impl Ref {
    /// How the reference names itself in a decision reason.
    fn label(&self) -> String {
        match self {
            Ref::Script(p) => p.clone(),
            Ref::Make(d, t) => format!("{d}::{t}"),
            Ref::Node(d, n) => format!("{d}::{n}"),
        }
    }

    /// How the reference names itself in the hashed unit list.
    fn unit_name(&self, resolved: &str) -> String {
        match self {
            Ref::Script(_) => resolved.to_string(),
            Ref::Make(d, t) => format!("make:{d}:{t}"),
            Ref::Node(d, n) => format!("node:{d}:{n}"),
        }
    }
}

#[derive(Debug, Default)]
pub struct Scan {
    pub verdict: Option<Verdict>,
    pub reason: String,
    /// The chain touches an infra tool or prod token in a mutating form, with no
    /// clean pattern match — a dynamic command, most likely.
    pub risk: bool,
    /// `"<sha256>  <name>"` per resolved unit, for the trust hash.
    pub units: Vec<String>,
    pub files: Vec<String>,
}

pub struct Limits {
    pub max_depth: usize,
    pub max_files: usize,
    pub max_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_depth: 5,
            max_files: 40,
            max_bytes: 524_288,
        }
    }
}

impl Limits {
    /// Depth from the environment. A non-numeric value falls back to the default
    /// rather than to 0, so a typo cannot silently drop the deep layer.
    pub fn from_env() -> Limits {
        let mut l = Limits::default();
        if let Ok(v) = std::env::var("INFRA_GUARD_DEPTH") {
            l.max_depth = v.parse().unwrap_or(5);
        }
        if let Ok(v) = std::env::var("INFRA_GUARD_MAX_FILES") {
            l.max_files = v.parse().unwrap_or(40);
        }
        if let Ok(v) = std::env::var("INFRA_GUARD_MAX_BYTES") {
            l.max_bytes = v.parse().unwrap_or(524_288);
        }
        l
    }
}

pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Every wrapper reference in a block of text, deduped and ordered.
pub fn extract_refs(text: &str) -> BTreeSet<Ref> {
    refs_in(&shell::logical_lines(text))
}

fn refs_in(lines: &[Vec<Segment>]) -> BTreeSet<Ref> {
    let mut out = BTreeSet::new();
    for line in lines {
        let mut cd: Option<String> = None;
        let mut prev: Option<&Segment> = None;
        for seg in line {
            let cmd = shell::command_words(&seg.words);
            if seg.head() == Some("cd") {
                cd = cmd.get(1).cloned();
            } else {
                let piped_from = prev.filter(|_| seg.piped);
                command_refs(cmd, seg, piped_from, cd.as_deref(), &mut out);
            }
            prev = Some(seg);
        }
    }
    out
}

/// A script run after `cd <dir>` is recorded under both spellings: the
/// resolver tries each against the root and the cwd.
fn add_script(out: &mut BTreeSet<Ref>, path: &str, cd: Option<&str>) {
    out.insert(Ref::Script(path.to_string()));
    if let Some(d) = cd {
        out.insert(Ref::Script(format!("{d}/{path}")));
    }
}

fn command_refs(
    cmd: &[String],
    seg: &Segment,
    piped_from: Option<&Segment>,
    cd: Option<&str>,
    out: &mut BTreeSet<Ref>,
) {
    let Some(head) = seg.head() else {
        return;
    };
    let args = &cmd[1..];

    if let Some(pos) = args.iter().position(|w| w == "-exec" || w == "-execdir") {
        let sub = Segment {
            words: args[pos + 1..].to_vec(),
            ..Segment::default()
        };
        command_refs(&sub.words, &sub, None, cd, out);
    }

    if head.ends_with(".sh") {
        add_script(out, head, cd);
    } else if shell::is_shell(head) {
        let mut i = 0;
        while i < args.len() && args[i].starts_with(['-', '+']) {
            match args[i].as_str() {
                "-c" => {
                    if let Some(body) = args.get(i + 1) {
                        out.extend(extract_refs(body));
                    }
                    return;
                }
                "-o" | "+o" => i += 2,
                _ => i += 1,
            }
        }
        if let Some(script) = args.get(i) {
            add_script(out, script, cd);
        } else if let Some(file) = &seg.stdin {
            add_script(out, file, cd);
        } else if let Some(from) = piped_from {
            for w in from.words.iter().filter(|w| w.ends_with(".sh")) {
                add_script(out, w, cd);
            }
        }
    } else if head == "source" || head == "." {
        if let Some(script) = args.first() {
            add_script(out, script, cd);
        }
    } else if head == "eval" {
        out.extend(extract_refs(&args.join(" ")));
    } else if matches!(head, "make" | "gmake" | "$(MAKE)" | "${MAKE}") {
        make_refs(args, out);
    } else if matches!(head, "bun" | "npm" | "pnpm") {
        node_refs(args, cd, out);
    }
}

fn make_refs(args: &[String], out: &mut BTreeSet<Ref>) {
    let mut dir = String::new();
    let mut targets = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "-C" {
            dir = args.get(i + 1).cloned().unwrap_or_default();
            i += 1;
        } else if let Some(d) = a.strip_prefix("-C") {
            dir = d.to_string();
        } else if a == "-f"
            || (a == "-j" && args.get(i + 1).is_some_and(|n| n.parse::<u32>().is_ok()))
        {
            i += 1;
        } else if !a.starts_with('-') && !a.contains('=') {
            targets.push(a.to_string());
        }
        i += 1;
    }
    for t in targets {
        out.insert(Ref::Make(dir.clone(), t));
    }
}

fn node_refs(args: &[String], cd: Option<&str>, out: &mut BTreeSet<Ref>) {
    let mut dir: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if matches!(a, "--cwd" | "--prefix" | "-C") {
            dir = args.get(i + 1).cloned();
            i += 1;
        } else if let Some(d) = a
            .strip_prefix("--cwd=")
            .or_else(|| a.strip_prefix("--prefix="))
        {
            dir = Some(d.to_string());
        } else if a == "run" {
            if let Some(name) = args.get(i + 1) {
                let dir = dir.or_else(|| cd.map(str::to_string)).unwrap_or_default();
                out.insert(Ref::Node(dir, name.clone()));
            }
            return;
        }
        i += 1;
    }
}

fn is_project_local(p: &Path, root: &Path) -> bool {
    if !p.starts_with(root) {
        return false;
    }
    let s = p.to_string_lossy();
    !["/node_modules/", "/.git/", "/dist/", "/build/"]
        .iter()
        .any(|seg| s.contains(seg))
}

/// Text, by the same test `grep -I` uses: no NUL byte in the content.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0)
}

/// Read-only invocations are deliberately not a risk signal. Flagging every
/// mention of an infra tool made ordinary wrappers prompt, which is what got the
/// deep layer switched off rather than fixed.
pub fn risky_lines(text: &str, prod_re: Option<&str>) -> bool {
    let tokens = match prod_re {
        Some(p) if !p.is_empty() => format!("(?i)({INFRA_TOOL_RE}|{p})"),
        _ => format!("(?i)({INFRA_TOOL_RE})"),
    };
    let tool = cached(&tokens);
    let mutate = cached(&format!("(?i){MUTATE_RE}"));
    let readonly = cached(&format!("(?i){READONLY_RE}"));
    text.lines()
        .any(|l| tool.is_match(l) && mutate.is_match(l) && !readonly.is_match(l))
}

fn resolve_script(p: &str, root: &Path, cwd: &Path) -> Option<PathBuf> {
    let bare = p.strip_prefix("./").unwrap_or(p);
    [PathBuf::from(p), root.join(bare), cwd.join(bare)]
        .into_iter()
        .find(|cand| cand.is_file())
}

/// The recipe body for one make target.
///
/// `-C` must be honoured: `make -C sub build` runs `sub/Makefile`, and reading
/// the root Makefile instead resolves a different recipe or none at all.
fn make_recipe(target: &str, root: &Path, cwd: &Path, dir: &str) -> Option<String> {
    let candidates = [
        root.join(dir).join("Makefile"),
        cwd.join(dir).join("Makefile"),
        root.join(dir).join("makefile"),
        root.join("Makefile"),
    ];
    for mf in candidates {
        let Ok(text) = std::fs::read_to_string(&mf) else {
            continue;
        };
        let mut body = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            if !inside {
                let head = line.split(':').next().unwrap_or("");
                if head.trim_end() == target && line.contains(':') {
                    inside = true;
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix('\t') {
                body.push(rest);
            } else if !line.is_empty() {
                break;
            }
        }
        if !body.is_empty() {
            return Some(body.join("\n"));
        }
    }
    None
}

fn node_script(name: &str, root: &Path, cwd: &Path, dir: &str) -> Option<String> {
    let candidates = [
        root.join(dir).join("package.json"),
        cwd.join(dir).join("package.json"),
        cwd.join("package.json"),
        root.join("package.json"),
    ];
    for pj in candidates {
        let Ok(text) = std::fs::read_to_string(&pj) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        if let Some(body) = json
            .get("scripts")
            .and_then(|s| s.get(name))
            .and_then(|v| v.as_str())
        {
            if !body.is_empty() {
                return Some(body.to_string());
            }
        }
    }
    None
}

/// Walk the chain. Scans the FULL tree with no early exit, so `units` is
/// complete for hashing even when a deny is already decided.
pub fn scan_tree(
    initial: &str,
    root: &Path,
    cwd: &Path,
    limits: &Limits,
    prod_re: Option<&str>,
) -> Scan {
    let mut scan = Scan::default();
    let mut seen: BTreeSet<Ref> = BTreeSet::new();
    // Breadth-first, so the reason names the SHALLOWEST reference that fired.
    // A depth-first walk reports the same verdict via a deeper file, which reads
    // as a different finding to whoever has to review the chain.
    let mut work: std::collections::VecDeque<(usize, Ref)> =
        extract_refs(initial).into_iter().map(|r| (1, r)).collect();
    let mut nfiles = 0usize;

    while let Some((depth, r)) = work.pop_front() {
        if !seen.insert(r.clone()) {
            continue;
        }
        if depth > limits.max_depth {
            continue;
        }
        if nfiles >= limits.max_files {
            break;
        }

        let (content, unit_name) = match &r {
            Ref::Script(p) => {
                let Some(rp) = resolve_script(p, root, cwd) else {
                    continue;
                };
                if !is_project_local(&rp, root) {
                    continue;
                }
                let Ok(bytes) = std::fs::read(&rp) else {
                    continue;
                };
                if !is_text(&bytes) || bytes.len() as u64 > limits.max_bytes {
                    continue;
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                nfiles += 1;
                let name = rp.to_string_lossy().to_string();
                scan.files.push(name.clone());
                (text, r.unit_name(&name))
            }
            Ref::Make(dir, target) => match make_recipe(target, root, cwd, dir) {
                Some(body) => (body, r.unit_name("")),
                None => continue,
            },
            Ref::Node(dir, name) => match node_script(name, root, cwd, dir) {
                Some(body) => (body, r.unit_name("")),
                None => continue,
            },
        };

        scan.units
            .push(format!("{}  {}", sha256_hex(&content), unit_name));

        let lines = shell::logical_lines(&content);
        let commands = shell::executed(&lines);
        if !scan.risk && risky_lines(&commands.join("\n"), prod_re) {
            scan.risk = true;
        }

        for command in &commands {
            let Some(v) = classify(&command.to_lowercase()) else {
                continue;
            };
            let reason = format!("{} (in {})", reason_of(&v), r.label());
            match (&v, &scan.verdict) {
                (Verdict::Deny(_), Some(Verdict::Deny(_))) => {}
                (Verdict::Deny(_), _) => {
                    scan.reason = reason;
                    scan.verdict = Some(v);
                }
                (Verdict::Ask(_), None) => {
                    scan.reason = reason;
                    scan.verdict = Some(v);
                }
                _ => {}
            }
        }

        for next in refs_in(&lines) {
            work.push_back((depth + 1, next));
        }
    }
    scan
}

fn reason_of(v: &Verdict) -> &str {
    match v {
        Verdict::Deny(r) | Verdict::Ask(r) => r,
    }
}

/// Combined hash over the sorted set of resolved unit hashes.
///
/// Covers script content, make-recipe text and node-script text, so editing a
/// Makefile recipe busts the trust even when no script file changed.
pub fn tree_hash(units: &[String]) -> Option<String> {
    if units.is_empty() {
        return None;
    }
    let sorted: BTreeSet<&String> = units.iter().collect();
    let mut joined = String::new();
    for u in sorted {
        joined.push_str(u);
        joined.push('\n');
    }
    Some(sha256_hex(&joined))
}

pub fn kosher_path() -> PathBuf {
    if let Ok(p) = std::env::var("INFRA_GUARD_KOSHER") {
        return PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude/infra-guard-kosher.txt")
}

pub fn is_kosher(hash: &str) -> bool {
    if hash.is_empty() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(kosher_path()) else {
        return false;
    };
    text.lines().any(|l| l.starts_with(&format!("{hash}  ")))
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod resolve_tests;
