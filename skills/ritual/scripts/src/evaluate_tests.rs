use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::cases::{self, Case};
use crate::evaluate::{self, Frozen, GATES, OBJECTIVE, Score};
use crate::feedback::{Check, Skill, Step};

const RITUAL: &str = "---\nname: ritual\ndescription: The flow.\n---\n\n# ritual\n\nSee [build](references/build.md) and [full](../ritual-full/SKILL.md#preflight).\n";
const FULL: &str = "---\nname: ritual-full\ndescription: Every tool on.\n---\n\n# ritual-full\n\nRead [ritual](../ritual/SKILL.md) and <https://example.com>.\n";

fn skills_tree() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let write = |rel: &str, body: &str| {
        let path = root.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    };
    write("skills/ritual/SKILL.md", RITUAL);
    write(
        "skills/ritual/references/build.md",
        "Close beads with a reason.\n",
    );
    write("skills/ritual-full/SKILL.md", FULL);
    write("skills/ritual/scripts/README.md", &"source ".repeat(500));
    write(
        "skills/ritual/scripts/invariants.txt",
        "# kept\nritual/references/build.md: close beads\n\nritual-full/SKILL.md: Every tool on\n",
    );
    root
}

fn case(id: &str, must: &str) -> Case {
    Case {
        id: id.into(),
        recorded_at: String::new(),
        skill: Skill::Ritual,
        step: Step::Build,
        reference: "references/build.md".into(),
        check: Check {
            must: Some(must.into()),
            must_not: None,
        },
        note: String::new(),
    }
}

fn frozen(root: &Path, holdout_max_open: usize) -> Frozen {
    Frozen {
        dev: root.join("unused-dev"),
        holdout: root.join("unused-holdout"),
        holdout_sha256: String::new(),
        holdout_max_open,
        max_words: evaluate::total_words(root),
    }
}

#[test]
fn the_baseline_tree_passes_every_gate() {
    let root = skills_tree();
    let dev = [case("a", "with a reason"), case("b", "run the tests")];
    let score = evaluate::score(root.path(), &dev, &[], &frozen(root.path(), 0), true);
    assert_eq!(
        score,
        Score {
            open_dev: 1,
            total_words: evaluate::total_words(root.path()),
            holdout_not_regressed: true,
            word_budget: true,
            invariants_kept: true,
            links_resolve: true,
            frontmatter_valid: true,
        }
    );
}

#[test]
fn deleting_an_invariant_rule_fails_invariants_kept() {
    let root = skills_tree();
    let build = root.path().join("skills/ritual/references/build.md");
    fs::write(&build, "Beads need a reason.\n").unwrap();
    assert!(!evaluate::invariants_kept(root.path()));
}

#[test]
fn a_missing_or_malformed_invariants_file_fails_closed() {
    let root = skills_tree();
    let list = root.path().join(evaluate::INVARIANTS);
    fs::write(&list, "ritual/SKILL.md no separator\n").unwrap();
    assert!(!evaluate::invariants_kept(root.path()));
    fs::write(&list, "../escape.md: anything\n").unwrap();
    assert!(!evaluate::invariants_kept(root.path()));
    fs::remove_file(&list).unwrap();
    assert!(!evaluate::invariants_kept(root.path()));
}

#[test]
fn the_shipped_invariants_hold_on_the_shipped_skills() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    assert!(evaluate::invariants(&repo).is_some_and(|l| l.len() >= 10));
    assert!(evaluate::invariants_kept(&repo));
    assert!(evaluate::links_resolve(&repo));
    assert!(evaluate::frontmatter_valid(&repo));
}

#[test]
fn the_evaluators_own_source_is_not_counted_as_prose() {
    let root = skills_tree();
    assert!(evaluate::total_words(root.path()) < 100);
}

#[test]
fn growing_the_text_fails_the_word_budget() {
    let root = skills_tree();
    let f = frozen(root.path(), 0);
    let build = root.path().join("skills/ritual/references/build.md");
    fs::write(
        &build,
        "Close beads with a reason. Then run the tests, always, every time.\n",
    )
    .unwrap();
    let score = evaluate::score(root.path(), &[case("b", "run the tests")], &[], &f, true);
    assert_eq!(score.open_dev, 0);
    assert!(!score.word_budget);
}

#[test]
fn a_broken_relative_link_fails_links_resolve() {
    let root = skills_tree();
    let skill = root.path().join("skills/ritual/SKILL.md");
    fs::write(
        &skill,
        RITUAL.replace("references/build.md", "references/gone.md"),
    )
    .unwrap();
    assert!(!evaluate::links_resolve(root.path()));
}

#[test]
fn a_renamed_skill_fails_frontmatter_valid() {
    let root = skills_tree();
    let skill = root.path().join("skills/ritual-full/SKILL.md");
    fs::write(&skill, FULL.replace("name: ritual-full", "name: other")).unwrap();
    assert!(!evaluate::frontmatter_valid(root.path()));
}

#[test]
fn reopening_a_held_out_case_fails_the_holdout_gate() {
    let root = skills_tree();
    let holdout = [case("h", "with a reason")];
    let f = frozen(root.path(), 0);
    assert!(evaluate::score(root.path(), &[], &holdout, &f, true).holdout_not_regressed);
    let build = root.path().join("skills/ritual/references/build.md");
    fs::write(&build, "Close beads.\n").unwrap();
    assert!(!evaluate::score(root.path(), &[], &holdout, &f, true).holdout_not_regressed);
}

#[test]
fn a_tampered_holdout_fails_the_holdout_gate() {
    let root = skills_tree();
    let f = frozen(root.path(), 10);
    assert!(!evaluate::score(root.path(), &[], &[], &f, false).holdout_not_regressed);
}

#[test]
fn the_success_line_declares_every_gate_and_metric_with_null_details() {
    let root = skills_tree();
    let score = evaluate::score(
        root.path(),
        &[case("a", "missing phrase")],
        &[],
        &frozen(root.path(), 0),
        true,
    );
    let line = evaluate::success_line(["ev", "run", "base", "cand"], &score);
    let v: Value = serde_json::from_str(&line).unwrap();
    let output = &v["result"]["output"];
    assert_eq!(output["evaluator_id"], "ev");
    let measurements = output["measurements"].as_array().unwrap();
    let names: Vec<&str> = measurements
        .iter()
        .map(|m| m["name"].as_str().unwrap())
        .collect();
    for gate in GATES {
        assert!(names.contains(&gate), "{gate} missing");
    }
    let objective = measurements
        .iter()
        .find(|m| m["name"] == OBJECTIVE)
        .unwrap();
    assert_eq!(objective["value"], 1.0);
    assert!(
        measurements
            .iter()
            .filter(|m| m["kind"] == "hard_gate")
            .all(|m| m["outcome"]["detail"].is_null())
    );
}

#[test]
fn relative_links_skip_external_and_anchor_only_targets() {
    let links =
        evaluate::relative_links("[a](b.md#x) [c](https://d) [e](#f) [g](mailto:h) [i](../j.md)");
    assert_eq!(links, ["b.md", "../j.md"]);
}

#[test]
fn open_cases_counts_only_unmet_checks() {
    let root = skills_tree();
    let dev = [
        case("a", "with a reason"),
        case("b", "nope"),
        case("c", "close beads"),
    ];
    assert_eq!(cases::open_cases(root.path(), &dev).len(), 1);
}
