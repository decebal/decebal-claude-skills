//! Sibling test file (rules/testing-gates.md — no inline test modules).

use super::*;

fn run(text: &str) -> Vec<String> {
    executed(&logical_lines(text))
}

#[test]
fn separators_inside_quotes_do_not_split() {
    let segs = segments("echo \"git push -u origin x && gh pr create\" ; ls");
    assert_eq!(segs.len(), 2, "{segs:?}");
    assert_eq!(segs[0].words[1], "git push -u origin x && gh pr create");
    assert_eq!(segs[1].head(), Some("ls"));
}

#[test]
fn a_pipe_marks_the_next_segment() {
    let segs = segments("cat x.sh | bash");
    assert!(!segs[0].piped);
    assert!(segs[1].piped);
}

#[test]
fn redirections_are_not_words() {
    let segs = segments("bash < x.sh 2>&1 >/dev/null");
    assert_eq!(segs[0].words, vec!["bash".to_string()]);
    assert_eq!(segs[0].stdin.as_deref(), Some("x.sh"));
}

#[test]
fn a_trailing_comment_is_dropped() {
    let segs = segments("ls -la # git push --force origin main");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].raw, "ls -la");
}

#[test]
fn prefix_runners_are_skipped_to_reach_the_command() {
    let words: Vec<String> = ["FOO=1", "timeout", "-k", "5", "300", "./x.sh", "a"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(command_words(&words)[0], "./x.sh");
}

#[test]
fn printed_text_is_not_executed() {
    assert!(run("echo \"  git push --force origin main\"").is_empty());
    assert!(run("printf '%s\\n' 'git push -f origin main'").is_empty());
}

#[test]
fn a_print_piped_into_a_shell_is_executed() {
    assert_eq!(run("echo 'git push -f origin main' | bash").len(), 2);
}

#[test]
fn a_substitution_inside_an_echo_still_runs() {
    let cmds = run("echo \"pushed: $(git push --force origin main)\"");
    assert!(
        cmds.iter().any(|c| c == "git push --force origin main"),
        "{cmds:?}"
    );
}

#[test]
fn heredoc_help_text_is_dropped() {
    let cmds = run("cat <<'EOF'\nusage: git push --force origin main\nEOF\nls\n");
    assert_eq!(cmds, vec!["ls".to_string()]);
}

#[test]
fn a_heredoc_fed_to_a_shell_is_kept() {
    let cmds = run("bash <<EOF\ngit push -f origin main\nEOF\n");
    assert!(
        cmds.iter().any(|c| c == "git push -f origin main"),
        "{cmds:?}"
    );
}

#[test]
fn continuations_and_multi_line_strings_join() {
    let cmds = run("git push \\\n  --force origin main\n");
    assert_eq!(cmds.len(), 1, "{cmds:?}");
    assert!(cmds[0].contains("--force"), "{cmds:?}");

    let cmds = run("echo \"line one\nline two\"\nls\n");
    assert_eq!(cmds, vec!["ls".to_string()]);
}

#[test]
fn an_apostrophe_in_a_comment_does_not_open_a_string() {
    let cmds = run("ls # don't\ngit status\n");
    assert_eq!(cmds, vec!["ls".to_string(), "git status".to_string()]);
}
