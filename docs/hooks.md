# Hooks Configuration

Hooks run shell commands in response to Claude events. They modify behavior without changing core instructions.

## Hook Types

| Hook | Fires When | Use Case |
|------|-----------|----------|
| **PreToolUse** | Before a tool executes | Security checks, validation |
| **PostToolUse** | After a tool executes | Logging, notifications |
| **SessionStart** | New session begins | Set context, load preferences |
| **Stop** | Agent finishes | Loop detection, cleanup |

## Active Hooks

### Security Guidance (PreToolUse)
Fires before `Edit`, `Write`, and `MultiEdit` operations. Reminds Claude to check for security vulnerabilities (OWASP top 10, injection attacks, etc.).

### Output Styles (SessionStart)
Two available styles:
- **learning-output-style** — Interactive learning with questions and exercises
- **explanatory-output-style** — Educational insights with context and reasoning

### Ralph Loop Detection (Stop)
Prevents self-referential loops where Claude agents call themselves endlessly. Critical for Ralph TUI orchestration.

## Guard-rail hooks (`hooks/`)

Three runnable hooks live in [`hooks/`](../hooks/), with install steps and the
full pattern list in [`hooks/README.md`](../hooks/README.md):

| Hook | Event | Effect |
|------|-------|--------|
| `claude-guard infra-guard` | PreToolUse, Bash | Denies live-service mutation, `terraform apply`/state surgery, storage deletion, package publishes, protected-branch force-pushes. Follows `make`/`bash`/`npm run` wrapper chains and classifies what they actually run |
| `claude-guard bash-hygiene` | PreToolUse, Bash | Blocks compound commands and substitution; rewrites a repairable `2>&1` rather than blocking it |
| `claude-guard comment-hygiene` | PostToolUse, Edit/Write | Feeds back comment lines an edit added, to be justified or deleted |

```sh
cargo install --path gates/rust/claude-guard
cargo test --manifest-path gates/rust/Cargo.toml -p claude-guard   # 63 tests
```

One binary rather than five shell scripts: a hook needing `jq` becomes a silent
no-op on a machine without it, and nothing says so.

## Hook Configuration Format

Hooks are registered in `settings.json` under `hooks`, keyed by event, with a
`matcher` selecting which tools they apply to:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [
          { "type": "command", "command": "$HOME/.claude/hooks/check.sh", "timeout": 5 }
        ]
      }
    ]
  }
}
```

Use `$CLAUDE_PROJECT_DIR` instead of `$HOME` for a project-scoped hook. Plugins
ship their own `hooks.json`; the shape above is the one that applies to user and
project settings.

### What a hook can return

A hook's exit code and JSON decide what reaches Claude and what only reaches you.
This drives both behaviour and token cost — see
[token-efficiency.md](token-efficiency.md#hook-payload-cost).

| Channel | Reaches the model |
|---|---|
| exit 2 + stderr | Yes — delivered as the denial reason |
| exit 0 + stdout | No — debug log only (except `UserPromptSubmit`/`SessionStart`) |
| `permissionDecisionReason` on `deny` | Yes |
| `permissionDecisionReason` on `ask`/`allow` | No — shown to you only |
| `additionalContext` | Yes, and it persists in the transcript |
| `systemMessage` | No, for synchronous hooks |
| `updatedInput` (PreToolUse) | Replaces the tool input — rewrite instead of block |
| `updatedToolOutput` (PostToolUse) | Replaces the tool result the model sees |

## Managing Hooks

The `hookify` plugin provides hook management capabilities:
- Create new hooks
- Enable/disable existing hooks
- List active hooks

## Best Practices

1. **Keep hooks fast** — They run synchronously and block tool execution
2. **Use PreToolUse sparingly** — Only for critical safety checks
3. **SessionStart for context** — Load project-specific preferences at session start
4. **Stop for safety** — Detect infinite loops and runaway agents
5. **Test hooks locally** — A broken hook can block all tool usage
