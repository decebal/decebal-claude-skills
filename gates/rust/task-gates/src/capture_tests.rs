use super::{run, tail, MAX_KEPT};
use std::process::Command;
use std::time::{Duration, Instant};

fn sh(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(script);
    cmd
}

#[test]
fn output_larger_than_the_pipe_buffer_does_not_block_the_check() {
    let start = Instant::now();
    let outcome = run(
        &mut sh("head -c 2000000 /dev/zero; head -c 2000000 /dev/zero >&2"),
        Duration::from_secs(20),
    )
    .unwrap();
    assert!(outcome.status.is_some_and(|s| s.success()));
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn a_check_past_its_timeout_is_killed() {
    let mut cmd = Command::new("sleep");
    cmd.arg("30");
    let start = Instant::now();
    let outcome = run(&mut cmd, Duration::from_secs(1)).unwrap();
    assert!(outcome.status.is_none());
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_timeout_kills_what_the_check_spawned() {
    let outcome = run(&mut sh("sleep 31 & echo $!; wait"), Duration::from_secs(1)).unwrap();
    assert!(outcome.status.is_none());
    let pid = String::from_utf8_lossy(&outcome.output).trim().to_string();
    assert!(!pid.is_empty());
    // Killed but not yet reaped by init still answers `kill -0`.
    let deadline = Instant::now() + Duration::from_secs(3);
    let alive = loop {
        let alive = Command::new("kill")
            .arg("-0")
            .arg(&pid)
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        if !alive || Instant::now() > deadline {
            break alive;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(!alive, "grandchild {pid} outlived the timeout");
}

#[test]
fn a_grandchild_holding_the_pipe_does_not_hold_the_check() {
    let start = Instant::now();
    let outcome = run(&mut sh("sleep 6 & echo done"), Duration::from_secs(20)).unwrap();
    assert!(outcome.status.is_some_and(|s| s.success()));
    assert!(String::from_utf8_lossy(&outcome.output).contains("done"));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_failing_check_keeps_its_last_lines() {
    let outcome = run(
        &mut sh("i=1; while [ $i -le 100 ]; do echo line$i; i=$((i+1)); done; exit 3"),
        Duration::from_secs(20),
    )
    .unwrap();
    assert_eq!(outcome.status.and_then(|s| s.code()), Some(3));
    let shown = tail(&outcome.output, 5);
    assert!(shown.contains("  line100"));
    assert!(shown.contains("  line96"));
    assert!(!shown.contains("line95"));
}

#[test]
fn kept_output_is_bounded() {
    let outcome = run(
        &mut sh("head -c 1000000 /dev/zero | tr '\\0' x; echo; echo last"),
        Duration::from_secs(20),
    )
    .unwrap();
    assert!(outcome.output.len() <= MAX_KEPT);
    assert_eq!(tail(&outcome.output, 1), "  last");
}
