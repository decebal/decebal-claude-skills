#!/usr/bin/env bash
# infra-guard-trust.sh '<command>'
# Vet a wrapper chain, then record its combined content-hash as kosher so the
# infra-guard deep scan trusts it until any file in the chain changes.
#
# Records YOUR judgement after you review the chain — it does not re-judge. Run
# from the repo the chain belongs to (uses CLAUDE_PROJECT_DIR or the current dir
# as the tree root).
set -u
. "$(dirname "${BASH_SOURCE[0]}")/infra-guard-scan.sh"

cmd="${1:-}"
[ -z "$cmd" ] && { echo "usage: infra-guard-trust.sh '<command>'" >&2; exit 2; }

root="${CLAUDE_PROJECT_DIR:-$PWD}"
scan_tree "$cmd" "$root" "$PWD"
th="$(tree_hash)"

if [ -z "$th" ]; then
  echo "Nothing project-local to trust in: $cmd" >&2
  echo "(no scripts resolved under $root — nothing to hash)" >&2
  exit 1
fi
if is_kosher "$th"; then
  echo "Already kosher ($th): $cmd"
  exit 0
fi

printf '%s  %s\n' "$th" "$cmd" >> "$KOSHER_FILE"
echo "Trusted ($th): $cmd"
echo "Hashed units (file content + make/node recipe text):"
printf '  %s\n' "${SCAN_UNITS[@]}"
if [ "$SCAN_RISK" = 1 ]; then
  echo "NOTE: chain references infra tools / prod tokens — trust covers only what resolved." >&2
fi
