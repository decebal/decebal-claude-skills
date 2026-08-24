//! PostToolUse — hand back the comment lines an edit added.
//!
//! Advisory only. It never blocks: a blocking exit surfaces as an error the user
//! has to dismiss, for a finding only the assistant needs to act on.

use crate::hook;
use serde_json::Value;

/// Comment openers per language. `#` opens a comment in shell and Python but
/// names a private field in TypeScript, so the marker set is chosen by
/// extension rather than tried universally.
pub fn markers_for(path: &str) -> Option<&'static [&'static str]> {
    const SLASH: &[&str] = &["//", "/*", "*"];
    const HASH: &[&str] = &["#"];
    const PUG: &[&str] = &["//-", "//"];
    let ext = path.rsplit('.').next()?;
    match ext {
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => Some(SLASH),
        "rs" | "go" | "java" | "kt" | "swift" => Some(SLASH),
        "c" | "h" | "cpp" | "hpp" | "cc" | "css" | "scss" => Some(SLASH),
        "pug" => Some(PUG),
        "py" | "rb" | "sh" | "bash" | "zsh" => Some(HASH),
        _ => None,
    }
}

/// Doc comments are API surface, not the narration this targets.
const DOC_OPENERS: &[&str] = &["#!", "///", "//!", "/**"];

const MAX_LINES: usize = 40;

/// Every text an edit added: a single replacement, a whole-file write, or each
/// edit of a multi-edit.
pub fn added_text(payload: &Value) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for key in ["new_string", "content"] {
        let s = hook::str_at(payload, &["tool_input", key]);
        if !s.is_empty() {
            parts.push(s);
        }
    }
    if let Some(edits) = payload
        .get("tool_input")
        .and_then(|t| t.get("edits"))
        .and_then(|e| e.as_array())
    {
        for edit in edits {
            if let Some(s) = edit.get("new_string").and_then(|v| v.as_str()) {
                parts.push(s);
            }
        }
    }
    parts.join("\n")
}

/// Comment lines in `text`, capped at [`MAX_LINES`].
///
/// Line-leading markers only, so a URL or a `//` inside a string literal is not
/// flagged. Deliberately unnumbered: an offset into the edit reads as a file
/// line number and is not one.
pub fn comment_lines(text: &str, markers: &[&str]) -> Vec<String> {
    text.lines()
        .filter(|line| {
            let t = line.trim_start();
            markers.iter().any(|m| t.starts_with(m))
                && !DOC_OPENERS.iter().any(|d| t.starts_with(d))
        })
        .take(MAX_LINES)
        .map(str::to_string)
        .collect()
}

/// The message. The three tests themselves live in CLAUDE.md, which is already
/// in context — restating them here costs ~130 tokens per firing, and
/// `additionalContext` persists in the transcript, so the same block would be
/// re-billed on every later turn.
pub fn context_for(path: &str, comments: &[String]) -> String {
    format!(
        "comment-hygiene: {} comment line(s) added to {}.\n\n{}\n\nRe-check each against the three comment tests (CLAUDE.md, Code Style). Delete any\nthat fail all three. Fix this turn; do not ask, do not report back.",
        comments.len(),
        path,
        comments.join("\n"),
    )
}

pub fn run(payload: &Value) -> ! {
    match hook::str_at(payload, &["tool_name"]) {
        "Edit" | "Write" | "MultiEdit" => {}
        _ => hook::allow(),
    }
    let path = hook::str_at(payload, &["tool_input", "file_path"]);
    let Some(markers) = markers_for(path) else {
        hook::allow()
    };
    let added = added_text(payload);
    if added.is_empty() {
        hook::allow();
    }
    let comments = comment_lines(&added, markers);
    if comments.is_empty() {
        hook::allow();
    }
    hook::additional_context(&context_for(path, &comments))
}

#[cfg(test)]
#[path = "comment_hygiene_tests.rs"]
mod comment_hygiene_tests;
