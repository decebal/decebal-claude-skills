# Skill Composition

Skills mostly stand alone, but real work chains them: a design becomes a spec
becomes a PRD becomes beads; a review skill leans on the language skill's rules; a
comprehension skill reuses the diagram skill's templates. This is the convention
for expressing those relationships without duplicating instructions.

## The convention (metadata-only)

Add either optional key to a skill's `SKILL.md` frontmatter. Both are lists of
skill names:

```yaml
---
name: claude-beads
description: ...
depends_on: [claude-prd]      # hard: this skill CONSUMES that skill's output
enhances: [typescript]        # soft: load that skill's rules while running this one
---
```

- **`depends_on`** — a *pipeline* edge. The named skill's output is this skill's
  input. `claude-beads` depends on `claude-prd` because it converts a PRD into
  beads: run the producer first (or expect its artifact).
- **`enhances`** — an *overlay* edge. While this skill runs, also pull in the named
  skill's rules/patterns. `perf-review enhances typescript` means the language-level
  optimizations apply on top of the app-level hot-path review.

The distinction: `depends_on` is about **order and data**; `enhances` is about
**loading extra knowledge at the same time**.

## How an agent uses it

- Sees `depends_on: [X]` → ensure X has already run (its output exists), or run X
  first. Don't start a consumer with no input.
- Sees `enhances: [Y]` → read Y's `SKILL.md` (and relevant `references/`) alongside
  this one, so both rule sets are in play.

This is guidance for chaining, not a runtime loader — Claude Code reads only `name`
and `description` from frontmatter. The keys are here so relationships are
**greppable** and an agent (or a human) can chain skills in the right order without
rediscovering the dependency each time.

## Why this shape (and not the others)

The issue proposed three options; this is Option A, deliberately.

| Option | What | Verdict |
|--------|------|---------|
| **A — declarative deps** | `depends_on` / `enhances` in frontmatter | **Chosen.** Metadata-only, greppable, zero runtime, extends later. |
| B — workflow skills | a `type: workflow` skill that orchestrates steps | Needs an orchestration engine we don't have; revisit only when a real multi-skill workflow demands it. |
| C — reference imports | `{{import other/references/x.md}}` in SKILL.md | Import-resolution + drift complexity; a plain relative link (`../typescript/SKILL.md`) already covers reuse without a build step. |

Start with A. If a genuine fixed pipeline emerges (design → spec → PRD → beads, run
end to end), that's the point to reconsider B — not before.

## Current relationship map

| Chain | Edge |
|-------|------|
| `claude-prd` → `claude-beads` | `claude-beads` **depends_on** `claude-prd` |
| `brainstorming` → `feature-spec` → `claude-prd` | design → spec → PRD (pipeline) |
| `perf-review` + `typescript` / `security-review` | `perf-review` **enhances** both (review overlay) |
| `security-review` + `typescript` | `security-review` **enhances** `typescript` |
| `explain-module` + `arch` | `explain-module` **enhances** `arch` (C4 / Mermaid templates) |
| `integration-test` + `bun-testing` | JS/TS projects: bun:test patterns apply during test authoring |
| `monorepo-expert` + `bun-testing` | turbo test caching wraps the bun:test runner |
| `rust-clean-architecture` + `rust-quality` | Rust quality pair — layering + lints together |

Only annotate a real relationship. A speculative `enhances` that never fires is
noise; leave it off until the chain is actually used.
