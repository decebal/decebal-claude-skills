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
@~/.claude/rules/debugging-discipline.md
@~/.claude/rules/token-efficiency.md
@~/.claude/rules/testing-gates.md
@~/.claude/rules/layer-boundaries.md
@~/.claude/rules/dependency-hygiene.md
@~/.claude/rules/error-channels.md
@~/.claude/rules/event-streams.md
@~/.claude/rules/ui-remote-states.md

## Stack
- Desktop framework: Tauri 2 ({macOS/Windows})
- Backend: Rust (clean architecture, 4 layers)
- Frontend: Svelte 5 + TypeScript
- Persistence: {SQLite via Turso / an event store}
- Styling: Tailwind CSS

## Commands
```bash
task dev             # Start Tauri dev mode
task build           # Build production app
task test            # cargo nextest run --features test-support
task lint            # Lint frontend + clippy
```

## Project Structure
```
src-tauri/
  src/
    presentation/    # Tauri IPC handlers / router
    application/     # Services (orchestration)
    infrastructure/  # Persistence + external I/O
    domain/          # Entities, pure logic
  Cargo.toml
src/
  lib/
    domains/{name}/  # components, state, services, index.ts barrel
    shared/          # cross-domain UI + services
    infrastructure/  # IPC adapters
  routes/
```

## Architecture
- Frontend ↔ Backend via Tauri IPC (`invoke()`)
- Dependency direction: presentation → application → infrastructure ← domain,
  **enforced by an architecture test suite**, not by convention
- Event streams: Activity (telemetry, never toasts) vs Alerts (always toasts)
- Services never import infrastructure directly — go through a facade

## Conventions
- **Rust tests run under `cargo nextest`** — process-per-test, so `--test-threads=1`
  is not needed. Serialize only what is machine-global (the OS keychain) in one
  named group with `max-threads = 1`
- **Invoke args are camelCase** — `invoke("cmd", { workflowId })`, not
  `{ workflow_id }`. Gate it; a name mismatch is a silently dead command
- Frontend: typed IPC wrappers, one per domain, checked against a command map
- Errors: Rust `thiserror` types mapped to human-readable frontend messages
- Clipboard: never `navigator.clipboard` — it silently fails in the desktop
  webview. Use a wrapper

## DO NOT
- Use `unwrap()` in production Rust code
- Access persistence directly from IPC handlers (route through services)
- Use `requestAnimationFrame` positioning for popovers — it produces phantom
  clicks in the desktop webview
- Render a raw payload to a user
- Run two cargo builds at once
