//! Read-only audit of Claude Code transcripts.
//!
//! `skill-audit listing <transcript.jsonl>` prints, as TSV, every entry of the
//! first skill listing, agent listing and MCP-instructions block that session
//! received, with its exact byte length.
//!
//! `skill-audit usage <root> --since YYYY-MM-DD` reads every `*.jsonl` under
//! `<root>` and prints, as TSV, per name: uses since the date, uses all-time,
//! first and last use date. Kinds: `skill` (Skill tool calls), `slash`
//! (user `<command-name>` tags), `agent` (Agent/Task `subagent_type`), `mcp`
//! (`mcp__<server>__*` tool calls, per server). Tool calls are deduplicated by
//! tool-use id and slash commands by record uuid, because resumed and forked
//! sessions copy earlier records into new files.
//!
//! `skill-audit history <history.jsonl> --since YYYY-MM-DD` counts typed
//! `/name` prompts, which `~/.claude/history.jsonl` keeps longer than
//! transcripts are retained.
//!
//! `join` and `render` combine those outputs into one row per listing entry
//! and an HTML table; see their modules.
//!
//! Only names, counts and dates are printed; no transcript text leaves the tool.

mod join;
mod parse;
mod render;
mod walk;

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::ExitCode;
use std::thread;

use serde_json::Value;

use parse::{civil_date, clean, entry_name, split_entries};

const USAGE: &str = "usage: skill-audit listing <transcript.jsonl>
       skill-audit usage <root> --since YYYY-MM-DD
       skill-audit history <history.jsonl> --since YYYY-MM-DD
       skill-audit join <listing.tsv> <usage.tsv> <repo> <home>
       skill-audit render <joined.tsv> <template.html> <out.html> [--disable p1,p2] [--keep p3,p4]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("listing") if args.len() == 2 => listing(Path::new(&args[1])),
        Some("usage") if args.len() == 4 && args[2] == "--since" => {
            usage(Path::new(&args[1]), &args[3])
        }
        Some("history") if args.len() == 4 && args[2] == "--since" => {
            history(Path::new(&args[1]), &args[3])
        }
        Some("join") if args.len() == 5 => {
            join::run(
                Path::new(&args[1]),
                Path::new(&args[2]),
                Path::new(&args[3]),
                Path::new(&args[4]),
            );
            ExitCode::SUCCESS
        }
        Some("render") if args.len() >= 4 => match render::Plugins::from_args(&args[4..]) {
            Some(plugins) => match render::run(
                Path::new(&args[1]),
                Path::new(&args[2]),
                Path::new(&args[3]),
                &plugins,
            ) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("render: {e}");
                    ExitCode::from(1)
                }
            },
            None => {
                eprintln!("{USAGE}");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn open_lines(path: &Path) -> Option<impl Iterator<Item = String>> {
    let file = File::open(path).ok()?;
    Some(
        BufReader::with_capacity(1 << 20, file)
            .lines()
            .map_while(Result::ok),
    )
}

fn listing(path: &Path) -> ExitCode {
    let Some(lines) = open_lines(path) else {
        eprintln!("cannot open {}", path.display());
        return ExitCode::from(1);
    };
    let (mut skills, mut agents, mut mcp) = (false, false, false);
    println!("kind\tname\tbytes\thas_description");
    for line in lines {
        if !line.contains("\"attachment\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let att = &v["attachment"];
        match att["type"].as_str() {
            Some("skill_listing") if !skills => {
                skills = true;
                let content = att["content"].as_str().unwrap_or_default();
                for entry in split_entries(content) {
                    emit("skill", &entry);
                }
                eprintln!(
                    "skill_listing: {} bytes, skillCount {}, rendered {} bytes",
                    content.len(),
                    att["skillCount"],
                    rendered_len(&v)
                );
            }
            Some("agent_listing_delta") if !agents => {
                agents = true;
                let lines: Vec<&str> = att["addedLines"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                for l in &lines {
                    emit("agent", l);
                }
                eprintln!(
                    "agent_listing: {} lines, {} bytes, rendered {} bytes",
                    lines.len(),
                    lines.iter().map(|l| l.len() + 1).sum::<usize>(),
                    rendered_len(&v)
                );
            }
            Some("mcp_instructions_delta") if !mcp => {
                mcp = true;
                let names: Vec<&str> = att["addedNames"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let blocks: Vec<&str> = att["addedBlocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                for (i, b) in blocks.iter().enumerate() {
                    println!(
                        "mcp\t{}\t{}\ttrue",
                        names.get(i).copied().unwrap_or("?"),
                        b.len() + 1
                    );
                }
                eprintln!(
                    "mcp_instructions: {} blocks, rendered {} bytes",
                    blocks.len(),
                    rendered_len(&v)
                );
            }
            _ => {}
        }
        if skills && agents && mcp {
            break;
        }
    }
    ExitCode::SUCCESS
}

fn rendered_len(v: &Value) -> usize {
    v["rendered"].to_string().len()
}

fn emit(kind: &str, entry: &str) {
    let (name, has_desc) = entry_name(entry);
    println!("{kind}\t{name}\t{}\t{has_desc}", entry.len() + 1);
}

struct Event {
    kind: &'static str,
    name: String,
    date: String,
    key: String,
}

type FileScan = (Vec<Event>, usize, Option<String>, Option<String>);

fn usage(root: &Path, since: &str) -> ExitCode {
    let files = walk::files(root, 8, &|p| p.extension().is_some_and(|x| x == "jsonl"));
    let workers = thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = files.len().div_ceil(workers).max(1);
    let per_file: Vec<FileScan> = thread::scope(|s| {
        let handles: Vec<_> = files
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|p| scan_file(p)).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });

    let mut seen: HashSet<String> = HashSet::new();
    // (kind, name) -> (since, all, first, last)
    let mut agg: BTreeMap<(&str, String), (u64, u64, String, String)> = BTreeMap::new();
    let (mut lines, mut min_d, mut max_d) = (0usize, String::from("9999"), String::new());
    for (events, n, lo, hi) in per_file {
        lines += n;
        if let Some(lo) = lo {
            if lo < min_d {
                min_d = lo;
            }
        }
        if let Some(hi) = hi {
            if hi > max_d {
                max_d = hi;
            }
        }
        for e in events {
            if !seen.insert(e.key) {
                continue;
            }
            let slot =
                agg.entry((e.kind, e.name))
                    .or_insert((0, 0, e.date.clone(), e.date.clone()));
            slot.1 += 1;
            if e.date.as_str() >= since {
                slot.0 += 1;
            }
            if e.date < slot.2 {
                slot.2 = e.date.clone();
            }
            if e.date > slot.3 {
                slot.3 = e.date;
            }
        }
    }
    eprintln!(
        "files {} lines {lines} dates {min_d}..{max_d} since {since}",
        files.len()
    );
    println!("kind\tname\tuses_since\tuses_all\tfirst_used\tlast_used");
    for ((kind, name), (s, a, f, l)) in agg {
        println!("{kind}\t{name}\t{s}\t{a}\t{f}\t{l}");
    }
    ExitCode::SUCCESS
}

fn history(path: &Path, since: &str) -> ExitCode {
    let Some(lines) = open_lines(path) else {
        eprintln!("cannot open {}", path.display());
        return ExitCode::from(1);
    };
    let mut agg: BTreeMap<String, (u64, u64, String, String)> = BTreeMap::new();
    let (mut lo, mut hi) = (String::from("9999"), String::new());
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(ms) = v["timestamp"].as_i64() else {
            continue;
        };
        let date = civil_date(ms / 1000);
        if date < lo {
            lo = date.clone();
        }
        if date > hi {
            hi = date.clone();
        }
        let Some(display) = v["display"].as_str() else {
            continue;
        };
        let Some(cmd) = display.strip_prefix('/') else {
            continue;
        };
        let Some(name) = clean(cmd.split_whitespace().next().unwrap_or_default()) else {
            continue;
        };
        let slot = agg
            .entry(name)
            .or_insert((0, 0, date.clone(), date.clone()));
        slot.1 += 1;
        if date.as_str() >= since {
            slot.0 += 1;
        }
        if date < slot.2 {
            slot.2 = date.clone();
        }
        if date > slot.3 {
            slot.3 = date;
        }
    }
    eprintln!("history dates {lo}..{hi} since {since}");
    println!("kind\tname\tuses_since\tuses_all\tfirst_used\tlast_used");
    for (name, (s, a, f, l)) in agg {
        println!("history\t{name}\t{s}\t{a}\t{f}\t{l}");
    }
    ExitCode::SUCCESS
}

fn scan_file(path: &Path) -> FileScan {
    let mut events = Vec::new();
    let (mut n, mut lo, mut hi): (usize, Option<String>, Option<String>) = (0, None, None);
    let Some(lines) = open_lines(path) else {
        return (events, 0, None, None);
    };
    for line in lines {
        n += 1;
        let tool = line.contains("\"tool_use\"");
        let slash = line.contains("<command-name>");
        if !tool && !slash {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(ts) = v["timestamp"].as_str() else {
            continue;
        };
        let date = ts.get(..10).unwrap_or(ts).to_owned();
        if lo.as_ref().is_none_or(|l| &date < l) {
            lo = Some(date.clone());
        }
        if hi.as_ref().is_none_or(|h| &date > h) {
            hi = Some(date.clone());
        }
        let rtype = v["type"].as_str().unwrap_or_default();
        if tool && rtype == "assistant" {
            for item in v["message"]["content"].as_array().into_iter().flatten() {
                if item["type"] != "tool_use" {
                    continue;
                }
                let id = item["id"].as_str().unwrap_or_default().to_owned();
                let tname = item["name"].as_str().unwrap_or_default();
                let input = &item["input"];
                let hit: Option<(&'static str, Option<String>)> = match tname {
                    "Skill" => Some((
                        "skill",
                        input["skill"]
                            .as_str()
                            .or(input["command"].as_str())
                            .and_then(clean),
                    )),
                    "Agent" | "Task" => Some((
                        "agent",
                        input["subagent_type"]
                            .as_str()
                            .and_then(clean)
                            .or_else(|| Some("general-purpose".into())),
                    )),
                    t if t.starts_with("mcp__") => {
                        let rest = &t[5..];
                        Some(("mcp", clean(rest.split("__").next().unwrap_or(rest))))
                    }
                    _ => input["agentType"]
                        .as_str()
                        .and_then(clean)
                        .map(|a| ("agent", Some(a))),
                };
                if let Some((kind, Some(name))) = hit {
                    events.push(Event {
                        kind,
                        name,
                        date: date.clone(),
                        key: format!("t:{id}"),
                    });
                }
            }
        }
        if slash && rtype == "user" {
            let uuid = v["uuid"].as_str().unwrap_or_default();
            let text = match &v["message"]["content"] {
                Value::String(s) => s.clone(),
                Value::Array(items) => items
                    .iter()
                    .filter_map(|i| i["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            let mut rest = text.as_str();
            let mut k = 0;
            while let Some(i) = rest.find("<command-name>") {
                rest = &rest[i + 14..];
                let Some(j) = rest.find("</command-name>") else {
                    break;
                };
                if let Some(name) = clean(&rest[..j]) {
                    events.push(Event {
                        kind: "slash",
                        name,
                        date: date.clone(),
                        key: format!("s:{uuid}:{k}"),
                    });
                    k += 1;
                }
                rest = &rest[j..];
            }
        }
    }
    (events, n, lo, hi)
}
