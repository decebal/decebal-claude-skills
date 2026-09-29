# Portable rules

Stack-agnostic rule fragments, one concern per file. Extracted from a production
Rust + TypeScript monorepo and de-identified. Every rule here carries the incident
that produced it — that is deliberate. A bare prohibition gets rationalized away by
the next agent under deadline; a prohibition with a cost attached does not.

## Index

Many of these rules are **path-scoped** — they load only when you read matching files, reducing session-start load. See [**Load mechanism**](#load-mechanism) below.

| Rule | Covers | Scope |
|---|---|---|
| [git-discipline.md](git-discipline.md) | Dead-branch liveness check, squash-merge detection, branch/push discipline, conventional commits, multi-agent git | unconditional |
| [evidence-discipline.md](evidence-discipline.md) | Check the destination before trusting an absence; read runtime state, never guess it | unconditional |
| [agent-parallelism.md](agent-parallelism.md) | Split by file count not concept; one worktree per agent; never two builds at once | unconditional (trimmed) |
| [timeouts.md](timeouts.md) | The 5-minute ceiling on every gate; ask which phase a slow step runs in before making it faster | unconditional (trimmed) |
| [process-ownership.md](process-ownership.md) | Every child has a parent that reaps it; bounded/cached/reaped hot-path spawns | unconditional (trimmed) |
| [definition-of-done.md](definition-of-done.md) | End-to-end or not done; size is never a signal; blocks are routed around | unconditional |
| [estimation.md](estimation.md) | Never estimate in time; rock/sand/water is confidence, not size | unconditional (trimmed) |
| [comments.md](comments.md) | What a comment must earn; never narrate the fix | unconditional |
| [anti-slop.md](anti-slop.md) | Report findings, don't perform them: no escalation openers, X-not-Y frames, significance labels | unconditional |
| [debugging-discipline.md](debugging-discipline.md) | Instrument before theorizing; revert before layering; find the regression commit | unconditional |
| [token-efficiency.md](token-efficiency.md) | Targeted reads, mental cache, batched calls, minimal diffs | unconditional |
| [documents-not-artifacts.md](documents-not-artifacts.md) | Deliverables are versioned, indexed documents under `docs/`; never hosted artifact pages | unconditional |
| [pr-evidence-report.md](pr-evidence-report.md) | The HTML report a PR ships: sections, screenshots, before/after, copy blocks | `docs/**/*.html`, `**/.github/pull_request_template.md` |
| [testing-gates.md](testing-gates.md) | What actually enforces anything; a source scan must not be a compiled test | unconditional (trimmed) |
| [testing-authoring.md](testing-authoring.md) | Process-per-test, sharding, test file conventions, un-hangable tests, module mocks | test files, gates, workflows |
| [layer-boundaries.md](layer-boundaries.md) | 4-layer direction as a test; ceilings not bans; how to open a god module | `**/*.rs`, `**/Cargo.toml`, `**/src/**/*.{ts,tsx,svelte}` |
| [dependency-hygiene.md](dependency-hygiene.md) | Before adding a package; no dead weight; the metric to watch | `**/Cargo.toml`, `**/Cargo.lock`, `**/package.json`, `**/bun.lock` |
| [error-channels.md](error-channels.md) | Two channels — user-actionable vs dev-only; never `console.error` | `**/*.{svelte,ts,tsx,js,jsx}` |
| [event-streams.md](event-streams.md) | Activity vs Alerts, strictly separated | `**/*.{svelte,ts,tsx,js,jsx}` |
| [ui-remote-states.md](ui-remote-states.md) | Never render a raw payload; plain-English copy; `ready` / `empty` / `unreachable` as a type | `**/*.{svelte,ts,tsx,js,jsx}` |
| [data-over-binary.md](data-over-binary.md) | Fix customer behaviour in published data, not in the shipped binary | unconditional (trimmed) |

**Detail and evidence:** incident narratives, measurements and code samples live
in [`docs/rules-reference/`](../docs/rules-reference/), installed to
`~/.claude/rules-reference/`. A rule names the file to read when it needs one.

## Load mechanism

Claude Code sums every instruction file loaded at session start and warns above
150k characters. A rule with `paths:` frontmatter loads only when Claude reads a
file matching one of its globs, and does not count until then
([Claude Code memory docs](https://code.claude.com/docs/en/memory#path-specific-rules)).
Rules about editing a kind of file are scoped; rules about behaviour (git,
reporting, estimating, gate failures, spawning agents) stay unconditional,
because no file read triggers them.

An `@`-import loads its file at launch whatever its `paths:` say, so never
`@`-import a scoped rule.

## How to use them

Rules are loaded by reference, not by copy. In a project's `CLAUDE.md`:

```markdown
## Rules

@~/.claude/rules/git-discipline.md
@~/.claude/rules/evidence-discipline.md
@~/.claude/rules/timeouts.md
```

Three placement options:

| Where | Loads for | Use when |
|---|---|---|
| `~/.claude/rules/` + `@` import from `~/.claude/CLAUDE.md` | every project on the machine | the rule is you, not the repo (git discipline, evidence discipline, comments) |
| `.claude/rules/` in the repo, imported from the repo's `CLAUDE.md` | everyone on the repo, agents included | the rule is the repo's (layer boundaries, testing gates) |
| Inline in `CLAUDE.md` | — | never; it drifts, and it costs context on every session |

Install the machine-wide set, without this index (it would load every session),
and the reference docs beside it (outside `~/.claude/rules/`, which is read
recursively):

```bash
diff -rq --exclude README.md rules/ ~/.claude/rules/   # an installed copy you edited by hand shows here; keep what it adds
rsync -a --exclude README.md rules/ ~/.claude/rules/
rsync -a docs/rules-reference/ ~/.claude/rules-reference/
```

Check the budget the same way Claude Code counts it: every instruction file with
no `paths:` line loads at session start. Sum them and stay well under 150k:

```bash
grep -L "^paths:" ~/.claude/rules/*.md .claude/rules/*.md   # the files that always load
wc -m ~/.claude/CLAUDE.md CLAUDE.md <those files>
```

`/context` in a session lists what actually loaded.

## Picking a subset

Don't take all twenty. Context is the budget.

- **Any repo, any stack:** `git-discipline`, `evidence-discipline`, `comments`,
  `anti-slop`, `definition-of-done`, `estimation`, `token-efficiency`,
  `documents-not-artifacts`.
- **Multi-agent work:** add `agent-parallelism`, `timeouts`, `process-ownership`.
- **Anything that shells out on a hot path** (statusline, hook, watcher, poller),
  or drives a browser/emulator/container from an agent: add `process-ownership`.
- **Has a test suite and hooks:** add `testing-gates`.
- **Layered backend:** add `layer-boundaries`, `dependency-hygiene`.
- **Has a UI:** add `ui-remote-states`, `error-channels`, `event-streams`.
- **Ships user-visible change through PRs:** add `pr-evidence-report` — the
  report is the only place a reviewer sees the feature work, since no gate
  demonstrates behaviour.
- **Engine + per-tenant data plane:** add `data-over-binary`.

## Adapting a rule

Each file names its own generics — `notifyUser` / `logDev`, "the trunk", "the task
tracker", "the build directory". Rename to your codebase's actual symbols on the
way in; a rule naming a function that does not exist gets ignored wholesale.

Keep the incidents. They are the load-bearing part. When one grows past a few
lines, move it to `docs/rules-reference/` and leave the cost in the rule.
