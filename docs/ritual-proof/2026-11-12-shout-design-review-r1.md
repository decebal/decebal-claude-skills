---
design: docs/plans/2026-11-12-shout-design.md
design-sha256: superseded (read e94261bb7ebfcc6527e616d049c84dfce4ec94118bc5eef4fda3711bf3324a1b)
verdict: findings
---

1. blocking Semantic ISA / Side-effect inventory (also Security and privacy): the inventory and line 29 say the only output is one line to stdout, but `env::args()`, which the design keeps in `main` (line 20), panics on a non-UTF-8 argument, writes a panic message to stderr and exits with status 101. Stderr and the exit status are side effects a calling script sees, and `LOAD(argv)` is listed as "Deterministic I/O" with no failure path. The behaviour exists today and the design does not change it, but the inventory is incomplete. Evidence: src/main.rs:8; `greeter "$(printf 'a\xff')"` on a scratch build of the current code printed `panicked at .../std/src/env.rs:876:51: called Result::unwrap() on an Err value: "a\xFF"` and `exit=101`. Fix: add an inventory row (stderr panic message plus exit 101 on a non-UTF-8 argv, irreversible n/a, affects the calling script), mark `LOAD(argv)` as able to fail and add a Violations row for it. Or switch to `env::args_os()` with `to_string_lossy` and add a matrix case for it.
2. should Acceptance 1 / ISA test matrix: "`cargo test` passes with the five matrix cases" cannot be checked as written. `cargo test` also passes when zero tests run, and nothing ties the five cases to named tests. The matrix also leaves out two behaviours the design states: `["--shout"]` with no name (gives `HELLO, WORLD!` by line 18) and `["--whisper"]` (gives `Hello, --whisper!` by line 22). Evidence: design lines 22, 68-75, 88. Fix: name one test per case, add the two missing cases, and make acceptance 1 check the count, e.g. `cargo test -- --list` lists the 7 named `render` tests and `cargo test` reports `7 passed`.
3. nit Design / Interface: today `greeter --shout` prints `Hello, --shout!`. After the change it prints `HELLO, WORLD!`, and there is no `--` terminator, so the literal name `--shout` can no longer be greeted. The design says "as today" for unknown flags (line 22) but does not mention this change for the one flag that becomes known. Evidence: the scratch build printed `Hello, --shout!` for `greeter --shout`; design lines 21-22. Fix: state it under Errors or Interface as accepted.

Claims checked and found correct:
- `src/main.rs` is 11 lines, with `greeting(name) -> String` returning `Hello, {name}!`. Evidence: src/main.rs:3-5.
- `main` takes the first argument, defaults to `world`, and prints with `println!`. Evidence: src/main.rs:8-10.
- There are no tests: there is no `#[cfg(test)]` in src/main.rs and no `tests/` directory (checked with a file listing of the repo).
- `[dependencies]` is empty. Evidence: Cargo.toml:6-7.
- `to_uppercase` maps `straße` to `STRASSE` (full case mapping). Evidence: a rustc 1.97.1 snippet printed `Hello, straße! -> HELLO, STRASSE!`, and `ﬁ` became `FI`.
- The acceptance commands can be run: the current crate (edition 2024) builds with rustc 1.97.1 via `cargo build --manifest-path ... --target-dir /tmp/goldfish-scratch/target`, and `cargo run -q -- <args>` forwards `--shout` to the binary.
- The one sink, `println!`, has a deterministic VERIFY (unit tests on the pure `render`). Acceptance 2-3 cover the wiring in `main` end to end.
