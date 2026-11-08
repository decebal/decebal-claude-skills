//! Integration tests: the binary against a fixture repo and a temp HOME.

use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU32 = AtomicU32::new(0);

const SETTINGS_TEMPLATE: &str = r#"{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [
        { "type": "command", "command": "claude-guard infra-guard", "timeout": 10 },
        { "type": "command", "command": "claude-guard pr-guard", "timeout": 10 }
      ] },
      { "matcher": "Write|Edit|MultiEdit", "hooks": [
        { "type": "command", "command": "claude-guard prompt-number", "timeout": 5 }
      ] }
    ]
  }
}
"#;

const CODEX_TEMPLATE: &str = r#"{
  "hooks": {
    "PreToolUse": [
      { "matcher": "^Bash$", "hooks": [
        { "type": "command", "command": "codex-guard infra-guard", "timeout": 10 }
      ] }
    ]
  }
}
"#;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "skillsync-{tag}-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create fixture root");
        let fixture = Self { root };
        fixture.write(
            "repo/skills/alpha/SKILL.md",
            "# alpha\nRun `cn ready --toon`.\n",
        );
        fixture.write("repo/skills/beta/SKILL.md", "# beta\nbody\n");
        fixture.write("repo/configs/settings.json", SETTINGS_TEMPLATE);
        fixture.write("repo/configs/codex-hooks.json", CODEX_TEMPLATE);
        fixture.write("repo/configs/prod-guard-tokens.txt", "prod:example-prod\n");
        fixture
    }

    fn repo(&self) -> PathBuf {
        self.root.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.path(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
        fs::write(path, text).expect("write fixture");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Out {
    code: i32,
    stdout: String,
}

fn skillsync(fx: &Fixture, args: &[&str]) -> Out {
    let output = Command::new(env!("CARGO_BIN_EXE_skillsync"))
        .args(["--repo", fx.repo().to_str().expect("utf8")])
        .args(["--home", fx.home().to_str().expect("utf8")])
        .args(["--cn", "/nonexistent/cn-for-tests"])
        .args(args)
        .output()
        .expect("run skillsync");
    Out {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

fn kinds(out: &Out) -> Vec<String> {
    let v: Value = serde_json::from_str(out.stdout.trim()).expect("json output");
    v["findings"]
        .as_array()
        .expect("findings array")
        .iter()
        .map(|f| f["kind"].as_str().expect("kind").to_string())
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("read file")
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) {
    fs::create_dir_all(at.parent().expect("parent")).expect("create dirs");
    symlink(target, at).expect("symlink");
}

#[test]
fn check_reports_drift_copy_dangling_and_missing() {
    let fx = Fixture::new("drift");
    link(
        &fx.repo().join("skills/alpha"),
        &fx.home().join(".claude/skills/alpha"),
    );
    fx.write("home/.claude/skills/beta/SKILL.md", "# beta, edited\n");
    link(&fx.path("nowhere"), &fx.home().join(".agents/skills/alpha"));
    fs::create_dir_all(fx.home().join(".agents/skills")).expect("mkdir");

    let out = skillsync(&fx, &["check", "--json", "--strict"]);
    assert_eq!(out.code, 1, "stdout: {}", out.stdout);
    let found = kinds(&out);
    for want in ["copy", "drift", "dangling", "missing", "hooks-file"] {
        assert!(found.iter().any(|k| k == want), "no {want} in {found:?}");
    }
}

#[test]
fn check_without_strict_summarises_uninstalled_skills_as_a_notice() {
    let fx = Fixture::new("subset");
    link(
        &fx.repo().join("skills/alpha"),
        &fx.home().join(".claude/skills/alpha"),
    );
    link(
        &fx.repo().join("skills/alpha"),
        &fx.home().join(".agents/skills/alpha"),
    );
    fx.write("home/.claude/settings.json", SETTINGS_TEMPLATE);

    let out = skillsync(&fx, &["check", "--json"]);
    let found = kinds(&out);
    assert!(!found.iter().any(|k| k == "missing"), "{found:?}");
    assert_eq!(out.code, 0, "stdout: {}", out.stdout);
    let v: Value = serde_json::from_str(out.stdout.trim()).expect("json output");
    let notices: Vec<&str> = v["notices"]
        .as_array()
        .expect("notices array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        notices
            .iter()
            .filter(|n| n.starts_with("1 of 2 repo skills"))
            .count(),
        2,
        "{notices:?}"
    );
}

#[test]
fn check_flags_prod_token_template_and_stale_matcher() {
    let fx = Fixture::new("tokens");
    fx.write("home/.claude/prod-guard-tokens.txt", "prod:example-prod\n");
    fx.write(
        "home/.claude/settings.json",
        r#"{"hooks":{"PreToolUse":[{"matcher":"Write|Edit","hooks":[{"type":"command","command":"claude-guard prompt-number"}]}]}}"#,
    );

    let out = skillsync(&fx, &["check", "--json"]);
    assert_eq!(out.code, 1);
    let found = kinds(&out);
    assert!(
        found.iter().any(|k| k == "prod-token-template"),
        "{found:?}"
    );
    assert!(found.iter().any(|k| k == "hook-matcher"), "{found:?}");
}

#[test]
fn check_flags_dangling_cn_mention_when_cn_answers() {
    let fx = Fixture::new("cnlint");
    fx.write(
        "repo/skills/alpha/SKILL.md",
        "# alpha\n`cn ready --toon`\n\n```sh\ncn create \"x\"\n```\n\ntrigger word cn create in prose\n",
    );
    let fake = fx.path("fake-cn");
    fs::write(
        &fake,
        "#!/bin/sh\ncase \"$*\" in\n  --version|\"ready --help\") exit 0;;\n  *) exit 2;;\nesac\n",
    )
    .expect("write fake cn");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let output = Command::new(env!("CARGO_BIN_EXE_skillsync"))
        .args(["--repo", fx.repo().to_str().expect("utf8")])
        .args(["--home", fx.home().to_str().expect("utf8")])
        .args(["--cn", fake.to_str().expect("utf8"), "check", "--json"])
        .output()
        .expect("run");
    let out = Out {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    };
    assert_eq!(out.code, 1, "{}", out.stdout);
    let v: Value = serde_json::from_str(out.stdout.trim()).expect("json");
    let findings = v["findings"].as_array().expect("array");
    let unknown: Vec<&str> = findings
        .iter()
        .filter(|f| f["kind"] == "cn-unknown")
        .filter_map(|f| f["detail"].as_str())
        .collect();
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert!(unknown[0].contains("cn create"), "{unknown:?}");
}

#[test]
fn hooks_install_is_idempotent_and_keeps_other_keys() {
    let fx = Fixture::new("hooks");
    fx.write(
        "home/.claude/settings.json",
        r#"{"model":"opus","hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    );
    fx.write("home/.codex/hooks.json", r#"{"hooks":{}}"#);

    let first = skillsync(&fx, &["hooks", "install"]);
    assert_eq!(first.code, 0, "{}", first.stdout);
    let after_first = read(&fx.home().join(".claude/settings.json"));
    assert!(
        after_first.find("\"model\"") < after_first.find("\"hooks\""),
        "key order lost"
    );
    assert!(after_first.contains("echo hi"));
    assert!(after_first.contains("claude-guard pr-guard"));
    assert!(read(&fx.home().join(".codex/hooks.json")).contains("codex-guard infra-guard"));

    let backups: Vec<_> = fs::read_dir(fx.home().join(".claude"))
        .expect("read dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".skillsync-"))
        .collect();
    assert_eq!(backups.len(), 1, "one backup of the original file");
    assert!(read(&backups[0].path()).contains("\"model\":\"opus\""));

    let second = skillsync(&fx, &["hooks", "install"]);
    assert_eq!(second.code, 0);
    assert_eq!(read(&fx.home().join(".claude/settings.json")), after_first);
    assert_eq!(
        fs::read_dir(fx.home().join(".claude"))
            .expect("read dir")
            .count(),
        backups.len() + 1,
        "rerun wrote no new backup"
    );
}

#[test]
fn skills_install_preserves_conflicts_and_installs_nothing() {
    let fx = Fixture::new("conflict");
    fx.write("home/.claude/skills/alpha/SKILL.md", "# user's own alpha\n");
    let elsewhere = fx.path("elsewhere");
    fs::create_dir_all(&elsewhere).expect("mkdir");
    link(&elsewhere, &fx.home().join(".agents/skills/beta"));

    let out = skillsync(&fx, &["skills", "install", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(kinds(&out), vec!["conflict", "conflict"]);
    assert_eq!(
        read(&fx.home().join(".claude/skills/alpha/SKILL.md")),
        "# user's own alpha\n"
    );
    assert!(fs::symlink_metadata(fx.home().join(".agents/skills/beta"))
        .expect("beta still there")
        .file_type()
        .is_symlink());
    assert!(
        fs::symlink_metadata(fx.home().join(".agents/skills/alpha")).is_err(),
        "a clean skill is not linked while another conflicts"
    );
}

#[test]
fn skills_install_links_then_check_passes() {
    let fx = Fixture::new("roundtrip");
    let install = skillsync(&fx, &["skills", "install"]);
    assert_eq!(install.code, 0, "{}", install.stdout);
    let hooks = skillsync(&fx, &["hooks", "install"]);
    assert_eq!(hooks.code, 0, "{}", hooks.stdout);

    let check = skillsync(&fx, &["check", "--json"]);
    assert_eq!(check.code, 0, "{}", check.stdout);
    assert!(kinds(&check).is_empty());
}

#[test]
fn dry_run_writes_nothing() {
    let fx = Fixture::new("dryrun");
    fx.write("home/.claude/settings.json", r#"{"hooks":{}}"#);
    let before = read(&fx.home().join(".claude/settings.json"));

    let hooks = skillsync(&fx, &["hooks", "install", "--dry-run"]);
    assert_eq!(hooks.code, 0, "{}", hooks.stdout);
    assert_eq!(read(&fx.home().join(".claude/settings.json")), before);
    assert_eq!(
        fs::read_dir(fx.home().join(".claude"))
            .expect("read dir")
            .count(),
        1
    );

    let skills = skillsync(&fx, &["skills", "install", "--dry-run"]);
    assert_eq!(skills.code, 0, "{}", skills.stdout);
    assert!(!fx.home().join(".agents").exists());
    assert!(fs::symlink_metadata(fx.home().join(".claude/skills")).is_err());
}

#[test]
fn hooks_install_counts_absolute_path_guards_as_present() {
    let fx = Fixture::new("abspath");
    let claude = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/Users/me/.cargo/bin/claude-guard infra-guard"},{"type":"command","command":"/Users/me/.cargo/bin/claude-guard pr-guard"}]},{"matcher":"Write|Edit|MultiEdit","hooks":[{"type":"command","command":"/Users/me/.cargo/bin/claude-guard prompt-number"}]}]}}"#;
    let codex = r#"{"hooks":{"PreToolUse":[{"matcher":"^Bash$","hooks":[{"type":"command","command":"/Users/me/.cargo/bin/codex-guard infra-guard"}]}]}}"#;
    fx.write("home/.claude/settings.json", claude);
    fx.write("home/.codex/hooks.json", codex);

    let out = skillsync(&fx, &["hooks", "install", "--json"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    let v: Value = serde_json::from_str(out.stdout.trim()).expect("json");
    assert_eq!(v["actions"], Value::Array(vec![]), "{}", out.stdout);
    assert_eq!(read(&fx.home().join(".claude/settings.json")), claude);
    assert_eq!(read(&fx.home().join(".codex/hooks.json")), codex);
}

#[test]
fn hooks_install_codex_rerun_is_byte_identical() {
    let fx = Fixture::new("codexrerun");
    fx.write("home/.codex/hooks.json", r#"{"hooks":{}}"#);
    assert_eq!(skillsync(&fx, &["hooks", "install"]).code, 0);
    let first = read(&fx.home().join(".codex/hooks.json"));
    assert_eq!(skillsync(&fx, &["hooks", "install"]).code, 0);
    assert_eq!(read(&fx.home().join(".codex/hooks.json")), first);
}

#[test]
fn hooks_install_stages_all_files_before_writing() {
    let fx = Fixture::new("stage");
    fx.write("home/.claude/settings.json", r#"{"model":"opus"}"#);
    fx.write(
        "home/.codex/hooks.json",
        r#"{"hooks":{"PreToolUse":"not an array"}}"#,
    );

    let out = skillsync(&fx, &["hooks", "install"]);
    assert_eq!(out.code, 2, "{}", out.stdout);
    assert_eq!(
        read(&fx.home().join(".claude/settings.json")),
        r#"{"model":"opus"}"#
    );
}

#[test]
fn hooks_install_writes_through_a_symlinked_settings_file() {
    let fx = Fixture::new("dotfiles");
    fx.write("dotfiles/settings.json", r#"{"model":"opus"}"#);
    link(
        &fx.path("dotfiles/settings.json"),
        &fx.home().join(".claude/settings.json"),
    );

    let out = skillsync(&fx, &["hooks", "install"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    let meta = fs::symlink_metadata(fx.home().join(".claude/settings.json")).expect("meta");
    assert!(
        meta.file_type().is_symlink(),
        "the link was replaced by a file"
    );
    assert!(read(&fx.path("dotfiles/settings.json")).contains("claude-guard pr-guard"));
}

#[test]
fn skills_install_rerun_links_nothing() {
    let fx = Fixture::new("rerun");
    assert_eq!(skillsync(&fx, &["skills", "install"]).code, 0);

    let again = skillsync(&fx, &["skills", "install", "--json"]);
    assert_eq!(again.code, 0, "{}", again.stdout);
    let v: Value = serde_json::from_str(again.stdout.trim()).expect("json");
    assert_eq!(v["actions"], Value::Array(vec![]), "{}", again.stdout);
    assert_eq!(v["notices"].as_array().expect("notices").len(), 4);
}
