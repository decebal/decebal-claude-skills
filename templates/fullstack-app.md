# {Project Name}

## Rules

Portable rules live in [`rules/`](../rules/). Copy the set you want to
`~/.claude/rules/` and import them — do not paste their text here, it drifts.

@~/.claude/rules/git-discipline.md
@~/.claude/rules/evidence-discipline.md
@~/.claude/rules/definition-of-done.md
@~/.claude/rules/comments.md
@~/.claude/rules/token-efficiency.md
@~/.claude/rules/testing-gates.md
@~/.claude/rules/error-channels.md
@~/.claude/rules/ui-remote-states.md

## Stack
- Frontend: Next.js 15 / Svelte 5 with TypeScript
- Backend: {Elysia/Express/Rust}
- Database: {Supabase/SQLite/Turso}
- Styling: Tailwind CSS + shadcn/ui
- Runtime: Bun

## Commands
Prefer Taskfile (`task`) for all commands:

```bash
task dev             # Start dev server
task build           # Production build
task test            # Run unit tests
task test:e2e        # Run Playwright E2E tests
task lint            # Biome lint + format check
task db:migrate      # Run database migrations
task db:seed         # Seed database
```

## Project Structure
```
src/
  app/               # Routes/pages
  components/        # UI components
  lib/               # Utilities and helpers
  server/            # Server-side logic
  types/             # TypeScript type definitions
public/              # Static assets
tests/
  e2e/               # Playwright E2E tests
  unit/              # Unit tests
```

## Conventions
- Package manager: Bun only
- Linting: Biome (single quotes JS, double quotes JSX, 100 char width)
- E2E tests required for critical paths (Playwright)
- `'use client'` directives enforced where needed
- Environment variables: `.env.local` for development (never committed)

## Integrations
{List external services: analytics, payments, email, AI, etc.}

## Key Patterns
- {Data fetching strategy}
- {Auth approach}
- {Error handling conventions}

## DO NOT
- Mix package managers
- Render a raw payload to a user — summarize, or state an honest fallback
- Offer a screen that cannot fetch its data with no way to retry
- Commit without running lint + tests
