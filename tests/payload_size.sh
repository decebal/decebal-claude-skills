#!/usr/bin/env bash
# Measures what each hook injects into model context per firing.
set -u

HOOKS="$(cd "$(dirname "$0")/.." && pwd)/hooks"

if ! command -v jq >/dev/null 2>&1; then
  echo "SKIP  payload_size: jq not installed"
  exit 0
fi

row() { printf '%-42s %6s B  ~%4s tok\n' "$1" "$2" "$(( ($2 + 3) / 4 ))"; }

n=$(printf '%s' '{"tool_name":"Edit","tool_input":{"file_path":"/x/a.ts","new_string":"// set up the listener\nconst a = 1;"}}' \
  | bash "$HOOKS/comment-hygiene.sh" | jq -r '.hookSpecificOutput.additionalContext' | wc -c | tr -d ' ')
row "comment-hygiene additionalContext (1 comment)" "$n"

n=$(printf '%s' '{"tool_name":"Edit","tool_input":{"file_path":"/x/a.ts","new_string":"// one\n// two\n// three\n// four\n// five\nconst a = 1;"}}' \
  | bash "$HOOKS/comment-hygiene.sh" | jq -r '.hookSpecificOutput.additionalContext' | wc -c | tr -d ' ')
row "comment-hygiene additionalContext (5 comments)" "$n"

if command -v perl >/dev/null 2>&1; then
  err=$(mktemp)
  printf '%s' '{"tool_name":"Bash","tool_input":{"command":"ls && pwd"}}' | bash "$HOOKS/bash-hygiene.sh" 2>"$err" >/dev/null
  row "bash-hygiene stderr (one violation)" "$(wc -c <"$err" | tr -d ' ')"
  rm -f "$err"
fi

n=$(printf '%s' '{"tool_input":{"command":"gcloud run deploy cdn"},"cwd":"/tmp"}' \
  | bash "$HOOKS/infra-guard.sh" | jq -r '.hookSpecificOutput.permissionDecisionReason' | wc -c | tr -d ' ')
row "infra-guard deny reason (billed)" "$n"

# ask reasons are shown to the user only, so this line is free at runtime.
n=$(printf '%s' '{"tool_input":{"command":"sudo launchctl list"},"cwd":"/tmp"}' \
  | bash "$HOOKS/infra-guard.sh" | jq -r '.hookSpecificOutput.permissionDecisionReason' | wc -c | tr -d ' ')
row "infra-guard ask reason (not billed)" "$n"
