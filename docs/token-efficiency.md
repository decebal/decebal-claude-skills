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

## Hook Payload Cost

Hooks are free until they emit on a channel the model reads. Three routing rules
decide what is actually billed:

- **`permissionDecisionReason` reaches the model only on `deny`.** On `ask` and
  `allow` it is shown to you alone, so an ask tier costs nothing — write those
  reasons as long as they need to be.
- **`additionalContext` persists.** It is inserted into the conversation and saved
  in the transcript, so it is re-sent on every later request in the session. A
  payload here is paid for the rest of the session, not once.
- **exit-2 stderr reaches the model** as the denial reason. Exit-0 stdout does not.

Two rules follow:

**Never restate in a hook payload what CLAUDE.md already carries.** Point at it.
A PostToolUse hook that re-stated a rule cost ~172 tokens per firing on ~46% of
all edits; replaced with a two-line pointer it costs ~57.

**Prefer `updatedInput` to a block.** A PreToolUse hook can rewrite the tool input
rather than reject it. A block costs a round trip *and* its stderr; a rewrite
costs neither. Only rewrite where the repair cannot change what the command does
— see the `2>&1` handling in
[`gates/rust/claude-guard`](../gates/rust/claude-guard/src/bash_hygiene.rs), which
rewrites a trailing merge but still blocks the piped and file-redirected forms
where the merge decides what the next stage reads.

Measure rather than guess, and hold the number: `payload_size_tests.rs` FAILS when
a payload grows past its ceiling. A script that printed the size only told you
after you had shipped it, and only if someone ran it.

## Output Limits Worth Knowing

| Knob | Default |
|------|---------|
| `BASH_MAX_OUTPUT_LENGTH` | 30,000 chars (max 150,000) |
| `MAX_MCP_OUTPUT_TOKENS` | 25,000 |
| `TASK_MAX_OUTPUT_LENGTH` | 32,000 chars (subagent result) |
| Hook output strings | capped at 10,000 chars, then spilled to a file |

Startup context is worth auditing too: `/context` breaks it down by category and
`/doctor` proposes CLAUDE.md trims. Skill *descriptions* load at startup; skill
bodies only on invocation, and stay loaded for the rest of the session once used.

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
