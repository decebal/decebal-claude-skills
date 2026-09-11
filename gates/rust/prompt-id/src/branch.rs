//! The feature branch a prompt reserves.
//!
//! Prompts wrap at ~80 columns, so the branch name is regularly on the line
//! AFTER the word introducing it. Six of the twenty prompts at one store's root
//! were wrapped that way, so a line-at-a-time extractor reports "no branch" for
//! nearly a third of them and they are never flagged as shipped.

/// The first branch a prompt reserves, or `None` when it names none.
pub fn reserved(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(start) = find_ci(bytes, b"branch", from) {
        let end = start + "branch".len();
        from = end;
        if !is_word(bytes, start, end) {
            continue;
        }
        if let Some(name) = span_after(text, end) {
            return Some(name);
        }
    }
    None
}

/// Case-insensitive ASCII search. Lowercasing the whole text first would be
/// shorter and wrong: `to_lowercase` can change a string's BYTE length, and the
/// offsets found in it are then used to slice the original.
fn find_ci(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + needle.len() <= haystack.len() {
        if haystack[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Whether the hit is the whole word, so `branched` and `branches` are skipped.
fn is_word(bytes: &[u8], start: usize, end: usize) -> bool {
    let before_ok = start == 0 || !is_wordish(bytes[start - 1]);
    let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
    before_ok && after_ok
}

fn is_wordish(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// The backticked name that follows, when only separators sit between.
///
/// The separator set is what keeps prose out: "the second branch of the
/// parser is reached" has a backtick further down the paragraph, and a
/// windowed search would happily return whatever it wrapped.
fn span_after(text: &str, pos: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = pos;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\r' | b'\n' | b',' | b':') {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'`' {
        return None;
    }
    let open = i + 1;
    let close = open + text.get(open..)?.find('`')?;
    let name = text.get(open..close)?;
    is_branch_name(name).then(|| name.to_string())
}

/// `<type>/<slug>` — the shape every prompt's `<work_organization>` block uses.
///
/// Requiring the slash is what lets a non-branch backtick span be skipped and
/// the search continue, rather than the first quoted thing after the word
/// winning.
fn is_branch_name(s: &str) -> bool {
    let Some((kind, rest)) = s.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && kind.chars().all(|c| c.is_ascii_lowercase())
        && !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}
