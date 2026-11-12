# Shout flag: plan

Design: docs/plans/2026-11-12-shout-design.md (approved, sha256
1e14b1c7da47892e3276a58b0c9fff8f5909abff7cb3688941385480a48e2d15).

The two plan questions were answered by the agent for this proof run, not asked of a person:
epic gate C (`cargo test`), story gate D (run the CLI and compare its output), trophy A (unit
only), block merge D (all).

### Quality gates

#### Epic-level (once, when every story is done)
- `cargo test`: `10 passed; 0 failed`
- `cargo test -- --list` lists the 10 matrix tests

#### Story-level
- CLI stories: run `cargo run -q -- <args>` and compare stdout and exit status with the design's
  acceptance list

### Testing trophy

#### Unit
- [ ] [Logic] `render` matrix, 7 cases (file: `src/main.rs`)
- [ ] [Logic] `emit` matrix, 3 cases: Ok, BrokenPipe, Other (file: `src/main.rs`)

Block merge: all 10. Block release: all 10. Follow-up allowed: none.

### Stories

#### US-001: `render` with the `--shout` flag [Backend]
As a user, I want `greeter --shout Ada` to print `HELLO, ADA!` so that I get a banner without
piping through `tr`.

- [ ] Test: the 7 `render` cases (file: `src/main.rs`), written first and failing
- [ ] `cargo run -q -- --shout Ada` and `cargo run -q -- Ada --shout` print `HELLO, ADA!`
- [ ] `cargo run -q -- Ada` prints `Hello, Ada!`
- [ ] All tests green: `cargo test`

#### US-002: `emit` and a panic-free `main` [Backend]
As a script author, I want `greeter` to exit 0 on a closed pipe and on a non-UTF-8 argument so that
`greeter | head` and odd bytes never look like a failure.

- [ ] Test: the 3 `emit` cases (file: `src/main.rs`), written first and failing
- [ ] `cargo run -q -- --shout $'\xff'` prints `HELLO, ` U+FFFD `!` and exits 0
- [ ] `target/debug/greeter --shout Ada | true`: `PIPESTATUS[0]` is 0, stderr empty
- [ ] All tests green: `cargo test`

US-002 is blocked by US-001 (it wires `render` into `main`).

### Functional requirements
- FR-1: `--shout` anywhere in the arguments upper-cases the greeting with `str::to_uppercase`.
- FR-2: without `--shout`, output is unchanged.
- FR-3: the process exits 0 on success and on a broken pipe, 1 on any other stdout write error,
  and never panics.

### Non-goals
- A `--` terminator or a parser crate.
- Rejecting unknown flags.

### Open questions
- None.
