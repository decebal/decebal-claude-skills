//! Finding processes whose parent died and left them running.
//!
//! Two safety properties this exists to hold, both learned by nearly breaking
//! them:
//!
//! 1. **A marker matches the orphan's own argv, never a shared executable
//!    name.** An automation's headless browser and the operator's real browser
//!    are the SAME binary; only argv separates them. Matching on the app name
//!    would kill the operator's session. `tests/sweep.rs` pins that.
//! 2. **A process whose parent is alive is never reported.** Reparenting to
//!    init is the whole signal; a live parent means someone is mid-flight and
//!    the process is theirs, not ours.
//!
//! The age floor carries a third: a process seconds old may be mid-spawn, with
//! its parent not yet scheduled. `DEFAULT_MIN_AGE_SECS` keeps the sweep off it.

use std::time::Duration;

/// Orphans younger than this are left alone: a just-spawned child can be seen
/// at PPID 1 before its parent is recorded, and a short-lived helper is not a
/// leak. Raise it, never lower it, when a host legitimately spawns detached
/// workers.
pub const DEFAULT_MIN_AGE_SECS: u64 = 120;

/// The init process. A parent of 1 means the real parent exited.
const INIT_PID: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub ppid: u32,
    pub cpu: f32,
    pub age: Duration,
    pub args: String,
}

pub struct Policy {
    /// Substrings looked for in a process's ARGV. Each must be specific enough
    /// that only the leaking process carries it — a private profile directory,
    /// a daemon's own binary name — never an application name shared with
    /// something the operator is using.
    pub markers: Vec<String>,
    pub min_age: Duration,
}

impl Policy {
    pub fn new(markers: Vec<String>, min_age_secs: u64) -> Self {
        Self {
            markers,
            min_age: Duration::from_secs(min_age_secs),
        }
    }
}

/// Parse `ps -Ao pid,ppid,pcpu,etime,args`, skipping its header.
///
/// Anything unparseable is skipped rather than guessed at: a sweep that
/// invented a field could signal the wrong pid.
pub fn parse_ps(output: &str) -> Vec<Proc> {
    let mut procs = Vec::new();
    for line in output.lines().skip(1) {
        // `args` contains spaces, so split only the four fixed leading columns.
        let mut it = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(cpu), Some(etime)) =
            (it.next(), it.next(), it.next(), it.next())
        else {
            continue;
        };
        let (Ok(pid), Ok(ppid), Ok(cpu), Some(age)) = (
            pid.parse::<u32>(),
            ppid.parse::<u32>(),
            cpu.parse::<f32>(),
            parse_etime(etime),
        ) else {
            continue;
        };
        let consumed = line.find(etime).map_or(0, |i| i + etime.len());
        procs.push(Proc {
            pid,
            ppid,
            cpu,
            age,
            args: line[consumed..].trim().to_string(),
        });
    }
    procs
}

/// `ps` elapsed time: `MM:SS`, `HH:MM:SS`, or `D-HH:MM:SS`.
///
/// The day-prefixed form is the one that matters here — a three-day-old orphan
/// is the case this tool exists for — and it is the form a naive `HH:MM:SS`
/// parser silently misreads as minutes.
pub fn parse_etime(raw: &str) -> Option<Duration> {
    let (days, clock) = match raw.split_once('-') {
        Some((d, rest)) => (d.parse::<u64>().ok()?, rest),
        None => (0, raw),
    };
    let mut secs = 0u64;
    for part in clock.split(':') {
        secs = secs
            .checked_mul(60)?
            .checked_add(part.parse::<u64>().ok()?)?;
    }
    Some(Duration::from_secs(days * 86_400 + secs))
}

/// Processes that have outlived their parent, are old enough to be a leak, and
/// carry one of the policy's markers in argv.
pub fn find_orphans<'a>(procs: &'a [Proc], policy: &Policy) -> Vec<&'a Proc> {
    procs
        .iter()
        .filter(|p| p.ppid == INIT_PID && p.pid != INIT_PID)
        .filter(|p| p.age >= policy.min_age)
        .filter(|p| policy.markers.iter().any(|m| p.args.contains(m)))
        .collect()
}

/// Total CPU percent held by a set of orphans. Reported rather than summed into
/// a verdict: the number is what makes the finding actionable, and one 100%
/// spinner matters more than twenty idle strays.
pub fn total_cpu(orphans: &[&Proc]) -> f32 {
    orphans.iter().map(|p| p.cpu).sum()
}
