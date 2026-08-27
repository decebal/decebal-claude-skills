# Hook Examples — copy-paste-ready configs

Battle-tested hook configurations you can drop into `settings.json`. Hooks are one
of Claude Code's most powerful features, but adoption is low because the JSON shape
isn't obvious and there are no examples to start from. These are.

For the conceptual model (events, matchers, what a hook can return), see
[hooks.md](hooks.md). For the compiled guard-rail binary (`claude-guard`), see
[hooks.md](hooks.md#guard-rail-hooks-hooks).

## PreToolUse — Security & Quality Gate

Fires before every edit. Reminds Claude of the checklist before code is written:
no leaked credentials, no swallowed errors, human-readable messages, canonical
imports. This one has caught credential leaks, empty `.catch()` handlers, and
import-convention violations before they hit disk.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit",
        "hooks": [
          {
            "type": "intercept",
            "command": "echo 'Before editing, verify: no credentials/secrets in the change, no console.error or empty .catch(), error messages are human-readable, imports use canonical paths.'"
          }
        ]
      }
    ]
  }
}
```

## Stop — Session End Checklist

Fires when the agent tries to finish. Prevents a session from ending with tests
unrun, changes uncommitted, or a discovered gotcha unrecorded.

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "intercept",
            "command": "echo 'Before stopping: have you run the relevant tests? Are there uncommitted changes the user should know about? Did you update progress notes if you discovered a pattern or gotcha?'"
          }
        ]
      }
    ]
  }
}
```

## Notes

- Both configs come from a Tauri/Rust/Svelte project where they've run for months.
- `type: "intercept"` feeds the `echo` text back to Claude as a reminder — it
  prompts, it doesn't block. For a hook that actually **denies** an action, use
  `type: "command"` with a non-zero exit and stderr (see the return-channel table
  in [hooks.md](hooks.md#what-a-hook-can-return)).
- Adapt the checklist text to your stack — the value is in naming *your* project's
  recurring mistakes, so the reminder is specific enough to act on.
- Keep hooks fast; they run synchronously and block tool execution.
