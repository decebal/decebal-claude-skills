use crate::markup::{blocks, inline, inline_marks, plain, Block};

#[test]
fn source_labels_and_tags_become_spans() {
    let out = inline_marks("x {{observed: `ls`}} and [[gap]] [[nope]]\n");
    assert_eq!(
        out,
        "x <span class=\"src\">observed: `ls`</span> and <span class=\"tag gap\">gap</span> [[nope]]\n"
    );
}

#[test]
fn marks_inside_code_are_left_alone() {
    let md = "`{{x}}` [[gap]]\n```\n{{y}}\n```\n";
    let out = inline_marks(md);
    assert!(out.contains("`{{x}}`"));
    assert!(out.contains("\n{{y}}\n"));
    assert!(out.contains("tag gap"));
}

#[test]
fn prose_runs_merge_and_special_blocks_stand_alone() {
    let md = "One.\n\n- a\n\n> pulled\n\n> [!NOTE]\n> limit\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```markdy Cap *x*\nscene\n```\n\nTwo.\n";
    let b = blocks(md);
    assert!(
        matches!(&b[0], Block::Prose(h) if h.contains("<p>One.</p>") && h.contains("<li>a</li>"))
    );
    assert!(matches!(&b[1], Block::Pull(h) if h.contains("pulled")));
    assert!(matches!(&b[2], Block::Note(h) if h.contains("limit") && !h.contains("NOTE")));
    assert!(matches!(&b[3], Block::Table(h) if h.contains("<table>")));
    assert!(
        matches!(&b[4], Block::Figure { caption, code } if caption == "Cap *x*" && code == "scene\n")
    );
    assert!(matches!(&b[5], Block::Prose(h) if h.contains("Two.")));
    assert_eq!(b.len(), 6);
}

#[test]
fn plain_strips_markup_and_escapes() {
    assert_eq!(
        plain("Fig of `cos` <b> & \"x\""),
        "Fig of cos  &amp; &quot;x&quot;"
    );
}

#[test]
fn inline_has_no_paragraph_wrapper() {
    assert_eq!(inline("a `b`"), "a <code>b</code>");
}
