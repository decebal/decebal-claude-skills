//! The story: Markdown with a frontmatter block, one `#` lead and `##` sections.
//!
//! A heading may carry `{#id .back}` / `{#id .notchecked}` attributes and is
//! followed by `@kicker`, `@deck`, `@byline` and `@short` directive lines.
//! Everything after the directives is the section's Markdown body.

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Front {
    pub title: String,
    pub date: String,
    pub desk: String,
    pub publisher: String,
    pub strap: Vec<String>,
    pub footer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Lead,
    Story,
    Back,
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub kind: Kind,
    pub line: usize,
    pub headline: String,
    pub id: String,
    pub kicker: Option<String>,
    pub deck: Option<String>,
    pub byline: Option<String>,
    pub short: Option<String>,
    pub body: String,
    /// Line number of the first body line, so findings point into the file.
    pub body_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    pub front: Front,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub msg: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Figure {
    pub line: usize,
    pub caption: String,
    pub code: String,
}

const FRONT_KEYS: [&str; 6] = ["title", "date", "desk", "publisher", "strap", "footer"];
const DIRECTIVES: [&str; 4] = ["kicker", "deck", "byline", "short"];

fn problem(line: usize, msg: impl Into<String>) -> Problem {
    Problem {
        line,
        msg: msg.into(),
    }
}

pub fn parse(src: &str) -> Result<Story, Vec<Problem>> {
    let lines: Vec<&str> = src.lines().collect();
    let mut problems = Vec::new();
    let (front, mut i) = parse_front(&lines, &mut problems);

    let mut sections: Vec<Section> = Vec::new();
    let mut fence = false;
    while i < lines.len() {
        let line = lines[i];
        let n = i + 1;
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        let heading = if fence {
            None
        } else if let Some(rest) = line.strip_prefix("# ") {
            Some((Kind::Lead, rest))
        } else {
            line.strip_prefix("## ").map(|rest| (Kind::Story, rest))
        };
        match heading {
            Some((kind, rest)) => {
                let (section, next) = parse_section(&lines, i, kind, rest, &mut problems);
                if kind == Kind::Lead && sections.iter().any(|s| s.kind == Kind::Lead) {
                    problems.push(problem(n, "a story has exactly one `#` lead"));
                }
                sections.push(section);
                i = next;
                continue;
            }
            None => {
                if let Some(last) = sections.last_mut() {
                    if !last.body.is_empty() || !line.trim().is_empty() {
                        if last.body.is_empty() {
                            last.body_line = n;
                        }
                        last.body.push_str(line);
                        last.body.push('\n');
                    }
                } else if !line.trim().is_empty() {
                    problems.push(problem(n, "text before the `#` lead"));
                }
                if !fence && line.starts_with('@') {
                    problems.push(problem(
                        n,
                        "directives (`@kicker` …) belong directly under a heading",
                    ));
                }
            }
        }
        i += 1;
    }

    if front.title.is_empty() {
        problems.push(problem(1, "frontmatter needs `title:`"));
    }
    if !valid_date(&front.date) {
        problems.push(problem(1, "frontmatter needs `date: YYYY-MM-DD`"));
    }
    if !sections.iter().any(|s| s.kind == Kind::Lead) {
        problems.push(problem(1, "a story needs one `#` lead headline"));
    }
    if problems.is_empty() {
        Ok(Story { front, sections })
    } else {
        Err(problems)
    }
}

fn parse_front(lines: &[&str], problems: &mut Vec<Problem>) -> (Front, usize) {
    let mut front = Front::default();
    if lines.first().map(|l| l.trim()) != Some("---") {
        problems.push(problem(1, "a story starts with a `---` frontmatter block"));
        return (front, 0);
    }
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.trim() == "---" {
            return (front, i + 1);
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            problems.push(problem(i + 1, "frontmatter lines are `key: value`"));
            continue;
        };
        let (key, value) = (key.trim(), value.trim().to_owned());
        match key {
            "title" => front.title = value,
            "date" => front.date = value,
            "desk" => front.desk = value,
            "publisher" => front.publisher = value,
            "footer" => front.footer = value,
            "strap" => {
                front.strap = value
                    .split('|')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            _ => problems.push(problem(
                i + 1,
                format!(
                    "unknown frontmatter key `{key}`; known: {}",
                    FRONT_KEYS.join(", ")
                ),
            )),
        }
    }
    problems.push(problem(1, "frontmatter block is never closed with `---`"));
    (front, lines.len())
}

fn parse_section(
    lines: &[&str],
    at: usize,
    kind: Kind,
    rest: &str,
    problems: &mut Vec<Problem>,
) -> (Section, usize) {
    let (headline, attrs) = split_attrs(rest);
    let mut kind = kind;
    let mut id = String::new();
    for attr in attrs.split_whitespace() {
        if let Some(v) = attr.strip_prefix('#') {
            id = v.to_owned();
        } else if attr == ".back" && kind == Kind::Story {
            kind = Kind::Back;
        } else if attr == ".notchecked" && kind == Kind::Story {
            kind = Kind::NotChecked;
        } else {
            problems.push(problem(
                at + 1,
                format!("unknown heading attribute `{attr}`; known: #id .back .notchecked"),
            ));
        }
    }
    if id.is_empty() {
        id = slug(&headline);
    }
    let mut section = Section {
        kind,
        line: at + 1,
        headline,
        id,
        kicker: None,
        deck: None,
        byline: None,
        short: None,
        body: String::new(),
        body_line: at + 2,
    };
    let mut i = at + 1;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let Some(directive) = line.strip_prefix('@') else {
            break;
        };
        let (name, value) = directive.split_once(' ').unwrap_or((directive, ""));
        let value = Some(value.trim().to_owned());
        match name {
            "kicker" => section.kicker = value,
            "deck" => section.deck = value,
            "byline" => section.byline = value,
            "short" => section.short = value,
            _ => problems.push(problem(
                i + 1,
                format!(
                    "unknown directive `@{name}`; known: @{}",
                    DIRECTIVES.join(" @")
                ),
            )),
        }
        i += 1;
    }
    (section, i)
}

/// `Headline text {#id .class}` → (`Headline text`, `#id .class`).
fn split_attrs(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if let Some(open) = rest.rfind('{') {
        if rest.ends_with('}') {
            return (
                rest[..open].trim().to_owned(),
                rest[open + 1..rest.len() - 1].to_owned(),
            );
        }
    }
    (rest.to_owned(), String::new())
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn valid_date(d: &str) -> bool {
    parse_date(d).is_some()
}

pub fn parse_date(d: &str) -> Option<(i32, u32, u32)> {
    let mut it = d.split('-');
    let y = it.next()?.parse::<i32>().ok()?;
    let m = it.next()?.parse::<u32>().ok()?;
    let day = it.next()?.parse::<u32>().ok()?;
    let ok =
        d.len() == 10 && it.next().is_none() && (1..=12).contains(&m) && (1..=31).contains(&day);
    ok.then_some((y, m, day))
}

/// `Wednesday 28 October 2026`, from an ISO date.
pub fn long_date(d: &str) -> String {
    const DAYS: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let Some((y, m, day)) = parse_date(d) else {
        return d.to_owned();
    };
    // Sakamoto's day-of-week, valid for the Gregorian calendar.
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let dow = (yy + yy / 4 - yy / 100 + yy / 400 + T[m as usize - 1] + day as i32).rem_euclid(7);
    format!(
        "{} {day} {} {y}",
        DAYS[dow as usize],
        MONTHS[m as usize - 1]
    )
}

/// The ```` ```markdy <caption> ```` blocks of a section body, with file lines.
pub fn figures(section: &Section) -> Vec<Figure> {
    let mut out = Vec::new();
    let mut current: Option<Figure> = None;
    let mut other_fence = false;
    for (i, line) in section.body.lines().enumerate() {
        let n = section.body_line + i;
        let t = line.trim_start();
        if let Some(fig) = current.as_mut() {
            if t.starts_with("```") {
                out.extend(current.take());
            } else {
                fig.code.push_str(line);
                fig.code.push('\n');
            }
        } else if other_fence {
            other_fence = !t.starts_with("```");
        } else if let Some(info) = t.strip_prefix("```") {
            match info.trim().strip_prefix("markdy") {
                Some(caption) if caption.is_empty() || caption.starts_with(' ') => {
                    current = Some(Figure {
                        line: n,
                        caption: caption.trim().to_owned(),
                        code: String::new(),
                    });
                }
                _ => other_fence = true,
            }
        }
    }
    out
}
