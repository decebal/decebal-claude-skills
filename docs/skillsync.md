# skillsync: install and check skills and guard hooks

`skillsync` keeps a home directory's skills and guard hooks in step with this
repo. It is one binary, [`gates/rust/skillsync`](../gates/rust/skillsync/README.md).

## Install

From this repo, once:

```sh
cargo install --path gates/rust/skillsync
```

Then, in any machine that has this repo checked out:

```sh
skillsync skills install               # links skills/<name> into ~/.claude/skills and ~/.agents/skills
skillsync hooks install                # merges the guard hooks into ~/.claude/settings.json and ~/.codex/hooks.json
skillsync check                        # read-only: exit 0 when skills and hooks match the repo
```

Run `skillsync hooks install --dry-run` first to see what would change. A changed
file is backed up beside it as `<name>.skillsync-<stamp>.bak`. A rerun changes
nothing.

## What check reports

| Kind | Meaning |
|---|---|
| `missing` | A repo skill is not installed in a skill root |
| `copy` | An installed skill is a directory, not a symlink into the repo |
| `dangling` | A symlink whose target no longer exists |
| `drift` | Installed `SKILL.md` differs from the repo's |
| `hook-missing` | A template guard hook is absent from the home's hook file |
| `hook-matcher` | A guard hook is under a different event or matcher than the template |
| `prod-token-template` | `~/.claude/prod-guard-tokens.txt` is still the repo template |
| `cn-unknown` | A `cn <subcommand>` named in a skill is not a `cn` subcommand |

## Where the guards come from

The templates are [`configs/settings.json`](../configs/settings.json) and
[`configs/codex-hooks.json`](../configs/codex-hooks.json). The guard binaries are
described in [hooks/README.md](../hooks/README.md).
