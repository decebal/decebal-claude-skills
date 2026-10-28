//! Markdown body → page blocks. Prose is grouped so the page can set it in
//! columns; tables, pull quotes, notes and `markdy` figures stand alone.

use pulldown_cmark::{html, BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Prose(String),
    Pull(String),
    Note(String),
    Table(String),
    Figure { caption: String, code: String },
}

pub const TAGS: [&str; 6] = ["have", "part", "gap", "rock", "sand", "water"];

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_GFM
}

/// `{{observed: x}}` → a source label; `[[gap]]` → a status tag. Code spans
/// and fenced blocks are left untouched.
pub fn inline_marks(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut fence = false;
    for line in md.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            out.push_str(line);
        } else if fence {
            out.push_str(line);
        } else {
            out.push_str(&marks_in_line(line));
        }
    }
    out
}

fn marks_in_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut code = false;
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if c == '`' {
            code = !code;
        } else if !code {
            if rest.starts_with("{{") {
                out.push_str("<span class=\"src\">");
                rest = &rest[2..];
                continue;
            }
            if rest.starts_with("}}") {
                out.push_str("</span>");
                rest = &rest[2..];
                continue;
            }
            if let Some(tag) = TAGS.iter().find(|t| rest.starts_with(&format!("[[{t}]]"))) {
                out.push_str(&format!("<span class=\"tag {tag}\">{tag}</span>"));
                rest = &rest[tag.len() + 4..];
                continue;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

pub fn blocks(md: &str) -> Vec<Block> {
    let md = inline_marks(md);
    let events: Vec<Event> = Parser::new_ext(&md, options()).collect();
    let mut out: Vec<Block> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        let end = block_end(&events, i);
        let slice = &events[i..end];
        let block = match &events[i] {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if is_markdy(info) => {
                let code = slice
                    .iter()
                    .filter_map(|e| match e {
                        Event::Text(t) => Some(t.as_ref()),
                        _ => None,
                    })
                    .collect::<String>();
                Block::Figure {
                    caption: info.trim()["markdy".len()..].trim().to_owned(),
                    code,
                }
            }
            Event::Start(Tag::BlockQuote(kind)) => {
                let inner = render(&slice[1..slice.len() - 1]);
                match kind {
                    None => Block::Pull(inner),
                    Some(BlockQuoteKind::Note) => Block::Note(inner),
                    Some(_) => Block::Prose(render(slice)),
                }
            }
            Event::Start(Tag::Table(_)) => Block::Table(render(slice)),
            _ => Block::Prose(render(slice)),
        };
        match (out.last_mut(), block) {
            (Some(Block::Prose(prev)), Block::Prose(next)) => prev.push_str(&next),
            (_, block) => out.push(block),
        }
        i = end;
    }
    out
}

fn is_markdy(info: &str) -> bool {
    let info = info.trim();
    info == "markdy" || info.starts_with("markdy ")
}

/// Index one past the top-level block starting at `start`.
fn block_end(events: &[Event], start: usize) -> usize {
    let mut depth = 0usize;
    for (i, e) in events.iter().enumerate().skip(start) {
        match e {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 {
            return i + 1;
        }
    }
    events.len()
}

fn render(events: &[Event]) -> String {
    let mut s = String::new();
    html::push_html(&mut s, events.iter().cloned());
    s
}

/// A caption or deck: inline Markdown without the wrapping `<p>`.
pub fn inline(md: &str) -> String {
    let md = inline_marks(md);
    let events: Vec<Event> = Parser::new_ext(&md, options())
        .filter(|e| {
            !matches!(
                e,
                Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph)
            )
        })
        .collect();
    render(&events).trim().to_owned()
}

/// Plain text for `aria-label` and `<title>`: markup removed, then escaped.
pub fn plain(md: &str) -> String {
    let text: String = Parser::new_ext(md, options())
        .filter_map(|e| match e {
            Event::Text(t) | Event::Code(t) => Some(t.into_string()),
            Event::SoftBreak | Event::HardBreak => Some(" ".to_owned()),
            _ => None,
        })
        .collect();
    escape(&text)
}

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
