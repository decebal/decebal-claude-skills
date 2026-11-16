use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::preflight::{self, Env, Options, Ran, Report, Runner, Source, Status};

const DOCTOR: &str = "{\n  \"executables_ready\": true,\n  \"setup_ready\": false\n}";

#[derive(Default)]
struct Fake {
    calls: Vec<String>,
    codes: HashMap<String, i32>,
    stdout: HashMap<String, String>,
}

impl Runner for Fake {
    fn run(&mut self, label: &str, program: &Path, _args: &[&str], _deadline: Instant) -> Ran {
        assert!(
            program.is_absolute(),
            "{} is not absolute",
            program.display()
        );
        self.calls.push(label.to_string());
        let default = if label == "allstack" { DOCTOR } else { "" };
        Ran {
            code: Some(self.codes.get(label).copied().unwrap_or(0)),
            stdout: self
                .stdout
                .get(label)
                .cloned()
                .unwrap_or_else(|| default.into()),
            timed_out: false,
            error: None,
        }
    }
}

struct Machine {
    _root: tempfile::TempDir,
    env: Env,
    bin: PathBuf,
}

fn write_exe(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn machine() -> Machine {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("home");
    let repo = root.path().join("repo");
    let bin = root.path().join("bin");
    for skill in [
        "semantic-isa",
        "pr-proof",
        "systematic-debugging",
        "forge-prove-it",
    ] {
        let dir = home.join(".claude/skills").join(skill);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "x").unwrap();
    }
    fs::write(
        home.join(".claude/settings.json"),
        r#"{"hooks":["claude-guard infra-guard","claude-guard bash-hygiene"]}"#,
    )
    .unwrap();
    for dir in [".git", ".chronis", "graphify-out"] {
        fs::create_dir_all(repo.join(dir)).unwrap();
    }
    fs::create_dir_all(&bin).unwrap();
    for tool in ["cn", "allstack", "graphify", "cos", "claude-guard"] {
        write_exe(&bin.join(tool), "#!/bin/sh\n");
    }
    let env = Env {
        cache: home.join(".ritual/preflight.json"),
        home,
        cwd: repo.join("src"),
        path: vec![bin.clone()],
    };
    Machine {
        _root: root,
        env,
        bin,
    }
}

fn full() -> Options {
    Options {
        full: true,
        recheck: false,
        stale: None,
    }
}

fn run(m: &Machine, opts: &Options, fake: &mut Fake) -> Report {
    preflight::run(opts, &m.env, fake).unwrap()
}

fn row<'a>(report: &'a Report, name: &str) -> &'a preflight::Row {
    report.rows.iter().find(|r| r.name == name).unwrap()
}

fn sorted(mut calls: Vec<String>) -> Vec<String> {
    calls.sort();
    calls
}

#[test]
fn a_cache_hit_spawns_nothing() {
    let m = machine();
    let first = run(&m, &full(), &mut Fake::default());
    assert_eq!((first.cached, first.checked), (0, 4));
    assert!(first.ok(), "{}", first.line());

    let mut fake = Fake::default();
    let second = run(&m, &full(), &mut fake);
    assert!(fake.calls.is_empty(), "spawned {:?}", fake.calls);
    assert_eq!((second.cached, second.checked), (4, 0));
    assert_eq!(row(&second, "allstack").source, Source::Cached);
    assert_eq!(
        row(&second, "allstack").note.as_deref(),
        Some("setup_ready: false")
    );
    assert!(second.line().ends_with("· 4 cached, 0 checked"));
}

#[test]
fn a_changed_binary_is_rechecked() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    write_exe(&m.bin.join("cn"), "#!/bin/sh\n# a new build\n");
    let mut fake = Fake::default();
    let report = run(&m, &full(), &mut fake);
    assert_eq!(fake.calls, ["cn"]);
    assert_eq!(row(&report, "cn").source, Source::Checked);
}

#[test]
fn an_mtime_only_change_is_rechecked() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    let cn = fs::File::options()
        .write(true)
        .open(m.bin.join("cn"))
        .unwrap();
    let later = cn.metadata().unwrap().modified().unwrap() + Duration::from_secs(60);
    cn.set_modified(later).unwrap();
    let mut fake = Fake::default();
    run(&m, &full(), &mut fake);
    assert_eq!(fake.calls, ["cn"]);
}

#[test]
fn a_retargeted_path_symlink_is_rechecked() {
    let m = machine();
    let store = m.bin.parent().unwrap().join("store");
    fs::create_dir_all(&store).unwrap();
    for name in ["cn-a", "cn-b"] {
        write_exe(&store.join(name), "#!/bin/sh\n");
    }
    let mtime = fs::metadata(store.join("cn-a"))
        .unwrap()
        .modified()
        .unwrap();
    fs::File::options()
        .write(true)
        .open(store.join("cn-b"))
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    let link = m.bin.join("cn");
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(store.join("cn-a"), &link).unwrap();
    run(&m, &full(), &mut Fake::default());

    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(store.join("cn-b"), &link).unwrap();
    let mut fake = Fake::default();
    run(&m, &full(), &mut fake);
    assert_eq!(fake.calls, ["cn"]);
}

#[test]
fn a_cached_missing_is_rechecked() {
    let m = machine();
    let mut broken = Fake::default();
    broken
        .stdout
        .insert("allstack".into(), "{\"executables_ready\": false}".into());
    let first = run(&m, &full(), &mut broken);
    assert_eq!(row(&first, "allstack").status, Status::Missing);

    let mut fake = Fake::default();
    let second = run(&m, &full(), &mut fake);
    assert_eq!(fake.calls, ["allstack"]);
    assert_eq!(row(&second, "allstack").status, Status::Ok);
    assert!(second.ok(), "{}", second.line());
}

#[test]
fn a_live_chronis_miss_is_sourced_live() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    fs::remove_dir(m.env.cwd.parent().unwrap().join(".chronis")).unwrap();
    let report = run(&m, &full(), &mut Fake::default());
    assert_eq!(row(&report, "cn").status, Status::Missing);
    assert_eq!(row(&report, "cn").source, Source::Live);
}

#[test]
fn stale_rechecks_only_that_tool() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    let mut fake = Fake::default();
    let opts = Options {
        stale: Some("graphify".into()),
        ..full()
    };
    run(&m, &opts, &mut fake);
    assert_eq!(fake.calls, ["graphify"]);

    let mut after = Fake::default();
    run(&m, &full(), &mut after);
    assert!(
        after.calls.is_empty(),
        "the stale mark is cleared once rechecked"
    );
}

#[test]
fn an_unknown_stale_tool_is_refused() {
    let m = machine();
    let opts = Options {
        stale: Some("semantic-isa".into()),
        ..full()
    };
    let err = preflight::run(&opts, &m.env, &mut Fake::default()).unwrap_err();
    assert!(err.contains("cn, allstack, graphify, cos"), "{err}");
}

#[test]
fn recheck_reruns_every_spawning_check() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    let mut fake = Fake::default();
    let opts = Options {
        recheck: true,
        ..full()
    };
    run(&m, &opts, &mut fake);
    assert_eq!(sorted(fake.calls), ["allstack", "cn", "cos", "graphify"]);
}

#[test]
fn a_schema_mismatch_rechecks_everything() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    let mut cache: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&m.env.cache).unwrap()).unwrap();
    cache["schema"] = (preflight::SCHEMA + 1).into();
    fs::write(&m.env.cache, cache.to_string()).unwrap();
    let mut fake = Fake::default();
    run(&m, &full(), &mut fake);
    assert_eq!(fake.calls.len(), 4);
}

#[test]
fn a_missing_hard_binary_reports_its_install_and_fails() {
    let m = machine();
    fs::remove_file(m.bin.join("graphify")).unwrap();
    let report = run(&m, &full(), &mut Fake::default());
    let graphify = row(&report, "graphify");
    assert_eq!(graphify.status, Status::Missing);
    assert!(
        graphify
            .install
            .as_deref()
            .unwrap()
            .contains("uv tool install graphifyy")
    );
    assert!(!report.ok());
    assert!(
        report
            .line()
            .contains("graphify missing (not on PATH: uv tool install graphifyy")
    );
}

#[test]
fn exit_126_is_missing_with_the_reinstall_hint() {
    let m = machine();
    let mut fake = Fake::default();
    fake.codes.insert("graphify".into(), 126);
    let report = run(&m, &full(), &mut fake);
    let graphify = row(&report, "graphify");
    assert_eq!(graphify.status, Status::Missing);
    assert_eq!(graphify.note.as_deref(), Some("exit 126, bad interpreter"));
    assert!(
        graphify
            .install
            .as_deref()
            .unwrap()
            .contains("graphify install")
    );
    assert!(!report.ok());
}

#[test]
fn a_missing_soft_row_does_not_fail() {
    let m = machine();
    fs::remove_file(m.bin.join("cos")).unwrap();
    let report = run(&m, &full(), &mut Fake::default());
    assert_eq!(row(&report, "cos").status, Status::Missing);
    assert!(!row(&report, "cos").hard);
    assert!(report.ok(), "{}", report.line());
}

#[test]
fn doctor_without_executables_ready_fails_allstack() {
    let m = machine();
    let mut fake = Fake::default();
    fake.stdout
        .insert("allstack".into(), "{\"executables_ready\": false}".into());
    let report = run(&m, &full(), &mut fake);
    assert_eq!(row(&report, "allstack").status, Status::Missing);
    assert!(!report.ok());
}

#[test]
fn stat_checks_run_live_every_time() {
    let m = machine();
    run(&m, &full(), &mut Fake::default());
    fs::remove_dir_all(m.env.home.join(".claude/skills/pr-proof")).unwrap();
    fs::remove_dir(m.env.cwd.parent().unwrap().join(".chronis")).unwrap();
    let report = run(&m, &full(), &mut Fake::default());
    assert_eq!(row(&report, "skills").note.as_deref(), Some("pr-proof"));
    assert_eq!(row(&report, "cn").status, Status::Missing);
    assert_eq!(
        row(&report, "cn").install.as_deref(),
        Some("cn init in the main checkout")
    );
    assert!(!report.ok());
}

#[test]
fn unregistered_guard_hooks_fail_claude_guard() {
    let m = machine();
    fs::write(m.env.home.join(".claude/settings.json"), "{}").unwrap();
    let report = run(&m, &full(), &mut Fake::default());
    assert_eq!(
        row(&report, "claude-guard").install.as_deref(),
        Some("skillsync hooks install")
    );
}

#[test]
fn the_lean_preflight_requires_only_cn_and_feedback() {
    let m = machine();
    fs::remove_file(m.bin.join("allstack")).unwrap();
    let mut fake = Fake::default();
    let report = run(
        &m,
        &Options {
            full: false,
            recheck: false,
            stale: None,
        },
        &mut fake,
    );
    let names: Vec<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["cn", "feedback", "allstack", "forge-prove-it"]);
    assert_eq!(fake.calls, ["cn"]);
    assert!(report.ok(), "{}", report.line());
}

#[test]
fn a_linked_worktree_finds_chronis_in_the_main_checkout() {
    let root = tempfile::tempdir().unwrap();
    let main = root.path().join("main");
    let gitdir = main.join(".git/worktrees/wt");
    fs::create_dir_all(&gitdir).unwrap();
    fs::write(gitdir.join("commondir"), "../..\n").unwrap();
    let wt = root.path().join("wt");
    fs::create_dir_all(wt.join("deep")).unwrap();
    fs::write(wt.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
    let (top, found) = preflight::repo_dirs(&wt.join("deep")).unwrap();
    assert_eq!(top, wt);
    assert_eq!(found, main.canonicalize().unwrap());
}
