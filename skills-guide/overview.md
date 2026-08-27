# Skills Overview

Skills extend Claude Code with domain-specific knowledge and workflows. They live in `~/.claude/skills/` (or symlinked from `~/.agents/skills/`).

## Installed Skills

### Development & Automation

| Skill | Purpose | Trigger Phrases |
|-------|---------|-----------------|
| **dev-browser** | Browser automation with Playwright | "go to url", "take screenshot", "fill form" |
| **typescript** | TS performance, tsconfig, type errors | "optimize typescript", TS error codes |
| **bun-development** | Bun runtime, migration from Node | Working with Bun projects |
| **docker-expert** | Docker builds, security, orchestration | Dockerfile work, container issues |
| **mcp-builder** | Create MCP servers for external APIs | "build mcp server" |

### Planning & Design

| Skill | Purpose | Trigger Phrases |
|-------|---------|-----------------|
| **brainstorming** | Explore ideas before implementation | Any creative/feature work (auto-triggers) |
| **feature-spec** | PRDs, requirements, scope management | "write prd", "define requirements" |
| **create-plans** | Hierarchical Claude-executable project plans; ships `/create-prompt` + `/run-prompt` | "plan this project", "create-prompt", "phase plan" |
| **claude-prd** | PRDs optimized for agent execution | "create a prd", "plan this feature" |

### Task Orchestration

| Skill | Purpose | Trigger Phrases |
|-------|---------|-----------------|
| **claude-beads** | Convert PRDs to executable beads; auto-detects the tracker CLI (cn/br/bd) | "create beads", "convert prd to beads" |

### Meta

| Skill | Purpose | Trigger Phrases |
|-------|---------|-----------------|
| **find-skills** | Discover & install new skills | "find a skill for X", "how do I do X" |
| **skill-creator** | Guide for authoring new skills | "create a new skill" |

## Skill Locations

```
~/.claude/skills/
├── typescript/                    # Direct install
├── bun-development/               # Direct install
├── docker-expert/                 # Direct install
├── skill-creator/                 # Direct install
├── brainstorming -> ~/.agents/skills/brainstorming
├── dev-browser -> ~/.agents/skills/dev-browser
├── feature-spec -> ~/.agents/skills/feature-spec
├── find-skills -> ~/.agents/skills/find-skills
├── mcp-builder -> ~/.agents/skills/mcp-builder
├── claude-beads -> ~/.agents/skills/claude-beads
└── claude-prd -> ~/.agents/skills/claude-prd
```

## Installing Skills

```bash
# From the skills marketplace
npx skills find <keyword>
npx skills add <skill-name> -g -y

# Manual install (symlink from custom location)
ln -s /path/to/skill ~/.claude/skills/skill-name
```
