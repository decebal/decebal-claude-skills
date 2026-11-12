//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn plugins() -> Plugins {
    Plugins {
        disable: vec!["noisy".into()],
        keep: vec!["busy".into()],
    }
}

#[test]
fn plugin_decisions_come_from_the_flags() {
    let p = Plugins::from_args(&args(&["--disable", "a,b", "--keep", "c"])).expect("parses");
    assert_eq!(
        p,
        Plugins {
            disable: vec!["a".into(), "b".into()],
            keep: vec!["c".into()]
        }
    );
}

#[test]
fn an_unknown_flag_or_a_missing_value_is_refused() {
    assert_eq!(Plugins::from_args(&args(&["--drop", "a"])), None);
    assert_eq!(Plugins::from_args(&args(&["--disable"])), None);
}

#[test]
fn a_plugin_with_no_decision_is_never_cut() {
    assert_eq!(
        proposal("skill", "plugin:other", 500, 0, 0, false, &plugins()),
        "plugin: no decision passed"
    );
    assert_eq!(
        proposal("skill", "plugin:noisy", 500, 9, 9, true, &plugins()),
        "disable plugin noisy"
    );
    assert_eq!(
        proposal("agent", "plugin:busy", 500, 0, 0, false, &plugins()),
        "keep (plugin in use)"
    );
}

#[test]
fn use_protection_and_history_each_keep_an_entry() {
    let p = plugins();
    assert_eq!(proposal("skill", "personal", 500, 1, 1, false, &p), "keep");
    assert_eq!(
        proposal("skill", "personal", 500, 0, 0, true, &p),
        "keep (protected)"
    );
    assert_eq!(
        proposal("skill", "personal", 500, 0, 3, false, &p),
        "keep (seasonal: used before window)"
    );
}

#[test]
fn an_unused_entry_is_proposed_by_what_it_costs() {
    let p = plugins();
    assert_eq!(
        proposal("skill", "personal", 500, 0, 0, false, &p),
        "disable-model-invocation or archive"
    );
    assert_eq!(
        proposal("skill", "personal", 20, 0, 0, false, &p),
        "archive (name-only entry)"
    );
    assert_eq!(
        proposal("agent", "personal", 20, 0, 0, false, &p),
        "archive agent file"
    );
    assert_eq!(
        proposal("mcp", "mcp-config", 20, 0, 0, false, &p),
        "disconnect server / connector"
    );
}
