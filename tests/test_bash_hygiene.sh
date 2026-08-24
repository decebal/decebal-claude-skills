#!/usr/bin/env bash
# Decision matrix for hooks/bash-hygiene.sh: allow / rewrite / block.
set -u

HOOK="$(cd "$(dirname "$0")/.." && pwd)/hooks/bash-hygiene.sh"

for dep in jq perl; do
  if ! command -v "$dep" >/dev/null 2>&1; then
    echo "SKIP  test_bash_hygiene: $dep not installed"
    exit 0
  fi
done

fails=0

check() {
  local label="$1" expect="$2" cmd="$3" out rc got
  out=$(jq -n --arg c "$cmd" '{tool_name:"Bash",tool_input:{command:$c,description:"d"}}' | bash "$HOOK" 2>/dev/null)
  rc=$?
  if [ "$rc" = 2 ]; then got=block
  elif [ -n "$out" ]; then got="rewrite:$(printf '%s' "$out" | jq -r '.hookSpecificOutput.updatedInput.command')"
  else got=allow
  fi
  if [ "$got" = "$expect" ]; then
    printf 'PASS  %-44s %s\n' "$label" "$got"
  else
    printf 'FAIL  %-44s expected %s, got %s\n' "$label" "$expect" "$got"
    fails=$((fails + 1))
  fi
}

check "plain command"              allow           "make build"
check "quoted semicolon"           allow           "git commit -m 'a; b'"
check "heredoc keeps its newlines" allow           'cat <<EOF
body
EOF'
check "&& chain"                   block           "ls && pwd"
check "newline chain"              block           'ls
pwd'
check "process substitution"       block           "diff <(ls) <(ls)"
check "trailing 2>&1 is rewritten" "rewrite:gh pr view 1" "gh pr view 1 2>&1"
check "2>&1 before a pipe blocks"  block           "make build 2>&1 | tail -5"
check "2>&1 with a file redirect"  block           "make build > log 2>&1"
check "2>&1 plus a chain blocks"   block           "ls 2>&1 && pwd"
check "no violation, no output"    allow           "ls -la"

# A rewrite replaces the whole tool_input, so the sibling keys have to survive it.
out=$(jq -n '{tool_name:"Bash",tool_input:{command:"ls 2>&1",description:"keep me",timeout:5000}}' | bash "$HOOK" 2>/dev/null)
kept=$(printf '%s' "$out" | jq -r '[.hookSpecificOutput.updatedInput.description, (.hookSpecificOutput.updatedInput.timeout|tostring)] | join("/")')
if [ "$kept" = "keep me/5000" ]; then
  printf 'PASS  %-44s %s\n' "rewrite preserves sibling input fields" "$kept"
else
  printf 'FAIL  %-44s expected "keep me/5000", got %s\n' "rewrite preserves sibling input fields" "$kept"
  fails=$((fails + 1))
fi

printf '\n%s failure(s)\n' "$fails"
exit "$fails"
