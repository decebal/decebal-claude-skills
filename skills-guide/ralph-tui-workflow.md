# Ralph TUI Workflow: PRD → Beads → Execution

Ralph TUI orchestrates parallel Claude agents for larger features. The workflow has three stages.

## Pipeline Overview

```
1. PRD Generation     →  2. Beads Creation     →  3. Parallel Execution
   (ralph-tui-prd)        (ralph-tui-create-       (ralph-tui run
                           beads-rust)               --tracker chronis)
```

## Stage 1: PRD Generation

Invoke with: "create a prd for [feature]"

The skill asks iterative clarifying questions with lettered options, then generates:

```
./tasks/prd-[feature-name].md
```

**PRD structure:**
- Overview & Goals
- Quality Gates (epic-level vs story-level)
- User Stories (tagged: `[Schema]`, `[Backend]`, `[UI]`, `[Integration]`)
- Functional Requirements
- Non-Goals
- Technical Considerations
- Success Metrics

**Key pattern:** Two-tier quality gates
- **Epic-level** — Run once at end (e.g., full E2E test suite)
- **Story-level** — Run per story (e.g., unit tests, type checks)

## Stage 2: Beads Creation

Invoke with: "create beads from [prd file]"

Converts each user story into a bead (task) using the `cn` CLI:

```bash
# Create epic
cn create --title "Epic: Feature Name" --body "..." --label epic

# Create child stories
cn create --title "Story 1: ..." --body "..." --parent <epic-id> --label story

# Add dependencies between stories
cn dep add <story-id> <dependency-id>
```

**Output:** Beads in `.beads/` directory with:
- Acceptance criteria as verifiable checkboxes
- Story-specific quality gate commands
- Dependencies between stories
- Project-detected tooling (Taskfile, Bun, etc.)

## Stage 3: Execution

```bash
ralph-tui run --tracker chronis
```

Ralph TUI:
1. Reads beads and their dependencies
2. Launches parallel Claude agents for independent stories
3. Runs story-level quality gates after each completion
4. Runs epic-level quality gates when all stories pass

## Story Sizing

Each story should fit within a single agent context window. Rules:
- One concern per story
- Clear file scope (which files to create/modify)
- Self-contained acceptance criteria
- Verifiable with automated checks

## Example

```bash
# 1. Generate PRD
claude "create a prd for user authentication with OAuth"

# 2. Create beads from PRD
claude "create beads from ./tasks/prd-user-auth.md"

# 3. Execute with ralph-tui
ralph-tui run --tracker chronis
```
