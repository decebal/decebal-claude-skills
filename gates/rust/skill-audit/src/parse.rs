//! Pure parsing of listing entries, names and dates.

/// Listing entries start with `- ` at column 0; a description may continue on
/// later lines.
pub fn split_entries(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in content.split('\n') {
        if line.starts_with("- ") || out.is_empty() {
            out.push(line.to_owned());
        } else if let Some(last) = out.last_mut() {
            last.push('\n');
            last.push_str(line);
        }
    }
    out
}

/// The entry's name, and whether it carries a description (`- name: text`).
/// An entry the listing budget truncated is the bare name.
pub fn entry_name(entry: &str) -> (&str, bool) {
    let body = entry.strip_prefix("- ").unwrap_or(entry);
    match body.find(": ") {
        Some(i) if !body[..i].contains(' ') => (&body[..i], true),
        _ => (body.trim_end(), false),
    }
}

/// Howard Hinnant's days-to-civil, UTC.
pub fn civil_date(secs: i64) -> String {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A skill, agent or server name, or `None` for anything that is not one
/// (prose after a slash, an empty tag).
pub fn clean(name: &str) -> Option<String> {
    let n = name.trim().trim_start_matches('/');
    (!n.is_empty()
        && n.len() <= 80
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || ":_.-".contains(c)))
    .then(|| n.to_owned())
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod parse_tests;
