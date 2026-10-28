use crate::lint::{check, check_figure, longest_chain, Level};
use crate::story::{parse, Figure};

fn fig(code: &str) -> Figure {
    Figure {
        line: 10,
        caption: "cap".to_owned(),
        code: code.to_owned(),
    }
}

fn msgs(code: &str) -> Vec<(Level, String)> {
    check_figure(&fig(code))
        .into_iter()
        .map(|f| (f.level, f.msg))
        .collect()
}

#[test]
fn chain_counts_nodes_and_cuts_cycles() {
    assert_eq!(longest_chain(&[]), 0);
    assert_eq!(longest_chain(&[("a", "b"), ("b", "c"), ("a", "c")]), 3);
    assert_eq!(longest_chain(&[("a", "b"), ("b", "a")]), 2);
}

#[test]
fn wide_left_to_right_figure_warns() {
    let code = "scene type=state\nlayout LR\nstate A \"a\"\nstate B \"b\"\nstate C \"c\"\nstate D \"d\"\nstate E \"e\"\nbeat x:\n  A -> B\n  B -> C\n  C -> D\n  D -> E\n";
    let m = msgs(code);
    assert!(
        m.iter()
            .any(|(l, s)| *l == Level::Warn && s.contains("5 ranks")),
        "{m:?}"
    );
    let tb = code.replace("layout LR", "layout TB");
    assert!(msgs(&tb).is_empty());
}

#[test]
fn edges_and_cues_must_name_real_nodes() {
    let code = "scene type=state\nstate A \"a\"\ngroup g \"G\": A\nbeat x:\n  A -> Z\n  glow g color=#fff\n  glow Q color=#fff\n";
    let m = msgs(code);
    assert!(
        m.iter()
            .any(|(l, s)| *l == Level::Error && s.contains("`Z`")),
        "{m:?}"
    );
    assert!(
        m.iter()
            .any(|(l, s)| *l == Level::Error && s.contains("`Q`")),
        "{m:?}"
    );
    assert!(!m.iter().any(|(_, s)| s.contains("`g`")), "{m:?}");
}

#[test]
fn scene_line_script_close_and_kinds() {
    assert!(msgs("state A\n").iter().any(|(l, _)| *l == Level::Error));
    assert!(msgs("scene\nservice A \"a </b>\"\n")
        .iter()
        .any(|(_, s)| s.contains("</")));
    let mut ticked = fig("scene\n");
    ticked.caption = "Fig of `cos`".to_owned();
    assert!(check_figure(&ticked)
        .iter()
        .any(|f| f.msg.contains("backtick")));
    let m = msgs("scene type=gantt\nwidget A \"a\"\n");
    assert!(m.iter().any(|(_, s)| s.contains("gantt")));
    assert!(m.iter().any(|(_, s)| s.contains("`widget`")));
}

const STORY: &str = "---
title: T
date: 2026-10-28
---

# Lead

{{observed: x}}

```markdy Lead fig
scene type=state
start A \"a\"
```

## Story without figure

Text {{observed: y}}.

## What I have not checked {.notchecked}

- n
";

#[test]
fn every_story_needs_a_figure_and_notchecked_comes_last() {
    let s = parse(STORY).unwrap();
    let f = check(&s);
    assert!(
        f.iter()
            .any(|x| x.level == Level::Error && x.msg.contains("markdy figure")),
        "{f:?}"
    );

    let moved = STORY.replace(
        "## Story without figure",
        "## What I have not checked {.notchecked}\n\n- n\n\n## Story without figure",
    );
    let moved = moved.trim_end_matches("## What I have not checked {.notchecked}\n\n- n\n");
    let f = check(&parse(moved).unwrap());
    assert!(f.iter().any(|x| x.msg.contains("last section")), "{f:?}");
}

#[test]
fn prose_warnings_for_filler_time_and_dashes() {
    let body = "We leverage this in 3 days. ".repeat(3) + &"a — b ".repeat(20);
    let src = STORY.replace(
        "Text {{observed: y}}.",
        &format!("{body} {{{{observed: y}}}}"),
    );
    let f = check(&parse(&src).unwrap());
    let warn = |needle: &str| {
        f.iter()
            .any(|x| x.level == Level::Warn && x.msg.contains(needle))
    };
    assert!(warn("`leverage`"), "{f:?}");
    assert!(warn("time estimate"), "{f:?}");
    assert!(warn("em dashes"), "{f:?}");
}
