use crate::brand::BRAND;
use crate::render::page;
use crate::story::parse;

const CSS: &str = include_str!("../assets/press.css");

/// Another company's name and colours; none may reach a page. Split with
/// `concat!` so `git grep` for them across the repo stays empty.
const FOREIGN: [&str; 4] = [
    concat!("#0230", "47"),
    concat!("#ff83", "00"),
    concat!("#feb3", "06"),
    concat!("ipr", "oov"),
];

fn channel(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.039_28 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(hex: &str) -> f64 {
    let hex = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
    0.2126 * channel(byte(0)) + 0.7152 * channel(byte(2)) + 0.0722 * channel(byte(4))
}

/// WCAG 2.x contrast ratio.
fn contrast(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The `--paper` values in press.css: light first, then the dark override.
fn papers() -> (&'static str, &'static str) {
    let mut it = CSS
        .match_indices("--paper:")
        .map(|(i, k)| &CSS[i + k.len()..i + k.len() + 7]);
    (it.next().unwrap(), it.next().unwrap())
}

#[test]
fn brand_colours_meet_wcag_aa_where_they_carry_text() {
    let (light, dark) = papers();
    let pairs = [
        ("ink on paper", BRAND.ink, light),
        ("accent on paper", BRAND.accent, light),
        ("accent-dark on dark paper", BRAND.accent_dark, dark),
        ("on-accent on accent", BRAND.on_accent, BRAND.accent),
        ("dark paper on accent-dark", dark, BRAND.accent_dark),
        ("highlight on dark paper", BRAND.highlight, dark),
        ("sand label on highlight", "#14212b", BRAND.highlight),
        ("white on ink", "#ffffff", BRAND.ink),
    ];
    for (what, fg, bg) in pairs {
        let ratio = contrast(fg, bg);
        println!("{what}: {fg} on {bg} = {ratio:.2}");
        assert!(
            ratio >= 4.5,
            "{what}: {fg} on {bg} is {ratio:.2}, below 4.5"
        );
    }
}

#[test]
fn page_prints_in_wolven_and_nothing_foreign() {
    let html = page(
        &parse("---\ntitle: Brand Check\ndate: 2026-11-12\n---\n\n# Lead\n\nText.\n").unwrap(),
    );
    for var in [
        format!("--brand-ink:{}", BRAND.ink),
        format!("--brand-accent:{}", BRAND.accent),
        format!("--brand-accent-dark:{}", BRAND.accent_dark),
        format!("--brand-on-accent:{}", BRAND.on_accent),
        format!("--brand-highlight:{}", BRAND.highlight),
    ] {
        assert!(html.contains(&var), "missing {var}");
    }
    assert!(
        html.contains("aria-label=\"Wolven Tech\""),
        "mark in masthead"
    );
    assert!(html.contains("<span class=\"wordmark\">Wolven Tech</span>"));
    let lower = html.to_lowercase();
    for bad in FOREIGN {
        assert!(!lower.contains(bad), "page carries {bad}");
    }
}
