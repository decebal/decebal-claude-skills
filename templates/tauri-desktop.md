# {Project Name}

## Stack
- Desktop framework: Tauri 2 (macOS)
- Backend: Rust (clean architecture)
- Frontend: Svelte 5 + TypeScript
- Database: SQLite via Turso
- Styling: Tailwind CSS

## Commands
```bash
task dev             # Start Tauri dev mode
task build           # Build production app
task test            # Run Rust tests (--test-threads=1)
task lint            # Lint frontend + clippy
task db:migrate      # Run SQLite migrations
```

## Project Structure
```
src-tauri/
  src/
    commands/        # Tauri IPC commands
    db/              # Database layer + migrations
    services/        # Business logic
    models/          # Data models
  Cargo.toml
src/
  lib/
    components/      # Svelte components
    stores/          # Svelte stores
    api/             # Frontend API layer (invoke wrappers)
  routes/            # SvelteKit routes
```

## Architecture
- Frontend ↔ Backend via Tauri IPC (`invoke()`)
- Event streams for real-time updates (activity vs alerts separation)
- Clean architecture: commands → services → repositories → database
- Database migrations versioned (current: v{N})

## Conventions
- Rust tests: sequential (`--test-threads=1`) due to shared SQLite state
- Frontend: typed IPC wrappers in `src/lib/api/`
- Errors: Rust `thiserror` types mapped to frontend-friendly messages
- Token efficiency: targeted grep/glob searches, batch parallel tool calls

## DO NOT
- Use `unwrap()` in production Rust code
- Access database directly from Tauri commands (use services layer)
- Skip migration versioning
