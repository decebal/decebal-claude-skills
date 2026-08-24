# Token Efficiency & Context Window Optimization

Patterns for maximizing effectiveness within Claude's context window, extracted from production Rust + TypeScript desktop and monorepo projects.

## Search Strategy

### Targeted First, Broad Second
```
1. Grep for specific function/class name
2. Glob for file pattern if grep misses
3. Read only the relevant sections (use offset + limit)
4. Full file read only when necessary
```

### Parallel Tool Calls
Call multiple independent tools in a single turn:
- Read 3 files simultaneously instead of sequentially
- Run grep + glob in parallel when searching
- Batch git status + git diff + git log

## Mental Caching

After reading a file, retain key information mentally:
- File structure and exports
- Function signatures
- Import patterns
- Config values

Avoid re-reading the same file unless it changed.

## Scope Discipline

- **Change only what's requested** — Don't refactor adjacent code
- **No speculative improvements** — Skip "while we're here" changes
- **Minimal diffs** — Fewer lines changed = fewer tokens spent on review
- **Skip docstrings for unchanged code** — Only document what you modify

## CLAUDE.md Optimization

Well-structured CLAUDE.md files front-load context so Claude doesn't need to discover it:

```markdown
# Commands (most referenced, put first)
task dev / task test / task lint

# Structure (quick mental model)
src/ → components/ → lib/ → server/

# Conventions (prevent mistakes)
Bun only. Biome for linting. Sequential Rust tests.

# DO NOT (prevent common errors)
Never use unwrap(). Never skip migrations.
```

## Agent Task Sizing

For Ralph TUI / parallel execution:
- One concern per agent task
- Clear file scope per task
- Self-contained acceptance criteria
- Each task fits in a single context window

## Configuration

Set effort level for routine tasks:
```json
{
  "effortLevel": "medium"
}
```

Use extended thinking for complex reasoning:
```json
{
  "alwaysThinkingEnabled": true
}
```

The combination of medium effort + always thinking gives good results: thinking activates for complex problems while keeping routine responses fast.
