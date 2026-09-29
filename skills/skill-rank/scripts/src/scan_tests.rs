use serde_json::json;

use super::*;
use crate::catalog::CLAUDE_DESCRIPTION_CAP;
use crate::test_support::{skill_md, write, TempDir};

#[test]
fn claude_plugins_reads_enabled_ids_and_prefers_the_user_install() {
    let settings = json!({"enabledPlugins": {"alpha@market": true, "beta@market": false, "gone@market": true}});
    let installed = json!({
        "version": 2,
        "plugins": {
            "alpha@market": [
                {"scope": "project", "installPath": "/plugins/alpha/project"},
                {"scope": "user", "installPath": "/plugins/alpha/1.0.0"}
            ],
            "beta@market": [{"scope": "user", "installPath": "/plugins/beta/1.0.0"}]
        }
    });
    assert_eq!(
        claude_plugins(&settings, &installed),
        vec![("alpha".to_owned(), PathBuf::from("/plugins/alpha/1.0.0"))]
    );
}

#[test]
fn codex_plugins_reads_enabled_tables_only() {
    let config = r#"
model = "x"

[plugins."on@market"]
enabled = true

[plugins."off@market"]
enabled = false

[plugins."unset@market"]
source = "x"

[mcp_servers.thing]
enabled = false
"#;
    assert_eq!(codex_plugins(config), vec!["on@market", "unset@market"]);
}

#[test]
fn discover_walks_every_root_and_marks_what_can_be_switched() {
    let home = TempDir::new("scan");
    let h = home.path();
    write(
        &h.join(".claude/skills/alpha/SKILL.md"),
        &skill_md("alpha", "Alpha work."),
    );
    write(
        &h.join(".claude/skills/no-name/SKILL.md"),
        "---\ndescription: Nameless.\n---\n",
    );
    write(
        &h.join(".claude/skills/.hidden/SKILL.md"),
        &skill_md("hidden", "Never listed."),
    );
    let plugin_dir = h.join("plugin-install/tools/1.0.0");
    write(
        &plugin_dir.join("skills/lint/SKILL.md"),
        &skill_md("lint", "Lint code."),
    );
    write(
        &h.join(".claude/settings.json"),
        &json!({"enabledPlugins": {"tools@market": true}}).to_string(),
    );
    write(
        &h.join(".claude/plugins/installed_plugins.json"),
        &json!({"version": 2, "plugins": {"tools@market": [
            {"scope": "user", "installPath": plugin_dir.to_string_lossy()}
        ]}})
        .to_string(),
    );
    write(
        &h.join(".agents/skills/alpha/SKILL.md"),
        &skill_md("alpha", "Alpha work."),
    );
    write(
        &h.join(".codex/skills/gamma/SKILL.md"),
        &skill_md("gamma", "Gamma work."),
    );
    write(
        &h.join(".codex/skills/.system/sys/SKILL.md"),
        &skill_md("sys", "System skill."),
    );
    write(
        &h.join(".codex/config.toml"),
        "[plugins.\"docs@market\"]\nenabled = true\n",
    );
    write(
        &h.join(".codex/plugins/cache/market/docs/2.0.0/skills/docs/SKILL.md"),
        &skill_md("docs", "Documents."),
    );

    let skills = discover(h);
    let seen: Vec<(Host, Source, String, bool)> = skills
        .iter()
        .map(|s| (s.host, s.source, s.display_name(), s.controllable))
        .collect();
    assert_eq!(
        seen,
        vec![
            (Host::Claude, Source::User, "alpha".to_owned(), true),
            (Host::Claude, Source::User, "no-name".to_owned(), true),
            (Host::Claude, Source::Plugin, "tools:lint".to_owned(), false),
            (Host::Codex, Source::User, "alpha".to_owned(), true),
            (Host::Codex, Source::User, "gamma".to_owned(), true),
            (Host::Codex, Source::System, "sys".to_owned(), false),
            (Host::Codex, Source::Plugin, "docs:docs".to_owned(), false),
        ]
    );
    assert_eq!(
        skills[0].key, skills[3].key,
        "identical copies share one rating key"
    );
    assert_eq!(skills[1].description, "Nameless.");
    assert_eq!(skills[2].tier, "plugin");
    assert_eq!(skills[5].tier, "system");
    assert_eq!(
        skills[2].cost,
        ("tools:lint".len() + "Lint code.".len()) as u64
    );
}

#[test]
fn claude_cost_caps_the_description_and_codex_does_not() {
    let long = "d".repeat(2000);
    let fm = Frontmatter {
        name: Some("big".to_owned()),
        description: Some(long.clone()),
    };
    let path = Path::new("/skills/big/SKILL.md");
    let claude = build_skill(
        &fm,
        "big",
        path,
        &Root::new(PathBuf::new(), Host::Claude, Source::User, None),
    );
    let codex = build_skill(
        &fm,
        "big",
        path,
        &Root::new(PathBuf::new(), Host::Codex, Source::User, None),
    );
    assert_eq!(claude.cost, (3 + CLAUDE_DESCRIPTION_CAP) as u64);
    assert_eq!(codex.cost, (3 + long.len()) as u64);
}

#[test]
fn a_symlinked_copy_in_a_second_root_is_listed_once() {
    let home = TempDir::new("scan-link");
    let h = home.path();
    write(
        &h.join(".codex/skills/shared/SKILL.md"),
        &skill_md("shared", "Shared."),
    );
    fs::create_dir_all(h.join(".agents/skills")).expect("mkdir");
    std::os::unix::fs::symlink(
        h.join(".codex/skills/shared"),
        h.join(".agents/skills/shared"),
    )
    .expect("symlink");
    let names: Vec<String> = discover(h).into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["shared"]);
}
