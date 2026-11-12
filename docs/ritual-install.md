# What to install so Claude Code and Codex both run ritual-full

One checklist for both agents. Each row names the install step and the check `ritual-full`'s
preflight runs, so a machine that passes this list passes the preflight in either agent. The
step-by-step walkthrough with a worked flow is [ritual, zero to hero](ritual-zero-to-hero.md);
the Codex skills live in [decebal-codex-skills](https://github.com/decebal/decebal-codex-skills).

## Shared by both

| Tool | Install | Check |
|---|---|---|
| Rust toolchain | [rustup.rs](https://rustup.rs) | `cargo --version` |
| GitHub CLI | [cli.github.com](https://cli.github.com), then `gh auth login` | `gh auth status` |
| Chronis `cn` | `cargo install chronis`, then `cn init --toon` once per repo, in its main checkout | `cn --help` exits 0; `.chronis/` in the main checkout |
| cos | clone [chief-of-staff-v2](https://github.com/decebal/chief-of-staff-v2), then `task setup` in it (needs [Task](https://taskfile.dev/installation/)) | `cos ritual check --help` exits 0; `cos ritual approve --help` lists `--yes` |
| uv | [docs.astral.sh/uv](https://docs.astral.sh/uv/) | `uv --version` |
| graphify | `uv tool install graphifyy` | `graphify --help` exits 0 (126 with `bad interpreter` means reinstall); `graphify-out/` in the repo, built with `/graphify .` |
| EGM | `claude plugin marketplace add google/elephant-goldfish-model`, then `claude plugin install egm@egm` | `claude plugin list` shows `egm@egm` enabled |

EGM is a Claude Code plugin whose skills only a person can start. A Codex flow hands each
`/egm:` command to the user to run in a Claude Code session on the same checkout, so Codex needs
Claude Code installed for the EGM steps, or the user says "continue without EGM".

`cos ritual approve --yes` is what lets an agent record an approval the user told it to make.
It arrives with [chief-of-staff-v2#23](https://github.com/decebal/chief-of-staff-v2/pull/23).
Without it, the agent stops and the user runs `cos ritual approve` in their own terminal.

## Claude Code

| Tool | Install | Check |
|---|---|---|
| Claude Code | [setup docs](https://code.claude.com/docs/en/setup) | `claude --version` |
| Skills | from this repo: `cp -R skills/<name> ~/.claude/skills/<name>` for `ritual`, `ritual-full`, `semantic-isa`, `pr-proof`, `systematic-debugging` (and `ui-refinement` for interface work) | the five folders exist under `~/.claude/skills/` |
| claude-guard and skillsync | `cargo install --path gates/rust/claude-guard`, `cargo install --path gates/rust/skillsync`, then `skillsync hooks install` | `command -v claude-guard`; `grep -cE 'claude-guard (infra-guard\|bash-hygiene)' ~/.claude/settings.json` prints 2 or more |
| graphify skill | `graphify install` | `/graphify` is listed in Claude Code |

claude-guard has no `--help`: run without a subcommand it prints its usage and exits 2. A build
older than #97 still denies `cn approve` and `cos ritual approve`, and with them the
`cos ritual approve --help` probe the agent runs before approving; rebuild from `main`.

## Codex

| Tool | Install | Check |
|---|---|---|
| Codex CLI | [Codex docs](https://developers.openai.com/codex/cli) | `codex --version` |
| Skills | from decebal-codex-skills: copy or symlink `skills/<name>` into `~/.agents/skills/` for `ritual`, `ritual-full`, `semantic-isa`, `pr-proof`, `systematic-debugging` | the five folders exist under `~/.agents/skills/` (or the repo's `.agents/skills/`) |
| codex-guard | from decebal-codex-skills: `cargo install --path gates/rust/codex-guard`, then register the hooks as its `configs/hooks.json` shows in `~/.codex/hooks.json` and trust them | `command -v codex-guard`; `~/.codex/hooks.json` names `codex-guard infra-guard`, `bash-hygiene` and `pr-guard` |
| graphify for Codex | `graphify codex install` in the repo | the repo's `AGENTS.md` has graphify's section |

codex-guard, like claude-guard, has no `--help`. Codex reads only `AGENTS.md` and never expands
`@` imports, so the skills carry their references as plain paths.

## This machine, 2026-11-12

Checked on the maintainer's macOS machine with the commands above.

| Row | Claude Code | Codex |
|---|---|---|
| Agent | Claude Code 2.1.312 | codex-cli 0.163.0 |
| `cn`, `gh` (logged in), `uv` 0.9.27, `task`, cargo 1.97.1 | ok | ok (shared) |
| Skills | ok: copied from `main` at `cdb489e` | ok: `ritual` and `ritual-full` linked to the decebal-codex-skills checkout; `semantic-isa`, `pr-proof`, `systematic-debugging` present |
| EGM | ok: `egm@egm` 0.1.1 enabled | runs in Claude Code (above) |
| graphify | ok | binary ok; per-repo `graphify codex install` not checked |
| Guard hooks | registered (2 hooks), but the binary predates #97 and denies approval | `codex-guard` hooks registered in `~/.codex/hooks.json` |
| cos | `cos ritual` ok; `--yes` not installed (#23 open) | same binary |

Two steps remain before an agent can approve a design the user told it to: merge
chief-of-staff-v2#23 and reinstall `cos`, and rebuild claude-guard from `main`.
