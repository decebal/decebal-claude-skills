# Portable rules

Stack-agnostic rule fragments, one concern per file. Extracted from a production
Rust + TypeScript monorepo and de-identified. Every rule here carries the incident
that produced it — that is deliberate. A bare prohibition gets rationalized away by
the next agent under deadline; a prohibition with a cost attached does not.

## Index

| Rule | Covers |
|---|---|
| [git-discipline.md](git-discipline.md) | Dead-branch liveness check, squash-merge detection, branch/push discipline, conventional commits, multi-agent git |
| [evidence-discipline.md](evidence-discipline.md) | Check the destination before trusting an absence; read runtime state, never guess it |
| [agent-parallelism.md](agent-parallelism.md) | Split by file count not concept; one worktree per agent; never two builds at once; seeding and cleanup |
| [timeouts.md](timeouts.md) | The 5-minute ceiling on every gate; ask which phase a slow step runs in before making it faster; how to fit under the cap |
| [process-ownership.md](process-ownership.md) | Every child has a parent that reaps it; bounded/cached/reaped hot-path spawns; the PPID-1 sweep |
| [definition-of-done.md](definition-of-done.md) | End-to-end or not done; size is never a signal; blocks are routed around |
| [estimation.md](estimation.md) | Never estimate in time; rock/sand/water is confidence, not size; the betting table |
| [comments.md](comments.md) | What a comment must earn; never narrate the fix |
| [anti-slop.md](anti-slop.md) | Report findings, don't perform them: no escalation openers, X-not-Y frames, significance labels or closing morals |
| [debugging-discipline.md](debugging-discipline.md) | Instrument before theorizing; revert before layering; find the regression commit |
| [token-efficiency.md](token-efficiency.md) | Targeted reads, mental cache, batched calls, minimal diffs |
| [documents-not-artifacts.md](documents-not-artifacts.md) | Deliverables are versioned, indexed documents under `docs/`; never hosted artifact pages |
| [pr-evidence-report.md](pr-evidence-report.md) | The HTML report a PR ships: seven sections claim→evidence→limits, screenshots with provenance, SHA-anchored before/after, copy blocks with a Pass line, what may be collapsed |
| [testing-gates.md](testing-gates.md) | What actually enforces anything; a source scan must not be a compiled test; process-per-test; sharding; un-hangable tests; complete-surface mocks |
| [layer-boundaries.md](layer-boundaries.md) | 4-layer direction as a test; ceilings not bans; how to open a god module |
| [dependency-hygiene.md](dependency-hygiene.md) | Before adding a package; no dead weight; the metric to watch |
| [error-channels.md](error-channels.md) | Two channels — user-actionable vs dev-only; never `console.error` |
| [event-streams.md](event-streams.md) | Activity vs Alerts, strictly separated |
| [ui-remote-states.md](ui-remote-states.md) | Never render a raw payload; plain-English copy; `ready` / `empty` / `unreachable` as a type |
| [data-over-binary.md](data-over-binary.md) | Fix customer behaviour in published data, not in the shipped binary |

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

Install the machine-wide set:

```bash
mkdir -p ~/.claude/rules
cp rules/*.md ~/.claude/rules/
```

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

Keep the incidents. They are the load-bearing part.
