use crate::render::{page, themed};
use crate::story::parse;

const STORY: &str = "---
title: The Test Edition
date: 2026-10-28
desk: Desk
strap: a | b
footer: foot `x`
---

# Lead `head`
@kicker K
@byline By me

First para.

```markdy Lead figure
scene type=state
start A \"a\"
```

## Story one {#s-one}
@short Short one

Prose {{observed: z}}.

```markdy Second
scene type=architecture
service B \"b\"
```

## What I have not checked {.notchecked}

- nothing
";

#[test]
fn page_is_self_contained_and_numbers_figures() {
    let html = page(&parse(STORY).unwrap());
    assert!(html.starts_with("<!doctype html>"));
    assert!(!html.contains("src=\"http"), "no external scripts");
    assert!(!html.contains("<link "), "no external styles");
    assert!(html.contains("window.__markdyWarnings"), "renderer inlined");
    assert!(html.contains("createDiagram"), "Markdy bundle inlined");
    assert!(html.contains("<b>Fig. 1</b>Lead figure"));
    assert!(html.contains("<b>Fig. 2</b>Second"));
    assert!(html.contains("Wednesday 28 October 2026"));
    assert!(html.contains("The Test <span>Edition</span>"));
    assert!(html.contains("<a href=\"#s-one\">Short one</a>"));
    assert!(
        !html.contains("href=\"#what-i-have-not-checked\""),
        "not-checked stays out of the index"
    );
    assert!(html.contains("<section class=\"box notchecked\""));
    assert!(html.contains("<span class=\"src\">observed: z</span>"));
    assert!(html.contains("--brand-ink:#1c2733"));
}

#[test]
fn theme_placeholder_is_added_once() {
    assert_eq!(
        themed("scene type=state\nx\n"),
        "scene theme=__THEME__ type=state\nx\n"
    );
    assert_eq!(
        themed("scene theme=paper type=state\n"),
        "scene theme=__THEME__ type=state\n"
    );
}
