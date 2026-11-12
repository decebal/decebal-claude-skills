# Shout flag: brief

Date: 2026-11-12.

## Problem
`greeter` prints `Hello, <name>!` in the case the user typed. A user who wants the greeting in
capitals (a banner, a terminal title) has to pipe it through `tr`, which is one more tool and
does not handle non-ASCII letters the same way on every platform.

## Goal
`greeter --shout Ada` prints `HELLO, ADA!`. Without the flag, output is byte-for-byte what it is
today. The flag may come before or after the name.

## Approaches
1. **A `--shout` flag, parsed by hand in `main.rs` (chosen).** The CLI has one positional
   argument and no dependencies; one flag does not justify a parser crate.
2. Add `clap`. Lost: one flag does not pay for a dependency and its compile time.
3. An environment variable `GREETER_SHOUT=1`. Lost: invisible in shell history and in `--help`.

## Constraints
- No new dependency.
- Default output unchanged (scripts may parse it).
- Upper-casing uses Rust's Unicode-aware `str::to_uppercase`.

## Unknowns
- Unknown flags (`--whisper`): today they are taken as the name. `rock`: keep that behaviour,
  out of scope; a parser change would move it.
