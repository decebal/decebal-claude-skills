# Claude Best Practices Repository

This repository contains documentation plus bundled Rust gates and skill runtimes.

## Purpose
Reference configs, templates, and guides for Claude Code usage across projects.

## Conventions
- All file paths in examples should use `~/.claude/` for global and `.claude/` for project-local
- Templates should be practical and copy-paste ready
- Keep docs concise — link to official docs rather than duplicating
- Use real examples from actual project usage, anonymized where needed
- Prefer Rust for durable scripts and tooling; keep independent skill crates under each skill's `scripts/`
- Commit Cargo.lock for executable skills

## Required checks for Rust skill runtimes

```sh
cargo fmt --check --manifest-path skills/claude-seo/scripts/Cargo.toml
cargo clippy --manifest-path skills/claude-seo/scripts/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path skills/claude-seo/scripts/Cargo.toml --all-targets --all-features
cargo fmt --check --manifest-path skills/aso-lint/scripts/Cargo.toml
cargo clippy --manifest-path skills/aso-lint/scripts/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path skills/aso-lint/scripts/Cargo.toml --all-targets --all-features
```
