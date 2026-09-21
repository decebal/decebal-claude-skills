# prompt-id

One prompt-number namespace per repo, for a machine that runs many `git worktree`s.

## Why this exists

`.prompts/` is gitignored (often via the user's GLOBAL gitignore), so **every worktree
holds its own copy**. A session that numbers a new prompt by listing the directory it
happens to be standing in sees a near-empty one and lands on a number the main checkout
already used.

Measured in one repo before this tool: **53 duplicated numbers across 246 files**, plus a
**186..503 hole** where a session escaped the collisions by jumping to the 500s by hand.
Duplicates are not harmless — `/run-prompt 179` matched three unrelated pieces of work,
and one `run 208` was ambiguous between a queue prompt that had shipped five days earlier
and a signup prompt that had not.

## Build

Zero dependencies. Plain `rustc` is enough:

```bash
rustc --edition 2021 -O gates/rust/prompt-id/src/main.rs -o ~/.claude/bin/prompt-id
```

The Cargo manifest exists so the tests run with every other gate in this workspace —
a tool whose tests run nowhere is indistinguishable from one that passes
([rules/testing-gates.md](../../../rules/testing-gates.md)):

```bash
cargo test --manifest-path gates/rust/Cargo.toml -p prompt-id
```

## Use

```bash
prompt-id alloc <slug>       # claim a number, create the file, print its path
prompt-id alloc --dir <slug> # same, as a meta-prompt stage DIRECTORY
prompt-id store              # the resolved store, for reading and listing
prompt-id next               # the number a caller would get right now
prompt-id archive <n>        # move a finished prompt into completed/ and make it tracked
prompt-id shipped            # root prompts whose reserved branch already merged
prompt-id audit [--since n]  # duplicates + unused runs; non-zero if a number is shared
```

## The two properties that matter

**One store.** `git rev-parse --git-common-dir` answers with the MAIN repository's `.git`
from inside any worktree, so every caller resolves the same `.prompts/` — unlike
`--show-toplevel`, which answers with the worktree the caller is in.

**The claim is on the NUMBER, not the filename.** Creating `NNN-<slug>.md` with
`create_new` looks atomic and is not: two sessions allocating the same number under
different slugs write different paths, so both succeed and both believe they own it.
That was observed — racing three allocations produced `189-race-b.md` and
`189-race-c.md`. The claim is a file named for the number alone, under
`.prompts/.numbers/`, so exactly one caller can win it.

Numbering is **monotonic**: allocation takes highest + 1 and never reaches back into a
hole, because a number's only job is to say what came after what.

**A numbered DIRECTORY holds its number too.** Meta-prompts are written as
`NNN-<topic>-<purpose>/` stage directories. While only `*.md` was scanned they were
invisible in both directions: with such a directory on disk at 214 and 213 the highest
file, `next` answered `214` — a number already in use. `alloc --dir` claims through the
same lock, so the two namespaces cannot drift apart.

## Being the only issuer is the whole point

A race-free allocator that most callers bypass is not an allocator. In one store, 6 of
27 prompts written after the tool existed bypassed it, and **everything issued after one
particular afternoon did** — every duplicate is proof the tool was not the issuer.

Two things close that, and both are needed:

- **`prompt-id alloc` creates the file**, so there is no window between being handed a
  number and using it. A subcommand that only PRINTED a number would reintroduce the
  race it exists to remove.
- **`claude-guard prompt-number`** ([../claude-guard](../claude-guard)) denies a `Write`
  or `Edit` creating `.prompts/NNN-*.md` when `.prompts/.numbers/NNN` does not exist.
  The file that made this necessary was created by a bare `Write` in a session that
  never invoked the slash command carrying the rule, so no wording anywhere could have
  reached it. It fails open on an existing file and on a store with no `.numbers/` at
  all, so it is inert in a repo that has not adopted the allocator.

## Archiving is part of the tool, not a checklist

A finished prompt has to (1) move into `completed/`, (2) be `git add -f`'d because
`.prompts/` is gitignored, and (3) be verified as tracked. Written down as three manual
steps, that was honoured **15 times out of 255** in one store — so 94% of the archive
existed only on the machine that ran it, and every other clone saw a store that looked
almost empty. `prompt-id archive <n>` does all three and fails loudly if the result is
not tracked.

It refuses a number two prompts share rather than guessing, because 55 numbers are
shared in that store and guessing archives the wrong one.

**Archiving promptly makes naive numbering WORSE**, which is why the two halves have to
land together. Archiving removes the only signal a non-allocator numberer reads: one
prompt was archived, and ten days later the store root's visible maximum was one BELOW
it, so a `max + 1` numberer reissued the number that had just shipped.

## Finding what already shipped

`prompt-id shipped` reads the branch each root prompt reserves, makes ONE
`gh pr list --state merged` call, and matches locally. A prompt whose branch has merged
is finished work still sitting in the queue — the signal that caught two prompts left
there for days, and six more on first run. It exits non-zero when it finds any.

Branch names wrap. Six of twenty prompts in one store had the name on the line AFTER the
word introducing it, so the extractor reads across the wrap; only separators may sit
between the word and the name, which is what keeps prose ("the second branch of the
parser is reached") out.

Prompts reserving no branch are listed separately and do **not** fail the check. A prompt
that never named a branch is unanswerable rather than wrong, and a check that fails on it
is red forever, which is how `audit` came to be wired to nothing.

## Why `audit` takes `--since`

`audit` returns failure on any duplicate, and 55 already existed before this tool did —
so an unfloored audit is permanently red and can be a gate for nothing.
`prompt-id audit --since <n>` fails only on duplicates at or above `n`.

Pick the floor from the store: one above the highest number that is already duplicated is
green today and fails the moment a NEW collision appears, which is the whole job.
Renumbering the old ones is the wrong fix — numbering is monotonic precisely so a number
says what came after what, and rewriting history there breaks every reference that
already exists.
