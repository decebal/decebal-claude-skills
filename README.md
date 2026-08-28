# Claude Code Best Practices & Tools

A reference repository documenting best practices, templates, and tooling for Claude Code — extracted from real-world usage across multiple projects.

## Overview

This repo captures patterns and configurations used across 25+ projects spanning Rust, TypeScript, Go, Elixir, Svelte, and Next.js stacks.

## Structure

```
.
├── skills/                     # Complete local skill collection (SKILL.md + references)
│   ├── blog-image/             # Screenshot → web-optimized hero/OG WebP + branded OG card (Pillow, bundled scripts)
│   ├── brainstorming/          # Creative design exploration
│   ├── bun-development/        # Bun runtime guide
│   ├── create-plans/           # Hierarchical project plans (/create-prompt, /run-prompt)
│   ├── dev-browser/            # Browser automation (Playwright)
│   ├── docker-expert/          # Docker optimization & security
│   ├── feature-spec/           # PRDs, requirements, scope (42 rules)
│   ├── find-skills/            # Skill discovery & install
│   ├── mcp-builder/            # MCP server creation guide
│   ├── claude-beads/           # PRD → beads conversion (auto-detects cn/br/bd)
│   ├── claude-prd/             # PRD generation for agent execution
│   ├── rust-clean-architecture/ # Rust layering enforced as a test (domain ← infra)
│   ├── rust-quality/           # Workspace clippy/unsafe/rustdoc lint gates + cargo sort
│   ├── integration-test/       # Audit coverage; write un-hangable cross-boundary tests
│   ├── arch/                   # C4 architecture diagrams in MermaidJS
│   ├── systematic-debugging/   # Structured diagnose-before-fix workflow (+ 4 refs)
│   ├── bun-testing/            # bun:test — mocking, snapshots, coverage, integration
│   ├── monorepo-expert/        # Turborepo + Bun workspaces: pipeline, cache, filters
│   ├── bundle-analysis/        # Bundle size: snapshot/diff/budget, tree-shaking
│   ├── security-review/        # CSP/crypto/injection review checklist (+ 4 refs)
│   ├── elixir-expert/          # Phoenix/OTP: supervision, Rustler NIFs, Ecto (+ 2 refs)
│   ├── fastify-expert/         # Fastify APIs: plugins, hooks, JWT, inject() tests (+ 2 refs)
│   ├── gcp-cloudrun/           # Cloud Run deploy/rollback, Terraform, cold-start (+ 2 refs)
│   ├── explain-module/         # Structured module explanation: file:line + Mermaid (+ 2 refs)
│   ├── perf-review/            # App-level hot-path performance review (+ 4 refs)
│   ├── wasm-development/       # WASM: Emscripten + wasm-pack, JS boundary, opt (+ 5 refs)
│   ├── protobuf-grpc/          # protobuf-es/buf, envelope encryption, compat (+ 5 refs)
│   ├── claude-seo/             # Evidence-led SEO + safe Rust audit/sitemap/drift CLI
│   ├── aso-lint/               # App Store/Google Play optimization + Rust linter
│   ├── skill-creator/          # How to author skills
│   ├── typescript/             # TS optimization (42 rules)
│   └── web-video/              # Screen recording → web-ready H.264 demo (+ poster/GIF) via ffmpeg (bundled script)
├── rules/                      # Portable rule fragments — @-import into any CLAUDE.md
│   ├── git-discipline.md       # Dead-branch guard, squash merges, branch/push rules
│   ├── evidence-discipline.md  # Verify the artifact, not the exit code; read state, never guess
│   ├── agent-parallelism.md    # Split by file count; one worktree per agent; never two builds
│   ├── timeouts.md             # The 5-minute ceiling on every gate
│   ├── definition-of-done.md   # End-to-end or not done; size is never a signal
│   ├── testing-gates.md        # What actually enforces anything; un-hangable tests
│   ├── layer-boundaries.md     # 4-layer direction as a test; opening a god module
│   └── …                       # 15 rules total — see rules/README.md
├── gates/                      # The tooling the rules reference
│   ├── rust/                   # 20 crates, 4 deps, 265 tests — one cargo workspace
│   │   ├── claude-guard/       # the guard-rail hooks: infra, bash, comments
│   │   ├── staged-scope/       # which gates does this diff need? default-deny
│   │   ├── layer-boundary-check/    # the dependency direction, with ratcheting ceilings
│   │   ├── authority-check/         # one file owns a verdict; the rest render it
│   │   ├── forbidden-pattern-check/ # named patterns, named paths, one allowlist
│   │   ├── test-script-check/       # a package whose suites run in no gate at all
│   │   ├── contract-set-drift/      # two contract halves must declare the same SETS
│   │   ├── trophy-check/            # every promised test must be a real test
│   │   ├── check-test-hangs/   # no unbounded blocking I/O in tests
│   │   ├── fmt-check/          # whole-repo rustfmt WITHOUT invoking cargo
│   │   ├── render-agent-docs/  # one manifest → CLAUDE.md + AGENTS.md, --check
│   │   └── …                   # vendor-attribution, workspace-isolation, price-table,
│   │                           # dev-preflight, rust-effective-diff, target-sweep, graph-audit
│   ├── sh/                     # git-hook glue: run_gate, dead-branch guard, worktrees
│   ├── ts/                     # RemoteState<T> + its backstop gate, git keepalive
│   ├── gates.toml              # scopes, inert paths, test-hang tiers
│   └── examples/pre-push       # a worked git hook wiring all of it together
├── hooks/README.md             # Guard-rail hooks: install, patterns, token cost
│                               # (the code is gates/rust/claude-guard)
├── configs/                    # Reference configurations
│   ├── settings.json           # Global settings reference
│   ├── settings.local.json     # Project permission patterns
│   ├── prod-guard-tokens.txt   # infra-guard prod/non-prod identifier template
│   └── plugins.md              # Plugin & LSP setup
├── templates/                  # CLAUDE.md templates
│   ├── monorepo.md             # For monorepo projects
│   ├── fullstack-app.md        # For fullstack applications
│   ├── minimal.md              # Minimal starter
│   └── tauri-desktop.md        # For Tauri desktop apps
├── skills-guide/               # Skills authoring & usage
│   ├── overview.md             # Installed skills inventory
│   ├── creating-skills.md      # How to create custom skills
│   └── claude-workflow.md      # PRD → Beads → Execution pipeline
├── docs/                       # Deep dives
│   ├── mcp-servers.md          # MCP server integrations
│   ├── hooks.md                # Hooks configuration
│   ├── permissions.md          # Permission patterns & security
│   └── token-efficiency.md     # Context window optimization
└── CLAUDE.md                   # This project's own config
```

## Quick Links

| Topic | File | Summary |
|-------|------|---------|
| **Portable Rules** | [rules/README.md](rules/README.md) | 15 stack-agnostic rule fragments, each with the incident that produced it |
| **Gates** | [gates/README.md](gates/README.md) | The tooling the rules reference — hooks, scope classifier, drift check |
| **Getting Started** | [configs/settings.json](configs/settings.json) | Global settings with extended thinking, LSP, status line |
| **Skills (full source)** | [skills/](skills/) | 54 skills with SKILL.md + all reference files |
| **CLAUDE.md Templates** | [templates/](templates/) | Battle-tested templates for different project types |
| **Skills Guide** | [skills-guide/overview.md](skills-guide/overview.md) | Skills inventory, authoring guide, PRD → beads pipeline |
| **MCP Servers** | [docs/mcp-servers.md](docs/mcp-servers.md) | GitHub, Linear, Slack, Supabase, Firebase, and more |
| **Permissions** | [docs/permissions.md](docs/permissions.md) | Granular command allowlists by project type |
| **Token Efficiency** | [docs/token-efficiency.md](docs/token-efficiency.md) | Patterns for staying within context limits |
| **Guard-rail hooks** | [hooks/README.md](hooks/README.md) | Blast-radius guard, Bash hygiene, comment hygiene — install steps + pattern list |

## Executable skills & tests

Some skills bundle runnable scripts (not just instructions):

- **`blog-image`** — `optimize_image.py` (screenshot → hero/OG WebP) and `og_card.py`
  (title → branded 1200×630 card). Pure-Python (Pillow); runs in the Claude Desktop
  sandbox. See `skills/blog-image/examples/sample-og.webp`.
- **`web-video`** — `optimize_video.sh` (screen recording → web-ready H.264 + poster + GIF).
  Needs `ffmpeg`.
- **`claude-seo`** — Rust `claude-seo` CLI for bounded static HTML audits, sitemap
  inspection, high-risk field drift, and capability diagnosis. Public fetches reject
  private/local targets, revalidate redirects, pin validated DNS, and cap time/bytes.
- **`aso-lint`** — Rust `aso-lint` CLI for Apple/Google metadata limits, starter
  templates, stable rule IDs, and two-proportion experiment snapshots.

The guard-rail hooks are runnable too — three PreToolUse/PostToolUse guards in one
binary. See [hooks/README.md](hooks/README.md).

Run the skill tests (dep-aware — each self-skips when its deps are missing):

```sh
bash tests/run.sh                  # everything
bash tests/test_blog_image.sh      # needs Pillow (or uv) — checks preset dims + WebP output
bash tests/test_web_video.sh       # needs ffmpeg — checks H.264, faststart, audio strip, poster/gif
```

The guard-rail hooks are Rust, and their tests come with the workspace — no
external dependency, nothing to skip:

```sh
cargo test --manifest-path gates/rust/Cargo.toml -p claude-guard   # 63 tests
cargo test --manifest-path gates/rust/Cargo.toml                   # 265, every gate
cargo test --manifest-path skills/claude-seo/scripts/Cargo.toml   # unit + CLI fixtures
cargo test --manifest-path skills/aso-lint/scripts/Cargo.toml     # unit + CLI fixtures
```

CI runs them on every push to `skills/**` or `tests/**` via
[`.github/workflows/test-skills.yml`](.github/workflows/test-skills.yml)
(installs `ffmpeg`, `uv`, and stable Rust with rustfmt/Clippy).

### Install a skill globally (Claude Code)

```sh
cp -R skills/blog-image ~/.claude/skills/blog-image
cp -R skills/web-video  ~/.claude/skills/web-video
cp -R skills/claude-seo ~/.claude/skills/claude-seo
cp -R skills/aso-lint   ~/.claude/skills/aso-lint
```

For Claude Desktop: upload the skill folder (SKILL.md + scripts) via Settings → Skills.

## SEO suite

Two skills share one evidence contract:

| Skill | Use it for | Deterministic Rust evidence | Honest boundary |
|---|---|---|---|
| [`claude-seo`](skills/claude-seo/) | Technical/on-page audits, schema, sitemaps, hreflang, drift, content plans, GEO | Static HTML signals, public response status, sitemap XML, audit diffs | Search/analytics/backlink/maps/render data stays `setup required` until observed |
| [`aso-lint`](skills/aso-lint/) | Apple App Store and Google Play listings, locales, visuals, experiments | Metadata budgets, keyword bytes, templates, experiment math | Store approval, rank, demand, downloads, and lift are never inferred |

Both ship locked Cargo workspaces, strict Clippy/rustdoc policy, fixture-driven
CLI tests, upstream MIT attribution, and progressive skill references. See
[`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for provenance.

## Key Principles

1. **Bun-first** — Use Bun as the default JS/TS runtime; never mix package managers
2. **Taskfile for automation** — Prefer Taskfile over raw scripts for project commands
3. **Granular permissions** — Allowlist specific commands per project in `settings.local.json`
4. **CLAUDE.md as source of truth** — Every project gets a CLAUDE.md with stack, commands, and conventions
5. **Skills for repeatable workflows** — Extract common patterns into reusable skills
6. **Extended thinking always on** — Better reasoning for complex tasks
7. **No separate orchestrator** — PRD → beads → Claude works the ready ones itself; parallelism is one worktree per agent
8. **Rules by reference, never by copy** — `@`-import from `~/.claude/rules/`; pasted rules drift
9. **Every rule carries its incident** — a bare prohibition gets rationalized away; a cost does not
10. **A rule without a gate is advice** — where no CI check can be required, the pre-push hook is the only enforcement there is
