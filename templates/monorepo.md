# {Project Name}

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

## Conventions
- Linting: Biome (single quotes JS, double quotes JSX, 100 char width)
- Tests: {Vitest/Playwright} — always run before committing
- Commits: conventional commits (`feat:`, `fix:`, `chore:`)
- Branches: feature branches off `main`

## DO NOT
- Mix package managers
- Import directly between apps (use packages/)
- Commit without running lint + tests
