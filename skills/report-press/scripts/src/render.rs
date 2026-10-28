//! Story → one self-contained HTML page: inline CSS, the renderer script and
//! the Markdy bundle, so the file opens from `file://` with no network.

use std::fmt::Write as _;

use crate::brand::BRAND;
use crate::markup::{blocks, escape, inline, plain, Block};
use crate::story::{long_date, Kind, Section, Story};

const CSS: &str = include_str!("../assets/press.css");
const RENDERER: &str = include_str!("../assets/renderer.js");
const MARKDY: &str = include_str!("../vendor/markdy/markdy-1.5.7.iife.min.js");

pub fn page(story: &Story) -> String {
    let f = &story.front;
    let mut fig_no = 0usize;
    let mut s = String::with_capacity(MARKDY.len() + 64 * 1024);

    let _ = write!(
        s,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"report-press {}\">\n\
         <title>{} · {}</title>\n<style>\n:root{{--brand-ink:{};--brand-accent:{};--brand-highlight:{}}}\n{CSS}</style>\n</head>\n<body>\n\
         <a class=\"skip\" href=\"#main\">Skip to content</a>\n<div class=\"brandbar\" aria-hidden=\"true\"></div>\n<div class=\"page\">\n",
        env!("CARGO_PKG_VERSION"),
        plain(&f.title),
        escape(&f.date),
        BRAND.ink,
        BRAND.accent,
        BRAND.highlight,
    );
    masthead(&mut s, story);
    s.push_str("<main id=\"main\">\n");

    let stories: Vec<&Section> = story
        .sections
        .iter()
        .filter(|x| x.kind != Kind::Lead)
        .collect();
    if let Some(lead) = story.sections.iter().find(|x| x.kind == Kind::Lead) {
        lead_section(&mut s, lead, &stories, &mut fig_no);
    }
    for sec in stories.iter().filter(|x| x.kind == Kind::Story) {
        story_section(&mut s, sec, &mut fig_no);
    }
    let back: Vec<&&Section> = stories
        .iter()
        .filter(|x| matches!(x.kind, Kind::Back | Kind::NotChecked))
        .collect();
    if !back.is_empty() {
        s.push_str("<div class=\"back\">\n");
        for sec in back {
            let class = if sec.kind == Kind::NotChecked {
                "box notchecked"
            } else {
                "box"
            };
            let _ = writeln!(
                s,
                "<section class=\"{class}\" id=\"{}\" aria-labelledby=\"h-{}\">\n<h2 id=\"h-{}\">{}</h2>",
                escape(&sec.id),
                escape(&sec.id),
                escape(&sec.id),
                inline(&sec.headline)
            );
            body(&mut s, &blocks(&sec.body), false, &mut fig_no);
            s.push_str("</section>\n");
        }
        s.push_str("</div>\n");
    }

    let _ = write!(
        s,
        "</main>\n<footer><span>{}</span><span>Printed by report-press {} · diagrams: Markdy 1.5.7 (MIT), inlined</span></footer>\n</div>\n\
         <script>\n{MARKDY}\n</script>\n<script>\n{RENDERER}</script>\n</body>\n</html>\n",
        inline(&f.footer),
        env!("CARGO_PKG_VERSION"),
    );
    s
}

fn masthead(s: &mut String, story: &Story) {
    let f = &story.front;
    let mark = match BRAND.logo {
        Some(svg) => svg.trim().to_owned(),
        None => format!("<span class=\"wordmark\">{}</span>", escape(BRAND.name)),
    };
    let mark = if f.publisher.is_empty() {
        mark
    } else {
        format!("<span class=\"wordmark\">{}</span>", escape(&f.publisher))
    };
    let title = escape(&f.title);
    let title = match title.rsplit_once(' ') {
        Some((head, last)) => format!("{head} <span>{last}</span>"),
        None => title,
    };
    let strap: String = f
        .strap
        .iter()
        .map(|x| format!("<span>{}</span>", inline(x)))
        .collect();
    let _ = writeln!(
        s,
        "<header class=\"mast\">\n<div class=\"mast-top\">{mark}<span>{}</span><span>{}</span></div>\n\
         <div class=\"title\">{title}</div>\n<div class=\"strap\">{strap}</div>\n</header>",
        inline(&f.desk),
        long_date(&f.date),
    );
}

fn header_bits(s: &mut String, sec: &Section, tag: &str) {
    if let Some(k) = &sec.kicker {
        let _ = writeln!(s, "<p class=\"kicker\">{}</p>", inline(k));
    }
    let _ = writeln!(
        s,
        "<{tag} id=\"h-{}\">{}</{tag}>",
        escape(&sec.id),
        inline(&sec.headline)
    );
    if let Some(d) = &sec.deck {
        let _ = writeln!(s, "<p class=\"deck\">{}</p>", inline(d));
    }
}

fn lead_section(s: &mut String, lead: &Section, stories: &[&Section], fig_no: &mut usize) {
    let all = blocks(&lead.body);
    let (figs, text): (Vec<Block>, Vec<Block>) = all
        .into_iter()
        .partition(|b| matches!(b, Block::Figure { .. }));
    let _ = writeln!(
        s,
        "<section class=\"lead\" id=\"{}\" aria-labelledby=\"h-{}\">\n<div class=\"lead-text\">",
        escape(&lead.id),
        escape(&lead.id)
    );
    header_bits(s, lead, "h1");
    if let Some(b) = &lead.byline {
        let _ = writeln!(s, "<div class=\"byline\">{}</div>", inline(b));
    }
    s.push_str("<div class=\"lead-body\">\n");
    body(s, &text, false, fig_no);
    s.push_str("</div>\n</div>\n<nav class=\"inbrief\" aria-label=\"In this edition\">\n<h2>In this edition</h2>\n<ol>\n");
    for sec in stories.iter().filter(|x| x.kind != Kind::NotChecked) {
        let label = sec.short.as_deref().unwrap_or(&sec.headline);
        let _ = writeln!(
            s,
            "<li><a href=\"#{}\">{}</a></li>",
            escape(&sec.id),
            inline(label)
        );
    }
    s.push_str("</ol>\n</nav>\n<div class=\"lead-figs\">\n");
    body(s, &figs, false, fig_no);
    s.push_str("</div>\n</section>\n");
}

fn story_section(s: &mut String, sec: &Section, fig_no: &mut usize) {
    let _ = writeln!(
        s,
        "<section class=\"story\" id=\"{}\" aria-labelledby=\"h-{}\">\n<header>",
        escape(&sec.id),
        escape(&sec.id)
    );
    header_bits(s, sec, "h2");
    s.push_str("</header>\n");
    body(s, &blocks(&sec.body), true, fig_no);
    s.push_str("</section>\n");
}

fn body(s: &mut String, blocks: &[Block], columns: bool, fig_no: &mut usize) {
    for b in blocks {
        match b {
            Block::Prose(h) if columns => {
                let _ = write!(s, "<div class=\"cols\">\n{h}</div>\n");
            }
            Block::Prose(h) => s.push_str(h),
            Block::Pull(h) => {
                let _ = write!(s, "<blockquote class=\"pull\">\n{h}</blockquote>\n");
            }
            Block::Note(h) => {
                let _ = write!(s, "<div class=\"limit\">\n{h}</div>\n");
            }
            Block::Table(h) => {
                let _ = write!(s, "<div class=\"tablewrap\">\n{h}</div>\n");
            }
            Block::Figure { caption, code } => {
                *fig_no += 1;
                let _ = write!(
                    s,
                    "<figure class=\"fig\" data-markdy>\n<div class=\"stage\" role=\"img\" aria-label=\"{}\"></div>\n\
                     <script type=\"text/markdy\">{}</script>\n<figcaption><b>Fig. {fig_no}</b>{}</figcaption>\n</figure>\n",
                    plain(caption),
                    themed(code),
                    inline(caption),
                );
            }
        }
    }
}

/// The renderer fills `__THEME__` per colour scheme; add it if the author left
/// the theme out.
pub fn themed(code: &str) -> String {
    let mut out = String::with_capacity(code.len() + 20);
    let mut done = false;
    for line in code.lines() {
        if !done && line.trim_start().starts_with("scene") {
            done = true;
            let toks: Vec<&str> = line
                .split_whitespace()
                .filter(|t| !t.starts_with("theme="))
                .collect();
            let (head, rest) = toks
                .split_first()
                .map_or(("scene", &[][..]), |(h, r)| (*h, r));
            out.push_str(head);
            out.push_str(" theme=__THEME__");
            for t in rest {
                out.push(' ');
                out.push_str(t);
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}
