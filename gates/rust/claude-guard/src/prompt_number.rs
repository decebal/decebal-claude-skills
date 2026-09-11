//! PreToolUse — refuse to create a numbered prompt whose number was never
//! claimed.
//!
//! `.prompts/` is gitignored, so every worktree holds its own near-empty copy. A
//! session that picks a number by looking at the directory it is standing in
//! sees almost nothing and lands on one the main checkout already issued.
//! `prompt-id alloc` exists to remove that: it takes a lock on the NUMBER, under
//! `<store>/.numbers/`, before anything is written.
//!
//! Prose cannot enforce it. The file that made this a rule was created by a bare
//! `Write` in a session that never invoked the command carrying the instruction
//! — it reasoned about the naming convention and never about allocation, so
//! there was no wording anywhere that could have reached it. A tool-call guard
//! can.
//!
//! Fails open in every direction that is not the exact damage: an existing file,
//! a store with no `.numbers/` at all (a repo that never adopted the allocator),
//! a path that is not a direct child of `.prompts/`, an unreadable payload.

use crate::hook;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A prompt path this guard has an opinion about: the store it belongs to, and
/// the number it carries.
pub struct Numbered {
    pub store: PathBuf,
    pub number: String,
}

/// The store and number a path carries, when it is a numbered prompt at the top
/// level of a `.prompts/` store.
///
/// Only a DIRECT child qualifies. `.prompts/completed/NNN-x.md` is an archive
/// move, and `.prompts/NNN-topic/NNN-topic.md` is a stage inside a directory
/// whose number was already claimed when the directory was made — neither is a
/// new allocation.
pub fn parse(path: &str) -> Option<Numbered> {
    let file = Path::new(path);
    let name = file.file_name()?.to_str()?;
    if !name.ends_with(".md") {
        return None;
    }
    let store = file.parent()?;
    if store.file_name()?.to_str()? != ".prompts" {
        return None;
    }
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || !name[digits.len()..].starts_with('-') {
        return None;
    }
    // Zero-padded to three, matching how the claim is written, so `0208` and
    // `208` resolve to the same claim rather than to two.
    let number = digits.parse::<u32>().ok()?;
    Some(Numbered {
        store: store.to_path_buf(),
        number: format!("{number:03}"),
    })
}

/// Why this write must not happen, or `None` to allow it.
pub fn refusal(path: &str) -> Option<String> {
    let found = parse(path)?;
    if Path::new(path).exists() {
        return None;
    }
    let claims = found.store.join(".numbers");
    if !claims.is_dir() {
        return None;
    }
    if claims.join(&found.number).exists() {
        return None;
    }
    Some(reason(&found.number))
}

/// The message. It names the missing claim and the one command that fixes it —
/// a refusal that does not say what to run instead just gets worked around.
///
/// This is a `deny` reason, so every byte is billed to the model's context.
/// Held under a ceiling by `payload_size_tests.rs`.
pub fn reason(number: &str) -> String {
    format!(
        "prompt-id: {number} is unallocated — .prompts/.numbers/{number} does not exist, so \
         another session can still be handed it.\n\
         Run `prompt-id alloc <slug>` and write to the path it prints."
    )
}

pub fn run(payload: &Value) -> ! {
    match hook::str_at(payload, &["tool_name"]) {
        "Edit" | "Write" | "MultiEdit" => {}
        _ => hook::allow(),
    }
    let path = hook::str_at(payload, &["tool_input", "file_path"]);
    match refusal(path) {
        Some(message) => hook::deny(&message),
        None => hook::allow(),
    }
}

#[cfg(test)]
#[path = "prompt_number_tests.rs"]
mod prompt_number_tests;
