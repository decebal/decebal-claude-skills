//! Report — and optionally reap — processes whose parent died.
//!
//! Exits 1 when orphans are found, so it works as a session-start gate: the
//! population is the signal, and a population that only grows is the leak.

use std::process::{exit, Command};

use orphan_sweep::{find_orphans, parse_ps, total_cpu, Policy, Proc, DEFAULT_MIN_AGE_SECS};

/// Markers known to leak. Each is specific to the leaking process's own argv —
/// a private profile directory or a daemon binary — never an application name
/// the operator also runs. See the crate docs for why that distinction is the
/// difference between reaping a stray and killing someone's browser session.
const DEFAULT_MARKERS: [&str; 2] = ["agent-browser", "cn list --status=in_progress"];

/// `ps` is invoked by absolute path on purpose. An output-filtering shell proxy
/// rewrote a plain `ps` during the 2026-09-18 incident and reported 31
/// processes against a real 1242, with argv truncated so the markers below were
/// invisible. A gate that can be filtered is not a gate.
const PS: &str = "/bin/ps";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let kill = argv.iter().any(|a| a == "--kill");
    let quiet = argv.iter().any(|a| a == "--quiet");
    let mut markers: Vec<String> = Vec::new();
    let mut min_age = DEFAULT_MIN_AGE_SECS;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--marker" => {
                if let Some(m) = it.next() {
                    markers.push(m.clone());
                }
            }
            "--min-age" => {
                if let Some(v) = it.next().and_then(|v| v.parse().ok()) {
                    min_age = v;
                }
            }
            _ => {}
        }
    }
    if markers.is_empty() {
        markers = DEFAULT_MARKERS.iter().map(|m| (*m).to_string()).collect();
    }

    let Some(output) = run_ps() else {
        eprintln!("orphan-sweep: could not run {PS}");
        exit(2);
    };
    let procs = parse_ps(&output);
    if procs.is_empty() {
        eprintln!("orphan-sweep: {PS} returned no parseable rows");
        exit(2);
    }

    let policy = Policy::new(markers, min_age);
    let orphans = find_orphans(&procs, &policy);
    if orphans.is_empty() {
        if !quiet {
            println!("orphan-sweep: clean ({} processes scanned)", procs.len());
        }
        return;
    }

    report(&orphans, procs.len());
    if kill {
        reap(&orphans);
    } else {
        println!("\nRe-run with --kill to reap them.");
    }
    exit(1);
}

fn report(orphans: &[&Proc], scanned: usize) {
    println!(
        "orphan-sweep: {} orphaned process(es) holding {:.1}% CPU ({scanned} scanned)\n",
        orphans.len(),
        total_cpu(orphans),
    );
    for p in orphans {
        let age = p.age.as_secs();
        let (d, h, m) = (age / 86_400, (age % 86_400) / 3600, (age % 3600) / 60);
        // argv is the evidence for WHY this pid matched, so it is shown rather
        // than summarized — but a Chrome command line is ~1 KiB of flags.
        let args: String = p.args.chars().take(110).collect();
        println!(
            "  pid {:<7} {d}d{h:02}h{m:02}m  {:>5.1}%  {args}",
            p.pid, p.cpu
        );
    }
}

fn reap(orphans: &[&Proc]) {
    let mut killed = 0;
    for p in orphans {
        // SIGTERM only. A sweep that escalates to SIGKILL on its own can cut a
        // process mid-write; whoever reads this report decides on escalation.
        let ok = Command::new("/bin/kill")
            .arg(p.pid.to_string())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            killed += 1;
        }
    }
    println!(
        "\norphan-sweep: sent TERM to {killed}/{} process(es)",
        orphans.len()
    );
}

fn run_ps() -> Option<String> {
    let out = Command::new(PS)
        .args(["-Ao", "pid,ppid,pcpu,etime,args"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn print_help() {
    println!(
        "orphan-sweep — report processes whose parent died (PPID 1) and left them running.

USAGE:
    orphan-sweep [--marker <substring>]... [--min-age <secs>] [--kill]

    --marker    Substring matched against a process's ARGV. Repeatable.
                Must be specific to the leaking process: a private profile
                directory or a daemon's own binary name, NEVER an application
                name the operator also runs.
    --min-age   Ignore orphans younger than this (default {DEFAULT_MIN_AGE_SECS}s).
    --quiet     Say nothing when clean. For a session-start hook, where a line
                per session is noise and a finding is the only thing worth
                spending context on.
    --kill      Send SIGTERM to what is found.

Exits 1 when orphans are found, 2 when ps could not be read."
    );
}
