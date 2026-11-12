---
design: docs/plans/2026-11-12-shout-design.md
design-sha256: 1e14b1c7da47892e3276a58b0c9fff8f5909abff7cb3688941385480a48e2d15
verdict: clean
---

1. nit Semantic ISA / Violations: a closed stdout (`greeter --shout Ada >&-`) writes nothing and exits 0, because Rust's `Stdout` treats `EBADF` as a successful write; the violations table does not list this case, and the first inventory row ("write one line to stdout") does not hold for it. Evidence: prototype of the design in /tmp/goldfish-scratch3 printed `closed-stdout exit=0` and `closed-both exit=0`. Fix: add a violations row "stdout closed (EBADF): nothing written, exit 0, std swallows it", or state it as accepted behaviour.
2. nit Design / `emit`: "printing one line `greeter: <error>` to stderr" does not name the call. `eprintln!` panics (exit 101) if the stderr write fails with anything other than EBADF, for example stderr is a pipe whose reader has exited while stdout hits a non-pipe error. That would break the "exit status (0 or 1)" inventory row. The case needs two failures at once, so it is rare. Evidence: design line 21-24 and inventory line 51; std `eprintln!` panics on a write error (from std docs, not reproduced here: macOS has no `/dev/full` to force the stdout error). Fix: specify `let _ = writeln!(io::stderr(), "greeter: {e}");`.
3. nit Design / Interface: arguments after the first non-flag one (`greeter Ada Bob`) are dropped silently, as today, and the design does not say so. Evidence: src/main.rs:9 takes `args.first()` only; design line 18-20 says "first non-flag argument". Fix: one line under Interface: "extra positional arguments are ignored, as today".

Claims checked:
- `src/main.rs` is 11 lines, `greeting(name) -> String` returns `Hello, {name}!`, `main` takes the first argument with default `world` and prints with `println!`: observed src/main.rs:1-11.
- No tests, empty `[dependencies]`: observed src/main.rs (no `#[cfg(test)]`), Cargo.toml:6-7.
- Non-UTF-8 argument panics today with exit 101: observed, `cargo run -q -- $'\xff'` on the current code printed `called Result::unwrap() on an Err value: "\xFF"`, `exit=101`. Cargo passes the non-UTF-8 argument through, so acceptance 4 is runnable and fails on a regression.
- `println!` on a closed pipe fails today: observed, current binary `greeter Ada | true` 200 runs: pass=0 fail=200 under acceptance 5's criteria (exit 0 and empty stderr). Acceptance 5 can fail.
- The designed code passes acceptance 5: observed, prototype `greeter --shout Ada | true` 200 runs: pass=200 fail=0.
- `args_os` + `to_string_lossy` yields U+FFFD and exit 0: observed, prototype output `HELLO, ` + `EF BF BD` + `!\n`, `ff exit=0`.
- `str::to_uppercase("straße")` gives `STRASSE`, `--shout` alone gives `HELLO, WORLD!`, and `emit` returns 0/0/1 for Ok/BrokenPipe/Other with buffer `x\n`: observed, prototype `cargo test` 5 of the matrix tests, `5 passed; 0 failed`.
- Side-effect inventory (stdout line, stderr line on non-pipe error, exit 0/1) matches the designed code. No file, network or env access: inferred from the design's `main` (lines 25-27) and the prototype. Besides nit 1 and nit 2, no missed side effect found.
- Every sink has a deterministic VERIFY: `stdout.write` is covered by `render` unit tests, `process.exit` by `emit` unit tests with a failing writer. Observed design lines 77-80, and the prototype tests ran.
- Acceptance 1 (10 named tests, `10 passed`) and 6 (empty `[dependencies]`) are runnable and can fail. Inferred from the commands in the design. Not run against the final code, which does not exist yet.
- Not checked: the r1/r2 review files (instructed not to read them).
