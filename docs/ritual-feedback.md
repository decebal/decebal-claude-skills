# Ritual feedback

Every `ritual` and `ritual-full` run records what happened at each step, and a weekly
[autoresearch](https://github.com/wolven-tech/autoresearch-rs) loop proposes edits to the two
skills that close the recorded problems. Nothing the loop produces is merged on its own: it
leaves a branch and a ledger for a person to review.

Source: [`skills/ritual/scripts`](../skills/ritual/scripts) (one Rust binary,
`ritual-feedback`). The recording rules the agent follows are in
[ritual's Feedback section](../skills/ritual/SKILL.md#feedback).

## Install

```sh
cargo install --locked --git https://github.com/decebal/decebal-claude-skills ritual-feedback
ritual-feedback --version
```

The weekly loop also needs `autoresearch` ([install](https://github.com/wolven-tech/autoresearch-rs))
and the `claude` CLI on `PATH`.

## Where events go

An embedded AllSource store (`allsource-core` with the `embedded` feature): a WAL plus Parquet
under `~/.ritual/feedback/allsource`, or `$RITUAL_FEEDBACK_DIR/allsource`. No server, no key, no
network. Writers take `~/.ritual/feedback/write.lock` first, because two sessions can finish a
step at the same moment and the embedded core has no cross-process lock of its own.

Read it with the binary, or through the AllSource MCP server bound to the directory:

```sh
ritual-feedback list                 # every event, oldest first
ritual-feedback list --epic <id> --json
allsource-mcp --store ritual=$HOME/.ritual/feedback/allsource
```

## The event

Type `ritual.feedback.<kind>`, entity `<repo>/<epic>`. The payload:

| Field | Meaning |
|---|---|
| `schema` | `1` |
| `repo`, `epic` | the main checkout's directory name and the flow's Chronis epic |
| `step` | `think`, `spec`, `plan`, `build`, `review`, `ship`, `reflect` |
| `kind` | `step_done`, `gate_failed`, `rework`, `correction`, `drift`, `friction`, `praise` |
| `skill` | `ritual` or `ritual-full` |
| `agent` | `claude` or `codex` |
| `note` | one sentence a later reader can act on |
| `reference` | optional: the skill file the event is about, e.g. `references/build.md` |
| `check` | optional: `{must, must_not}`, the phrase `reference` must, or must no longer, contain once fixed |

An event with a `check` is a **case**. Only cases drive the loop; the rest is the run history
Reflect reads.

## The weekly loop

`ritual-feedback weekly` advances the loop by at most one step per call, and each call finishes
inside five minutes. `ritual-feedback schedule` installs a LaunchAgent
(`com.ritual-feedback.weekly`) that calls it every 15 minutes. A call returns at once unless a
run is in progress or the next start (`--start-weekday`, `--start-hour`, local time; Sunday
07:00 by default) has passed since the last run started. A Mac asleep at the start time starts
on its first call after waking.

**A started run is carried by `~/.ritual/weekly/state.json` until it is reported.** Nothing
resets it: not a failed call, not sleep, not a new week. A run leaves the candidate phase when
its candidate budget is used, when twice that many mutator attempts have been made (failed ones
count), when every dev case is closed or every open case has been tried, or after three failed
calls in a row. Three failed calls in a row at setup, or at the report, end the run as
`failed`. One process at a time holds `weekly.lock`; a corrupt state file is an error, never a
fresh start.

1. **Setup.** Refresh a dedicated clone at `~/.ritual/weekly/repo` to `origin/main`. Export
   every case whose target file exists there (the rest are counted as dropped: no candidate can
   create a file); the newest 30% become a holdout, and both files and the holdout SHA-256 are
   frozen under `~/.ritual/weekly/cases/<run>/`. Write and commit `autoresearch.toml` and
   `program.md` in the clone, then `doctor` and `baseline`. No cases, or none open, ends the
   run as `skipped`.
2. **One candidate per call.** `run` prepares a worktree. `claude -p --restricted` writes the
   candidate with Read, Edit, Glob and Grep only, file access confined to the worktree (so the
   holdout outside it stays unread), no MCP servers and no permission prompts. Changes outside
   `skills/ritual*/**/*.md` are reverted, then `run --hypothesis` evaluates and the policy keeps
   or discards. A submit cut off mid-way is read back on the next call.
3. **Report.** `stop`, `verify` when something was kept, then `report --html` and a Markdown
   ledger under `~/.ritual/weekly/reports/<run>.{html,md}`. A report that fails is retried.

autoresearch can neither stop nor verify a run while a candidate is prepared, and has no way to
drop one. The loop decides to stop before it prepares a candidate whenever it can. When it
cannot (three failed calls in a row, say), it closes the prepared candidate with an **abandon**
candidate: ritual-full's frontmatter renamed, so `frontmatter_valid` fails and the policy
discards it. It appears in the ledger as case `-`.

The evaluator is the same binary (`ritual-feedback evaluate`), speaking autoresearch JSONL v1:

| Measurement | Kind | Rule |
|---|---|---|
| `open_feedback_cases` | objective, minimize | dev cases whose check fails on the candidate |
| `total_words` | diagnostic | words in the skills' Markdown, `scripts/` excluded |
| `holdout_not_regressed` | gate | no more holdout cases open than at baseline, and the holdout file unchanged |
| `word_budget` | gate | `total_words` no larger than at baseline: fixes replace text, they do not append it |
| `invariants_kept` | gate | every rule in [`invariants.txt`](../skills/ritual/scripts/invariants.txt) still appears in its file, so the budget cannot be met by deleting a rule |
| `links_resolve` | gate | every relative link in the skills' Markdown points at a file |
| `frontmatter_valid` | gate | both `SKILL.md` files keep `name` and `description` |

Gate details are always null, so `verify` compares exactly and no holdout count reaches the
mutator.

## Reviewing a week

```sh
cat ~/.ritual/weekly/reports/<run>.md
git -C ~/.ritual/weekly/repo diff <base> autoresearch/<run-id> -- skills/
```

Take what you agree with into a PR here, and port it to
[decebal-codex-skills](https://github.com/decebal/decebal-codex-skills) by hand: the loop
edits only the Claude skills.

## Limits

- A check is a phrase match. A candidate can close a case by saying the words without the
  meaning; the mutator is told not to, the holdout catches some of it, and the review catches
  the rest. Keep is not ship.
- The loop runs on the machine that recorded the feedback. Events from other machines are not
  merged; the store has no sync configured.
- `claude -p --restricted` is confined by Claude Code itself, not by an OS sandbox, and runs
  with the user's credentials.
- `ritual-feedback weekly` has no SIGTERM handler. Unloading the LaunchAgent in the middle of a
  candidate leaves that `claude -p` running; the next call kills its process group (recorded in
  `~/.ritual/weekly/mutator.pgid`) before doing anything else, and resets the worktree.
