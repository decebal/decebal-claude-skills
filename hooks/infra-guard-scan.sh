#!/usr/bin/env bash
# infra-guard scanning library — THE single source of the deny/ask patterns
# (classify), plus bounded recursive wrapper resolution and the kosher
# (vetted-chain) content-hash cache.
#
# Sourced by infra-guard.sh (hook) and infra-guard-trust.sh (helper).
# Bash 3.2-safe (no associative arrays/mapfile); macOS-safe (shasum, wc, grep -I).
#
# Resolution FOLLOWS project-local text scripts reached by static resolution:
#   [cd <dir> &&] bash/sh/source/. <path> ; ./<path> ; make [-C <dir>] <target> ;
#   $(MAKE) <target> ; [cd <dir> &&] bun|npm|pnpm run <script>.
# STOPS at installed binaries / outside-repo / node_modules|dist|build /
# non-text / already-visited. Caps: INFRA_GUARD_DEPTH layers, file count, file
# size (stop silently).
#
# Decision posture ("silent unless a risk signal"):
#   - clean classify hit on resolved content -> deny / ask.
#   - otherwise, if resolved content uses an infra tool or prod identifier in a
#     MUTATING form without a clean hit (dynamic/partial) -> ask. Read-only use
#     is not a signal, and comment lines are stripped before either check.
#   - otherwise -> silent allow. Benign and fully-unresolvable-benign chains
#     never prompt.
#
# Kosher hash covers every resolved UNIT — script-file content, make-recipe
# text, node-script text — so editing a Makefile recipe or package.json script
# busts the cache even when no script file changed.
#
# Known limits: a script that references a sibling by a path relative to a
# `cd`'d subdir is not resolved (the `cd <dir> && bash <path>` form itself is).
# A dynamic command naming no infra tool is a silent blind spot. Both are
# covered by the literal layer + kosher.

KOSHER_FILE="${INFRA_GUARD_KOSHER:-$HOME/.claude/infra-guard-kosher.txt}"

# Layers of wrapper to follow. 0 follows none, leaving the literal deny/ask
# tiers untouched. A non-numeric value falls back to the default rather than to
# 0, so a typo can never silently drop the deep layer.
case "${INFRA_GUARD_DEPTH:-5}" in
  '' | *[!0-9]*) SCAN_MAX_DEPTH=5 ;;
  *) SCAN_MAX_DEPTH="${INFRA_GUARD_DEPTH:-5}" ;;
esac

SCAN_MAX_FILES="${INFRA_GUARD_MAX_FILES:-40}"
SCAN_MAX_BYTES="${INFRA_GUARD_MAX_BYTES:-524288}"

INFRA_TOOL_RE='gcloud|gsutil|terraform|kubectl|helm|(^| )aws |(^| )az '
MUTATE_RE='(update|delete|deploy|destroy|create| apply|publish|unpublish|deprecate|set-iam|add-iam|remove-iam|--force|prune|rmi| push| cp | rm | mv )'
READONLY_RE='(list|describe|get-value|get-iam-policy|auth login|auth list|logging read| plan| validate| fmt| output|--help|--version|--dry-run)'

SCAN_FILES=(); SCAN_UNITS=(); SCAN_VERDICT=""; SCAN_REASON=""; SCAN_RISK=0

# A surface prefix or a global flag sits between the tool and its command group,
# so `gcloud alpha run deploy` and `gcloud --project X run deploy` reach none of
# the `gcloud +<group>` rules below unless they are folded out first.
fold_tool_prefixes() {
  local t="$1" n i=0
  t="$(printf '%s' "$t" | sed -E 's/(gcloud|gsutil) +(alpha|beta) +/\1 /g')"
  while [ "$i" -lt 5 ]; do
    n="$(printf '%s' "$t" | sed -E \
      -e 's/(gcloud|gsutil) +--[a-z][a-z0-9-]*=[^ ]* +/\1 /g' \
      -e 's/(gcloud|gsutil) +--(project|account|billing-project|configuration|impersonate-service-account|verbosity|format) +[^ -][^ ]* +/\1 /g' \
      -e 's/(gcloud|gsutil) +--[a-z][a-z0-9-]* +(alpha|beta|run|storage|artifacts|iam|container|compute|functions|sql|projects) +/\1 \2 /g')"
    [ "$n" = "$t" ] && break
    t="$n"; i=$((i + 1))
  done
  printf '%s' "$t"
}

# classify <lowercased-text> -> "deny|<reason>" / "ask|<reason>" / nothing.
classify() {
  local t
  t="$(fold_tool_prefixes "$1")"
  _m() { printf '%s' "$t" | grep -Eq -- "$1"; }

  _m 'gcloud +run +(services|jobs) +(update|delete|replace)' && { echo "deny|Cloud Run live-service mutation — never run direct; propose it for manual/CI execution"; return; }
  _m 'gcloud +run +deploy'                                   && { echo "deny|gcloud run deploy mutates a live service — propose it (deploys are CI-only here)"; return; }
  _m '(add-iam-policy-binding|remove-iam-policy-binding|set-iam-policy)' && { echo "deny|IAM change on a live resource — propose it for manual/CI run"; return; }
  _m 'terraform +(-chdir=[^ ]+ +)?(apply|destroy)'           && { echo "deny|terraform apply/destroy mutates real infra — propose it (terraform plan is fine)"; return; }
  _m 'terraform +(-chdir=[^ ]+ +)?state +(rm|mv|push)'       && { echo "deny|terraform state surgery can corrupt shared state — propose it"; return; }
  _m 'gcloud +storage +rm'                                   && { echo "deny|deleting GCS objects (possible prod CDN assets) — propose it"; return; }
  _m 'gsutil +rm'                                            && { echo "deny|gsutil rm deletes storage objects — propose it"; return; }
  _m 'gcloud +artifacts +.*delete'                           && { echo "deny|deleting an artifact/version from GAR — propose it"; return; }
  _m 'helm +uninstall'                                       && { echo "deny|helm uninstall tears down a release — propose it"; return; }
  _m 'kubectl +delete +(namespace|ns|pvc|persistentvolumeclaim)' && { echo "deny|deleting a namespace or PVC destroys live workloads and their data — propose it"; return; }
  if _m '(npm|bun|yarn|pnpm) +publish' && ! _m '\-\-dry-run'; then echo "deny|direct package publish — use a make target / CI, or propose it"; return; fi
  _m '(npm|bun|yarn|pnpm) +(unpublish|deprecate)'            && { echo "deny|npm unpublish/deprecate breaks downstream consumers irreversibly — propose it"; return; }
  if _m 'git +push' && _m '(\-\-force|\-\-force-with-lease| -f( |$))' && _m '(develop|main|master|rc/)'; then
    echo "deny|force-push to a protected branch rewrites shared history"; return
  fi

  _m '(make +[^&|]*publish-prod|publish:prod|publish-prod\.sh)' && { echo "ask|local prod publish (CI-only here, no laptop fallback) — confirm you mean it"; return; }
  _m 'kubectl +(delete|drain|cordon|taint|scale)'   && { echo "ask|kubectl mutates live cluster state — confirm the target and context"; return; }
  _m 'kubectl +(apply|replace|patch|rollout +undo)' && { echo "ask|kubectl writes to the cluster the current context points at — confirm the context"; return; }
  _m 'helm +(upgrade|rollback)'                     && { echo "ask|helm upgrade/rollback replaces a live release — confirm"; return; }
  _m '(^| )pkill( |$)'              && { echo "ask|pkill matches by name — easy to hit the wrong process; confirm the match"; return; }
  _m '(^| )killall( |$)'            && { echo "ask|killall by name — confirm the target"; return; }
  _m 'kill +-(9|kill|s +9|s +kill)' && { echo "ask|kill -9/-KILL is ungraceful — confirm the PID/target"; return; }
  _m 'docker +system +prune'        && { echo "ask|docker system prune wipes images/cache/volumes — confirm"; return; }
  _m '(docker|podman) +(rmi|image +rm|volume +rm)' && { echo "ask|removing container images/volumes — confirm"; return; }
  _m 'rm +-[a-z]+ +/( |$)'          && { echo "ask|rm -rf targeting / — catastrophic; confirm"; return; }
  _m 'rm +-[a-z]+ +/\*'             && { echo "ask|rm -rf /* wipes the filesystem root; confirm"; return; }
  _m 'rm +-[a-z]+ +~( |$)'          && { echo "ask|rm -rf on bare home (~); confirm"; return; }
  _m 'rm +-[a-z]+ +\$home( |$)'     && { echo "ask|rm -rf on \$HOME; confirm"; return; }
  # A `..` target leaves the tree the command was issued from — build output
  # never lives there, so the odds it is a mistake are high.
  _m 'rm +-[a-z]*r[a-z]* +[^ ]*\.\.' && { echo "ask|rm -r reaches outside the current tree (..); confirm the target"; return; }
  _m '(^| )sudo '                   && { echo "ask|sudo — elevated, broad blast radius on a managed machine; confirm"; return; }
  _m 'launchctl +(bootout|unload|disable|kill)' && { echo "ask|launchctl teardown can stop system services; confirm"; return; }
  _m 'gcloud +config +set +project' && { echo "ask|switching gcloud project silently re-aims every later command; confirm"; return; }
}

# --- resolution helpers ------------------------------------------------------
is_project_local() {
  local p="$1" root="$2"
  case "$p" in "$root"/*) ;; *) return 1;; esac
  case "$p" in */node_modules/*|*/.git/*|*/dist/*|*/build/*) return 1;; esac
  return 0
}
is_text() { grep -Iq . "$1" 2>/dev/null; }
strip_comments() { printf '%s\n' "$1" | grep -vE '^[[:space:]]*#'; }

# Read-only invocations are deliberately not a risk signal. Flagging every
# mention of an infra tool made ordinary wrappers prompt, which is what got the
# deep layer switched off rather than fixed.
risky_lines() {
  local text="$1" tokens="$INFRA_TOOL_RE"
  [ -n "${PROD_RE:-}" ] && tokens="$tokens|$PROD_RE"
  printf '%s\n' "$text" | grep -Ei "($tokens)" | grep -Ei "$MUTATE_RE" | grep -Eqvi "$READONLY_RE"
}
file_size() { wc -c < "$1" 2>/dev/null | tr -d ' '; }
content_hash() { printf '%s' "$1" | shasum -a 256 | awk '{print $1}'; }

resolve_script() {
  local p="$1" root="$2" cwd="$3" bare cand
  bare="${p#./}"
  for cand in "$p" "$root/$bare" "$cwd/$bare"; do
    [ -f "$cand" ] && { printf '%s' "$cand"; return; }
  done
}

# make_recipe <target> <root> <cwd> <dir> — <dir> is an optional `-C` directory.
# It must be honoured: `make -C sub build` runs sub/Makefile, and reading the
# root Makefile instead resolves a different recipe or none at all.
make_recipe() {
  local target="$1" root="$2" cwd="$3" dir="$4" mf body
  for mf in "$root/$dir/Makefile" "$cwd/$dir/Makefile" "$root/$dir/makefile" "$root/Makefile"; do
    [ -f "$mf" ] || continue
    body="$(awk -v t="$target" '
      $0 ~ "^"t"[ \t]*:" {inr=1; next}
      inr && /^\t/ {sub(/^\t/,""); print; next}
      inr && /^[^\t]/ {inr=0}
    ' "$mf")"
    [ -n "$body" ] && { printf '%s' "$body"; return; }
  done
}

# node_script <name> <root> <cwd> <dir> — <dir> is an optional `cd`/`--cwd` dir.
node_script() {
  local name="$1" root="$2" cwd="$3" dir="$4" pj body
  for pj in "$root/$dir/package.json" "$cwd/$dir/package.json" "$cwd/package.json" "$root/package.json"; do
    [ -f "$pj" ] || continue
    body="$(jq -r --arg n "$name" '.scripts[$n] // empty' "$pj" 2>/dev/null)"
    [ -n "$body" ] && { printf '%s' "$body"; return; }
  done
}

# extract_refs <text> -> "script:<path>" / "make:<dir>::<target>" /
# "node:<dir>::<name>". make and node refs always carry a (possibly empty) dir.
extract_refs() {
  local text="$1"
  printf '%s\n' "$text" | grep -oE 'cd +[^ ;&|]+ +&& +(bash|sh|source|\.) +[^ ;&|<>]+\.sh' | sed -E 's/cd +([^ ;&|]+) +&& +(bash|sh|source|\.) +/script:\1\//'
  printf '%s\n' "$text" | grep -oE '(bash|sh|source|\.) +[^ ;&|<>]+\.sh' | sed -E 's/^(bash|sh|source|\.) +/script:/'
  # Direct execution: ./x.sh, ../x.sh, scripts/x.sh — no interpreter to key on.
  printf '%s\n' "$text" | grep -oE '(^|[[:space:]])[^ ;&|<>()]*\.sh' | sed -E 's/^[[:space:]]*/script:/'
  printf '%s\n' "$text" | grep -oE '(^|[^a-z])source +[^ ;&|<>]+' | sed -E 's/.*source +/script:/'
  printf '%s\n' "$text" | grep -oE '(^|[^a-z])make +-C +[^ ;&|]+ +[a-zA-Z0-9_.:][a-zA-Z0-9_.:-]*' | sed -E 's/.*make +-C +([^ ]+) +/make:\1::/'
  # -C is excluded from the bare form so the flag is not read as the target.
  printf '%s\n' "$text" | grep -oE '(^|[^a-z])make +[a-zA-Z0-9_.:][a-zA-Z0-9_.:-]*' | sed -E 's/.*make +/make:::/'
  printf '%s\n' "$text" | grep -oE '[$][({]MAKE[)}] +[a-zA-Z0-9_.:-]+' | sed -E 's/.*[)}] +/make:::/'
  printf '%s\n' "$text" | grep -oE 'cd +[^ ;&|]+ +&& +(bun|npm|pnpm) +run +[a-zA-Z0-9:_.-]+' | sed -E 's/cd +([^ ;&|]+) +&& +(bun|npm|pnpm) +run +/node:\1::/'
  printf '%s\n' "$text" | grep -oE '(bun|npm|pnpm) +(--cwd|--prefix|-C) +[^ ;&|]+ +run +[a-zA-Z0-9:_.-]+' | sed -E 's/(bun|npm|pnpm) +(--cwd|--prefix|-C) +([^ ]+) +run +/node:\3::/'
  printf '%s\n' "$text" | grep -oE '(^|[^&] )(bun|npm|pnpm) +run +[a-zA-Z0-9:_.-]+' | sed -E 's/.*(bun|npm|pnpm) +run +/node:::/'
}

# scan_tree <command> <repo-root> <cwd> ; sets SCAN_* globals. Scans the FULL
# tree (no early-out) so SCAN_UNITS is complete for hashing even on a deny.
scan_tree() {
  local initial="$1" root="$2" cwd="$3"
  local seen="" work=() ref ref2 item depth kind val rp content code v vk vr mdir mtarget ndir nname nfiles=0
  SCAN_FILES=(); SCAN_UNITS=(); SCAN_VERDICT=""; SCAN_REASON=""; SCAN_RISK=0

  while IFS= read -r ref; do [ -n "$ref" ] && work+=("1|$ref"); done < <(extract_refs "$initial" | sort -u)

  while [ "${#work[@]}" -gt 0 ]; do
    item="${work[0]}"; work=("${work[@]:1}")
    depth="${item%%|*}"; ref="${item#*|}"
    case "$seen" in *"|$ref|"*) continue;; esac
    seen="$seen|$ref|"
    [ "$depth" -gt "$SCAN_MAX_DEPTH" ] && continue
    [ "$nfiles" -ge "$SCAN_MAX_FILES" ] && break

    kind="${ref%%:*}"; val="${ref#*:}"; content=""
    case "$kind" in
      script)
        rp="$(resolve_script "$val" "$root" "$cwd")"
        [ -z "$rp" ] && continue
        is_project_local "$rp" "$root" || continue
        is_text "$rp" || continue
        [ "$(file_size "$rp")" -gt "$SCAN_MAX_BYTES" ] && continue
        content="$(cat "$rp" 2>/dev/null)"
        SCAN_FILES+=("$rp"); nfiles=$((nfiles + 1))
        SCAN_UNITS+=("$(content_hash "$content")  $rp")
        ;;
      make)
        mdir="${val%%::*}"; mtarget="${val##*::}"
        content="$(make_recipe "$mtarget" "$root" "$cwd" "$mdir")"
        [ -z "$content" ] && continue
        SCAN_UNITS+=("$(content_hash "$content")  make:$mdir:$mtarget")
        ;;
      node)
        ndir="${val%%::*}"; nname="${val##*::}"
        content="$(node_script "$nname" "$root" "$cwd" "$ndir")"
        [ -z "$content" ] && continue
        SCAN_UNITS+=("$(content_hash "$content")  node:$ndir:$nname")
        ;;
      *) continue;;
    esac

    code="$(strip_comments "$content")"

    if [ "$SCAN_RISK" = 0 ]; then
      risky_lines "$code" && SCAN_RISK=1
    fi

    v="$(classify "$(printf '%s' "$code" | tr '[:upper:]' '[:lower:]')")"
    if [ -n "$v" ]; then
      vk="${v%%|*}"; vr="${v#*|}"
      if [ "$vk" = "deny" ]; then [ "$SCAN_VERDICT" != "deny" ] && SCAN_REASON="$vr (in $val)"; SCAN_VERDICT="deny"; fi
      if [ "$vk" = "ask" ] && [ "$SCAN_VERDICT" != "deny" ]; then SCAN_VERDICT="ask"; [ -z "$SCAN_REASON" ] && SCAN_REASON="$vr (in $val)"; fi
    fi

    while IFS= read -r ref2; do [ -n "$ref2" ] && work+=("$((depth + 1))|$ref2"); done < <(extract_refs "$code" | sort -u)
  done
  return 0
}

# tree_hash -> combined sha256 over the sorted set of resolved unit hashes.
tree_hash() {
  [ "${#SCAN_UNITS[@]}" -eq 0 ] && return
  printf '%s\n' "${SCAN_UNITS[@]}" | sort -u | shasum -a 256 | awk '{print $1}'
}

is_kosher() { [ -n "$1" ] && [ -f "$KOSHER_FILE" ] && grep -q "^$1  " "$KOSHER_FILE"; }

# deep_decide <command> <cwd> — emits a deny/ask (and exits) on a finding;
# returns silently otherwise. Relies on deny()/ask() from the sourcing hook.
deep_decide() {
  local cmd="$1" cwd="$2" root th
  root="${CLAUDE_PROJECT_DIR:-$cwd}"
  case "$cmd" in
    *bash\ *|*sh\ *|*make\ *|*source\ *|*.\ *|*bun\ *|*npm\ *|*pnpm\ *|*.sh*) : ;;
    *) return ;;
  esac
  scan_tree "$cmd" "$root" "$cwd" || return
  th="$(tree_hash)"
  if [ "$SCAN_VERDICT" = "deny" ]; then
    is_kosher "$th" && return
    deny "Wrapper chain runs a blocked command — $SCAN_REASON. Review the chain; propose it instead. (infra-guard deep)"
  fi
  if [ "$SCAN_VERDICT" = "ask" ]; then
    is_kosher "$th" && return
    ask "Wrapper chain runs — $SCAN_REASON. Confirm. (infra-guard deep)"
  fi
  if [ "$SCAN_RISK" = 1 ]; then
    is_kosher "$th" && return
    ask "Wrapper chain references an infra tool / prod token with no clean match (dynamic command?). Review it; trust with infra-guard-trust.sh '$cmd' if safe. (infra-guard deep)"
  fi
  return
}
