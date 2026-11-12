# Shout flag: what shipped

Date: 2026-11-12. Epic: t-de1322. PR: none, local-only proof run; shipped as `ff1fd72`, a local
fast-forward of `master`.

## Shipped
`greeter --shout Ada` and `greeter Ada --shout` print `HELLO, ADA!`; `greeter Ada` still prints
`Hello, Ada!`. A non-UTF-8 argument prints U+FFFD and exits 0 (it panicked with exit 101 before),
and `greeter --shout Ada | true` exits 0 with nothing on stderr in 200 of 200 runs (it panicked
with exit 101 before). Evidence: `cargo test` 10 passed; the code reviewer ran acceptance 1 to 6
(`docs/plans/2026-11-12-shout-code-review.md`).

## Graph
Planned: 2 beads, US-001 (`render`) then US-002 (`emit`, blocked by US-001). Added during Build:
none. Wrong edges: none. Added in Reflect: `t-f497b3`, design drift follow-up.

## Gates
| Gate | Caught |
|---|---|
| Design review, round 1 | blocking: `env::args()` panics on non-UTF-8 argv (exit 101), missing from the ISA inventory |
| Design review, round 2 | blocking: `println!` panics on a broken pipe (exit 101); the design claimed "exit always 0" |
| Design review, round 3 | clean, 3 nits |
| `cos ritual check` before approval | `unapproved`, exit 1: Plan could not start |
| Approval | the first attempt, run through Claude Code's `!` shell, was refused by `cos` itself: "needs a terminal" |
| Build → Review | tests ran red first in both beads (7 failed, then 3 failed), then green |
| Code review | clean, 2 nits |

Both blocking findings were bugs that already existed in the 11-line program; the Semantic ISA
inventory is what made the reviewers look for them.

## Drift
`emit` calls `out.flush()` after `writeln!`; the approved design names `writeln!` only (code
review nit 1). The design was not edited here: changing its bytes reads as `changed since
approval`. Follow-up bead `t-f497b3` takes it through review and approval.

## Next
- `t-f497b3`: name the flush in the design; re-review; a person re-approves.
