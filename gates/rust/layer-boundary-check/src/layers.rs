//! The direction model, and the scan over one file's content.
//!
//! Everything here takes strings, so the whole gate is testable without a
//! filesystem or a repo.

use std::collections::BTreeMap;

/// One declared layer: its name, and the path prefix its files live under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    pub dir: String,
}

/// The whole model.
pub struct Model {
    /// Dependency direction. A layer may import only layers AFTER it.
    pub order: Vec<Layer>,
    /// Layers that may import no other declared layer at all, in either
    /// direction — the domain, in the canonical four.
    pub pure: Vec<Layer>,
    /// How a path into another layer starts, e.g. `crate::`. A reference is an upward
    /// import when this prefix is immediately followed by an upper layer's NAME —
    /// whether or not a `use` keyword precedes it.
    pub path_prefix: String,
    /// `from_to_to` -> the highest count that still passes.
    pub ceilings: BTreeMap<String, usize>,
    /// `from_to_to` -> the one file allowed to cross that edge.
    pub facades: BTreeMap<String, String>,
}

/// A directed pair that must not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: Layer,
    pub to: Layer,
}

impl Edge {
    /// The config key for this edge's ceiling or facade.
    pub fn key(&self) -> String {
        format!("{}_to_{}", self.from.name, self.to.name)
    }

    pub fn label(&self) -> String {
        format!("{} -> {}", self.from.name, self.to.name)
    }
}

impl Model {
    /// Every declared layer, ordered ones first.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.order.iter().chain(self.pure.iter())
    }

    /// The layer a file belongs to: the one whose `dir` is the LONGEST matching
    /// prefix.
    ///
    /// Layer directories legitimately nest — a repo may put `application` at
    /// `src/` and `domain` at `src/domain/`. Matching on "any prefix" then
    /// attributes every domain file to BOTH layers, which double-counts each
    /// violation and, worse, reads domain files as application files when
    /// checking the edges application owns. Longest-prefix makes the
    /// attribution single-valued, which is what a count can be trusted from —
    /// and these counts are compared against ceilings.
    pub fn owning_layer(&self, path: &str) -> Option<&Layer> {
        self.layers()
            .filter(|layer| path.starts_with(layer.dir.as_str()))
            .max_by_key(|layer| layer.dir.len())
    }

    /// Every pair the direction forbids.
    ///
    /// Two sources, and they are different rules: an ORDERED layer may not
    /// import anything before it (that is the arrow pointing backwards), while a
    /// PURE layer may not import any other declared layer at all, nor be the
    /// target of one from below — it sits outside the ordering rather than at
    /// one end of it.
    pub fn forbidden_edges(&self) -> Vec<Edge> {
        let mut edges = Vec::new();
        for (i, from) in self.order.iter().enumerate() {
            for to in self.order.iter().take(i) {
                edges.push(Edge {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        }
        for from in &self.pure {
            for to in self.order.iter().chain(self.pure.iter()) {
                if to.name != from.name {
                    edges.push(Edge {
                        from: from.clone(),
                        to: to.clone(),
                    });
                }
            }
        }
        edges
    }

    pub fn ceiling_for(&self, edge: &Edge) -> usize {
        self.ceilings.get(&edge.key()).copied().unwrap_or(0)
    }

    pub fn facade_for(&self, edge: &Edge) -> Option<&str> {
        self.facades.get(&edge.key()).map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub line: usize,
    pub text: String,
}

/// Blank the comment bytes of `content`, preserving every other byte and every newline.
///
/// Line and column offsets are therefore unchanged, so a hit's line number is the line
/// number in the original file.
///
/// **String literals are tracked but NOT blanked**, and that asymmetry is the whole design.
/// They are tracked so a `//` inside `"http://x"` cannot open a comment and blank the rest of
/// a real line. They are not blanked because a layer name inside a string is a *false
/// positive* — loud, and fixable by the author — whereas a wrongly-blanked span is a false
/// negative, and a gate that passes by reading nothing is the failure this gate exists to
/// prevent.
///
/// Rust block comments nest, so the depth is counted rather than closed on the first `*/`.
fn strip_comments(content: &str) -> String {
    let b: Vec<char> = content.chars().collect();
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut depth = 0usize;
    while i < b.len() {
        let c = b[i];
        let next = b.get(i + 1).copied();
        if depth > 0 {
            if c == '/' && next == Some('*') {
                depth += 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            if c == '*' && next == Some('/') {
                depth -= 1;
                out.push(' ');
                out.push(' ');
                i += 2;
                continue;
            }
            out.push(if c == '\n' { '\n' } else { ' ' });
            i += 1;
            continue;
        }
        if c == '/' && next == Some('/') {
            while i < b.len() && b[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            depth = 1;
            out.push(' ');
            out.push(' ');
            i += 2;
            continue;
        }
        if c == 'r' && matches!(next, Some('"') | Some('#')) {
            if let Some((raw, len)) = raw_string_at(&b, i) {
                out.push_str(&raw);
                i += len;
                continue;
            }
        }
        if c == '"' {
            out.push(c);
            i += 1;
            while i < b.len() {
                let d = b[i];
                out.push(d);
                i += 1;
                if d == '\\' {
                    if let Some(&e) = b.get(i) {
                        out.push(e);
                        i += 1;
                    }
                    continue;
                }
                if d == '"' {
                    break;
                }
            }
            continue;
        }
        // A `'` is a lifetime far more often than a char literal, and only `'"'` would
        // otherwise desynchronise the string state. Consume a char literal when the shape is
        // unambiguously one; treat everything else as a lifetime tick.
        if c == '\'' {
            if let Some(len) = char_literal_len(&b, i) {
                for k in 0..len {
                    out.push(b[i + k]);
                }
                i += len;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// Length of the raw string starting at `i`, and its text, if one starts there.
fn raw_string_at(b: &[char], i: usize) -> Option<(String, usize)> {
    let mut j = i + 1;
    let mut hashes = 0usize;
    while b.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    if b.get(j) != Some(&'"') {
        return None;
    }
    j += 1;
    let closing: String = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    let closing: Vec<char> = closing.chars().collect();
    while j < b.len() {
        if b[j] == '"' && b[j..].starts_with(&closing[..]) {
            j += closing.len();
            return Some((b[i..j].iter().collect(), j - i));
        }
        j += 1;
    }
    Some((b[i..].iter().collect(), b.len() - i))
}

/// Length of a char literal starting at `i`, or `None` when the `'` is a lifetime.
fn char_literal_len(b: &[char], i: usize) -> Option<usize> {
    if b.get(i + 1) == Some(&'\\') {
        let mut j = i + 2;
        while j < b.len() && b[j] != '\'' {
            j += 1;
        }
        return (j < b.len()).then_some(j - i + 1);
    }
    (b.get(i + 2) == Some(&'\'')).then_some(3)
}

/// References in `content` that reach into `layer`, ignoring comments and test code.
///
/// A reference is `prefix` (`crate::`) followed by the layer's name as the FIRST path
/// segment — so it catches a fully-qualified call, not only a `use` statement. That shape is
/// what the old substring needle could not see: `use crate::ui::x` matched, while
/// `crate::ui::x()` written inline did not, and the gate reported zero for a real violation.
///
/// A brace group is expanded, and the first segment of each element is what counts:
/// `use crate::{ui::a, call::b}` names both `ui` and `call`, not `ui::a` and `call::b`.
///
/// Test code is cut at the first `#[cfg(test)]`: by this repo's own convention
/// a test module sits at the bottom of the file, so everything from that
/// attribute on is test code. A test that names a forbidden import while
/// asserting it is absent must not fail the gate that asserts the same thing.
pub fn references_in(path: &str, content: &str, prefix: &str, layer: &str) -> Vec<Hit> {
    let cut = content
        .lines()
        .position(|l| l.trim_start().starts_with("#[cfg(test)]"))
        .unwrap_or(usize::MAX);
    let masked = strip_comments(content);
    let originals: Vec<&str> = content.lines().collect();

    masked
        .lines()
        .enumerate()
        .take_while(|(idx, _)| *idx < cut)
        .filter(|(_, line)| line_reaches(line, prefix, layer))
        .map(|(idx, _)| Hit {
            path: path.to_string(),
            line: idx + 1,
            text: originals.get(idx).unwrap_or(&"").trim().to_string(),
        })
        .collect()
}

fn line_reaches(line: &str, prefix: &str, layer: &str) -> bool {
    let mut rest = line;
    while let Some(at) = rest.find(prefix) {
        let after = &rest[at + prefix.len()..];
        if segments_after(after).iter().any(|s| s == layer) {
            return true;
        }
        rest = after;
    }
    false
}

/// The layer-name candidates a path opens with: one identifier, or the first segment of
/// every element of a brace group.
fn segments_after(after: &str) -> Vec<String> {
    let t = after.trim_start();
    if let Some(group) = t.strip_prefix('{') {
        let end = group.find('}').unwrap_or(group.len());
        return group[..end]
            .split(',')
            .map(|e| leading_ident(e.trim()))
            .filter(|s| !s.is_empty())
            .collect();
    }
    let one = leading_ident(t);
    if one.is_empty() {
        Vec::new()
    } else {
        vec![one]
    }
}

/// Read while the character can be part of an identifier. Enumerating terminators instead
/// would miss `crate::ui)`, `crate::ui>` and `crate::ui!`.
fn leading_ident(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Is this path a test file, whichever convention the repo uses?
pub fn is_test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.split('/').any(|segment| segment == "tests")
        || name == "tests.rs"
        || name.ends_with("_tests.rs")
        || name.ends_with("_test.rs")
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod layers_tests;
