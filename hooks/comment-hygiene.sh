#!/usr/bin/env bash
# PostToolUse hook — feeds back the comment lines an edit added, as context for the assistant to act on.

input=$(cat)

tool=$(printf '%s' "$input" | jq -r '.tool_name // ""')
path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // ""')

case "$tool" in
  Edit | Write | MultiEdit) ;;
  *) exit 0 ;;
esac

# Per language: '#' opens a comment in shell and Python but names a private field in TypeScript.
case "$path" in
  *.ts | *.tsx | *.js | *.jsx | *.mjs | *.cjs) markers='//|/\*|\*' ;;
  *.rs | *.go | *.java | *.kt | *.swift) markers='//|/\*|\*' ;;
  *.c | *.h | *.cpp | *.hpp | *.cc | *.css | *.scss) markers='//|/\*|\*' ;;
  *.pug) markers='//-|//' ;;
  *.py | *.rb | *.sh | *.bash | *.zsh) markers='#' ;;
  *) exit 0 ;;
esac

added=$(printf '%s' "$input" | jq -r '
  [.tool_input.new_string?, .tool_input.content?, (.tool_input.edits? // [])[]?.new_string]
  | map(select(. != null)) | join("\n")
')

if [ -z "$added" ]; then
  exit 0
fi

# Line-leading markers only, so a URL or a "//" inside a string literal is not
# flagged. Unnumbered: an offset into the edit reads as a file line and is not one.
# Doc comments (///, //!, /**, #!) are API surface, not the narration this targets.
comments=$(printf '%s\n' "$added" | grep -E "^[[:space:]]*($markers)" \
  | grep -vE '^[[:space:]]*(#!|///|//!|/\*\*)' | head -40)

if [ -z "$comments" ]; then
  exit 0
fi

count=$(printf '%s\n' "$comments" | grep -c '')

# The tests themselves live in CLAUDE.md, which is already in context. Restating
# them here costs ~130 tokens per firing and additionalContext persists in the
# transcript, so the same block would be re-billed on every later turn.
context=$(
  cat <<EOF
comment-hygiene: $count comment line(s) added to $path.

$comments

Re-check each against the three comment tests (CLAUDE.md, Code Style). Delete any
that fail all three. Fix this turn; do not ask, do not report back.
EOF
)

# Advisory by design. Never exit 2 here: a blocking exit surfaces as an error the
# user has to dismiss, for a finding only the assistant needs to act on.
jq -n --arg context "$context" \
  '{hookSpecificOutput: {hookEventName: "PostToolUse", additionalContext: $context}}'

exit 0
