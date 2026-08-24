# Ledger — `.beads/` references in chronis context

Driving to zero the `.beads/` paths that claim chronis stores tasks there.

## Setup

**Frozen corpus** — three trees, unchanged during the loop:

1. `decebal-claude-skills` @ `742f8e8`, tracked `*.md` / `*.json` / `*.sh`
2. `~/.claude/skills/` — the global install
3. `~/.agents/skills/` — a second skill root on the same machine, holding
   independent copies rather than links

**Ground truth**, read rather than assumed. `cn --help` says `cn init` creates
"a new **.chronis** workspace", and the only command naming the other directory
is `cn migrate-beads`, which *imports* `.beads/issues.jsonl`. A live chronis
project (`longhand`) has `.chronis/{config.toml,storage/,wal/}` and no `.beads/`.

**Scalar** — count of `.beads` OCCURRENCES asserting that chronis stores there.
Lower is better. Occurrences, not lines: two comparison tables carry two each on
a single line, and a line count would hide half the corpus.

An occurrence is RIGHT when it attributes `.beads/` to `bd` or `br`, or documents
`cn migrate-beads`. WRONG when it implies chronis reads or writes there.

**Gate**

- every surviving occurrence justified, enumerated by hand — not sampled
- the `bd` and `br` columns of the CLI comparison tables unchanged
- relative markdown links: 0 broken
- no NEW unclosed fence
- `tests/run.sh` green

**Baseline — 17 wrong**, 6 right, 23 total.

## Ledger

| # | Proposal | Scalar | Δ vs baseline | Verdict |
|---|---|---|---|---|
| 0 | Baseline | 17 | — | — |
| 1 | `chronis-git-best-practices.md`: delete the invented second store | **11** | **−6** | **Keep.** Not a path typo. The file documented a `.beads/` directory "created by `cn create`" holding "the SQLite DB and JSONL exports", and an export recipe using `cn sync --flush-only`. None of it exists: there is no `cn create`, `cn sync` syncs with a remote Core, and chronis has no JSONL export. Substituting the path would have left every false claim standing |
| 2 | Allowlist the 3 new occurrences in that file | 11 → 8 measured as right | 0 | **Keep, and worth flagging.** The replacement section has to name `.beads/` to explain the distinction, and quotes `cn migrate-beads` from `--help`. Growing an allowlist to move a number is how this loop cheats, so: these three assert the OPPOSITE of the wrong claim, and each was read before being listed |
| 3 | Four repo skills + `claude-workflow.md`: storage claims | **3** | **−14** | **Keep.** `claude-cn-beads`, `claude-create-beads-rust`, `rewind-beads` all said "Beads are stored in: `.beads/` directory (SQLite DB + JSONL export)". All three drive `cn`. Now `.chronis/` (WAL + Parquet), with the SQLite claim dropped |
| 4 | `create-plans`: the chronis-detection hint | 3 | — | **Keep.** It told the reader to detect chronis by looking for `.beads/`, which is the one directory a chronis project does not have — so the check it describes never fires |
| 5 | The two global roots | **0** | **−17** | **Keep.** `~/.claude/skills/create-plans` and `~/.agents/skills/{create-plans,ralph-tui-create-beads-rust}` are independent COPIES, not links, so the repo fix did not reach them |

**Net: 17 → 0.** 9 occurrences remain, all verified right: 6 in the `bd`/`br`
columns of three comparison tables, 3 in the section explaining the distinction.

## Gate results

| Check | Result |
|---|---|
| Survivors justified | 9/9, read individually |
| `bd`/`br` table columns | unchanged |
| Broken links | 0 |
| Unclosed fences | 1, pre-existing and unchanged — see below |
| `tests/run.sh` | green |

## Open

**`cn create` does not exist.** Found while reading `cn --help` for the ground
truth. The surface is `cn task create`; a bare `cn create` exits 2 with
"unrecognized subcommand". It appears **35 times** across five files that ship as
instructions — `claude-cn-beads` (12), `rewind-beads` (11),
`claude-create-beads-rust` (8), and both workflow guides (2 each). Every one of
them tells the agent to run a command that fails. Strictly outside this loop's
scalar, and a bigger correctness problem than the paths were.

**`skills/create-plans/workflows/execute-phase.md`** has an unclosed fence: 982
lines, 57 fences, seven places where a tagged fence lands as a closer. Pre-existing,
untouched here, and already documented in the fence pass.

**The global copies drift, and linking them is NOT the fix.** An earlier draft of
this ledger said it was. That was wrong, and the correction is worth more than
the original claim.

`~/.claude/skills/create-plans` and `~/.agents/skills/create-plans` are
directories, not symlinks, so a repo fix has to be applied three times — which is
how the `.beads` line got fixed in the `~/.agents` copy while a dead
`create-agent-skills` link two files over did not.

But `~/.agents/skills/` is **Codex's** root, loaded via `HOME`, and its copy is
deliberately different: *"tasks that **Codex** will execute"*, *"**Codex**-executable
plans"*. Linking it to the repo would tell Codex that Claude runs its plans.

The same asymmetry defeats a blanket sync. The repo now points readers at
`skill-creator`; that skill exists in `~/.claude/skills` and does **not** exist in
the Codex root, so copying the fix across would have traded one dead link for
another. The Codex copy names `find-skills` instead, which is installed there.

Of the seven skills present in both the repo and the Codex root, **six differ**,
and several carry files the repo does not have — `dev-browser` alone has a
`node_modules` and 484 MB of browser `profiles`. Exactly one, `find-skills`, is
byte-identical and could be linked without loss.

**`~/.agents/skills/` still holds pre-rename skills** — `ralph-tui-create-beads-rust`,
`ralph-tui-create-json`, `ralph-tui-prd`. The rename landed in the repo and in
`~/.claude/skills`; it never reached this root.

## Instrument notes

Two ways the measurement lied, both caught, both worth knowing:

- **`rtk`'s `diff` wrapper reported "Files are identical" for files that differ.**
  `diff -rq` flagged the pair, per-file `diff` called them identical, and `cmp`
  settled it: char 4754, line 155. Any conclusion drawn from the wrapper alone
  would have been backwards.
- **The counter counted this ledger.** A document about `.beads` is citations, not
  claims. `docs/ledger/` is excluded — it postdates the frozen corpus rather than
  being carved out of it.

## Method note

The scalar was measured by a script, and the script was wrong once. Its first
version counted lines rather than occurrences, which would have reported the
comparison tables as one reference each instead of two. Worth stating because the
number in this ledger is only as good as the thing that produced it.
