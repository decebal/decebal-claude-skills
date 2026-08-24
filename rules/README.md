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
| [timeouts.md](timeouts.md) | The 5-minute ceiling on every gate, and how to fit under it |
| [definition-of-done.md](definition-of-done.md) | End-to-end or not done; size is never a signal; blocks are routed around |
| [comments.md](comments.md) | What a comment must earn; never narrate the fix |
| [debugging-discipline.md](debugging-discipline.md) | Instrument before theorizing; revert before layering; find the regression commit |
| [token-efficiency.md](token-efficiency.md) | Targeted reads, mental cache, batched calls, minimal diffs |
| [testing-gates.md](testing-gates.md) | What actually enforces anything; process-per-test; sharding; un-hangable tests; complete-surface mocks |
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

Don't take all fifteen. Context is the budget.

- **Any repo, any stack:** `git-discipline`, `evidence-discipline`, `comments`,
  `definition-of-done`, `token-efficiency`.
- **Multi-agent work:** add `agent-parallelism`, `timeouts`.
- **Has a test suite and hooks:** add `testing-gates`.
- **Layered backend:** add `layer-boundaries`, `dependency-hygiene`.
- **Has a UI:** add `ui-remote-states`, `error-channels`, `event-streams`.
- **Engine + per-tenant data plane:** add `data-over-binary`.

## Adapting a rule

Each file names its own generics — `notifyUser` / `logDev`, "the trunk", "the task
tracker", "the build directory". Rename to your codebase's actual symbols on the
way in; a rule naming a function that does not exist gets ignored wholesale.

Keep the incidents. They are the load-bearing part.
