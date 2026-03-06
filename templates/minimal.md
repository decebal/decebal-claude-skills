# {Project Name}

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
