#!/usr/bin/env bash
# Decision matrix for hooks/infra-guard.sh, run against a throwaway repo.
set -u

HOOK="$(cd "$(dirname "$0")/.." && pwd)/hooks/infra-guard.sh"

if ! command -v jq >/dev/null 2>&1; then
  echo "SKIP  test_infra_guard: jq not installed"
  exit 0
fi

LAB="$(mktemp -d)"
trap 'rm -rf "$LAB"' EXIT

mkdir -p "$LAB/scripts" "$LAB/sub" "$LAB/apps" "$LAB/.claude"

cat > "$LAB/.claude/prod-guard-tokens.txt" <<'EOF'
prod:my-company-prod
nonprod:my-company-staging
EOF

cat > "$LAB/Makefile" <<'EOF'
deploy-cdn:
	bash scripts/deploy.sh

logs:
	bash scripts/logs.sh

build:
	echo building the bundle

deploy-inline:
	gcloud run deploy web-cdn --source .
EOF

cat > "$LAB/sub/Makefile" <<'EOF'
deploy-sub:
	gcloud run deploy web-cdn --source .

build:
	echo building the sub bundle
EOF

cat > "$LAB/apps/package.json" <<'EOF'
{"scripts": {"release": "npm publish --access public"}}
EOF

cat > "$LAB/scripts/deploy.sh" <<'EOF'
#!/usr/bin/env bash
gcloud run services update web-cdn --region europe-west2
EOF

# The comment here is load-bearing: it proves comment lines are stripped before
# classification, so a command named only in prose cannot trigger a decision.
cat > "$LAB/scripts/logs.sh" <<'EOF'
#!/usr/bin/env bash
# Reads logs. gcloud run deploy appears in this comment only.
gcloud logging read 'resource.type="cloud_run_revision"' --project my-company-prod --limit 10
EOF

cat > "$LAB/scripts/nested.sh" <<'EOF'
#!/usr/bin/env bash
cd sub && bash inner.sh
EOF

cat > "$LAB/sub/inner.sh" <<'EOF'
#!/usr/bin/env bash
gcloud run deploy web-cdn --source .
EOF

fails=0

check() {
  local label="$1" expect="$2" cmd="$3" depth="${4:-}"
  local json out decision
  json=$(jq -n --arg cmd "$cmd" --arg cwd "$LAB" '{tool_input: {command: $cmd}, cwd: $cwd}')
  if [ -n "$depth" ]; then
    out=$(printf '%s' "$json" | env -u CLAUDE_PROJECT_DIR "INFRA_GUARD_DEPTH=$depth" bash "$HOOK")
  else
    out=$(printf '%s' "$json" | env -u CLAUDE_PROJECT_DIR bash "$HOOK")
  fi
  decision=$(printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecision // "silent"' 2>/dev/null)
  [ -z "$decision" ] && decision=silent
  if [ "$decision" = "$expect" ]; then
    printf 'PASS  %-47s %s\n' "$label" "$decision"
  else
    printf 'FAIL  %-47s expected %s, got %s\n' "$label" "$expect" "$decision"
    fails=$((fails + 1))
  fi
}

# --- literal layer -----------------------------------------------------------
check "literal: gcloud run deploy"                deny   "gcloud run deploy web-cdn"
check "literal: mixed prod + non-prod"            deny   "gcloud run jobs create sync --project my-company-prod --set-env-vars T=my-company-staging"
check "literal: dangerous phrase inside a string" silent "git commit -m 'block gcloud run deploy'"

# --- deep layer and the depth dial -------------------------------------------
check "deep: make target wrapping a deploy"       deny   "make deploy-cdn"
check "deep: cd-prefixed nested script"           deny   "bash scripts/nested.sh"
check "quiet: read-only gcloud with prod token"   silent "make logs"
check "quiet: ordinary build target"              silent "make build"
check "depth 0 disables the deep layer"           silent "make deploy-cdn" 0
check "depth 0 keeps the literal layer"           deny   "gcloud run deploy web-cdn" 0
check "depth 1 catches an inlined recipe"         deny   "make deploy-inline" 1
check "depth 2 reaches the recipe's script"       deny   "make deploy-cdn" 2
check "non-numeric depth falls back to default"   deny   "make deploy-cdn" abc

# --- resolution forms a cd-averse convention pushes you toward ---------------
check "make -C resolves the subdir Makefile"      deny   "make -C sub deploy-sub"
check "make -C . resolves the root Makefile"      deny   "make -C . deploy-inline"
check "make -C stays quiet on a build target"     silent "make -C sub build"
check "bun --cwd resolves the package script"     deny   "bun --cwd apps run release"
check "./script.sh is scanned"                    deny   "./scripts/deploy.sh"
check "relative path with no dot is scanned"      deny   "scripts/deploy.sh"

# --- gcloud surface prefixes and global flags --------------------------------
check "gcloud alpha run deploy"                   deny   "gcloud alpha run deploy web-cdn"
check "gcloud beta run deploy"                    deny   "gcloud beta run deploy web-cdn --source ."
check "gcloud --project X run deploy"             deny   "gcloud --project foo run deploy web-cdn"
check "gcloud --project=X run deploy"             deny   "gcloud --project=foo run deploy web-cdn"

# --- kubernetes --------------------------------------------------------------
check "kubectl delete namespace"                  deny   "kubectl delete namespace staging"
check "helm uninstall"                            deny   "helm uninstall web -n prod"
check "kubectl delete deployment"                 ask    "kubectl delete deployment api -n default"
check "kubectl apply"                             ask    "kubectl apply -f manifest.yaml"
check "kubectl get is read-only"                  silent "kubectl get pods -n default"

# --- terraform ---------------------------------------------------------------
check "terraform apply"                           deny   "terraform apply"
check "terraform -chdir apply"                    deny   "terraform -chdir=infra/dev apply -auto-approve"
check "terraform -chdir state rm"                 deny   "terraform -chdir=infra/dev state rm aws_s3_bucket.x"
check "terraform -chdir plan stays quiet"         silent "terraform -chdir=infra/dev plan"

# --- destructive local commands and the git early-exit -----------------------
check "npm publish"                               deny   "npm publish"
check "rm -r reaching outside the tree"           ask    "rm -rf ../some-repo"
check "git prefix does not cover a separator"     deny   "git commit -m x ; gcloud run deploy cdn"
check "plain git commit still exits early"        silent "git commit -m 'ship it'"

printf '\n%s failure(s)\n' "$fails"
exit "$fails"
