---
design: docs/plans/2026-11-12-shout-design.md
design-sha256: superseded (read 2648209dc203019fef4469b6038ba660cd18bff09c38849e8b02fd3d44f76107)
verdict: findings
---

1. blocking Semantic ISA / Side-effect inventory: the inventory misses the `println!` failure path. When stdout is a pipe whose reader has exited, `println!` panics, writes `failed printing to stdout: Broken pipe (os error 32)` to stderr, and exits 101. That makes two claims wrong: "its only output is `println!` and the exit status (0)" (design line 45-46) and "the exit status is always 0" (line 49-50). The path exists in today's code too (src/main.rs:10). The design does not cause it, but it does assert it away. Evidence: a scratch crate in /tmp/goldfish-scratch2 implementing the design exactly (`args_os` + `to_string_lossy` + `render` + `println!`); `greeter --shout Ada | true` printed the panic on stderr and returned `PIPESTATUS[0]=101`. Fix: add a stderr / exit-101 row to the inventory and a Violations row ("stdout write fails: panic message on stderr, exit 101, nothing else written"), and drop "always 0". Or have the design write with `writeln!(io::stdout(), ..)` and handle the error (for example, exit 0 quietly on `BrokenPipe`) so the claim becomes true, with a check for it.
2. should Semantic ISA / Instructions + Taint: the inventory lists "exit status" as a capability, but no instruction emits it, it is not a sink, and no VERIFY covers it. No test or acceptance check asserts the exit code. Evidence: design lines 43, 53-58, 65-67, 102-107. Fix: add `TOOL_CALL(process.exit)` (or similar) as a sink with a deterministic check, for example an acceptance step `cargo run -q -- --shout Ada; test $? -eq 0`, plus the failure-path code from finding 1.
3. should Acceptance / ISA test matrix: no runnable check covers the non-UTF-8 path. The design names it as a fixed round-1 blocker (lines 48-50, Violations row 2). The conversion lives in `main`, which no unit test reaches, because `render` takes `&[String]`. Evidence: design lines 20-21, 80-90, 102-107. In the scratch crate, `greeter --shout $'\xff'` printed `HELLO, \u{FFFD}!` and exited 0, so the behaviour is reachable and checkable. Fix: add an acceptance step such as `cargo run -q -- $'\xff'` that prints `Hello, \u{FFFD}!` and exits 0, so a regression to `env::args()` fails the check (that regression exits 101).
4. nit Confidence: "Pure function, five fixed cases" contradicts the 7-row matrix and acceptance 1 ("7 passed"). Evidence: design line 98 vs lines 84-90, 103-104. Fix: say seven.

Claims checked and found correct:
- `src/main.rs` is 11 lines, with `greeting(name) -> String` returning `Hello, {name}!`, `main` taking the first arg with default `world`, and `println!`. Evidence: src/main.rs:1-11.
- There are no tests and `[dependencies]` is empty. Evidence: src/main.rs (no `#[cfg(test)]`), Cargo.toml:6.
- Today, `env::args()` panics on a non-UTF-8 arg. Evidence: src/main.rs:8 (std `Args` panics on invalid Unicode).
- Before the change, `greeter --shout` printed `Hello, --shout!`, and `--whisper` was taken as the name. Evidence: src/main.rs:9.
- `"Hello, straße!".to_uppercase() == "HELLO, STRASSE!"`. Evidence: scratch crate test `shout_unicode` passed (`cargo test`, 1 passed).
- `args_os` + `to_string_lossy` turns invalid bytes into U+FFFD with exit 0. Evidence: scratch run output `H e l l o , \357\277\275 !` (od), status=0.
- A closed stdout (`>&-`) exits 0 with no panic, because Rust treats EBADF on stdout as success. Evidence: scratch run, status=0.
- The decision is clear and the matrix rows are deterministic and can fail. Acceptance 1-4 are runnable commands with exact expected output.
