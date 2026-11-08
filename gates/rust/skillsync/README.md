# skillsync

Keeps a home directory's skills and guard hooks in step with this repo.

```sh
cargo install --path gates/rust/skillsync
skillsync check                  # read-only; exit 1 on findings
skillsync hooks install          # merge guard hooks from configs/ into ~/.claude and ~/.codex
skillsync skills install         # symlink skills/ into ~/.claude/skills and ~/.agents/skills
```

Flags: `--home <dir>` (default `$HOME`), `--repo <dir>` (default: nearest
ancestor holding `skills/` and `configs/`), `--cn <bin>` (default `cn`),
`--dry-run`, `--json`, `--strict`.

## What `check` compares

| Finding | Meaning |
|---|---|
| `missing` | a repo skill is not in a skill root (`--strict` only; otherwise one notice per root) |
| `dangling` | a symlink whose target does not exist |
| `elsewhere` | a symlink to a directory other than this repo's skill |
| `copy` | a real directory where a symlink belongs |
| `drift` | `SKILL.md` sha256 differs from the repo |
| `hook-missing` | a guard command from `configs/` is absent from the hook file |
| `hook-matcher` | the guard is present under a different event or matcher |
| `prod-token-template` | `~/.claude/prod-guard-tokens.txt` still equals the template |
| `cn-unknown` / `cn-timeout` | a `cn <sub>` command in a SKILL.md fails `cn <sub> --help` |

`~/.codex` checks run only when that directory exists. `cn` lint runs only when
`cn --version` answers; every child process has a 10 s deadline and is killed and
reaped at it.

## What the installers do

- `hooks install` merges only the templates' guard entries (commands whose
  executable ends in `-guard`). An entry counts as present when its executable's
  file name and subcommand match, so `/abs/codex-guard infra-guard` and
  `codex-guard infra-guard` are one guard. Other keys and entries keep their order
  (`serde_json` `preserve_order`). Every file is merged in memory before any is
  written. A changed file is copied to `<name>.skillsync-<stamp>.bak` first, and a
  symlinked file is written through to its target. A rerun changes nothing.
- `skills install` links each repo skill. A path that already exists and is not a
  symlink to this repo is a conflict. When any conflict exists, nothing is linked,
  the conflicts are listed, and the exit code is 1. An I/O error partway through
  removes the links this run made and exits 2; directories created on the way stay.

Matchers already present are not rewritten by `hooks install`; `check` reports
them as `hook-matcher` so they can be edited by hand.

`codex-guard` is not a member of `gates/rust` yet, so the Codex entries in
`configs/codex-hooks.json` name a binary this workspace does not build.

## Tests

```sh
cargo test --manifest-path gates/rust/Cargo.toml -p skillsync
```

Integration tests in `tests/cli.rs` run the binary against a fixture repo and a
temp HOME: drift, copy, dangling, missing, hook matcher, prod-token template, cn
lint with a stand-in `cn`, idempotent hook install (including absolute-path
guards and a Codex rerun), staged merge on a malformed file, a symlinked
settings file, conflict preservation, a skills rerun, a check-after-install round
trip, and dry-run.
