# Slash commands

Commands are not skills. A skill is loaded when its description matches what you
are doing; a command runs because you typed its name. These two are typed.

Install by copying into the directory Claude Code reads commands from:

```bash
mkdir -p ~/.claude/commands
cp commands/*.md ~/.claude/commands/
```

Per-project instead of global: `<repo>/.claude/commands/`.

| Command | Does |
|---|---|
| `/create-prompt` | Authors a prompt for another Claude to execute. Opens with an intake gate, and asks rather than guessing when the goal is underspecified. |
| `/run-prompt <N>` | Dispatches one or more saved prompts to fresh sub-task contexts, in parallel or in sequence. |

## They are a pair, and they share a numbering scheme

`/create-prompt` writes to `.prompts/{N}-{topic}-{purpose}/`; `/run-prompt N`
executes by that number. The number is the whole interface between them, which
is why it is allocated rather than chosen:

- `gates/rust/prompt-id` owns the allocation and keeps one namespace per repo,
  across worktrees.
- `claude-guard prompt-number` refuses a `Write` to `.prompts/NNN-*.md` when
  `.prompts/.numbers/NNN` does not exist, so two agents cannot mint the same
  number and silently overwrite one another.

Run `prompt-id alloc <slug>` and write to the path it prints. Both live in
[`gates/`](../gates/) and [`hooks/`](../hooks/README.md).

## Why they are in this repo

`skills/create-plans/` has referred to `/create-prompt` and `/run-prompt` since
it was published, and neither command was ever here — the reference pointed at
nothing a reader could install.
