use super::*;
use crate::test_support::{skill, TempDir};

#[test]
fn rating_key_depends_on_name_and_description_only() {
    assert_eq!(rating_key("a", "b"), rating_key("a", "b"));
    assert_ne!(rating_key("a", "b"), rating_key("a", "c"));
    assert_ne!(
        rating_key("a\nb", ""),
        rating_key("a", "b"),
        "the separator is part of the key"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn catalog_survives_a_save_and_load() {
    let dir = TempDir::new("catalog");
    let path = dir.path().join("nested/catalog.json");
    let mut c = Catalog {
        scanned_at: 42,
        days: 60,
        skills: vec![
            skill("alpha", "Alpha.", Host::Claude, Source::User),
            skill("beta", "Beta.", Host::Codex, Source::Plugin),
        ],
        ..Catalog::default()
    };
    c.skills[0].tier = "name-only".to_owned();
    c.usage.entry(Host::Claude).or_default().insert(
        "alpha".to_owned(),
        Use {
            count: 3,
            last_seen: Some("2026-09-01T00:00:00Z".to_owned()),
        },
    );
    c.ratings.insert(
        c.skills[0].key.clone(),
        Rating {
            rating: 0.75,
            confidence: Some(0.6),
            probabilities: serde_json::json!({"3": 1.0}),
            model: "jev".to_owned(),
            profile_sha: "abc".to_owned(),
            rated_at: 7,
        },
    );
    c.managed.insert(Host::Claude, vec!["alpha".to_owned()]);
    c.save(&path).expect("save");
    assert_eq!(Catalog::load(&path).expect("load"), Some(c));
    assert_eq!(
        Catalog::load(&dir.path().join("missing.json")).expect("missing"),
        None
    );
}

#[test]
fn uses_look_up_the_name_on_claude_and_the_directory_on_codex() {
    let mut s = skill("pretty-name", "d", Host::Codex, Source::User);
    s.dir_name = "dir-name".to_owned();
    let mut c = Catalog::default();
    let one = |n| Use {
        count: n,
        last_seen: None,
    };
    c.usage
        .entry(Host::Claude)
        .or_default()
        .insert("pretty-name".to_owned(), one(2));
    c.usage
        .entry(Host::Codex)
        .or_default()
        .insert("dir-name".to_owned(), one(5));
    assert_eq!(c.uses(&s), (2, 5));
}
