---
title: The Ritual Edition
date: 2026-11-12
desk: Skills · ritual and ritual-full
strap: Explainer | one real flow, measured | evidence in docs/ritual-proof/
publisher: Wolven Tech
footer: Built from `docs/ritual-explained.md` with report-press · evidence in `docs/ritual-proof/`
---

# One flow, seven steps, one human gate: a real run on a toy CLI
@kicker Explainer
@deck The ritual skill took a one-flag feature from idea to a shipped commit in 15m26s of agent time, and waited 22m20s for the one thing it is not allowed to do: approve its own design.
@byline A proof run of `ritual` on 2026-11-12 · every duration measured from `log.jsonl` · sources labelled inline

The feature was deliberately small: `greeter --shout Ada` prints `HELLO, ADA!` in an 11-line Rust CLI. The flow ran every ritual step on it in a scratch repo: Think, Spec, Plan, Build, Review, Ship, Reflect, each step writing one file and naming it on a Chronis epic, `t-de1322` {{observed: `docs/ritual-proof/epic.toon`}}. Every command went through a wrapper that logged its start time, exit code and output, 89 lines in all {{observed: `docs/ritual-proof/log.jsonl`}}.

Three fresh-context design reviews ran before the design was clean. The first two each found a real panic in the existing 11 lines that exits 101: one on a non-UTF-8 argument, one on a closed pipe {{observed: `2026-11-12-shout-design-review-r1.md`, `…-r2.md`}}. The shipped commit fixes both, and 10 tests prove it {{observed: `cargo test` in the log, `10 passed; 0 failed`}}.

> The agent can write every file in the flow. It cannot approve the design, and two separate guards refused when it was asked to.

> [!NOTE]
> **Limit.** One run, one machine (macOS), one toy feature, the lean `ritual` skill. Nothing was pushed: Ship was a local squash and fast-forward. EGM commands and `cos` hires were not run; `ritual-full` appears here as its preflight and the commands it would run.

```markdy State diagram of the ritual: seven steps, three gates, one person, and the loops back
scene type=state
layout TB
start Think "Think: brief"
state Spec "Spec: design + review"
decision Gate "approved, unchanged?"
state Plan "Plan: plan + beads"
state Build "Build: commits"
decision Fresh "fresh review clean?"
state Ship "Ship: one commit"
end Reflect "Reflect: report"
beat reveal "Steps":
  show $nodes stagger=70ms
beat flow "The happy path":
  Think -> Spec "brief.md"
  Spec -> Gate "design + clean review"
  Gate -> Plan "a person approved"
  Plan -> Build "beads"
  Build -> Fresh "beads done, tests ran"
  Fresh -> Ship "clean"
  Ship -> Reflect "shipped"
beat loops "Drift and findings go back":
  Spec <- Gate "no, or drift"
  Build <- Fresh "findings"
  glow Gate color=#c9a95c
```

## The run: 15m26s of agent work, 22m20s waiting for a person {#s-timeline}
@kicker Timeline
@deck Each pip is one step, its measured duration, and the file it handed to the next step.
@short The measured timeline

Durations are wall-clock gaps between the first logged command of a step and the first of the next, at one-second resolution, so they include the model's thinking between commands {{observed: `log.jsonl` timestamps}}. Spec is the long step because it holds three review rounds: 1m42s, 1m46s and 2m22s from dispatch to verdict {{observed: `log.jsonl`, `spec` lines}}.

| Step | Measured | Handed over |
|---|---|---|
| Think | 14s | `2026-11-12-shout-brief.md`, epic `t-de1322` |
| Spec | 9m34s | design with a Semantic ISA section, 3 review rounds, gate check `unapproved` |
| Waited for a person | 22m20s | `ritual-approved: … sha256:1e14b1c7… by <name>` |
| Plan | 49s | `2026-11-12-shout-plan.md`, beads `t-d70fcd` → `t-032709` |
| Build | 1m17s | 2 commits, tests red first then green |
| Review | 2m22s | `2026-11-12-shout-code-review.md`, verdict `clean` |
| Ship | 32s | one commit `ff1fd72`, diff against the pre-squash head empty |
| Reflect | 38s | `2026-11-12-shout-report.md`, follow-up bead `t-f497b3` |

The person's wait runs from the agent stopping at the gate (09:43:59) to Chronis recording `workflow.approval.granted` (10:06:19) {{observed: `log.jsonl` line `approval-wait`, `epic.toon` event list}}. The approval then took another 6m51s to reach the agent through the orchestrating session {{observed: `log.jsonl` 10:13:10}}. Wall clock from Think to the closed epic: 44m38s {{inferred from the first `think` and last `reflect` timestamps}}.

```markdy Timeline of the proof run with each measured step duration and its hand-over
scene type=timeline
layout TB
step Think "Think 14s: brief"
step Spec "Spec 9m34s: design, 3 reviews"
user Wait "A person 22m20s: approve"
step Plan "Plan 49s: plan, 2 beads"
step Build "Build 1m17s: 2 commits"
step Review "Review 2m22s: code review"
step Ship "Ship 32s: ff1fd72"
step Reflect "Reflect 38s: report"
beat reveal "Seven steps and one wait":
  show $nodes stagger=150ms
beat gate "Waited for a person":
  glow Wait color=#c9a95c
```

## Three design reviews found two panics the author missed {#s-reviews}
@kicker Spec
@deck Each reviewer saw only the design and the code, never the author's conversation.
@short Three review rounds

Each review was a new subagent given two paths: the design and the repo root {{observed: `log.jsonl`, "goldfish review … dispatched" lines}}. Round 1 ran the existing binary with a byte `0xFF` in the argument and saw `env::args()` panic with exit 101; the design's side-effect inventory said "stdout only", so the finding was blocking {{observed: `2026-11-12-shout-design-review-r1.md`, finding 1}}. The fix moved `main` to `args_os` with `to_string_lossy`.

Round 2 built a scratch crate from the revised design and piped it into `true`: `println!` panicked on the closed pipe, again exit 101, and the design now claimed "exit always 0" {{observed: `2026-11-12-shout-design-review-r2.md`, finding 1}}. The fix added `emit`, which matches the write result and treats `BrokenPipe` as success. Round 3 ran that prototype 200 times against the pipe, saw exit 0 every time, and returned `clean` with 3 nits {{observed: `2026-11-12-shout-design-review.md`}}.

Both bugs were in the program before the feature. The Semantic ISA inventory asks for every side effect, including stderr and the exit status, and that question is what sent the reviewers to look {{inferred from both findings citing the inventory}}. Each superseded review stays in the repo with `design-sha256: superseded` {{observed: `…-r1.md` line 3}}.

```markdy State diagram of the design review loop over three rounds
scene type=state
layout LR
start Draft "design drafted"
decision Review "fresh review clean?"
state Fix "resolve in design"
end Clean "round 3: clean"
beat reveal "States":
  show $nodes stagger=60ms
beat loop "Rounds 1 and 2: blocking":
  Draft -> Review "sha256 e94261…, 264820…"
  Fix <- Review "panic on 0xFF; panic on closed pipe"
  Draft <- Fix "new digest"
beat done "Round 3":
  Review -> Clean "sha256 1e14b1…"
  glow Clean color=#2E7D32
```

## Who does what: the agent drives, a person approves, the tools keep the record {#s-who}
@kicker Sequence
@deck The messages below are the real commands from the run, in order.
@short Who does what

The agent ran every command except one {{observed: `log.jsonl`}}. Chronis (`cn`) held the epic, the beads and their dependency edge; `cos` answered the gate (`unapproved`, exit 1, then `approved: … unchanged since approval`) and inferred the step for `cos flows` at seven points, reading spec, spec, build, review, ship, reflect and done {{observed: `log.jsonl`, `cos … flows` lines}}. The fresh reviewers wrote their verdict files and nothing else.

In `ritual-full` two more parties join: EGM, whose `/egm:` commands the user types because they set `disable-model-invocation: true`, and `cos agents spawn` hires for the Goldfish, builder and reviewer {{observed: `skills/ritual-full/SKILL.md`}}. Neither ran here; they appear dashed below.

```markdy Sequence of messages between the person, the agent, Chronis, cos, fresh reviewers and EGM
scene type=sequence
layout LR
user You "You"
service Agent "Agent"
database Cn "Chronis cn"
security Cos "cos"
service Fresh "Fresh reviewer"
service Egm "EGM, not run"
beat reveal "Parties":
  show $nodes stagger=60ms
beat think "Think and Spec":
  You -> Agent "plan --shout"
  Agent -> Cn "cn task create --type epic"
  Agent -> Fresh "design path + repo root"
  Agent <- Fresh "findings, findings, clean"
  Agent -> Cos "cos ritual check"
  Agent <- Cos "unapproved, exit 1"
beat gate "The one human gate":
  You -> Cos "cos ritual approve, own terminal"
  Cos -> Cn "ritual-approved + cn approve"
beat build "Plan to Reflect":
  Agent -> Cn "cn task create US-001, US-002"
  Agent -> Fresh "diff + design"
  Agent <- Fresh "clean"
  Agent -> Cn "cn done t-de1322"
  Agent ~> Egm "/egm:egm-review, typed by you"
```

## The epic is the flow's memory: nine lines name every hand-over {#s-epic}
@kicker Source of truth
@deck Any session can find the flow's step and files from the epic alone.
@short The epic's trail

The epic's description at the end of the run, one line per step, newest line of each key wins {{observed: `cn show t-de1322 --toon`, `docs/ritual-proof/epic.toon`}}:

```text
ritual-brief: docs/plans/2026-11-12-shout-brief.md
ritual-branch: feat/shout
ritual-design: docs/plans/2026-11-12-shout-design.md
ritual-approved: docs/plans/2026-11-12-shout-design.md sha256:1e14b1c7da47892e3276a58b0c9fff8f5909abff7cb3688941385480a48e2d15 by <name>
ritual-plan: docs/plans/2026-11-12-shout-plan.md
ritual-review: docs/plans/2026-11-12-shout-code-review.md clean
ritual-pr: local-only (proof)
ritual-shipped: ff1fd72 (local fast-forward of master, proof; nothing pushed)
ritual-report: docs/reports/2026-11-12-shout.md
```

`ritual-approved:` is the only line the agent did not write: `cos ritual approve` appended it when the person confirmed, and the same second Chronis logged `workflow.approval.granted` {{observed: `epic.toon` events at 10:06:19}}. The design's digest is in that line, so any later edit to the design reads as `changed since approval` and sends the flow back to Spec {{observed: `skills/ritual/references/plan.md`}}. That is why the code reviewer's nit about a missing word in the design became bead `t-f497b3` rather than an edit {{observed: `2026-11-12-shout-report.md`, Drift}}.

```markdy C4 container view of the files each step hands to the epic
scene type=architecture
layout LR
repo Brief "brief.md"
repo Design "design.md + reviews"
repo Plan "plan.md"
repo Code "commits"
repo Report "report.md"
database Epic "epic t-de1322"
user You "a person"
group branch "On the flow's branch": Design Plan Code
beat reveal "Containers":
  show $nodes stagger=60ms
beat name "Each step names its file":
  Brief -> Epic "ritual-brief"
  Design -> Epic "ritual-design"
  You -> Epic "ritual-approved"
  Plan -> Epic "ritual-plan"
  Code -> Epic "ritual-review, ritual-shipped"
  Report -> Epic "ritual-report"
  glow Epic color=#5f6b76
```

## ritual-full adds five tools to the same seven steps, and refuses to start without them {#s-full}
@kicker ritual vs ritual-full
@deck Same files, same epic lines, same gates; more checks inside Think, Spec, Build and Review.
@short ritual vs ritual-full

| | `ritual` (this run) | `ritual-full` |
|---|---|---|
| Missing tool | that path is skipped | preflight stops and prints the install command |
| Think | brief in conversation | you type `/egm:egm-brainstorm`; `graphify query` on the touched modules |
| Spec design | decision, context, design, security, testing, acceptance | the same plus a mandatory Semantic ISA section |
| Spec review | one fresh reviewer per round | two in separate contexts: `/egm:egm-review` and a hired `ritual-goldfish` |
| Build, Review | in session | `cos agents spawn --role builder` per bead, `--role reviewer` with a sink-to-VERIFY check |
| Reflect | report | plus `/egm:egm-drift-report` and a new-sink diff |

This run used ritual-full's Semantic ISA section anyway, which is where both review findings came from {{observed: the design's `## Semantic ISA`}}. The ritual-full preflight ran for real on the same machine {{observed: `log.jsonl`, `preflight` lines}}:

| Tool | Check run | Result |
|---|---|---|
| Chronis `cn` | `cn --help`; `.chronis/` present | ok [[have]] |
| EGM | `claude plugin list` | `egm@egm` 0.1.1, enabled [[have]] |
| semantic-isa | `~/.claude/skills/semantic-isa/SKILL.md` | present [[have]] |
| cos | `cos ritual check --help`; `cos --version` | exit 0; `cos 0.5.0` [[have]] |
| graphify | `graphify --help` | exit 126, `bad interpreter` (its pipx venv is gone); fix: `uv tool install graphifyy` [[gap]] |
| claude-guard | payload test for `cn approve` and `cos ritual approve` | `"permissionDecision":"deny"` for both [[have]] |
| skills | `pr-proof`, `systematic-debugging` | present [[have]] |

So the preflight line on this machine reads `graphify missing (uv tool install graphifyy)`, and ritual-full would stop there until it is reinstalled or the user says "continue without graphify" {{inferred from `skills/ritual-full/SKILL.md` preflight rules}}. The commands it would then run, none run in this proof: `/egm:egm-draft`, `/egm:egm-review docs/plans/2026-11-12-shout-design.md`, `cos agents spawn --role ritual-goldfish --repo ../greeter-shout`, `cos agents spawn --role builder`, `cos agents spawn --role reviewer`, `/egm:egm-drift-report docs/plans/` {{not checked}}.

```markdy C4 container view of where the ritual-full tools plug into the steps
scene type=architecture
layout LR
step Think "Think"
step Spec "Spec"
step Build "Build + Review"
step Reflect "Reflect"
runtime Egm "EGM"
policy Isa "Semantic ISA"
security Two "two Goldfish"
service Hires "cos hires"
database Graph "graphify"
group steps "The seven steps": Think Spec Build Reflect
beat reveal "Containers":
  show $nodes stagger=50ms
beat plug "Where each plugs in":
  Egm -> Think "brainstorm"
  Graph -> Think "context"
  Egm -> Spec "draft, review"
  Isa -> Spec "ISA section"
  Two -> Spec "two reviews"
  Hires -> Build "builder, reviewer"
  Egm -> Reflect "drift report"
  glow Isa color=#5f6b76
```

## The toy's only sinks are stdout and the exit code, and each has a deterministic check {#s-isa}
@kicker Semantic ISA
@deck Taint enters through argv; nothing reaches a sink except through a VERIFY with a test.
@short The ISA section

The design's inventory lists three side effects and no others: one line to stdout, one line to stderr only on a non-pipe write error, and the exit status 0 or 1 {{observed: `2026-11-12-shout-design.md`, Side-effect inventory}}. There is no file, network, API, message, deploy or data write. The taint source is the `NAME` argument; the source and both sinks belong to the person typing the command, so no trust boundary is crossed, and the design says so instead of inventing a sanitiser {{observed: the design's Taint and sinks}}.

The code reviewer mapped each sink to its check at file:line: `stdout.write` at `src/main.rs:22` is reached only from `:39`, after `render`; `process.exit` at `:39` takes only the code from `emit`'s match at `:22-30`; `rg` found no other `println!` or `exit` {{observed: `2026-11-12-shout-code-review.md`, Sinks}}. Ten tests cover the matrix, including a writer that fails with `BrokenPipe` and one that fails with `Other` {{observed: `docs/ritual-proof/greeter-main.rs`}}.

```markdy C4 container view of taint from argv through VERIFY to the two sinks
scene type=architecture
layout TB
user Argv "argv: NAME"
service Render "render"
security V1 "VERIFY render_shape"
security V2 "VERIFY write_result"
desktop Stdout "stdout sink"
runtime Exit "exit 0 or 1"
beat reveal "Containers":
  show $nodes stagger=60ms
beat taint "Taint to sink":
  Argv -> Render "lossy UTF-8"
  Render -> V1 "line"
  V1 -> Stdout "7 render tests"
  Stdout -> V2 "io::Result"
  V2 -> Exit "3 emit tests"
  glow V1 color=#c9a95c
  glow V2 color=#c9a95c
```

## Two separate guards refused agent approval; only a person at a terminal got through {#s-gates}
@kicker Gates
@deck claude-guard denies the command in an agent's shell; cos refuses any approval without a terminal.
@short Why agents cannot approve

The first guard is claude-guard's `infra-guard` hook. Fed a payload, it denies both approval commands {{observed: `log.jsonl`, `preflight` lines}}:

```text
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"cn approve marks a design approved — ask the user to run it in their own terminal (infra-guard)"}}
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"cos ritual approve is a person's sign-off on a design — ask the user to run it in their own terminal (infra-guard)"}}
```

The same hook also refused the agent's own `cos ritual approve --help` during this run, before any approval existed {{observed: this session's tool output}}.

The second guard is `cos` itself. The first approval attempt was typed through Claude Code's `!` shell, which has no terminal on stdin, and `cos` refused it: `Error: cos ritual approve needs a terminal: run it yourself, not through an agent` {{observed: `log.jsonl`, `approval-wait` line at 10:13:10, as relayed by the orchestrating session}}. The second attempt, in the person's own terminal, asked for `yes` and wrote the approval {{observed: `epic.toon` event `workflow.approval.granted`}}.

| Gate | Checked by | In this run |
|---|---|---|
| Spec → Plan | `cos ritual check t-de1322` | `unapproved`, exit 1; after approval `approved: … unchanged since approval` [[have]] |
| Build → Review | both beads done; `cargo test` ran | 7 failed then 7 passed; 3 failed then 10 passed [[have]] |
| Review → Ship | fresh review newer than the last change | `clean`, then the squash re-checked: diff empty, gate still green [[have]] |
| Ship | one PR, one commit | one commit; no PR, local proof [[part]] |

The cos docs name the limit: `--by` is a name, not an identity, and a call hidden behind a variable or a script gets past the hook {{observed: `chief-of-staff-v2/docs/ritual.md`}}. The terminal check is what holds then.

```markdy State diagram of three approval attempts and the guard that handled each
scene type=state
layout LR
start Ask "approve the design"
decision Who "who runs it?"
end Deny "denied"
end Done "approved"
beat reveal "States":
  show $nodes stagger=60ms
beat try "Attempts":
  Ask -> Who "cos ritual approve"
  Who -> Deny "agent Bash: claude-guard"
  Who -> Deny "! shell: cos, no terminal"
  Who -> Done "a person, own terminal, yes"
  glow Done color=#2E7D32
```

## Run it yourself {.back}

1. `ritual` on any repo with `cn`: ask Claude "plan this feature: …"; the steps and commands are in `docs/ritual-zero-to-hero.md`.
2. Read the raw evidence: `docs/ritual-proof/log.jsonl` has one JSON line per command (time, step, command, exit, output), with scratch paths replaced by `<scratch>`.
3. Re-derive any duration in the timeline from those timestamps; nothing on this page is an estimate.

## What I have not checked {.notchecked}

- EGM commands (`/egm:egm-brainstorm`, `-draft`, `-review`, `-resolve`, `-drift-report`): not run; they need the user to type them.
- `cos agents spawn` hires (Goldfish, builder, reviewer): not run; they start paid Claude runs.
- A real PR, CI and a forge merge: Ship was a local squash and a local fast-forward of `master`; nothing was pushed.
- graphify context: the binary is broken on this machine (exit 126) and the toy repo has no `graphify-out/`.
- Linux: everything ran on macOS only.
- The exit-1 branch of `emit` end to end: covered by a unit test only; macOS has no `/dev/full`.
- Durations at sub-second resolution, and how much of each was model thinking versus tool time.
