//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;
use serde_json::json;

fn slash() -> &'static [&'static str] {
    markers_for("a.ts").unwrap()
}

#[test]
fn picks_the_marker_set_by_extension() {
    assert_eq!(markers_for("a.rs"), markers_for("a.ts"));
    assert_eq!(markers_for("a.py"), Some(&["#"][..]));
    assert_eq!(markers_for("a.pug"), Some(&["//-", "//"][..]));
    // `#` opens a comment in shell but names a private field in TypeScript,
    // which is the whole reason the sets differ.
    assert!(!markers_for("a.ts").unwrap().contains(&"#"));
}

#[test]
fn a_file_type_with_no_marker_set_is_out_of_scope() {
    assert_eq!(markers_for("a.md"), None);
    assert_eq!(markers_for("LICENSE"), None);
}

#[test]
fn flags_a_line_leading_comment() {
    let found = comment_lines("// set up the listener\nconst a = 1", slash());
    assert_eq!(found, vec!["// set up the listener"]);
}

#[test]
fn a_marker_inside_a_line_is_not_a_comment() {
    let found = comment_lines("const u = \"https://x.dev\"", slash());
    assert!(found.is_empty());
}

#[test]
fn doc_comments_are_api_surface_and_are_left_alone() {
    let src = "/// doc\n//! module doc\n/** jsdoc */\n#!/usr/bin/env bash\n// narration";
    assert_eq!(comment_lines(src, slash()), vec!["// narration"]);
}

#[test]
fn indented_comments_still_count() {
    let found = comment_lines("    // indented", slash());
    assert_eq!(found.len(), 1);
}

#[test]
fn the_line_count_is_capped() {
    let src = "// x\n".repeat(60);
    assert_eq!(comment_lines(&src, slash()).len(), MAX_LINES);
}

#[test]
fn collects_every_shape_of_added_text() {
    let payload = json!({
        "tool_input": {
            "new_string": "// a",
            "content": "// b",
            "edits": [{"new_string": "// c"}, {"new_string": "// d"}]
        }
    });
    let text = added_text(&payload);
    assert_eq!(comment_lines(&text, slash()).len(), 4);
}

#[test]
fn a_payload_with_no_added_text_yields_nothing() {
    assert_eq!(
        added_text(&json!({"tool_input": {"file_path": "a.ts"}})),
        ""
    );
}

#[test]
fn the_message_carries_the_count_the_path_and_the_lines() {
    let out = context_for("src/a.ts", &["// one".into(), "// two".into()]);
    assert!(out.contains("2 comment line(s) added to src/a.ts"));
    assert!(out.contains("// one\n// two"));
    // The tests themselves stay in CLAUDE.md: repeating them here would be
    // re-billed on every later turn, because additionalContext persists.
    assert!(!out.contains("Would a reader"), "{out}");
}
