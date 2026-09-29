use serde_json::Value;

use super::*;
use crate::catalog::{Rating, Source};
use crate::test_support::skill;

fn q(words: &str) -> Vec<String> {
    tokens(words)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn tokens_are_lowercase_alphanumeric_runs() {
    assert_eq!(
        tokens("PDF-Tools: read_PDFs, v2!"),
        ["pdf", "tools", "read", "pdfs", "v2"]
    );
}

#[test]
fn name_hits_weigh_three_and_description_hits_one() {
    assert!(close(
        relevance(&q("pdf"), "pdf-tools", "Other things."),
        3.0
    ));
    assert!(close(relevance(&q("pdf"), "docs", "Read a pdf."), 1.0));
    assert!(close(relevance(&q("pdf"), "pdf", "Read a pdf."), 4.0));
    assert!(close(relevance(&q("zzz"), "pdf", "Read a pdf."), 0.0));
}

#[test]
fn prefixes_of_four_or_more_characters_count_half() {
    assert!(close(
        relevance(&q("test"), "testing", "Write tests."),
        1.5 + 0.5
    ));
    assert!(
        close(relevance(&q("testing"), "test", ""), 1.5),
        "either side may be the prefix"
    );
    assert!(
        close(relevance(&q("tes"), "testing", "tests"), 0.0),
        "three characters are too short"
    );
}

#[test]
fn relevance_is_normalised_by_query_length() {
    assert!(close(
        relevance(&q("pdf merge"), "pdf", "Merge files."),
        2.0
    ));
    assert!(
        close(relevance(&q("pdf pdf"), "pdf", ""), 3.0),
        "repeated words count once"
    );
}

#[test]
fn final_score_blends_relevance_with_rating() {
    assert!(close(final_score(2.0, Some(1.0)), 2.0));
    assert!(close(final_score(2.0, Some(0.0)), 0.7));
    assert!(close(final_score(2.0, None), 2.0 * (0.35 + 0.65 * 0.5)));
}

fn rated(c: &mut Catalog, index: usize, rating: f64) {
    c.ratings.insert(
        c.skills[index].key.clone(),
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

#[test]
fn search_orders_by_final_score_filters_host_and_limits() {
    let mut c = Catalog {
        skills: vec![
            skill("deploy-web", "Deploy a site.", Host::Claude, Source::User),
            skill("deploy-api", "Deploy an API.", Host::Claude, Source::User),
            skill("deploy-api", "Deploy an API.", Host::Codex, Source::User),
            skill("notes", "Unrelated.", Host::Codex, Source::User),
            skill("ship", "Deploy then announce.", Host::Codex, Source::Plugin),
        ],
        ..Catalog::default()
    };
    rated(&mut c, 0, 0.1);
    rated(&mut c, 1, 0.9);

    let words = ["deploy".to_owned()];
    let order: Vec<(String, Host)> = search(&c, &words, None, 10)
        .iter()
        .map(|h| (c.skills[h.index].display_name(), c.skills[h.index].host))
        .collect();
    assert_eq!(
        order,
        [
            ("deploy-api".to_owned(), Host::Claude),
            ("deploy-api".to_owned(), Host::Codex),
            ("deploy-web".to_owned(), Host::Claude),
            ("plug:ship".to_owned(), Host::Codex),
        ]
    );

    let codex: Vec<usize> = search(&c, &words, Some(Host::Codex), 1)
        .iter()
        .map(|h| h.index)
        .collect();
    assert_eq!(codex, [2]);
    assert!(search(&c, &["nothing".to_owned()], None, 8).is_empty());
}
