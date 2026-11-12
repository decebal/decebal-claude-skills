# Shout flag: design

Date: 2026-11-12. Brief: `docs/plans/2026-11-12-shout-brief.md`.

## Decision
Add a `--shout` flag to `greeter`, parsed by hand in `src/main.rs`. When present anywhere in the
arguments, the greeting is upper-cased with `str::to_uppercase` before it is printed. Without it,
output is unchanged. No new dependency. While touching `main`, close two panic paths that exist
today: a non-UTF-8 argument and a closed stdout pipe.

## Context
`src/main.rs` (11 lines) has `greeting(name) -> String` returning `Hello, {name}!`, and `main`
which takes the first argument as the name (default `world`) and prints the greeting with
`println!`. There are no tests and no dependencies (`Cargo.toml` has an empty
`[dependencies]`).

## Design
- `fn render(args: &[String]) -> String`: pure. Splits `args` into the flag (`--shout`, exact
  match) and the rest; the name is the first non-flag argument, default `world`; returns
  `greeting(name)`, upper-cased when the flag was present.
- `fn emit(out: &mut impl Write, line: &str) -> i32`: writes `line` and a newline with
  `writeln!`. Returns the exit code: `0` on success, `0` on `ErrorKind::BrokenPipe` (the reader
  left; nothing to report, as `head` expects), `1` on any other write error after printing one
  line `greeter: <error>` to stderr.
- `main` collects `env::args_os().skip(1)`, converts each with `to_string_lossy` (an invalid
  UTF-8 byte becomes U+FFFD instead of a panic), calls `render`, then
  `process::exit(emit(&mut io::stdout().lock(), &line))`.
- Interface: `greeter [--shout] [NAME]`, flag before or after the name.
- An unrecognised flag such as `--whisper` is still taken as the name, as today.
- Accepted change: `greeter --shout` printed `Hello, --shout!` before this change and prints
  `HELLO, WORLD!` after it. There is no `--` terminator, so the literal name `--shout` can no
  longer be greeted.

## Alternatives
- `clap`: one flag does not justify a dependency.
- `GREETER_SHOUT=1`: invisible in shell history and help text.
- Keep `println!`: it panics with exit 101 when the pipe reader has exited (seen by the round-2
  reviewer); a script piping into `head` would see a failure that is not one.

## Security and privacy
Reads the process arguments. Writes one line to stdout, and on a non-pipe write error one line to
stderr. Stores nothing, sends nothing over a network, reads no file.

## Semantic ISA

### Side-effect inventory
| Capability | System | Reversible? | Affected if it misfires |
|---|---|---|---|
| write one line to stdout | the invoking terminal or pipe | n/a (output only) | the user who ran it, or the script reading the pipe |
| write one line to stderr, only on a non-pipe write error | the invoking terminal | n/a | the user who ran it |
| exit status (0 or 1) | the calling shell or script | n/a | a script that branches on it |

No file write, network call, API, message, deploy or data write: the change touches only
`src/main.rs`.

Before this change two panic paths existed, each printing a panic message to stderr and exiting
101: `env::args()` on a non-UTF-8 argument (round-1 reviewer) and `println!` on a broken pipe
(round-2 reviewer). This design removes both.

### Instructions
| Instruction | Core | Governance property | Sink? |
|---|---|---|---|
| `LOAD(argv)` | Memory | Deterministic I/O, total (`args_os` + `to_string_lossy` cannot fail) | no |
| `COMPUTE(render)` | Execution | Pure function, total | no |
| `VERIFY(render_shape)` | Normative | Deterministic checkpoint (unit tests) | no |
| `TOOL_CALL(stdout.write)` | Execution | Terminal action | **yes** (low consequence) |
| `VERIFY(write_result)` | Normative | Deterministic checkpoint: the `io::Result` of the write is matched, never unwrapped | no |
| `TOOL_CALL(process.exit)` | Execution | Terminal action | **yes** (low consequence) |

Bindings: `stdout.write` takes `{ line: string }`; `process.exit` takes `{ code: 0 | 1 }`; both
`additionalProperties: false`.

### Taint and sinks
Source: the `NAME` argument (user input). Sinks: `TOOL_CALL(stdout.write)`,
`TOOL_CALL(process.exit)`.

| Sink | Cleared by | Check |
|---|---|---|
| `TOOL_CALL(stdout.write)` | `VERIFY(render_shape)` | the line is exactly `Hello, <name>!`, or its `to_uppercase` when `--shout` is present; unit tests on `render` |
| `TOOL_CALL(process.exit)` | `VERIFY(write_result)` | the code comes only from `emit`'s match: 0 on Ok or BrokenPipe, 1 otherwise; unit tests on `emit` with a writer that fails |

The source and the sinks belong to the same principal (the user typing the command), so no trust
boundary is crossed. Control characters in `NAME` reach the terminal unchanged; that is the
behaviour before this change and is not altered by it.

### Violations
| Violation | Evidence | Recovery |
|---|---|---|
| output differs from the shape above | failing unit test name and diff | fix `render`; nothing was written anywhere but stdout |
| non-UTF-8 byte in an argument | the line printed contains U+FFFD | none needed: printed, exit 0, no panic |
| stdout reader has exited | `BrokenPipe` from the write | exit 0, nothing on stderr |
| other stdout write error | `greeter: <error>` on stderr | exit 1 |

### ISA test matrix
One named unit test per row, in `src/main.rs`:

| Test | Case | Expectation |
|---|---|---|
| `default_name` | `render([])` | `Hello, world!` |
| `name_unchanged` | `render(["Ada"])` | `Hello, Ada!` (unchanged default) |
| `shout_before_name` | `render(["--shout", "Ada"])` | `HELLO, ADA!` |
| `shout_after_name` | `render(["Ada", "--shout"])` | `HELLO, ADA!` |
| `shout_unicode` | `render(["--shout", "straße"])` | `HELLO, STRASSE!` (Unicode upper-casing) |
| `shout_alone` | `render(["--shout"])` | `HELLO, WORLD!` |
| `unknown_flag_is_name` | `render(["--whisper"])` | `Hello, --whisper!` |
| `emit_ok` | `emit(Vec<u8>, "x")` | returns 0, buffer is `x\n` |
| `emit_broken_pipe` | writer failing with `BrokenPipe` | returns 0 |
| `emit_other_error` | writer failing with `Other` | returns 1 |

## Testing
Unit tests cover the matrix above (testing trophy: unit layer; `main` is three lines around two
tested functions). `cargo test` runs them. The end-to-end checks are acceptance 2 to 5.

## Confidence
- `render`, `emit` and the flag: `rock`. Two small functions, ten fixed cases.
- Unicode upper-casing: `rock`. `str::to_uppercase` applies full case mapping (`straße` →
  `STRASSE`, observed by the round-1 reviewer).

## Acceptance
1. `cargo test -- --list` lists the 10 named tests of the matrix, and `cargo test` reports
   `10 passed; 0 failed`.
2. `cargo run -q -- Ada` prints `Hello, Ada!` and exits 0.
3. `cargo run -q -- --shout Ada` and `cargo run -q -- Ada --shout` print `HELLO, ADA!` and exit 0.
4. `cargo run -q -- --shout $'\xff'` prints `HELLO, ` + U+FFFD + `!` and exits 0 (a regression to
   `env::args()` exits 101).
5. `cargo build -q` then `target/debug/greeter --shout Ada | true`: `PIPESTATUS[0]` is 0 and
   stderr is empty.
6. `Cargo.toml` `[dependencies]` is still empty.

## Review history
- Round 1: `docs/plans/2026-11-12-shout-design-review-r1.md`, verdict `findings` (1 blocking:
  non-UTF-8 panic missing from the inventory; 1 should: untestable acceptance 1; 1 nit: `--shout`
  alone). Resolved.
- Round 2: `docs/plans/2026-11-12-shout-design-review-r2.md`, verdict `findings` (1 blocking:
  broken-pipe panic, "exit always 0" was false; 2 should: exit status not a sink, non-UTF-8 fix
  not checked end to end; 1 nit: test count). Resolved.
- Round 3: `docs/plans/2026-11-12-shout-design-review.md` (fresh-context subagent).
