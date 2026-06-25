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
│   ├── ralph-tui-create-beads-rust/  # PRD → beads conversion
│   ├── ralph-tui-prd/          # PRD generation for agent execution
│   ├── skill-creator/          # How to author skills
│   ├── typescript/             # TS optimization (42 rules)
│   └── web-video/              # Screen recording → web-ready H.264 demo (+ poster/GIF) via ffmpeg (bundled script)
├── configs/                    # Reference configurations
│   ├── settings.json           # Global settings reference
│   ├── settings.local.json     # Project permission patterns
│   └── plugins.md              # Plugin & LSP setup
├── templates/                  # CLAUDE.md templates
│   ├── monorepo.md             # For monorepo projects
│   ├── fullstack-app.md        # For fullstack applications
│   ├── minimal.md              # Minimal starter
│   └── tauri-desktop.md        # For Tauri desktop apps
├── skills-guide/               # Skills authoring & usage
│   ├── overview.md             # Installed skills inventory
│   ├── creating-skills.md      # How to create custom skills
│   └── ralph-tui-workflow.md   # PRD → Beads → Execution pipeline
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
| **Getting Started** | [configs/settings.json](configs/settings.json) | Global settings with extended thinking, LSP, status line |
| **Skills (full source)** | [skills/](skills/) | 11 skills with SKILL.md + all reference files |
| **CLAUDE.md Templates** | [templates/](templates/) | Battle-tested templates for different project types |
| **Skills Guide** | [skills-guide/overview.md](skills-guide/overview.md) | Skills inventory, authoring guide, Ralph TUI workflow |
| **MCP Servers** | [docs/mcp-servers.md](docs/mcp-servers.md) | GitHub, Linear, Slack, Supabase, Firebase, and more |
| **Permissions** | [docs/permissions.md](docs/permissions.md) | Granular command allowlists by project type |
| **Token Efficiency** | [docs/token-efficiency.md](docs/token-efficiency.md) | Patterns for staying within context limits |

## Executable skills & tests

Some skills bundle runnable scripts (not just instructions):

- **`blog-image`** — `optimize_image.py` (screenshot → hero/OG WebP) and `og_card.py`
  (title → branded 1200×630 card). Pure-Python (Pillow); runs in the Claude Desktop
  sandbox. See `skills/blog-image/examples/sample-og.webp`.
- **`web-video`** — `optimize_video.sh` (screen recording → web-ready H.264 + poster + GIF).
  Needs `ffmpeg`.

Run their tests (dep-aware — each self-skips when its deps are missing):

```sh
bash tests/run.sh                  # all skill tests
bash tests/test_blog_image.sh      # needs Pillow (or uv) — checks preset dims + WebP output
bash tests/test_web_video.sh       # needs ffmpeg — checks H.264, faststart, audio strip, poster/gif
```

CI runs them on every push to `skills/**` or `tests/**` via
[`.github/workflows/test-skills.yml`](.github/workflows/test-skills.yml)
(installs `ffmpeg` + `uv`).

### Install a skill globally (Claude Code)

```sh
cp -R skills/blog-image ~/.claude/skills/blog-image
cp -R skills/web-video  ~/.claude/skills/web-video
```

For Claude Desktop: upload the skill folder (SKILL.md + scripts) via Settings → Skills.

## Key Principles

1. **Bun-first** — Use Bun as the default JS/TS runtime; never mix package managers
2. **Taskfile for automation** — Prefer Taskfile over raw scripts for project commands
3. **Granular permissions** — Allowlist specific commands per project in `settings.local.json`
4. **CLAUDE.md as source of truth** — Every project gets a CLAUDE.md with stack, commands, and conventions
5. **Skills for repeatable workflows** — Extract common patterns into reusable skills
6. **Extended thinking always on** — Better reasoning for complex tasks
7. **Ralph TUI for orchestration** — PRD → Beads → parallel agent execution for larger features
