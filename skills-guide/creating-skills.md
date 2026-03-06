# Creating Custom Skills

## Skill Structure

A skill is a directory with at minimum a `SKILL.md` file:

```
my-skill/
├── SKILL.md           # Required: instructions and knowledge
├── references/        # Optional: additional context files
│   ├── guide.md
│   └── examples.md
└── package.json       # Optional: if skill needs dependencies
```

## SKILL.md Format

```markdown
---
name: my-skill
description: One-line description of what this skill does
trigger: keyword1, keyword2, phrase to match
---

# Skill Title

## When to Use
{Describe when this skill activates}

## Instructions
{Step-by-step behavior for Claude}

## Rules
{Constraints and guidelines}
```

## Key Principles

1. **Specific triggers** — Define clear trigger phrases so the skill activates at the right time
2. **Actionable instructions** — Tell Claude exactly what to do, not just what to know
3. **Reference files for depth** — Keep SKILL.md concise; put detailed guides in `references/`
4. **Verifiable outputs** — Include acceptance criteria or checkboxes for validation
5. **Tool awareness** — Reference specific tools (Bash, Read, Write) if the skill needs them

## Examples from Installed Skills

### Questionnaire Pattern (ralph-tui-prd)
Ask lettered clarifying questions (A, B, C, D) before generating output. Adapt follow-up questions based on answers.

### Multi-phase Workflow (mcp-builder)
1. Research & Planning
2. Implementation
3. Review & Test
4. Evaluation with scoring

### Rule-based Knowledge (feature-spec, typescript)
Organize knowledge as numbered rules with category prefixes (e.g., `scope-01`, `req-03`). Allows precise referencing.

### Auto-trigger (brainstorming)
Mark as "MUST use before any creative work" to auto-activate without explicit invocation.

## Installation

```bash
# Symlink into Claude's skills directory
ln -s /path/to/my-skill ~/.claude/skills/my-skill

# Or use the skills CLI
npx skills add my-skill -g -y
```

## Testing

1. Start a new Claude session
2. Use one of the trigger phrases
3. Verify the skill activates and follows its instructions
4. Check that outputs match expected format
