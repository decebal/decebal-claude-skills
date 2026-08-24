#!/usr/bin/env bash
# PreToolUse guard rail — blocks/prompts on high-blast-radius Bash commands.
# Matcher "Bash". Reads the hook JSON on stdin; emits a permission decision.
#
# Literal layer (always on): classify() the typed command (deny/ask tiers) +
# prod-token rules + --all breadth. Deep layer (on by default, INFRA_GUARD_DEPTH
# layers): scans wrapper chains. All deny/ask PATTERNS live once, in
# infra-guard-scan.sh.
#
# INFRA_GUARD_OFF=1 downgrades every would-be deny to an ask (never a silent
# allow) — it lets the user approve a normally-blocked command at the prompt; it
# does NOT disable the guard. True disable = remove this hook from settings.json.

command -v jq >/dev/null 2>&1 || { echo "infra-guard: jq not found — guard inactive" >&2; exit 0; }

input="$(cat)"
cmd="$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null)"
cwd="$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null)"
[ -z "$cmd" ] && exit 0

scan_lib="$(dirname "${BASH_SOURCE[0]}")/infra-guard-scan.sh"
[ -f "$scan_lib" ] && . "$scan_lib"

lc="$(printf '%s' "$cmd" | tr '[:upper:]' '[:lower:]')"

# Match against a skeleton with quoted data stripped, so a dangerous phrase
# inside a "…"/'…' string (message, echo arg) is not mistaken for an executed
# command. Shell -c / eval forms keep their quoted body — it IS executed.
case "$lc" in
  *bash\ -c\ *|*sh\ -c\ *|*zsh\ -c\ *|*eval\ *) scan="$lc" ;;
  *) scan="$(printf '%s' "$lc" | sed -e 's/"[^"]*"//g' -e "s/'[^']*'//g")" ;;
esac
has() { printf '%s' "$scan" | grep -Eq -- "$1"; }

# Known-safe local git subcommands never touch infra — allow before any pattern
# matching. Their -m/-F message text is data, not an executed command (a commit
# message like "block gcloud run services update" must not trip the guard).
# git push stays OUT of this list — force-push to a protected branch is caught.
#
# Matched on the quote-stripped skeleton, and only when no separator survives
# it: a raw prefix match would wave through everything after `git commit -m x ;`.
case "$scan" in
  *'&&'*|*'||'*|*';'*|*'|'*|*'$('*|*'`'*|*'&'*|*"$(printf '\n')"*) : ;;
  git\ commit*|git\ add*|git\ status*|git\ diff*|git\ log*|git\ show*|git\ stash*|git\ fetch*|git\ branch*|git\ checkout*|git\ switch*|git\ restore*|git\ tag*|git\ cherry-pick*|git\ revert*|git\ blame*)
    exit 0 ;;
esac

# INFRA_GUARD_OFF=1 downgrades a deny to an ask — the user approves a normally
# blocked command at the prompt; the guard is never silently bypassed.
deny() {
  [ "${INFRA_GUARD_OFF:-0}" = "1" ] && ask "NORMALLY BLOCKED — $1  (INFRA_GUARD_OFF is set: approve only if you intend this)"
  jq -n --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"deny",permissionDecisionReason:$r}}'; exit 0
}
ask()  { jq -n --arg r "$1" '{hookSpecificOutput:{hookEventName:"PreToolUse",permissionDecision:"ask",permissionDecisionReason:$r}}'; exit 0; }

# Gates the prod-token and breadth rules to commands that actually mutate state.
is_mutating() { has '(update|delete|deploy|destroy|create| apply|publish|unpublish|deprecate|set-iam|add-iam|remove-iam|--force|prune|rmi|reset +--hard|rm +-[a-z]*r)'; }

# --- literal command: shared classify() (deny wins; exits on deny) ---
lit=""
command -v classify >/dev/null 2>&1 && lit="$(classify "$scan")"
case "$lit" in deny\|*) deny "${lit#deny|} (infra-guard)";; esac

# --- prod-token rules (per-repo data file, then the personal fallback) ---
tokens_file=""
for base in "${CLAUDE_PROJECT_DIR:-}" "$cwd" "$HOME"; do
  if [ -n "$base" ] && [ -f "$base/.claude/prod-guard-tokens.txt" ]; then
    tokens_file="$base/.claude/prod-guard-tokens.txt"; break
  fi
done
prod_hit=0; nonprod_hit=0
if [ -n "$tokens_file" ]; then
  esc() { printf '%s' "$1" | sed 's/[][\.*^$(){}+?|]/\\&/g'; }
  prod_re=""; nonprod_re=""
  while IFS= read -r line; do
    case "$line" in
      \#*|"") : ;;
      prod:*)    t="$(esc "${line#prod:}")";    [ -n "$t" ] && prod_re="${prod_re:+$prod_re|}$t" ;;
      nonprod:*) t="$(esc "${line#nonprod:}")"; [ -n "$t" ] && nonprod_re="${nonprod_re:+$nonprod_re|}$t" ;;
    esac
  done < "$tokens_file"
  [ -n "$prod_re" ] && has "($prod_re)" && prod_hit=1
  [ -n "$nonprod_re" ] && has "($nonprod_re)" && nonprod_hit=1
fi
if is_mutating; then
  [ "$prod_hit" = 1 ] && [ "$nonprod_hit" = 1 ] && deny "Command targets PROD and non-prod together — never batch prod with anything; split per-env. (infra-guard)"
fi

# --- collect ask-level signals (any deny already exited above) ---
ask_reason=""
case "$lit" in ask\|*) ask_reason="${lit#ask|}";; esac
if is_mutating; then
  [ "$prod_hit" = 1 ] && [ -z "$ask_reason" ] && ask_reason="mutating command touches PROD — state blast radius + rollback, then confirm"
  has '(\-\-all( |$)| -a( |$))' && [ -z "$ask_reason" ] && ask_reason="mutation with --all / -A — wide blast radius; confirm scope"
fi
[ -n "$ask_reason" ] && ask "$ask_reason (infra-guard)"

# --- deep wrapper scan (INFRA_GUARD_DEPTH layers, 0 disables) ---
# Runs last: the literal tiers above already had their say, so a bug here can
# only fail open — never weaken the literal floor.
if [ "${SCAN_MAX_DEPTH:-0}" -gt 0 ] && command -v deep_decide >/dev/null 2>&1; then
  PROD_RE="${prod_re:-}"   # let the deep risk-signal see prod tokens too
  deep_decide "$cmd" "$cwd"
fi

exit 0
