//! Word search over the catalog, weighted by rating.

use crate::catalog::{Catalog, Host};

/// Rating assumed for a skill Jev has not rated yet.
pub const UNRATED: f64 = 0.5;
const NAME_WEIGHT: f64 = 3.0;
const DESCRIPTION_WEIGHT: f64 = 1.0;
const MIN_PREFIX: usize = 4;

pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn prefix_match(a: &str, b: &str) -> bool {
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.chars().count() >= MIN_PREFIX && long.starts_with(short)
}

/// 1 for an exact token hit, 0.5 for a prefix hit of at least four characters.
fn hit(query: &str, doc: &[String]) -> f64 {
    if doc.iter().any(|t| t == query) {
        1.0
    } else if doc.iter().any(|t| prefix_match(query, t)) {
        0.5
    } else {
        0.0
    }
}

pub fn relevance(query: &[String], name: &str, description: &str) -> f64 {
    let mut unique: Vec<&String> = query.iter().collect();
    unique.sort();
    unique.dedup();
    if unique.is_empty() {
        return 0.0;
    }
    let name = tokens(name);
    let description = tokens(description);
    let sum: f64 = unique
        .iter()
        .map(|q| NAME_WEIGHT * hit(q, &name) + DESCRIPTION_WEIGHT * hit(q, &description))
        .sum();
    sum / unique.len() as f64
}

pub fn final_score(relevance: f64, rating: Option<f64>) -> f64 {
    relevance * (0.35 + 0.65 * rating.unwrap_or(UNRATED))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub index: usize,
    pub score: f64,
    pub relevance: f64,
}

/// Matching catalog entries, best first.
pub fn search(catalog: &Catalog, words: &[String], host: Option<Host>, limit: usize) -> Vec<Hit> {
    let query: Vec<String> = words.iter().flat_map(|w| tokens(w)).collect();
    let mut hits: Vec<Hit> = catalog
        .skills
        .iter()
        .enumerate()
        .filter(|(_, s)| host.is_none_or(|h| s.host == h))
        .filter_map(|(index, s)| {
            let relevance = relevance(&query, &s.display_name(), &s.description);
            (relevance > 0.0).then(|| Hit {
                index,
                score: final_score(relevance, catalog.rating_of(s)),
                relevance,
            })
        })
        .collect();
    let key = |h: &Hit| {
        let s = &catalog.skills[h.index];
        (s.display_name(), s.host)
    };
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| key(a).cmp(&key(b)))
    });
    hits.truncate(limit);
    hits
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
