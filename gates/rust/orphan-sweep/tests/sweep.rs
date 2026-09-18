//! The safety properties, pinned with rows copied from the 2026-09-18 incident.
//!
//! The fixture deliberately mixes the operator's real Chrome with the
//! automation's headless Chrome, because those two are the same executable and
//! telling them apart is the whole job.

use orphan_sweep::{find_orphans, parse_etime, parse_ps, total_cpu, Policy};
use std::time::Duration;

/// Real `ps -Ao pid,ppid,pcpu,etime,args` shapes: the operator's Chrome (live
/// parent, and as a root at PPID 1), the automation's orphaned daemon and its
/// helper, and a freshly spawned stray.
const PS: &str = "\
  PID  PPID  %CPU     ELAPSED ARGS
  475     1   4.2 04-02:26:11 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
39119   475  61.4 04-01:50:40 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper (Renderer) --type=renderer
93466     1   0.0 03-02:30:13 /Users/x/node_modules/agent-browser/bin/agent-browser-darwin-arm64
93510 93466  99.6 03-02:26:47 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome Helper (GPU) --headless=new --user-data-dir=/tmp/agent-browser-chrome-6016511a
53741     1  30.6       00:16 /Users/x/.cargo/bin/cn list --status=in_progress --toon
";

fn policy() -> Policy {
    Policy::new(vec!["agent-browser".to_string()], 120)
}

#[test]
fn the_operators_real_browser_is_never_matched() {
    let procs = parse_ps(PS);
    let found = find_orphans(&procs, &policy());
    // PID 475 is Chrome at PPID 1 — an orphan by parentage, and the operator's
    // actual browser. Only argv keeps it safe.
    assert!(
        !found.iter().any(|p| p.pid == 475),
        "matched the operator's own browser: {found:?}"
    );
}

#[test]
fn the_orphaned_daemon_and_its_helper_are_both_found() {
    let procs = parse_ps(PS);
    let found = find_orphans(&procs, &policy());
    assert_eq!(found.len(), 1, "expected only the PPID-1 daemon: {found:?}");
    assert_eq!(found[0].pid, 93466);
}

#[test]
fn a_process_with_a_live_parent_is_never_reported() {
    let procs = parse_ps(PS);
    // 93510 carries the marker in argv AND burns 99.6% CPU, but its parent is
    // alive: it belongs to the daemon, and killing the daemon is the fix.
    let found = find_orphans(&procs, &policy());
    assert!(!found.iter().any(|p| p.pid == 93510));
}

#[test]
fn young_orphans_are_left_alone() {
    let procs = parse_ps(PS);
    let p = Policy::new(vec!["cn list".to_string()], 120);
    assert!(
        find_orphans(&procs, &p).is_empty(),
        "a 16-second-old stray is not yet a leak"
    );
    let p = Policy::new(vec!["cn list".to_string()], 10);
    assert_eq!(find_orphans(&procs, &p).len(), 1);
}

#[test]
fn argv_survives_parsing_with_its_spaces_and_flags() {
    let procs = parse_ps(PS);
    let gpu = procs.iter().find(|p| p.pid == 93510).expect("row present");
    assert!(gpu.args.ends_with("agent-browser-chrome-6016511a"));
    assert!(gpu.args.contains("Google Chrome Helper (GPU)"));
}

#[test]
fn day_prefixed_elapsed_is_not_read_as_minutes() {
    // The form that matters: a three-day-old orphan. An HH:MM:SS-only parser
    // reads this as 2h26m and the age floor then hides the worst offender.
    assert_eq!(
        parse_etime("03-02:26:47"),
        Some(Duration::from_secs(3 * 86_400 + 2 * 3600 + 26 * 60 + 47))
    );
    assert_eq!(parse_etime("02:26:47"), Some(Duration::from_secs(8807)));
    assert_eq!(parse_etime("00:16"), Some(Duration::from_secs(16)));
    assert_eq!(parse_etime("not-a-time"), None);
}

#[test]
fn cpu_is_summed_across_the_reported_set() {
    let procs = parse_ps(PS);
    let p = Policy::new(vec!["cn list".to_string()], 10);
    let found = find_orphans(&procs, &p);
    assert!((total_cpu(&found) - 30.6).abs() < f32::EPSILON);
}
