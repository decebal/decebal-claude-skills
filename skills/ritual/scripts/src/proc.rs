use std::fs::{self, File};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub struct Finished {
    pub status: Option<ExitStatus>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Finished {
    pub fn success(&self) -> bool {
        self.status.is_some_and(|s| s.success())
    }

    pub fn code(&self) -> Option<i32> {
        self.status.and_then(|s| s.code())
    }
}

/// Runs `cmd` in its own process group with output captured to files under `log_dir`.
/// Past `deadline` the whole group is killed and reaped: a child that outlives this
/// process would run on with nobody waiting for it.
pub fn run(
    cmd: Command,
    log_dir: &Path,
    label: &str,
    deadline: Instant,
) -> Result<Finished, String> {
    run_inner(cmd, log_dir, label, deadline, None)
}

/// As [`run`], with the process group id written to `pid_file` while the child lives,
/// so a later call can kill a group this process died without reaping.
pub fn run_tracked(
    cmd: Command,
    log_dir: &Path,
    label: &str,
    deadline: Instant,
    pid_file: &Path,
) -> Result<Finished, String> {
    run_inner(cmd, log_dir, label, deadline, Some(pid_file))
}

/// Kills the process group recorded in `pid_file` if its leader still runs a command
/// containing `needle`, then removes the file. A pid reused by an unrelated process
/// fails the `needle` check and is left alone.
pub fn reap_stale(pid_file: &Path, needle: &str) {
    let Ok(text) = fs::read_to_string(pid_file) else {
        return;
    };
    let pid = text.trim();
    if pid.parse::<u32>().is_ok() {
        let command = Command::new("ps")
            .args(["-o", "command=", "-p", pid])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default();
        if command.contains(needle) {
            let _ = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{pid}")])
                .stderr(Stdio::null())
                .status();
        }
    }
    let _ = fs::remove_file(pid_file);
}

fn run_inner(
    mut cmd: Command,
    log_dir: &Path,
    label: &str,
    deadline: Instant,
    pid_file: Option<&Path>,
) -> Result<Finished, String> {
    fs::create_dir_all(log_dir).map_err(|e| format!("create {}: {e}", log_dir.display()))?;
    let out_path = log_dir.join(format!("{label}.stdout"));
    let err_path = log_dir.join(format!("{label}.stderr"));
    let out = File::create(&out_path).map_err(|e| e.to_string())?;
    let err = File::create(&err_path).map_err(|e| e.to_string())?;
    cmd.stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0);
    let mut child = cmd.spawn().map_err(|e| format!("spawn {label}: {e}"))?;
    let group = format!("-{}", child.id());
    if let Some(file) = pid_file {
        let _ = fs::write(file, child.id().to_string());
    }
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = Command::new("/bin/kill").args(["-KILL", &group]).status();
            let _ = child.kill();
            break child.wait().ok();
        }
        thread::sleep(Duration::from_millis(100));
    };
    // Grandchildren the leader left behind would otherwise outlive this process.
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &group])
        .stderr(Stdio::null())
        .status();
    if let Some(file) = pid_file {
        let _ = fs::remove_file(file);
    }
    Ok(Finished {
        status,
        stdout: fs::read_to_string(&out_path).unwrap_or_default(),
        stderr: fs::read_to_string(&err_path).unwrap_or_default(),
        timed_out,
    })
}
