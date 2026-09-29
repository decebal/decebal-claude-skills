//! The `name` and `description` keys of a `SKILL.md` YAML frontmatter block.
//! Handles plain, single- and double-quoted scalars, multi-line plain scalars,
//! and `>` / `|` block scalars; everything else in the block is ignored.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
}

pub fn parse(text: &str) -> Frontmatter {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Frontmatter::default();
    }
    let mut body = Vec::new();
    let mut closed = false;
    for line in lines {
        let t = line.trim_end();
        if t == "---" || t == "..." {
            closed = true;
            break;
        }
        body.push(line);
    }
    if !closed {
        return Frontmatter::default();
    }

    let mut fm = Frontmatter::default();
    let mut i = 0;
    while i < body.len() {
        let line = body[i];
        i += 1;
        if line.starts_with([' ', '\t', '#']) || line.trim().is_empty() {
            continue;
        }
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().trim_matches(['"', '\'']);
        if key != "name" && key != "description" {
            continue;
        }
        let start = i;
        while i < body.len() && (body[i].trim().is_empty() || body[i].starts_with([' ', '\t'])) {
            i += 1;
        }
        let value = scalar(rest, &body[start..i]);
        let value = (!value.is_empty()).then_some(value);
        if key == "name" {
            fm.name = value;
        } else {
            fm.description = value;
        }
    }
    fm
}

fn scalar(rest: &str, continuation: &[&str]) -> String {
    let head = rest.trim();
    if head.starts_with('|') || head.starts_with('>') {
        return block(head.starts_with('>'), continuation);
    }
    if head.starts_with('"') || head.starts_with('\'') {
        let mut joined = head.to_owned();
        for line in continuation {
            joined.push(' ');
            joined.push_str(line.trim());
        }
        return quoted(&joined);
    }
    let mut parts = vec![strip_comment(head)];
    parts.extend(
        continuation
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty()),
    );
    parts.retain(|p| !p.is_empty());
    parts.join(" ")
}

fn strip_comment(plain: &str) -> &str {
    match plain.find(" #") {
        Some(i) => plain[..i].trim_end(),
        None => plain,
    }
}

fn block(folded: bool, lines: &[&str]) -> String {
    let indent = lines
        .iter()
        .find(|l| !l.trim().is_empty())
        .map_or(0, |l| l.len() - l.trim_start().len());
    let content: Vec<&str> = lines
        .iter()
        .map(|l| {
            let own = l.len() - l.trim_start().len();
            if l.trim().is_empty() {
                ""
            } else {
                &l[own.min(indent)..]
            }
        })
        .collect();
    if !folded {
        return content.join("\n").trim_end().to_owned();
    }
    let mut out = String::new();
    let mut prev_blank = true;
    for line in content {
        if line.is_empty() {
            out.push('\n');
            prev_blank = true;
            continue;
        }
        if !prev_blank {
            out.push(' ');
        }
        out.push_str(line);
        prev_blank = false;
    }
    out.trim().to_owned()
}

fn quoted(s: &str) -> String {
    let mut chars = s.chars();
    let Some(q) = chars.next() else {
        return String::new();
    };
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if q == '\'' && c == '\'' {
            if chars.clone().next() == Some('\'') {
                chars.next();
                out.push('\'');
                continue;
            }
            break;
        }
        if q == '"' && c == '"' {
            break;
        }
        if q == '"' && c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => break,
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_owned()
}

#[cfg(test)]
#[path = "frontmatter_tests.rs"]
mod tests;
