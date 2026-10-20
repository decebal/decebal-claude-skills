use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// A grandchild that inherited the pipes (rustc under cargo, a backgrounded
/// server) can hold them open after the check exits; reading stops here.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Only the tail is ever printed, so a check that writes megabytes costs this much.
pub const MAX_KEPT: usize = 64 * 1024;

pub struct Outcome {
    /// `None` when the check was killed at the timeout.
    pub status: Option<ExitStatus>,
    pub output: Vec<u8>,
}

/// Runs `cmd` with stdout and stderr read into one buffer while it runs. A
/// check whose output outgrows the pipe buffer would otherwise block on write
/// and sit there until the timeout killed it.
pub fn run(cmd: &mut Command, timeout: Duration) -> Result<Outcome, String> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn check: {e}"))?;

    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        drain(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        drain(stderr, tx.clone());
    }
    drop(tx);

    let start = Instant::now();
    let mut output = Vec::new();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() > timeout => {
                kill_tree(&mut child);
                break None;
            }
            Ok(None) => {}
            Err(e) => {
                kill_tree(&mut child);
                return Err(format!("wait failed: {e}"));
            }
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => keep(&mut output, &chunk),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => thread::sleep(Duration::from_millis(100)),
        }
    };

    let deadline = Instant::now() + DRAIN_GRACE;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(chunk) => keep(&mut output, &chunk),
            Err(_) => break,
        }
    }

    Ok(Outcome { status, output })
}

/// The last `lines` lines of `output`, each indented under the check's verdict.
pub fn tail(output: &[u8], lines: usize) -> String {
    let text = String::from_utf8_lossy(output);
    let all: Vec<&str> = text.lines().collect();
    let from = all.len().saturating_sub(lines);
    all[from..]
        .iter()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn drain(mut pipe: impl Read + Send + 'static, tx: Sender<Vec<u8>>) {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Kills the check and everything it spawned, then reaps the check. Killing
/// only the direct child leaves cargo's rustc processes running unowned. The
/// check stays in the terminal's process group so Ctrl-C still reaches it.
fn kill_tree(child: &mut Child) {
    let mut pids = vec![child.id()];
    let mut i = 0;
    while i < pids.len() {
        pids.extend(children_of(pids[i]));
        i += 1;
    }
    for pid in pids.iter().skip(1) {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn children_of(pid: u32) -> Vec<u32> {
    Command::new("pgrep")
        .arg("-P")
        .arg(pid.to_string())
        .output()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| line.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn keep(output: &mut Vec<u8>, chunk: &[u8]) {
    output.extend_from_slice(chunk);
    if output.len() > MAX_KEPT {
        let excess = output.len() - MAX_KEPT;
        output.drain(..excess);
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod tests;
