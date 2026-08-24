# Gates

The tooling the [rules](../rules/) reference, extracted from a production
Rust + TypeScript monorepo and de-identified.

Everything here exists because the rule alone was not enough. A rule tells the
next person what to do; a gate is what happens when they do not — and where no
CI check can be REQUIRED (a private repo on a free plan cannot have protected
branches or rulesets), the pre-push hook is not belt-and-braces, **it is the
braces**. See [rules/testing-gates.md](../rules/testing-gates.md).

## What is here

| Gate | Language | Does |
|---|---|---|
| [`sh/check-branch-not-merged.sh`](sh/check-branch-not-merged.sh) | bash | Refuses a push to a squash-merged (dead) branch. Fails OPEN with no `gh` |
| [`sh/hook-gate-lib.sh`](sh/hook-gate-lib.sh) | bash | `run_gate` — the 300s ceiling, the clamp, the bounded runner, the cold-cache probe and warm-up |
| [`sh/agent-worktree.sh`](sh/agent-worktree.sh) | bash | One isolated worktree per agent, seeded from [`worktree-seed.conf`](worktree-seed.conf) and verified on disk |
| [`rust/staged-scope`](rust/staged-scope) | Rust | Which gates does this diff actually need? Default-deny: anything unmatched is `unknown` |
| [`rust/check-no-id-refs`](rust/check-no-id-refs) | Rust | No task-tracker ids in source comments — they mean nothing without the tracker |
| [`rust/check-test-hangs`](rust/check-test-hangs) | Rust | Rejects unbounded real blocking I/O in test files, at authoring time |
| [`rust/fmt-check`](rust/fmt-check) | Rust | Whole-repo `rustfmt --check` **without invoking cargo**, so it takes no build locks |
| [`rust/rust-effective-diff`](rust/rust-effective-diff) | Rust | Is this `.rs` change code-effective, or comments and whitespace? |
| [`rust/target-sweep`](rust/target-sweep) | Rust | Reclaim a build directory without forcing a cold rebuild |
| [`rust/graph-audit`](rust/graph-audit) | Rust | Cross-check docs and tasks against a code-graph oracle |
| [`rust/render-agent-docs`](rust/render-agent-docs) | Rust | One manifest → `CLAUDE.md` + `AGENTS.md`, with a `--check` drift gate |
| [`ts/check-remote-recovery.ts`](ts/check-remote-recovery.ts) | TypeScript | A failure branch that offers the reader nothing fails the push |
| [`ts/remote-state.ts`](ts/remote-state.ts) | TypeScript | The `RemoteState<T>` type the gate above backs up — the primary mechanism |
| [`ts/setup-git-keepalive.ts`](ts/setup-git-keepalive.ts) | TypeScript | SSH keepalive so a long hook does not lose the push transport |

## Why some are shell, some Rust, one TypeScript

Default is Rust. The exceptions each have a reason:

- **`hook-gate-lib.sh` is SOURCED by the hooks.** It defines shell functions the
  hook body calls; nothing else can.
- **`check-branch-not-merged.sh` runs before anything is built.** It is the one
  gate that must never be skippable, on a fresh clone with no toolchain, so it
  cannot be a binary that has to be compiled first.
- **`agent-worktree.sh` is git, `cp`, and package installs** — process
  orchestration, where the shell is the natural glue. It is also the
  implementation whose failure modes were actually paid for; a rewrite would put
  that back at risk in the one place where "reports ready but is not" is
  expensive.
- **`check-remote-recovery.ts` reads Svelte templates.** A real parse of one
  belongs in the ecosystem that has one.
- **`setup-git-keepalive.ts` runs from `prepare`, inside the package install**,
  on every platform including Windows — before any toolchain is guaranteed, and
  where a `.sh` cannot be executed at all.

Everything that is classification, scanning, or rendering is Rust, with tests.

## Install

```bash
# every gate, one lockfile
cargo build --release --manifest-path gates/rust/Cargo.toml

# or put them on PATH
cargo install --path gates/rust/staged-scope
cargo install --path gates/rust/check-test-hangs
cargo install --path gates/rust/fmt-check
cargo install --path gates/rust/rust-effective-diff
cargo install --path gates/rust/render-agent-docs
cargo install --path gates/rust/check-no-id-refs
```

The workspace declares **three** external dependencies — `regex`, `serde_json`,
`proc-macro2` — which resolve to 11 crates in the graph. Three of the nine
members declare no dependency at all, and two more depend only on the
in-workspace config reader
([rules/dependency-hygiene.md](../rules/dependency-hygiene.md)). Config is read
by `gates-config`, a dependency-free reader for the small TOML subset these
gates use, rather than by pulling `toml` + `serde` into a binary a git hook has
to build.

```bash
cargo test  --manifest-path gates/rust/Cargo.toml   # 79 tests
cargo clippy --manifest-path gates/rust/Cargo.toml --all-targets
```

## Wire it up

1. Copy [`gates.toml`](gates.toml) to your repo root and edit the paths.
2. Copy [`examples/agent-docs.toml`](examples/agent-docs.toml) if you want
   generated agent docs, and write the overlay it points at.
3. Copy [`worktree-seed.conf`](worktree-seed.conf) and declare every gitignored
   thing a fresh worktree needs.
4. Copy [`examples/pre-push`](examples/pre-push) to `.husky/pre-push` and edit
   the per-scope blocks.

## The three failure modes these are shaped around

**A gate that runs nowhere.** Wiring a check into a CI workflow buys visibility,
not enforcement. A gate that runs in neither CI nor the hook is indistinguishable
from one that passes. Both `check-test-hangs` and `check-remote-recovery` exit
**2** when they match zero files, because a moved directory otherwise turns them
into no-ops that print a tick.

**A ceiling that gets raised instead of respected.** `GATE_TIMEOUT` may be
lowered and is clamped at 300 — the library does not honour a higher value. A
step that needs more than five minutes is a bug in the step
([rules/timeouts.md](../rules/timeouts.md)). When a gate times out, check the
competing build count before you touch the gate: ten worktrees on twelve cores
once made 300s unreachable for every session at once, with the compiler cache
healthy at a 79% hit rate.

**A cold cache read as a broken gate.** A fresh worktree compiles the whole graph
the first time anything builds in it, and the first thing that does is usually
the capped lint gate — killed before it evaluates a line of your diff. Seed the
build directory (`agent-worktree.sh` does, via a copy-on-write clone) or warm it
once, uncapped, with `hook_warm_compile`.
