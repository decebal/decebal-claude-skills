use serde_json::json;

use super::*;
use crate::test_support::{skill_md, write, TempDir};

struct Fixture {
    _dir: TempDir,
    env: Env,
}

fn fixture() -> Fixture {
    let dir = TempDir::new("cli");
    let home = dir.path().join("home");
    let h = home.as_path();
    write(
        &h.join(".claude/skills/skill-rank/SKILL.md"),
        &skill_md("skill-rank", "Find a skill."),
    );
    write(
        &h.join(".claude/skills/pdf/SKILL.md"),
        &skill_md("pdf", "Read and write PDF files."),
    );
    write(
        &h.join(".claude/skills/deploy/SKILL.md"),
        &skill_md("deploy", "Deploy the Acme web app."),
    );
    write(
        &h.join(".claude/settings.json"),
        "{\n  \"model\": \"x\",\n  \"skillOverrides\": {\n    \"mine\": \"off\"\n  }\n}\n",
    );
    write(
        &h.join(".agents/skills/pdf/SKILL.md"),
        &skill_md("pdf", "Read and write PDF files."),
    );
    write(&h.join(".codex/config.toml"), "model = \"m\"\n");
    let call = json!({
        "type": "assistant",
        "timestamp": "2026-09-20T00:00:00Z",
        "message": {"content": [{"type": "tool_use", "name": "Skill", "input": {"skill": "pdf"}}]}
    });
    write(&h.join(".claude/projects/p/s.jsonl"), &format!("{call}\n"));
    let env = Env {
        home: home.clone(),
        data_dir: dir.path().join("data"),
        now: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
    };
    Fixture { _dir: dir, env }
}

fn run_ok(env: &Env, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let out = run(&args, env).expect("command runs");
    assert_eq!(out.code, 0, "{}", out.text);
    out.text
}

#[test]
fn scan_then_list_and_search_without_ratings() {
    let f = fixture();
    let text = run_ok(&f.env, &["scan"]);
    assert!(text.contains("claude: 3 skills (user 3)"), "{text}");
    assert!(text.contains("codex: 1 skills (user 1)"), "{text}");
    assert!(text.contains("claude 1 skills, 1 uses"), "{text}");

    let listed = run_ok(&f.env, &["list", "--host", "claude"]);
    assert!(listed.contains("1/0  pdf"), "{listed}");

    let found = run_ok(&f.env, &["search", "pdf"]);
    let first = found.lines().next().expect("a hit");
    assert!(first.starts_with("2.70  --  on  claude  pdf  "), "{found}");
    assert!(first.ends_with("/.claude/skills/pdf/SKILL.md"), "{found}");
    assert_eq!(found.lines().nth(1), Some("    Read and write PDF files."));
}

#[test]
fn rate_without_a_profile_exits_two() {
    let f = fixture();
    run_ok(&f.env, &["scan"]);
    let out = run(&["rate".to_owned()], &f.env).expect("runs");
    assert_eq!(out.code, 2);
    assert!(out.text.contains("profile.md"), "{}", out.text);
}

#[test]
fn rate_dry_run_prints_the_redacted_first_request() {
    let f = fixture();
    run_ok(&f.env, &["scan"]);
    write(
        &f.env.data_dir.join("profile.md"),
        "I ship the Acme web app.\n",
    );
    write(&f.env.data_dir.join("redact.txt"), "acme\n");
    let text = run_ok(&f.env, &["rate", "--dry-run", "--batch", "2"]);
    assert!(text.starts_with("3 skills to rate in 2 requests"), "{text}");
    assert!(!text.to_lowercase().contains("acme"), "{text}");
    assert!(text.contains("[redacted]"), "{text}");
    assert!(
        !text.contains(&f.env.home.to_string_lossy().into_owned()),
        "no local paths leave the machine"
    );
}

#[test]
fn apply_claude_writes_name_only_overrides_with_a_backup() {
    let f = fixture();
    run_ok(&f.env, &["scan"]);
    let budget = "skill-rank".len() + "Find a skill.".len() + "pdf".len() + "deploy".len();
    let dry = run_ok(
        &f.env,
        &[
            "apply",
            "--host",
            "claude",
            "--budget-chars",
            &budget.to_string(),
            "--dry-run",
        ],
    );
    assert!(dry.contains("dry run: nothing written"), "{dry}");
    let settings_path = f.env.home.join(".claude/settings.json");
    let before = fs::read_to_string(&settings_path).expect("settings");

    run_ok(
        &f.env,
        &[
            "apply",
            "--host",
            "claude",
            "--budget-chars",
            &budget.to_string(),
        ],
    );
    let after: Value =
        serde_json::from_str(&fs::read_to_string(&settings_path).expect("settings")).expect("json");
    assert_eq!(
        after,
        json!({"model": "x", "skillOverrides": {"mine": "off", "deploy": "name-only", "pdf": "name-only"}})
    );
    let mut backup = settings_path.into_os_string();
    backup.push(".skill-rank.bak");
    assert_eq!(fs::read_to_string(backup).expect("backup"), before);

    let listed = run_ok(&f.env, &["list", "--host", "claude"]);
    assert!(listed.contains("name-only  claude"), "{listed}");

    run_ok(
        &f.env,
        &["apply", "--host", "claude", "--budget-chars", "100000"],
    );
    let promoted: Value = serde_json::from_str(
        &fs::read_to_string(f.env.home.join(".claude/settings.json")).expect("settings"),
    )
    .expect("json");
    assert_eq!(
        promoted,
        json!({"model": "x", "skillOverrides": {"mine": "off"}})
    );
}

#[test]
fn apply_refuses_settings_that_do_not_parse() {
    let f = fixture();
    run_ok(&f.env, &["scan"]);
    write(&f.env.home.join(".claude/settings.json"), "{ not json");
    let args: Vec<String> = ["apply", "--host", "claude"]
        .iter()
        .map(|a| (*a).to_owned())
        .collect();
    let err = run(&args, &f.env).expect_err("refused");
    assert!(err.contains("does not parse"), "{err}");
    assert_eq!(
        fs::read_to_string(f.env.home.join(".claude/settings.json")).expect("unchanged"),
        "{ not json"
    );
}

#[test]
fn apply_codex_writes_the_managed_block() {
    let f = fixture();
    run_ok(&f.env, &["scan"]);
    run_ok(&f.env, &["apply", "--host", "codex", "--budget-chars", "1"]);
    let config = fs::read_to_string(f.env.home.join(".codex/config.toml")).expect("config");
    assert_eq!(
        config,
        format!(
            "model = \"m\"\n\n{}\n[[skills.config]]\nname = \"pdf\"\nenabled = false\n{}\n",
            apply::BEGIN,
            apply::END
        )
    );
}

#[test]
fn unknown_options_and_missing_values_are_errors() {
    let f = fixture();
    let err = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        run(&args, &f.env).expect_err("error")
    };
    assert!(err(&["scan", "--nope"]).contains("unknown option --nope"));
    assert!(err(&["scan", "--days"]).contains("--days needs a value"));
    assert!(err(&["apply", "--host", "vim"]).contains("--host takes claude or codex"));
    assert!(err(&["search"]).contains("search needs words"));
}

#[cfg(unix)]
#[test]
fn a_write_keeps_the_first_original_and_the_file_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = TempDir::new("write-backup");
    let path = dir.path().join("config.toml");
    write(&path, "a = 1\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;

    write_with_backup(&path, Some("a = 1\n"), "a = 2\n").expect("first write");
    write_with_backup(&path, Some("a = 2\n"), "a = 3\n").expect("second write");

    let orig = dir.path().join("config.toml.skill-rank.orig");
    let bak = dir.path().join("config.toml.skill-rank.bak");
    assert_eq!(fs::read_to_string(&path).unwrap(), "a = 3\n");
    assert_eq!(
        fs::read_to_string(&orig).unwrap(),
        "a = 1\n",
        "the first original survives"
    );
    assert_eq!(fs::read_to_string(&bak).unwrap(), "a = 2\n");
    for p in [&path, &orig, &bak] {
        assert_eq!(mode(p), 0o600, "{}", p.display());
    }
    assert!(!dir.path().join("config.toml.skill-rank.tmp").exists());

    let err = write_with_backup(&path, Some("a = 2\n"), "a = 9\n").expect_err("changed underneath");
    assert!(
        err.contains("changed while skill-rank was planning"),
        "{err}"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "a = 3\n");
}
