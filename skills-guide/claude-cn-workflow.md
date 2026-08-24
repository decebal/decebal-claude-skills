# PRD → Beads → Execution (chronis)

Three stages for a feature too big to hold in one context: write it down, split
it into tracked units, then work them.

There is no separate orchestrator. Claude reads the ready beads itself, and
parallelism is one worktree per agent — see
[`rules/agent-parallelism.md`](../rules/agent-parallelism.md), which also covers
when splitting is worth it and when it costs more in merge than it saves.

## Pipeline Overview

```
1. PRD Generation   →   2. Beads Creation   →   3. Execution
   (claude-cn-prd)        (claude-cn-beads)      (claude, reading `cn ready`)
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

**Output:** Beads in `.chronis/` with:
- Acceptance criteria as verifiable checkboxes
- Story-specific quality gate commands
- Dependencies between stories
- Project-detected tooling (Taskfile, Bun, etc.)

## Stage 3: Execution

```bash
claude "work the ready beads: cn ready --toon, claim each, implement, run its quality gates, cn done"
```

Claude then:
1. Reads the ready beads and their dependencies (`cn ready --toon` shows only
   unblocked, unclaimed work)
2. Claims one before starting it — the claim is what stops two sessions doing
   the same bead
3. Runs that story's quality gates before `cn done`
4. Runs the epic-level gates when every child bead is closed

Independent stories can go to concurrent agents, one worktree each. Split by
disjoint file sets, never by concept, and never run two builds at once —
[`rules/agent-parallelism.md`](../rules/agent-parallelism.md) has the measurements.

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

# 3. Work them
claude "work the ready beads: cn ready --toon, claim each, implement, run its quality gates, cn done"
```
