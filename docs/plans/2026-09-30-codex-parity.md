# Codex parity: the same rules, guards and skills

Status: built. The Codex side is
[decebal-codex-skills#10](https://github.com/decebal/decebal-codex-skills/pull/10);
this repo carries the `render-agent-docs` inline target and the pr-guard fixes
found while porting. Every Codex behaviour below was read from `openai/codex`
at `main` on 2026-09-30 or observed with `codex-cli 0.147.0`; the source file
is named beside each claim.

The Claude side of this repo has three layers: rules the model reads
(`rules/`), guards that refuse an action (`claude-guard`), and skills. Codex
has an equivalent for each. The Codex side is its own repository,
[decebal-codex-skills](https://github.com/decebal/decebal-codex-skills): its
`codex-guard` already speaks Codex's hook dialect, and its `render-agent-docs`
already inlines rule bodies. This document records what Codex does and where
each piece of parity lands.

## What Codex does

| Mechanism | Behaviour | Source |
|---|---|---|
| Global instructions | `~/.codex/AGENTS.override.md`, else `~/.codex/AGENTS.md`, read whole. No size cap. `@path` lines are not resolved; they reach the model as text. | `codex-rs/codex-home/src/instructions/mod.rs` |
| Project instructions | `AGENTS.override.md` / `AGENTS.md` / fallback names from the project root down to the cwd, concatenated. All of them share `project_doc_max_bytes` (32 KiB). The file that crosses the limit is cut at that byte, with only a tracing warning. An untrusted project loads none. | `codex-rs/core/src/agents_md.rs:58-165` |
| Path-scoped instructions | None. Nothing loads because a matching file was read. | same |
| Hooks | A Claude-compatible engine (`ClaudeHooksEngine`). `~/.codex/hooks.json`, `<repo>/.codex/hooks.json`, or `[hooks]` in either `config.toml`. Each non-managed definition needs a one-time trust review. Default timeout 600 s. | `codex-rs/hooks/src/events/pre_tool_use.rs`, developer docs "Hooks" |
| Hook payload | `session_id`, `turn_id`, `transcript_path`, `cwd`, `model`, `permission_mode`, `tool_name`, `tool_input`, `tool_use_id`. Shell calls arrive as `tool_name: "Bash"`, `tool_input: {command}`. File edits arrive as `tool_name: "apply_patch"`, `tool_input: {command: <patch text>}`; matchers `Write` and `Edit` select them. | `hooks/schema/generated/pre-tool-use.command.input.schema.json`, `core/src/tools/hook_names.rs`, `core/src/tools/handlers/apply_patch.rs:415` |
| Hook decisions | `permissionDecision: "deny"` + reason blocks. Exit 2 + stderr blocks. **`ask` is unsupported**: the hook is marked failed and the command runs. `updatedInput` is applied only with `permissionDecision: "allow"`; without it the rewrite is dropped. `additionalContext` reaches the model from `PreToolUse`, `PostToolUse` and `UserPromptSubmit`. | `hooks/src/engine/output_parser.rs` |
| Where hooks fire | In the shared tool dispatch, before the handler runs. | `core/src/tools/registry.rs:603` |
| Execution policy | `~/.codex/rules/*.rules`, plus `<repo>/.codex/rules/` when the project is trusted. Starlark `prefix_rule(pattern, decision = allow \| prompt \| forbidden, justification, match, not_match)`; the strictest match wins; a plain shell script is split into commands (tree-sitter) and each is checked. `match` / `not_match` examples are validated at load. The TUI's "always allow" appends to `default.rules`. | `codex-rs/execpolicy/README.md`, developer docs "Rules" |
| Skills | `~/.codex/skills` (deprecated, still read), `~/.agents/skills`, `<repo>/.codex/skills`, every `.agents/skills` from the project root to the cwd, `/etc/codex/skills`. Symlinks are followed. | `codex-rs/ext/skills/src/host_roots.rs:84-100` |
| Skill listing budget | 8,000 characters, or 2% of the context window. Over it, descriptions are shortened, then all removed, then skills are omitted behind a count. | `codex-rs/ext/skills/src/render.rs:19-27` |
| Session record | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`. A shell call is a `response_item` whose `payload.type` is `custom_tool_call` (code mode: `tools.exec_command({cmd: …})` inside JavaScript) or `function_call`; its result is the `*_output` record with the same `call_id`. | observed in local rollouts |

## What one machine had

- `~/.codex/AGENTS.md`: 740 bytes. Its first line, `@RTK.md`, is dead text in
  Codex. None of the portable rules.
- `~/.codex/hooks.json`: the rtk rewrite, caveman, voice recall. No guard.
- `~/.codex/rules/default.rules` holds `prefix_rule(["git", "push"], allow)`.
  Sessions run `on-request` with a `workspace-write` sandbox, so every push,
  a push to the trunk included, runs without approval.
- 152 skills in `~/.codex/skills`, 21 in `~/.agents/skills`, maintained apart
  from `~/.claude/skills`.
- September's Codex rollouts show 37 distinct PR numbers as `ok created #N`
  (36 in one repository), one session accounting for six. The count misses PRs
  whose creation printed only a URL. The one-open-PR rule stops at the Claude
  boundary.
- This repo's `render-agent-docs` renders `AGENTS.md` targets as
  `@<rules_dir>/<rule>.md` lines, which Codex does not resolve.
- The skill listing: 170 descriptions totalling 33,337 characters across
  `~/.codex/skills` and `~/.agents/skills`, against a default budget of 8,000.

## Where each piece lands

| Claude mechanism | Codex equivalent | Where |
|---|---|---|
| Always-loaded portable rules | Global `AGENTS.md`, uncapped | decebal-codex-skills `render-agent-docs`, which inlines rule bodies, renders `~/.codex/AGENTS.md` from the Codex copy of `rules/`. |
| A repo's own rules | Project `AGENTS.md`, 32 KiB shared | The same tool per repo, with a `max_bytes` check, because Codex truncates the chain silently. |
| Path-scoped rules (`paths:`) | None | New `codex-guard rule-scope` (`PreToolUse`, `apply_patch` and `Bash`): matches patch headers and shell arguments against each rule's `paths:` globs and returns the rule as `additionalContext` once per session. `render-agent-docs` leaves those rules out of `AGENTS.md`. |
| `rules-reference/` | Read on demand | Installed to `~/.codex/rules-reference/`; the Codex copies of the rules point there. |
| `infra-guard` | `PreToolUse` deny | Already in `codex-guard`. Its ask tier denies, since Codex's hook `ask` lets the command run. |
| Trunk push | execpolicy `forbidden` | `forbidden` prefix rules for pushes naming the trunk, beside any `git push` allow. The strictest match wins. |
| `pr-guard` | `transcript_path` is the rollout, but a code-mode create can print its result on a later call | Ported to `codex-guard` with a session ledger written when a create is allowed; the rollout is a secondary source. |
| `prompt-number` | `apply_patch` payload | Ported to `codex-guard`, reading `*** Add File:` and `*** Move to:` headers. |
| `comment-hygiene`, `bash-hygiene` | `PostToolUse` / `PreToolUse` | Already in `codex-guard`. |
| Rewrites (`2>&1` repair, rtk) | `allow` + `updatedInput` | Already how `codex-guard` rewrites. |
| `claude-rule-select` (Jev) | `UserPromptSubmit` `additionalContext` | Same binary when wanted; not registered for Codex yet. |
| Skills | Four roots, symlinks followed | Each host's skills link to its repo checkout: `~/.claude/skills` to this repo, `~/.agents/skills` to decebal-codex-skills. |
| Auto-memory | Codex memories are generated state | Not ported. Voice and project memory already share one MCP store. |
| This repo's `AGENTS.md` targets | `@` unresolved | `render-agent-docs` gains `inline = true` per target, so a repo rendering both files gives Codex the rule text. |

## What was checked

- `codex execpolicy check` on the execution-policy rules: `git push origin main`
  and `git push origin HEAD:main` are `forbidden`; `git push -u origin
  HEAD:feat/x` stays `prompt` (or `allow` beside an "always allow"), and
  `git push -n origin feat/x` is not caught by the `--no-verify` rule.
- The installed `codex-guard pr-guard`, with a ledger naming a head whose PR is
  open and the real `gh`: `git switch -c feat/y` is denied with the PR named;
  `git branch -f main origin/main` is allowed.
- The installed `codex-guard rule-scope` on a patch touching a `*_tests.rs`
  file: `testing-authoring` and `layer-boundaries` arrive as
  `additionalContext`, with no `permissionDecision`.
- decebal-codex-skills: fmt, clippy `-D warnings`, 346 workspace tests. This
  repo: `claude-guard` and `render-agent-docs`, 105 tests.

## What I have not checked

- A live Codex session with the new hooks. Codex reviews a new hook definition
  before running it, and that review has not been done here.
- Whether hooks fire for `exec_command` calls made inside code mode's `exec`
  tool. The source puts hooks in the shared dispatch; no live session has
  confirmed it.
- Whether `allow` + `updatedInput` also bypasses Codex's own approval for the
  rewritten command.
- The trust-review prompt for new hook definitions; not exercised.
- The model's context window, so the real 2% skill budget. No omission warning
  appears in the last two days of sessions; a shortened description leaves no
  marker.
