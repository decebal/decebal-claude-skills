use crate::story::{figures, long_date, parse, slug, Kind};

const MIN: &str = "---
title: The Test Edition
date: 2026-10-28
strap: one | two
---

# Lead headline
@kicker Recommendation
@deck A deck.

Lead text.

```markdy Fig caption
scene type=state
start A \"a\"
```

## Second story {#s-two}
@kicker Desk
@short Two

Body.

## Questions {.back}

- q

## What I have not checked {.notchecked}

- nothing
";

#[test]
fn parses_front_lead_stories_and_back_page() {
    let s = parse(MIN).expect("valid story");
    assert_eq!(s.front.title, "The Test Edition");
    assert_eq!(s.front.strap, vec!["one", "two"]);
    let kinds: Vec<Kind> = s.sections.iter().map(|x| x.kind).collect();
    assert_eq!(
        kinds,
        [Kind::Lead, Kind::Story, Kind::Back, Kind::NotChecked]
    );
    let lead = &s.sections[0];
    assert_eq!(lead.kicker.as_deref(), Some("Recommendation"));
    assert_eq!(lead.id, "lead-headline");
    assert_eq!(s.sections[1].id, "s-two");
    assert_eq!(s.sections[1].short.as_deref(), Some("Two"));
}

#[test]
fn figure_lines_point_into_the_file() {
    let s = parse(MIN).unwrap();
    let figs = figures(&s.sections[0]);
    assert_eq!(figs.len(), 1);
    assert_eq!(figs[0].caption, "Fig caption");
    assert_eq!(
        MIN.lines().nth(figs[0].line - 1),
        Some("```markdy Fig caption")
    );
    assert!(figs[0].code.starts_with("scene type=state"));
}

#[test]
fn heading_inside_a_fence_is_not_a_section() {
    let src = MIN.replace("Body.", "```text\n## not a heading\n```");
    let s = parse(&src).unwrap();
    assert_eq!(s.sections.len(), 4);
}

#[test]
fn rejects_unknown_keys_directives_and_attributes() {
    let src = MIN
        .replace("strap:", "colour: red\nstrap:")
        .replace("@short Two", "@shrot Two")
        .replace("{.back}", "{.side}");
    let errs = parse(&src).unwrap_err();
    let msgs: Vec<&str> = errs.iter().map(|p| p.msg.as_str()).collect();
    assert!(msgs.iter().any(|m| m.contains("`colour`")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("`@shrot`")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("`.side`")), "{msgs:?}");
}

#[test]
fn needs_frontmatter_date_and_one_lead() {
    assert!(parse("# Lead\n").is_err());
    let no_date = MIN.replace("date: 2026-10-28", "date: 28/10/2026");
    assert!(parse(&no_date).is_err());
    let two_leads = MIN.replace("## Second story", "# Second story");
    assert!(parse(&two_leads).is_err());
}

#[test]
fn long_date_names_the_weekday() {
    assert_eq!(long_date("2026-10-28"), "Wednesday 28 October 2026");
    assert_eq!(long_date("2024-02-29"), "Thursday 29 February 2024");
    assert_eq!(long_date("2000-01-01"), "Saturday 1 January 2000");
}

#[test]
fn slug_is_lowercase_and_dashed() {
    assert_eq!(
        slug("gstack, translated: 4 gaps!"),
        "gstack-translated-4-gaps"
    );
}
