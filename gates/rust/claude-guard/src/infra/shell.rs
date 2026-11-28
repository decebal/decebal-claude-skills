//! Just enough shell lexing to tell text that RUNS from text that is only
//! printed, quoted or documented.
//!
//! Not a parser. Where it is unsure it keeps the text, because a line the deep
//! layer drops is a line the guard never sees.

use super::classify::cached;

/// One simple command: the text between two unquoted separators.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Source text with its quoting intact, which is what gets classified.
    pub raw: String,
    /// Words with quoting removed. Redirections are not words.
    pub words: Vec<String>,
    /// The `< file` target.
    pub stdin: Option<String>,
    /// The `<<WORD` delimiter.
    pub heredoc: Option<String>,
    /// Reached through a `|`.
    pub piped: bool,
}

const SHELLS: [&str; 5] = ["bash", "sh", "zsh", "dash", "ksh"];

/// Commands whose arguments are data: printed or read, never run.
const DATA_ONLY: [&str; 15] = [
    "echo", "printf", "cat", "head", "tail", "less", "more", "grep", "egrep", "fgrep", "rg", "wc",
    ":", "true", "false",
];

const KEYWORDS: [&str; 12] = [
    "if", "then", "else", "elif", "do", "while", "until", "!", "{", "}", "time", "function",
];

pub fn is_shell(word: &str) -> bool {
    SHELLS.contains(&word)
}

impl Segment {
    /// The word in command position, with a make recipe's `@`/`-`/`+` prefix
    /// removed.
    pub fn head(&self) -> Option<&str> {
        command_words(&self.words)
            .first()
            .map(|w| w.trim_start_matches(['@', '-', '+']))
    }

    fn data_only(&self) -> bool {
        self.head().is_some_and(|h| DATA_ONLY.contains(&h))
    }
}

fn is_assignment(w: &str) -> bool {
    cached(r"^[A-Za-z_][A-Za-z0-9_]*=").is_match(w)
}

/// Past any flags; a flag in `with_arg` also consumes the word after it.
fn skip_flags(words: &[String], mut i: usize, with_arg: &[&str]) -> usize {
    while i < words.len() && words[i].starts_with('-') {
        if with_arg.contains(&words[i].as_str()) {
            i += 1;
        }
        i += 1;
    }
    i
}

/// The words from the command itself onward: assignments, keywords and
/// prefix runners (`env`, `timeout 300`, `xargs -n1`, ...) skipped.
pub fn command_words(words: &[String]) -> &[String] {
    let mut i = 0;
    while i < words.len() {
        let w = words[i].as_str();
        if is_assignment(w) || KEYWORDS.contains(&w) {
            i += 1;
            continue;
        }
        i = match w {
            "env" => skip_flags(words, i + 1, &["-u"]),
            "nohup" | "exec" | "command" | "builtin" => skip_flags(words, i + 1, &[]),
            "nice" => skip_flags(words, i + 1, &["-n"]),
            "sudo" => skip_flags(words, i + 1, &["-u", "-g"]),
            "timeout" | "gtimeout" => skip_flags(words, i + 1, &["-s", "-k"]) + 1,
            "xargs" => skip_flags(
                words,
                i + 1,
                &["-n", "-P", "-I", "-L", "-d", "-s", "-E", "-a"],
            ),
            _ => break,
        };
    }
    &words[i.min(words.len())..]
}

#[derive(Clone, Copy, PartialEq)]
enum Pending {
    Word,
    Stdin,
    Heredoc,
    Discard,
}

struct Lexer {
    chars: Vec<char>,
    out: Vec<Segment>,
    seg: Segment,
    start: usize,
    word: String,
    in_word: bool,
    pending: Pending,
}

impl Lexer {
    fn end_word(&mut self) {
        if !self.in_word {
            return;
        }
        let w = std::mem::take(&mut self.word);
        self.in_word = false;
        match std::mem::replace(&mut self.pending, Pending::Word) {
            Pending::Word => self.seg.words.push(w),
            Pending::Stdin => self.seg.stdin = Some(w),
            Pending::Heredoc => self.seg.heredoc = Some(w),
            Pending::Discard => {}
        }
    }

    fn end_segment(&mut self, at: usize, next_piped: bool) {
        self.end_word();
        self.pending = Pending::Word;
        let mut seg = std::mem::take(&mut self.seg);
        seg.raw = self.chars[self.start..at]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !seg.words.is_empty() || seg.heredoc.is_some() || seg.stdin.is_some() {
            self.out.push(seg);
        }
        self.seg.piped = next_piped;
    }

    /// Index of the char closing a `$(`, `${` or backtick opened at `open`.
    fn closing(&self, open: usize, close: char) -> usize {
        let opener = self.chars[open];
        let mut depth = 0usize;
        let mut i = open + 1;
        while i < self.chars.len() {
            let c = self.chars[i];
            if c == close && depth == 0 {
                return i;
            }
            if c == opener && opener != close {
                depth += 1;
            } else if c == close {
                depth -= 1;
            }
            i += 1;
        }
        self.chars.len() - 1
    }

    /// Consume an expansion starting at `i` into the current word.
    fn expansion(&mut self, i: usize) -> Option<usize> {
        let c = self.chars[i];
        let next = self.chars.get(i + 1).copied();
        let end = match (c, next) {
            ('$', Some('(')) => self.closing(i + 1, ')'),
            ('$', Some('{')) => self.closing(i + 1, '}'),
            ('`', _) => self.closing(i, '`'),
            _ => return None,
        };
        self.word.extend(&self.chars[i..=end]);
        self.in_word = true;
        Some(end + 1)
    }

    fn redirect(&mut self, mut i: usize) -> usize {
        if self.in_word && self.word.chars().all(|c| c.is_ascii_digit()) {
            self.word.clear();
            self.in_word = false;
        }
        self.end_word();
        let mut op = String::new();
        while let Some(&c) = self.chars.get(i) {
            if c != '<' && c != '>' && !(c == '&' && op.is_empty()) {
                break;
            }
            op.push(c);
            i += 1;
        }
        if op == "<<" && self.chars.get(i) == Some(&'-') {
            i += 1;
        }
        if self.chars.get(i) == Some(&'&') {
            i += 1;
            while self
                .chars
                .get(i)
                .is_some_and(|c| c.is_ascii_digit() || *c == '-')
            {
                i += 1;
            }
            return i;
        }
        self.pending = match op.as_str() {
            "<" => Pending::Stdin,
            "<<" => Pending::Heredoc,
            _ => Pending::Discard,
        };
        i
    }
}

/// Split one logical line at unquoted separators.
pub fn segments(line: &str) -> Vec<Segment> {
    let mut lx = Lexer {
        chars: line.chars().collect(),
        out: Vec::new(),
        seg: Segment::default(),
        start: 0,
        word: String::new(),
        in_word: false,
        pending: Pending::Word,
    };
    let n = lx.chars.len();
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < n {
        let c = lx.chars[i];
        let next = lx.chars.get(i + 1).copied();
        if quote == Some('\'') {
            if c == '\'' {
                quote = None;
            } else {
                lx.word.push(c);
            }
            i += 1;
            continue;
        }
        if quote == Some('"') {
            if c == '"' {
                quote = None;
            } else if c == '\\' && next.is_some() {
                lx.word.push(lx.chars[i + 1]);
                i += 1;
            } else if let Some(j) = lx.expansion(i) {
                i = j;
                continue;
            } else {
                lx.word.push(c);
            }
            i += 1;
            continue;
        }
        if let Some(j) = lx.expansion(i) {
            i = j;
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                lx.in_word = true;
            }
            '\\' => {
                if let Some(e) = next {
                    lx.word.push(e);
                    lx.in_word = true;
                    i += 1;
                }
            }
            ' ' | '\t' => lx.end_word(),
            '#' if !lx.in_word => {
                lx.end_segment(i, false);
                return lx.out;
            }
            '<' | '>' => {
                i = lx.redirect(i);
                continue;
            }
            '&' if next == Some('>') => {
                i = lx.redirect(i);
                continue;
            }
            ';' | '\n' | '(' | ')' | '&' | '|' => {
                let doubled = (c == '&' || c == '|') && next == Some(c);
                lx.end_segment(i, c == '|' && !doubled);
                if doubled {
                    i += 1;
                }
                lx.start = i + 1;
            }
            _ => {
                lx.word.push(c);
                lx.in_word = true;
            }
        }
        i += 1;
    }
    lx.end_segment(n, false);
    lx.out
}

/// Does a quote opened on this text stay open at its end?
fn open_quote(text: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut prev = ' ';
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some('\'') if c == '\'' => quote = None,
            Some('"') if c == '"' => quote = None,
            Some('"') if c == '\\' => {
                chars.next();
            }
            Some(_) => {}
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    chars.next();
                }
                '#' if prev.is_whitespace() => return false,
                _ => {}
            },
        }
        prev = c;
    }
    quote.is_some()
}

/// Bodies of `$(...)` and backticks, which run wherever they appear —
/// inside a quoted `echo` argument too.
fn substitutions(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for c in cached(r"\$\(([^()]*)\)").captures_iter(text) {
        out.push(c[1].to_string());
    }
    for c in cached(r"`([^`]*)`").captures_iter(text) {
        out.push(c[1].to_string());
    }
    out
}

/// A string left open longer than this is a lexing miss, not a multi-line
/// literal; the line is then taken alone.
const MAX_JOINED: usize = 50;

/// Every command line in a script: comments dropped, `\` continuations and
/// multi-line strings joined, and heredoc bodies dropped unless the heredoc
/// feeds a shell.
pub fn logical_lines(text: &str) -> Vec<Vec<Segment>> {
    let physical: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < physical.len() {
        let first = physical[i];
        i += 1;
        if first.trim_start().starts_with('#') {
            continue;
        }
        let mut line = first.to_string();
        let mut j = i;
        loop {
            if line.ends_with('\\') && !line.ends_with("\\\\") {
                line.pop();
                line.push(' ');
            } else if open_quote(&line) {
                line.push('\n');
            } else {
                i = j;
                break;
            }
            if j >= physical.len() || j - i >= MAX_JOINED {
                line = first.trim_end_matches('\\').to_string();
                break;
            }
            line.push_str(physical[j]);
            j += 1;
        }

        let segs = segments(&line);
        let feeds_shell = segs
            .iter()
            .any(|s| s.head().is_some_and(|h| is_shell(h) || h == "ssh"));
        for delim in segs.iter().filter_map(|s| s.heredoc.clone()) {
            let mut body = Vec::new();
            while i < physical.len() {
                let l = physical[i];
                i += 1;
                if l.trim() == delim {
                    break;
                }
                body.push(l);
            }
            let body = body.join("\n");
            if feeds_shell {
                out.extend(logical_lines(&body));
            } else {
                for sub in substitutions(&body) {
                    out.extend(logical_lines(&sub));
                }
            }
        }
        for sub in substitutions(&line) {
            out.extend(logical_lines(&sub));
        }
        out.push(segs);
    }
    out
}

/// The source text of every segment that runs something, one per command.
/// A printing command is kept only when its output is piped into a shell.
pub fn executed(lines: &[Vec<Segment>]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        for (k, seg) in line.iter().enumerate() {
            let into_shell = line
                .get(k + 1)
                .is_some_and(|n| n.piped && n.head().is_some_and(is_shell));
            if seg.data_only() && !into_shell {
                continue;
            }
            out.push(seg.raw.clone());
        }
    }
    out
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod shell_tests;
