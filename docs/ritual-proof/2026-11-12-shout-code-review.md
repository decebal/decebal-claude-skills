---
design: docs/plans/2026-11-12-shout-design.md
diff: master...feat/shout
verdict: clean
---

## Sinks
| Sink | VERIFY at | Test |
|---|---|---|
| `TOOL_CALL(stdout.write)`: `src/main.rs:22` (`writeln!` in `emit`), reached only from `src/main.rs:39` | `VERIFY(render_shape)`: `render`, `src/main.rs:9-18` (line built only by `greeting` at `src/main.rs:5`, upper-cased only when `--shout` present) | `default_name` :81, `name_unchanged` :86, `shout_before_name` :91, `shout_after_name` :96, `shout_unicode` :101, `shout_alone` :106, `unknown_flag_is_name` :111; `emit_ok` :59 checks bytes written are `x\n` |
| `TOOL_CALL(process.exit)`: `src/main.rs:39` | `VERIFY(write_result)`: match at `src/main.rs:22-30`; code is the return of `emit`, no other `exit` call | `emit_ok` :59 (0), `emit_broken_pipe` :66 (0), `emit_other_error` :71 (1) |

`rg -n "process::exit|writeln!|println!|eprintln!|print!|stdout|stderr|env::args" src` finds no other sink call site: no `println!`, one `process::exit`, the stdout write only inside `emit`. The stderr write at `src/main.rs:27` is in the design's side-effect inventory (row 2) and carries the `io::Error` text, not `NAME`, so it is not a taint sink and not drift.

## Findings
1. nit `src/main.rs:22`: `emit` also calls `out.flush()`; the design says `writeln!` only. Evidence: diff line `match writeln!(out, "{line}").and_then(|()| out.flush())`. The flush keeps a write error inside the match before `process::exit`, which skips the `Stdout` drop, so it serves the design's intent. Fix: add "then flush" to the `emit` bullet of the design so design and code agree.
2. nit `src/main.rs:27`: the `greeter: <error>` stderr line has no test. `emit_other_error` asserts only the return code, and the line leaks into test output (`greeter: other error` printed during `cargo test`). Fix: optional. Pass the stderr writer into `emit` as a second `impl Write` and assert its contents. Not required by the design's matrix.

No blocking or should-fix findings.

## Ran
- `git -C <scratch>/greeter-shout diff master...feat/shout -- src Cargo.toml Cargo.lock`: only `src/main.rs` changed; no `Cargo.toml`/`Cargo.lock` change.
- `git -C ... diff --stat master...feat/shout`: `src/main.rs` plus `docs/` files only.
- `rg -n -A3 dependencies Cargo.toml`: `[dependencies]` at line 6, empty (acceptance 6).
- `timeout -k 15 300 cargo test --manifest-path .../Cargo.toml --target-dir /tmp/review-target -- --list`: 10 tests, names match the design matrix exactly (acceptance 1).
- `timeout -k 15 300 cargo test ... --target-dir /tmp/review-target`: `10 passed; 0 failed; 0 ignored; 0 filtered out` (acceptance 1).
- `timeout -k 15 300 cargo build -q ... --target-dir /tmp/review-target`: ok.
- `bash /tmp/review-scratch/accept.sh` against `/tmp/review-target/debug/greeter`:
  - no args: `Hello, world!` exit 0
  - `Ada`: `Hello, Ada!` exit 0 (acceptance 2)
  - `--shout Ada` and `Ada --shout`: `HELLO, ADA!` exit 0 (acceptance 3)
  - `--shout`: `HELLO, WORLD!` exit 0; `--whisper`: `Hello, --whisper!` exit 0; `--shout straße`: `HELLO, STRASSE!` exit 0
  - `--shout $'\xff' | od -c`: `H E L L O ,   <EF BF BD> !  \n`, exit 0 (U+FFFD, acceptance 4)
  - `--shout Ada 2>err | true`: `PIPESTATUS[0]`=0, stderr 0 bytes (acceptance 5)
  - reader closed before write: exit 0, stderr 0 bytes
  - stdout closed (`>&-`): exit 0, stderr empty. Rust std treats `EBADF` on stdout as success, so this does not reach the error arm. The exit-1 arm is covered only by `emit_other_error`; `/dev/full` is not available on macOS, so it was not exercised end to end.

Tests can fail: each asserts a concrete string or code. An `emit` returning a constant 0 fails `emit_other_error`; ignoring the flag fails the four `shout_*` tests; dropping the flush filter on the name fails `shout_before_name` and `shout_alone`. Inferred from the assertions; no mutation run was done (repo files are read-only for this review).

Security: argv is read with `args_os` + `to_string_lossy` (no panic path); the name reaches stdout unescaped, as before the change and as the design states. Nothing reads files, the network or the environment.
