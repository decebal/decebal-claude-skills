# {Project Name}

## Rules

Portable rules live in [`rules/`](../rules/). Copy the set you want to
`~/.claude/rules/` and import them — do not paste their text here, it drifts.

@~/.claude/rules/git-discipline.md
@~/.claude/rules/evidence-discipline.md
@~/.claude/rules/agent-parallelism.md
@~/.claude/rules/timeouts.md
@~/.claude/rules/definition-of-done.md
@~/.claude/rules/comments.md
@~/.claude/rules/token-efficiency.md
@~/.claude/rules/testing-gates.md
@~/.claude/rules/layer-boundaries.md
@~/.claude/rules/dependency-hygiene.md

## Overview
Monorepo managed with {Turborepo/Nx/Taskfile}.

## Structure
```
apps/
  api/              # Backend API ({Elysia/Express/Actix})
  web/              # Frontend ({Next.js/Svelte/React})
packages/
  shared/           # Shared types and utilities
  ui/               # Shared UI components
  config/           # Shared configuration
```

## Package Manager
Bun only. NEVER use npm or yarn.

## Commands

### Global (from root)
```bash
task dev             # Start all services
task build           # Build everything
task test            # Run all tests
task lint            # Lint everything
task clean           # Clean build artifacts
```

### Per-app
```bash
task dev:api         # Start API only
task dev:web         # Start frontend only
task test:api        # Test API only
task test:web        # Test frontend only
```

### Docker
```bash
task docker:up       # Start Docker stack
task docker:down     # Stop Docker stack
task docker:logs     # View logs
```

## Architecture
{Describe key architectural decisions, data flow, service communication}

## Gates

Every gate runs under `timeout -k 15 300`. A sub-project that is NOT a workspace
member needs its own dependency install, or its gate fails for a reason that has
nothing to do with the diff.

```bash
gates/sh/check-branch-not-merged.sh              # dead-branch guard, runs first
staged-scope --range "$BASE"                     # run only the gates this diff touches
```

## Conventions
- Linting: Biome (single quotes JS, double quotes JSX, 100 char width)
- Tests: {Vitest/Playwright} — always run before committing
- Commits: conventional commits (`feat:`, `fix:`, `chore:`)
- Branches: feature branches off `main`
- A package with test files but no `test` script runs in NO gate — check it

## DO NOT
- Mix package managers
- Import directly between apps (use `packages/`)
- Run two builds at once — one build on the machine at a time
- Commit without running lint + tests
