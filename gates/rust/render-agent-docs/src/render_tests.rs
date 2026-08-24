//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;

fn manifest() -> Manifest {
    let cfg = Config::parse(
        r#"
rules = ["git-discipline", "timeouts"]
rules_dir = "~/.claude/rules"
overlay = "docs/overlay.md"

[targets.claude]
path = "CLAUDE.md"
title = "CLAUDE.md — Acme"

[targets.agents]
path = "AGENTS.md"
title = "AGENTS.md — Acme"
"#,
    )
    .expect("parses");
    Manifest::from_config(&cfg)
}

#[test]
fn reads_every_target_from_the_manifest() {
    let m = manifest();
    assert_eq!(m.targets.len(), 2);
    assert_eq!(m.targets[0].path, "AGENTS.md");
    assert_eq!(m.targets[1].path, "CLAUDE.md");
}

#[test]
fn renders_the_rules_as_import_lines_in_declared_order() {
    let m = manifest();
    let out = render(&m, &m.targets[1], None);
    let imports: Vec<&str> = out.lines().filter(|l| l.starts_with('@')).collect();
    assert_eq!(
        imports,
        vec![
            "@~/.claude/rules/git-discipline.md",
            "@~/.claude/rules/timeouts.md"
        ]
    );
}

#[test]
fn both_targets_render_the_same_body_and_differ_only_in_title() {
    // The whole point: one source, two files, no room to drift.
    let m = manifest();
    let claude = render(&m, &m.targets[1], Some("## Stack\n\nRust."));
    let agents = render(&m, &m.targets[0], Some("## Stack\n\nRust."));
    let strip_title = |s: &str| s.lines().skip(1).collect::<Vec<_>>().join("\n");
    assert_eq!(strip_title(&claude), strip_title(&agents));
    assert!(claude.starts_with("# CLAUDE.md — Acme"));
    assert!(agents.starts_with("# AGENTS.md — Acme"));
}

#[test]
fn carries_a_banner_so_a_hand_edit_is_warned_in_the_file_itself() {
    let m = manifest();
    assert!(render(&m, &m.targets[1], None).contains(BANNER));
}

#[test]
fn the_overlay_lands_after_the_rules_block() {
    let m = manifest();
    let out = render(&m, &m.targets[1], Some("## Stack\n\nRust."));
    let rules_at = out
        .find("@~/.claude/rules/timeouts.md")
        .expect("rules block");
    let overlay_at = out.find("## Stack").expect("overlay");
    assert!(rules_at < overlay_at);
}

#[test]
fn a_trailing_slash_on_rules_dir_does_not_double_up() {
    let cfg = Config::parse(
        "rules = [\"comments\"]\nrules_dir = \"rules/\"\n[targets.a]\npath = \"A.md\"\n",
    )
    .unwrap();
    let m = Manifest::from_config(&cfg);
    assert!(render(&m, &m.targets[0], None).contains("@rules/comments.md"));
}

#[test]
fn a_target_with_no_title_falls_back_to_its_path() {
    let cfg = Config::parse("[targets.a]\npath = \"A.md\"\n").unwrap();
    let m = Manifest::from_config(&cfg);
    assert_eq!(m.targets[0].title, "A.md");
}

#[test]
fn a_manifest_with_no_rules_renders_no_rules_section() {
    let cfg = Config::parse("[targets.a]\npath = \"A.md\"\n").unwrap();
    let m = Manifest::from_config(&cfg);
    assert!(!render(&m, &m.targets[0], Some("body")).contains("## Rules"));
}
