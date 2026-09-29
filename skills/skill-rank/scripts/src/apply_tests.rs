use serde_json::json;

use super::*;
use crate::catalog::{Rating, Use};
use crate::test_support::skill;

fn rate(c: &mut Catalog, name: &str, rating: f64) {
    for s in c.skills.iter().filter(|s| s.name == name) {
        c.ratings.insert(
            s.key.clone(),
            Rating {
                rating,
                confidence: None,
                probabilities: Value::Null,
                model: String::new(),
                profile_sha: String::new(),
                rated_at: 0,
            },
        );
    }
}

fn used(c: &mut Catalog, host: Host, name: &str, count: u64) {
    c.usage.entry(host).or_default().insert(
        name.to_owned(),
        Use {
            count,
            last_seen: None,
        },
    );
}

fn tiers(plan: &Plan) -> Vec<(&str, &'static str)> {
    plan.rows
        .iter()
        .map(|r| (r.name.as_str(), r.tier.as_str()))
        .collect()
}

/// Every description is 10 chars, so a claude user skill costs its name plus 10.
fn catalog(host: Host) -> Catalog {
    let d = "0123456789";
    let mut c = Catalog {
        skills: vec![
            skill("aaaa", d, host, Source::User),
            skill("bbbb", d, host, Source::User),
            skill("cccc", d, host, Source::User),
            skill("dddd", d, host, Source::User),
            skill(PINNED, d, host, Source::User),
            skill("plug", d, host, Source::Plugin),
            skill("sys", d, host, Source::System),
        ],
        ..Catalog::default()
    };
    rate(&mut c, "aaaa", 0.2);
    rate(&mut c, "bbbb", 0.9);
    rate(&mut c, "cccc", 0.9);
    used(&mut c, host, "cccc", 5);
    c
}

#[test]
fn budget_is_a_share_of_the_context_window() {
    assert_eq!(budget_chars(Host::Claude, None, 200_000), 8_000);
    assert_eq!(budget_chars(Host::Codex, None, 200_000), 16_000);
    assert_eq!(budget_chars(Host::Codex, Some(123), 200_000), 123);
}

#[test]
fn codex_plan_charges_fixed_entries_then_keeps_the_best_rated_prefix() {
    let c = catalog(Host::Codex);
    // fixed: plug:plug (9 + 10) + sys (3 + 10) = 32; skill-rank 20; cccc 14; bbbb 14.
    let plan = plan(&c, Host::Codex, 32 + 20 + 14 + 14 + 5, &BTreeMap::new());
    assert_eq!(plan.fixed_cost, 32);
    assert_eq!(plan.reserved, 0);
    assert_eq!(
        tiers(&plan),
        [
            (PINNED, "on"),
            ("cccc", "on"),
            ("bbbb", "on"),
            ("aaaa", "disabled"),
            ("dddd", "disabled"),
        ],
        "pinned first, ties on rating broken by use, unrated after rated"
    );
    assert_eq!(plan.total, 80);
    assert_eq!(plan.demoted(), ["aaaa", "dddd"]);
    let fixed: Vec<(&str, Tier)> = plan
        .fixed
        .iter()
        .map(|r| (r.name.as_str(), r.tier))
        .collect();
    assert_eq!(fixed, [("plug:plug", Tier::Plugin), ("sys", Tier::System)]);
}

#[test]
fn the_pinned_skill_stays_on_over_budget() {
    let c = catalog(Host::Codex);
    let plan = plan(&c, Host::Codex, 10, &BTreeMap::new());
    assert_eq!(plan.tier_of(PINNED), Some(Tier::On));
    assert!(plan
        .rows
        .iter()
        .filter(|r| r.name != PINNED)
        .all(|r| r.tier == Tier::Disabled));
    assert!(plan.total > plan.budget);
}

#[test]
fn a_skill_that_does_not_fit_ends_the_on_list() {
    let mut c = catalog(Host::Codex);
    c.skills[1].cost = 1_000;
    let plan = plan(&c, Host::Codex, 32 + 20 + 14 + 14 + 14, &BTreeMap::new());
    assert_eq!(plan.tier_of("cccc"), Some(Tier::On));
    assert_eq!(plan.tier_of("bbbb"), Some(Tier::Disabled));
    assert_eq!(
        plan.tier_of("aaaa"),
        Some(Tier::Disabled),
        "a lower-rated skill never jumps the queue"
    );
}

#[test]
fn claude_reserves_every_controllable_name_before_descriptions() {
    let c = catalog(Host::Claude);
    let names = 4 * 4 + PINNED.len() as u64;
    let budget = 32 + names + 10 + 10;
    let plan = plan(&c, Host::Claude, budget, &BTreeMap::new());
    assert_eq!(plan.reserved, names);
    assert_eq!(
        tiers(&plan)[..3],
        [(PINNED, "on"), ("cccc", "on"), ("bbbb", "name-only")]
    );
    assert_eq!(plan.total, budget);
}

#[test]
fn user_tiers_are_charged_but_never_changed() {
    let c = catalog(Host::Claude);
    let mut user = BTreeMap::new();
    user.insert("dddd".to_owned(), Tier::On);
    user.insert("cccc".to_owned(), Tier::Disabled);
    let plan = plan(&c, Host::Claude, 1_000, &user);
    let dddd = plan.rows.iter().find(|r| r.name == "dddd").expect("dddd");
    assert!(dddd.user_set);
    assert_eq!(dddd.tier, Tier::On);
    assert_eq!(plan.tier_of("cccc"), Some(Tier::Disabled));
    assert_eq!(
        plan.reserved,
        4 * 3 + PINNED.len() as u64,
        "a disabled skill lists no name"
    );
    assert!(!plan.demoted().contains(&"cccc".to_owned()));
}

fn claude_plan(demote: &[&str], keep: &[&str]) -> Plan {
    let row = |name: &&str, tier| Row {
        name: (*name).to_owned(),
        rating: None,
        uses: 0,
        cost: 0,
        tier,
        running: 0,
        user_set: false,
    };
    Plan {
        host: Host::Claude,
        budget: 0,
        fixed_cost: 0,
        reserved: 0,
        total: 0,
        fixed: Vec::new(),
        rows: demote
            .iter()
            .map(|n| row(n, Tier::NameOnly))
            .chain(keep.iter().map(|n| row(n, Tier::On)))
            .collect(),
    }
}

#[test]
fn settings_merge_keeps_user_entries_other_keys_and_their_order() {
    let settings: Value = serde_json::from_str(
        r#"{"env": {"B": "1", "A": "2"}, "skillOverrides": {"mine": "off", "old": "name-only", "back": "name-only"}, "zeta": true, "alpha": [1]}"#,
    )
    .expect("json");
    let managed = vec![
        "back".to_owned(),
        "old".to_owned(),
        "uninstalled".to_owned(),
    ];
    let user = claude_user_overrides(&settings, &managed);
    assert_eq!(user.keys().collect::<Vec<_>>(), ["mine"]);

    let plan = claude_plan(&["old", "new"], &["back", "mine"]);
    let merged = merge_claude(settings, &plan, &managed).expect("merge");
    assert!(merged.changed);
    assert_eq!(
        settings_text(&merged.settings).expect("text"),
        "{\n  \"env\": {\n    \"B\": \"1\",\n    \"A\": \"2\"\n  },\n  \"skillOverrides\": {\n    \"mine\": \"off\",\n    \"old\": \"name-only\",\n    \"new\": \"name-only\"\n  },\n  \"zeta\": true,\n  \"alpha\": [\n    1\n  ]\n}\n"
    );
    assert_eq!(merged.managed, ["new", "old"]);
}

#[test]
fn a_user_edit_to_a_managed_entry_is_respected() {
    let settings = json!({"skillOverrides": {"tool": "off"}});
    let managed = vec!["tool".to_owned()];
    let user = claude_user_overrides(&settings, &managed);
    assert_eq!(user.get("tool"), Some(&Tier::Disabled));
    let mut plan = claude_plan(&[], &["tool"]);
    plan.rows[0].user_set = true;
    plan.rows[0].tier = Tier::Disabled;
    let merged = merge_claude(settings.clone(), &plan, &managed).expect("merge");
    assert_eq!(merged.settings, settings);
    assert!(merged.managed.is_empty(), "the entry is the user's now");
}

#[test]
fn null_overrides_are_filled_in_place_and_absent_ones_stay_absent() {
    let settings: Value =
        serde_json::from_str(r#"{"a": 1, "skillOverrides": null, "z": 2}"#).expect("json");
    let merged = merge_claude(settings, &claude_plan(&["x"], &[]), &[]).expect("merge");
    let keys: Vec<&String> = merged
        .settings
        .as_object()
        .expect("object")
        .keys()
        .collect();
    assert_eq!(keys, ["a", "skillOverrides", "z"]);
    assert_eq!(merged.settings["skillOverrides"], json!({"x": "name-only"}));

    let merged = merge_claude(json!({"a": 1}), &claude_plan(&[], &["x"]), &[]).expect("merge");
    assert!(!merged.changed);
    assert_eq!(merged.settings, json!({"a": 1}));
}

#[test]
fn settings_that_are_not_an_object_are_refused() {
    assert!(merge_claude(json!([1, 2]), &claude_plan(&["x"], &[]), &[]).is_err());
}

#[test]
fn codex_block_is_appended_after_a_blank_line() {
    let config = "model = \"gpt\"\n\n[features]\nx = true";
    let out = merge_codex(config, &["alpha".to_owned(), "we\"ird".to_owned()]).expect("merge");
    assert_eq!(
        out,
        format!(
            "model = \"gpt\"\n\n[features]\nx = true\n\n{BEGIN}\n[[skills.config]]\nname = \"alpha\"\nenabled = false\n\n\
             [[skills.config]]\nname = \"we\\\"ird\"\nenabled = false\n{END}\n"
        )
    );
}

#[test]
fn codex_block_is_replaced_in_place_and_removed_when_empty() {
    let config = format!("a = 1\n\n{BEGIN}\n[[skills.config]]\nname = \"old\"\nenabled = false\n{END}\n\n[tail]\nb = 2\n");
    let out = merge_codex(&config, &["new".to_owned()]).expect("merge");
    assert_eq!(
        out,
        format!("a = 1\n\n{BEGIN}\n[[skills.config]]\nname = \"new\"\nenabled = false\n{END}\n\n[tail]\nb = 2\n")
    );
    assert_eq!(
        merge_codex(&out, &["new".to_owned()]).expect("again"),
        out,
        "re-running changes nothing"
    );
    let cleared = merge_codex(&out, &[]).expect("clear");
    assert_eq!(cleared, "a = 1\n\n\n[tail]\nb = 2\n");
    assert_eq!(merge_codex("", &[]).expect("empty"), "");
}

#[test]
fn codex_refuses_foreign_skill_config_entries() {
    let config = format!(
        "[[skills.config]]\nname = \"mine\"\nenabled = false\n\n{BEGIN}\n[[skills.config]]\nname = \"ours\"\nenabled = false\n{END}\n"
    );
    assert_eq!(
        foreign_skill_config(&config),
        ["  line 1: [[skills.config]]"]
    );
    let err = merge_codex(&config, &["x".to_owned()]).expect_err("refused");
    assert!(err.contains("line 1: [[skills.config]]"), "{err}");

    let unterminated = format!("{BEGIN}\n[[skills.config]]\nname = \"x\"\n");
    assert!(merge_codex(&unterminated, &[]).is_err());
}

#[test]
fn codex_refuses_inline_and_dotted_skill_config() {
    for config in [
        "[skills]\nconfig = [{ name = \"mine\", enabled = false }]\n",
        "skills.config = [{ name = \"mine\", enabled = false }]\n",
    ] {
        assert!(!foreign_skill_config(config).is_empty(), "{config}");
        assert!(merge_codex(config, &["x".to_owned()]).is_err(), "{config}");
    }
}

#[test]
fn codex_refuses_a_file_that_does_not_parse() {
    let err = merge_codex("model = \n[broken", &["x".to_owned()]).expect_err("refused");
    assert!(err.contains("does not parse"), "{err}");
}

#[test]
fn codex_refuses_to_drop_a_line_it_did_not_write() {
    let config = format!(
        "a = 1\n\n{BEGIN}\n[[skills.config]]\nname = \"old\"\nenabled = false\n\n[notice]\nkeep = true\n{END}\n"
    );
    let err = merge_codex(&config, &["new".to_owned()]).expect_err("refused");
    assert!(err.contains("[notice]"), "{err}");
}

#[test]
fn codex_refuses_a_key_that_would_join_the_last_entry() {
    let config =
        format!("{BEGIN}\n[[skills.config]]\nname = \"old\"\nenabled = false\n{END}\nstray = 1\n");
    let err = merge_codex(&config, &["new".to_owned()]).expect_err("refused");
    assert!(err.contains("table header"), "{err}");
}

#[test]
fn codex_budget_comes_from_max_context_tokens_capped() {
    assert_eq!(
        codex_max_context_tokens("[skills]\nmax_context_tokens = 6000\n"),
        Some(6000)
    );
    assert_eq!(
        codex_max_context_tokens("[skills]\nmax_context_tokens = 50000\n"),
        Some(CODEX_MAX_CONTEXT_TOKENS)
    );
    assert_eq!(codex_max_context_tokens("model = \"x\"\n"), None);
}

#[test]
fn a_codex_user_skill_sharing_a_system_name_stays_on() {
    let d = "0123456789";
    let mut c = Catalog {
        skills: vec![
            skill("alpha", d, Host::Codex, Source::User),
            skill("imagegen", d, Host::Codex, Source::User),
            skill("imagegen", d, Host::Codex, Source::System),
        ],
        ..Catalog::default()
    };
    rate(&mut c, "alpha", 0.9);
    let p = plan(&c, Host::Codex, 1, &BTreeMap::new());
    assert_eq!(tiers(&p), [("alpha", "disabled"), ("imagegen", "on")]);
    assert!(!p.demoted().contains(&"imagegen".to_owned()));
}

#[test]
fn a_rating_for_another_profile_reads_as_missing() {
    let mut c = catalog(Host::Codex);
    let bbbb = c.skills.iter().find(|s| s.name == "bbbb").unwrap().clone();
    assert_eq!(c.rating_of(&bbbb), Some(0.9));
    c.current_profile = Some("another".to_owned());
    assert_eq!(c.rating_of(&bbbb), None);
    assert!(c.stale_ratings() > 0);
}
