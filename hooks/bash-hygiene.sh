#!/usr/bin/env bash
# PreToolUse hook — blocks compound Bash commands, command substitution and combined redirects.
#
# Quote-stripping is non-nested and ignores heredocs and escaped quotes. Do not
# grow it into a parser: the failure mode is a false negative, never a bad block.

input=$(cat)

tool=$(printf '%s' "$input" | jq -r '.tool_name // ""')
cmd=$(printf '%s' "$input"  | jq -r '.tool_input.command // ""')

if [ "$tool" != "Bash" ] || [ -z "$cmd" ]; then
  exit 0
fi

# -0777 slurps as one string; without it a quoted value spanning newlines survives.
stripped=$(printf '%s' "$cmd" | perl -0777 -pe "s/'[^']*'//g; s/\"[^\"]*\"//g;")

violations=()
# Which remediation lines to print. A block that explains all four classes when
# one fired is ~70 wasted tokens, and exit-2 stderr is billed to model context.
want_split=0; want_inner=0; want_capture=0; want_scratch=0

case "$stripped" in
  *'&&'*) violations+=("'&&' chain"); want_split=1 ;;
esac
case "$stripped" in
  *'||'*) violations+=("'||' chain"); want_split=1 ;;
esac
case "$stripped" in
  *';'*) violations+=("';' separator"); want_split=1 ;;
esac
# Newline separates two commands exactly as ';' does, and the permission matcher
# treats it the same way. A heredoc body is one command's data, so skip those.
# The pattern must be $'\n' — a $(printf) here yields "" and matches everything.
newline=$'\n'
case "$stripped" in
  *'<<'*) : ;;
  *"$newline"*) violations+=("newline separator"); want_split=1 ;;
esac
case "$stripped" in
  *'$('*) violations+=('$() substitution'); want_inner=1 ;;
esac
case "$stripped" in
  *'<('*|*'>('*) violations+=("<() process substitution"); want_inner=1 ;;
esac
case "$stripped" in
  *'`'*) violations+=("backtick substitution"); want_inner=1 ;;
esac
case "$stripped" in
  *'| tee'*|*'|tee'*) violations+=("'| tee' output capture"); want_capture=1 ;;
esac
case "$stripped" in
  *'&>'*) violations+=("'&>' combined redirect"); want_capture=1 ;;
esac
case "$stripped" in
  *'2>&1'*) violations+=("'2>&1' stderr merge"); want_capture=1 ;;
esac

# Opt-in: unset CLAUDE_SCRATCH_DIR means no opinion on where temp files go.
if [ -n "${CLAUDE_SCRATCH_DIR:-}" ]; then
  # Keep the boundary anchors — a bare /tmp match hits /tmpfs/ and mytmp/.
  if [[ "$stripped" =~ (^|[[:space:]=])/tmp(/|[[:space:]]|$) ]]; then
    violations+=("'/tmp/' temp path"); want_scratch=1
  fi
  if [[ "$stripped" =~ (^|[[:space:]=])/private/tmp(/|[[:space:]]|$) ]]; then
    violations+=("'/private/tmp/' temp path"); want_scratch=1
  fi
fi

[ ${#violations[@]} -eq 0 ] && exit 0

# A trailing `2>&1` with no other redirect is repairable without changing what
# runs — the harness captures both streams — so rewrite rather than spend a round
# trip. Piped or otherwise redirected forms are left alone: there the merge decides
# which stream the next stage reads.
if [ ${#violations[@]} -eq 1 ] && [ "$want_capture" = 1 ] \
   && [[ "$cmd" =~ [[:space:]]2\>\&1[[:space:]]*$ ]] \
   && [ "$(printf '%s' "$cmd" | tr -cd '>' | wc -c | tr -d ' ')" = "1" ]; then
  fixed=$(printf '%s' "$cmd" | sed -E 's/[[:space:]]+2>&1[[:space:]]*$//')
  printf '%s' "$input" | jq -c --arg c "$fixed" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse",
                           updatedInput: (.tool_input + {command: $c})}}'
  exit 0
fi

{
  printf 'Blocked by bash-hygiene: %s\n' "${violations[*]}"
  [ "$want_split" = 1 ]   && printf 'Split into separate Bash calls, or use a tool flag (-C, --cwd, --manifest-path, -p).\n'
  [ "$want_inner" = 1 ]   && printf 'Run the inner command as its own Bash call and use its result.\n'
  [ "$want_capture" = 1 ] && printf 'Use run_in_background:true — the harness keeps the full log.\n'
  [ "$want_scratch" = 1 ] && printf 'Use %s for scratch files.\n' "${CLAUDE_SCRATCH_DIR:-}"
  :
} >&2
exit 2
