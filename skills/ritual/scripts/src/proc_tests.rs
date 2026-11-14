use std::fs;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::proc;

fn sleeper() -> std::process::Child {
    let mut cmd = Command::new("/bin/sleep");
    cmd.arg("30").stdout(Stdio::null()).process_group(0);
    cmd.spawn().unwrap()
}

#[test]
fn a_stale_group_is_killed_only_when_its_command_matches() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("mutator.pgid");

    let mut other = sleeper();
    fs::write(&file, other.id().to_string()).unwrap();
    proc::reap_stale(&file, "claude");
    assert!(
        other.try_wait().unwrap().is_none(),
        "unrelated process survives"
    );
    assert!(!file.exists());
    other.kill().unwrap();
    other.wait().unwrap();

    let mut stale = sleeper();
    fs::write(&file, stale.id().to_string()).unwrap();
    proc::reap_stale(&file, "sleep");
    let deadline = Instant::now() + Duration::from_secs(5);
    while stale.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "stale group was not killed");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_tracked_run_removes_its_pid_file_and_a_deadline_kills_the_group() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("mutator.pgid");
    let mut cmd = Command::new("/bin/sleep");
    cmd.arg("30");
    let started = Instant::now();
    let done = proc::run_tracked(
        cmd,
        dir.path(),
        "sleep",
        Instant::now() + Duration::from_millis(300),
        &file,
    )
    .unwrap();
    assert!(done.timed_out);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(!file.exists());
}
