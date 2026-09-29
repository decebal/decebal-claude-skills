use super::*;

fn fm(text: &str) -> (Option<String>, Option<String>) {
    let f = parse(text);
    (f.name, f.description)
}

#[test]
fn plain_scalars() {
    let (name, desc) =
        fm("---\nname: pdf-tools\ndescription: Read and write PDF files.\n---\n# body\n");
    assert_eq!(name.as_deref(), Some("pdf-tools"));
    assert_eq!(desc.as_deref(), Some("Read and write PDF files."));
}

#[test]
fn plain_scalar_continues_on_indented_lines() {
    let (_, desc) =
        fm("---\ndescription: Starts here\n  and continues\n  over lines.\nname: x\n---\n");
    assert_eq!(
        desc.as_deref(),
        Some("Starts here and continues over lines.")
    );
}

#[test]
fn plain_scalar_drops_trailing_comment_but_keeps_hash_inside_words() {
    let (name, desc) = fm("---\nname: tool # the tool\ndescription: Use for C# work\n---\n");
    assert_eq!(name.as_deref(), Some("tool"));
    assert_eq!(desc.as_deref(), Some("Use for C# work"));
}

#[test]
fn double_quoted_with_escapes_and_colons() {
    let (_, desc) = fm("---\ndescription: \"Use when: the user says \\\"ship\\\"\"\n---\n");
    assert_eq!(desc.as_deref(), Some("Use when: the user says \"ship\""));
}

#[test]
fn single_quoted_doubles_the_apostrophe() {
    let (name, _) = fm("---\nname: 'it''s: fine'\n---\n");
    assert_eq!(name.as_deref(), Some("it's: fine"));
}

#[test]
fn quoted_scalar_over_several_lines_folds_to_spaces() {
    let (_, desc) = fm("---\ndescription: \"one\n  two\"\n---\n");
    assert_eq!(desc.as_deref(), Some("one two"));
}

#[test]
fn folded_block_joins_lines_and_keeps_paragraphs() {
    let text =
        "---\ndescription: >\n  First line\n  continues here.\n\n  New paragraph.\nname: x\n---\n";
    let (name, desc) = fm(text);
    assert_eq!(
        desc.as_deref(),
        Some("First line continues here.\nNew paragraph.")
    );
    assert_eq!(name.as_deref(), Some("x"));
}

#[test]
fn literal_block_keeps_newlines() {
    let (_, desc) = fm("---\ndescription: |-\n    line one\n    line two\n---\n");
    assert_eq!(desc.as_deref(), Some("line one\nline two"));
}

#[test]
fn folded_block_with_keep_indicator_is_trimmed() {
    let (_, desc) = fm("---\ndescription: >+\n  only line\n\n---\n");
    assert_eq!(desc.as_deref(), Some("only line"));
}

#[test]
fn missing_name_is_none() {
    let (name, desc) = fm("---\ndescription: No name here\n---\n");
    assert_eq!(name, None);
    assert_eq!(desc.as_deref(), Some("No name here"));
}

#[test]
fn nested_keys_are_not_read_as_top_level() {
    let (name, _) = fm("---\nmetadata:\n  name: inner\nname: outer\n---\n");
    assert_eq!(name.as_deref(), Some("outer"));
}

#[test]
fn no_frontmatter_or_unclosed_block_yields_nothing() {
    assert_eq!(parse("# Title\nname: x\n"), Frontmatter::default());
    assert_eq!(parse("---\nname: x\n"), Frontmatter::default());
}

#[test]
fn byte_order_mark_and_crlf_are_accepted() {
    let (name, desc) = fm("\u{feff}---\r\nname: bom\r\ndescription: Windows file\r\n---\r\n");
    assert_eq!(name.as_deref(), Some("bom"));
    assert_eq!(desc.as_deref(), Some("Windows file"));
}
