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

## Hook Configuration Format

Hooks are defined in `hooks.json` files within plugin directories:

```json
{
  "hooks": [
    {
      "type": "PreToolUse",
      "tools": ["Edit", "Write", "MultiEdit"],
      "command": "echo 'Check for security vulnerabilities before editing'"
    }
  ]
}
```

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
