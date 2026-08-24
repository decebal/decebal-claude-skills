# Guard-rail hooks

Three hooks that make Claude Code safer and cheaper to run, plus the scanning
library they share. Drop them in `~/.claude/hooks/` for every project, or
`.claude/hooks/` for one.

They need `jq`; `bash-hygiene.sh` also needs `perl`. A missing dependency makes a
hook no-op rather than fail closed — check your tooling before relying on them.

| Hook | Event | Effect |
|---|---|---|
| `infra-guard.sh` | PreToolUse, Bash | Denies or prompts on high-blast-radius commands, by blast radius rather than apparent simplicity |
| `bash-hygiene.sh` | PreToolUse, Bash | Blocks compound commands, substitution and combined redirects. Rewrites a repairable `2>&1` instead of blocking it |
| `comment-hygiene.sh` | PostToolUse, Edit/Write | Feeds back the comment lines an edit added, to be justified or deleted |

`infra-guard-scan.sh` is a library, not a hook — every deny/ask pattern and the
wrapper resolver live there, sourced by `infra-guard.sh` and
`infra-guard-trust.sh`.

## Install

```sh
cp hooks/*.sh ~/.claude/hooks/
cp configs/prod-guard-tokens.txt ~/.claude/prod-guard-tokens.txt   # then edit it
chmod +x ~/.claude/hooks/*.sh
```

Register them in `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/infra-guard.sh", "timeout": 10 }] },
      { "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/bash-hygiene.sh", "timeout": 5 }] }
    ],
    "PostToolUse": [
      { "matcher": "Edit|Write|MultiEdit",
        "hooks": [{ "type": "command", "command": "$HOME/.claude/hooks/comment-hygiene.sh", "timeout": 5 }] }
    ]
  }
}
```

For a project-scoped install use `$CLAUDE_PROJECT_DIR/.claude/hooks/...` instead.

## infra-guard

Denies live-service mutation, IAM changes, `terraform apply`/`destroy`/state
surgery (bare and `-chdir=`), storage and artefact deletion, `helm uninstall`,
`kubectl delete` of a namespace or PVC, direct package publishes, and force-pushes
to a protected branch. Prompts for other `kubectl`/`helm` writes, `pkill`-class
commands, `sudo`, `rm -rf` on a root, home or `..` path, and any mutation touching
a production identifier.

Production and non-production identifiers live in `prod-guard-tokens.txt`,
resolved from `CLAUDE_PROJECT_DIR`, then the session cwd, then `~/.claude/`. A
mutating command matching both a prod and a non-prod token is denied outright, so
one command can never span environments.

### Wrapper chains

A blocked command hidden one layer down is still a blocked command, so the guard
follows chains inside the repo and classifies what they actually run:

- `[cd <dir> &&] bash|sh|source|. <path>`
- `./<path>.sh`, `<dir>/<path>.sh` — direct execution, no interpreter
- `make [-C <dir>] <target>`, `$(MAKE) <target>`
- `[cd <dir> &&] bun|npm|pnpm run <script>`, and the `--cwd`/`--prefix`/`-C` forms

`-C` and `--cwd` matter more than they look. Any convention that prefers them over
`cd <dir> && …` routes every command through the flag form, so a resolver blind to
it is blind to the common case.

`INFRA_GUARD_DEPTH` sets how many layers to follow: default 5, 0 follows none
while leaving the literal tiers untouched. A non-numeric value falls back to the
default rather than to 0, so a typo cannot silently drop the layer. Layers count
per hop and a make recipe is a hop — at depth 1 `make deploy` sees the recipe text
(catching an inlined `gcloud run deploy`) but not the body of a script the recipe
calls, which is depth 2.

Only a *mutating* use of an infra tool or a production identifier is a risk
signal. Read-only invocations (`gcloud … list`, `terraform plan`,
`gcloud logging read`) and comment lines are ignored, so ordinary wrappers stay
silent — without that rule the deep layer prompts on everything and gets switched
off, which defeats it.

If a chain prompts and is genuinely safe, fix the guard or record your review:

```sh
~/.claude/hooks/infra-guard-trust.sh '<command>'
```

That hashes every resolved unit — script contents, make recipe text, package
script text — and trusts the chain until any of them changes. Editing a Makefile
recipe busts the cache even when no script file changed. Lowering the depth to
silence a prompt removes the protection instead of fixing it.

`INFRA_GUARD_OFF=1` downgrades a deny to a prompt. It never silently allows.

### Known limits

- A script referencing a sibling by a path relative to a `cd`'d subdir is not
  resolved. The `cd <dir> && bash <path>` form itself is.
- A dynamic command naming no infra tool is a silent blind spot.
- Resolution stops at installed binaries, anything outside the repo, and
  `node_modules`/`dist`/`build`.

The first two are backstopped by the literal layer plus the trust hashes.

## Token cost

What a hook emits on certain channels is billed to the model's context, so payload
size is a running cost rather than a one-off. Three routing rules decide what is
actually billed:

- `permissionDecisionReason` reaches the model **only on `deny`**. On `ask` and
  `allow` it is shown to the user alone — the ask tier is free.
- `additionalContext` is inserted into the conversation and saved in the
  transcript, so it is re-sent on every later request in the session. A payload
  here is paid for the rest of the session, not once.
- exit-2 stderr reaches the model as the denial reason. Exit-0 stdout does not.

Two rules follow, both applied here:

1. **Never restate in a hook payload what `CLAUDE.md` already carries.** Point at
   it instead. Doing that to `comment-hygiene` took it from ~172 to ~57 tokens per
   firing, on a hook that fires on roughly half of all edits.
2. **Prefer `updatedInput` to a block** wherever the command can be repaired
   without changing what it runs — a block costs a round trip as well as its
   message. `bash-hygiene` rewrites a trailing `2>&1` that is the sole redirect,
   because the harness captures both streams anyway. Piped and file-redirected
   forms still block: there the merge decides what the next stage reads.

Measure before and after any change:

```sh
bash tests/payload_size.sh
```

Current: `comment-hygiene` ~57 tok, `bash-hygiene` block ~31 tok, `infra-guard`
deny ~25 tok.

## Tests

```sh
bash tests/test_infra_guard.sh     # 34 cases
bash tests/test_bash_hygiene.sh    # 12 cases: allow / rewrite / block
```

Both self-skip when `jq` or `perl` is missing. Changing a pattern means re-running
them — the matrix covers both layers, the depth dial, the quoted-string skeleton,
the read-only exclusions, and the rewrite guards.
