# {Project Name}

## Rules

Portable rules live in [`rules/`](../rules/). Copy the set you want to
`~/.claude/rules/` and import them — do not paste their text here, it drifts.

@~/.claude/rules/git-discipline.md
@~/.claude/rules/evidence-discipline.md
@~/.claude/rules/comments.md
@~/.claude/rules/token-efficiency.md

## Stack
- Runtime: Bun
- Language: TypeScript

## Commands
```bash
bun install          # Install dependencies
bun run dev          # Start dev server
bun run build        # Production build
bun run test         # Run tests
bun run lint         # Lint with Biome
```

## Conventions
- Package manager: Bun only (never npm/yarn)
- Linting: Biome (single quotes JS, double quotes JSX, 100 char width)
- Always run `bun run lint` before committing

## DO NOT
- Mix package managers
- Commit without running lint + tests
- Push to `main` — feature branch and PR, always
