---
description: Create a new prompt that another Claude can execute
argument-hint: [task description]
allowed-tools: [Read, Write, Glob, Skill, SlashCommand, AskUserQuestion, Bash(~/.claude/bin/prompt-id:*), Bash(rustc:*), Bash(git rev-parse:*), Bash(ls:*), mcp__prime__prime_search, mcp__prime__prime_recall, mcp__prime__prime_add_node, mcp__prime__prime_add_edge, mcp__prime__prime_embed, mcp__prime__prime_neighbors]
---

<context>
**Never number a prompt by listing the directory you are standing in.** `.prompts/`
is gitignored in most repos, so every `git worktree` holds its OWN copy: a session
in a worktree sees a near-empty directory, numbers from that, and lands on top of a
number the main checkout already used. That is not hypothetical — it produced 53
duplicated numbers across 246 files in one repo, plus a 300-number hole where a
session escaped the collisions by hand.

Allocate the number through `prompt-id`, this skill's own tool. It resolves the ONE
store for the repo (the main checkout's `.prompts/`, via
`git rev-parse --git-common-dir`) and CLAIMS the number atomically, so two sessions
racing from different worktrees cannot be handed the same one. It lives with the
skill, not in any repo, so it works the same everywhere:

```bash
# Build once (seconds, no cargo, no manifest, zero dependencies):
[ -x ~/.claude/bin/prompt-id ] || rustc --edition 2021 -O ~/.claude/tools/prompt-id/main.rs -o ~/.claude/bin/prompt-id

~/.claude/bin/prompt-id alloc <slug>   # claims a number, creates the file, prints its path
~/.claude/bin/prompt-id store          # the resolved store, for reading/listing
~/.claude/bin/prompt-id next           # the number a caller would get
~/.claude/bin/prompt-id audit          # duplicates + unused runs; exits non-zero if a number is shared
```

Write your content INTO the path `alloc` prints — never compose a filename yourself,
and never save into the worktree you happen to be standing in.

If `rustc` is unavailable, fall back to scanning BOTH the store and its archive —
`"$(git rev-parse --git-common-dir)/../.prompts"`, including `completed/` — and take
the highest number plus one. Never scan only the local directory, and never reuse an
archived number: `/run-prompt 184` must resolve to exactly one piece of work.
</context>

<prime_recall_gate>
After intake but BEFORE generation, semantically recall related prior work from this repo's `.prime/` graph:

1. `mcp__prime__prime_recall { text: <user's task description verbatim>, node_type: "prompt", top_k: 5, depth: 1 }` — server embeds the query via in-process fastembed (`AllMiniLML6V2`, 384 dims); `depth: 1` pulls in connected `prompt_outcome` nodes so success/failure is visible.

2. For each match above ~0.75 similarity, surface inline:

"Found related prior prompts:
- .prompts/003-… (similarity 0.84, last status: success)
- .prompts/005-… (similarity 0.78, last status: failure — validation_failed)

Re-run one of these, fork from it, or generate fresh? [run-N / fork-N / fresh]"

3. If no matches above threshold, no `prime_recall` results, or Prime MCP unavailable, proceed silently — files are canonical, Prime is best-effort.

First call in a fresh `.prime/` triggers a one-time fastembed model download (~25 MB into `.fastembed_cache/`, gitignored); subsequent calls are ~1–3 ms.
</prime_recall_gate>

<objective>
Act as an expert prompt engineer for Claude Code, specialized in crafting optimal prompts using XML tag structuring and best practices.

Create highly effective prompts for: $ARGUMENTS

Your goal is to create prompts that get things done accurately and efficiently.
</objective>

<process>

<step_0_intake_gate>
<title>Adaptive Requirements Gathering</title>

<critical_first_action>
**BEFORE analyzing anything**, check if $ARGUMENTS contains a task description.

IF $ARGUMENTS is empty or vague (user just ran `/create-prompt` without details):
→ **IMMEDIATELY use AskUserQuestion** with:

- header: "Task type"
- question: "What kind of prompt do you need?"
- options:
  - "Coding task" - Build, fix, or refactor code
  - "Analysis task" - Analyze code, data, or patterns
  - "Research task" - Gather information or explore options

After selection, ask: "Describe what you want to accomplish" (they select "Other" to provide free text).

IF $ARGUMENTS contains a task description:
→ Skip this handler. Proceed directly to adaptive_analysis.
</critical_first_action>

<adaptive_analysis>
Analyze the user's description to extract and infer:

- **Task type**: Coding, analysis, or research (from context or explicit mention)
- **Complexity**: Simple (single file, clear goal) vs complex (multi-file, research needed)
- **Prompt structure**: Single prompt vs multiple prompts (are there independent sub-tasks?)
- **Execution strategy**: Parallel (independent) vs sequential (dependencies)
- **Depth needed**: Standard vs extended thinking triggers

Inference rules:
- Dashboard/feature with multiple components → likely multiple prompts
- Bug fix with clear location → single prompt, simple
- "Optimize" or "refactor" → needs specificity about what/where
- Authentication, payments, complex features → complex, needs context
</adaptive_analysis>

<contextual_questioning>
Generate 2-4 questions using AskUserQuestion based ONLY on genuine gaps.

<question_templates>

**For ambiguous scope** (e.g., "build a dashboard"):
- header: "Dashboard type"
- question: "What kind of dashboard is this?"
- options:
  - "Admin dashboard" - Internal tools, user management, system metrics
  - "Analytics dashboard" - Data visualization, reports, business metrics
  - "User-facing dashboard" - End-user features, personal data, settings

**For unclear target** (e.g., "fix the bug"):
- header: "Bug location"
- question: "Where does this bug occur?"
- options:
  - "Frontend/UI" - Visual issues, user interactions, rendering
  - "Backend/API" - Server errors, data processing, endpoints
  - "Database" - Queries, migrations, data integrity

**For auth/security tasks**:
- header: "Auth method"
- question: "What authentication approach?"
- options:
  - "JWT tokens" - Stateless, API-friendly
  - "Session-based" - Server-side sessions, traditional web
  - "OAuth/SSO" - Third-party providers, enterprise

**For performance tasks**:
- header: "Performance focus"
- question: "What's the main performance concern?"
- options:
  - "Load time" - Initial render, bundle size, assets
  - "Runtime" - Memory usage, CPU, rendering performance
  - "Database" - Query optimization, indexing, caching

**For output/deliverable clarity**:
- header: "Output purpose"
- question: "What will this be used for?"
- options:
  - "Production code" - Ship to users, needs polish
  - "Prototype/POC" - Quick validation, can be rough
  - "Internal tooling" - Team use, moderate polish

</question_templates>

<question_rules>
- Only ask about genuine gaps - don't ask what's already stated
- Each option needs a description explaining implications
- Prefer options over free-text when choices are knowable
- User can always select "Other" for custom input
- 2-4 questions max per round
</question_rules>
</contextual_questioning>

<decision_gate>
After receiving answers, present decision gate using AskUserQuestion:

- header: "Ready"
- question: "I have enough context to create your prompt. Ready to proceed?"
- options:
  - "Proceed" - Create the prompt with current context
  - "Ask more questions" - I have more details to clarify
  - "Let me add context" - I want to provide additional information

If "Ask more questions" → generate 2-4 NEW questions based on remaining gaps, then present gate again
If "Let me add context" → receive additional context via "Other" option, then re-evaluate
If "Proceed" → continue to generation step
</decision_gate>

<finalization>
After "Proceed" selected, state confirmation:

"Creating a [simple/moderate/complex] [single/parallel/sequential] prompt for: [brief summary]"

Then proceed to generation.
</finalization>
</step_0_intake_gate>

<step_1_generate_and_save>
<title>Generate and Save Prompts</title>

<pre_generation_analysis>
Before generating, determine:

1. **Single vs Multiple Prompts**:
   - Single: Clear dependencies, single cohesive goal, sequential steps
   - Multiple: Independent sub-tasks that could be parallelized or done separately

2. **Execution Strategy** (if multiple):
   - Parallel: Independent, no shared file modifications
   - Sequential: Dependencies, one must finish before next starts

3. **Reasoning depth**:
   - Simple → Standard prompt
   - Complex reasoning/optimization → Extended thinking triggers

4. **Required tools**: File references, bash commands, MCP servers

5. **Prompt quality needs**:
   - "Go beyond basics" for ambitious work?
   - WHY explanations for constraints?
   - Examples for ambiguous requirements?
</pre_generation_analysis>

<trunk_based_work_organization>
**Invoke the `scope-beads` skill** (Skill tool → `scope-beads`) before saving — it governs how the generated work is organized under trunk-based development. Apply its contract to this output:

- **One feature = one short-lived branch = one PR.** If you generate MULTIPLE prompts for ONE feature, they all belong to that single feature branch and merge as ONE PR — NOT a branch or PR per prompt. "Independent / parallel" describes how the prompts *execute* (no shared files), NOT separate deliverables: a `--parallel` run means the sub-agents **converge on the SAME feature branch** (each in its own `git worktree` / detached HEAD to avoid file-stomping), never a branch each.
- **Separate prompts get separate PRs ONLY if they are genuinely independent features** (no shared end-to-end). If they must all land for the feature to work, they are ONE PR. Size is never a reason to split.
- **Stamp every generated prompt** with a `<work_organization>` block naming the shared feature branch:

  ```xml
  <work_organization>
  Trunk-based development: part of feature `<feature-name>`, branch `<type>/<slug>`.
  Implement your slice as commit(s) on THAT one branch — `git fetch origin main && git rebase origin/main` first, keep the build green. Do NOT open a separate PR for this prompt. The feature merges as ONE PR to `main` when all slices are done and it works end-to-end.
  </work_organization>
  ```

If the `scope-beads` skill is unavailable, apply this summary directly — the rule holds regardless.
</trunk_based_work_organization>

Create the prompt(s) and save to the prompts folder.

**For single prompts:**

- Generate one prompt file following the patterns below
- Save as `.prompts/[number]-[name].md`

**For multiple prompts:**

- Determine how many prompts are needed (typically 2-4)
- Generate each prompt with clear, focused objectives
- Save sequentially: `.prompts/[N]-[name].md`, `.prompts/[N+1]-[name].md`, etc.
- Each prompt should be self-contained and executable independently — but for ONE feature they still share ONE feature branch and merge as ONE PR (see `<trunk_based_work_organization>`); stamp each with its `<work_organization>` block.

**Prompt Construction Rules**

Always Include:

- XML tag structure with clear, semantic tags like `<objective>`, `<context>`, `<requirements>`, `<constraints>`, `<output>`
- **Contextual information**: Why this task matters, what it's for, who will use it, end goal
- **Explicit, specific instructions**: Tell Claude exactly what to do with clear, unambiguous language
- **Sequential steps**: Use numbered lists for clarity
- File output instructions using relative paths: `./filename` or `./subfolder/filename`
- Reference to reading the CLAUDE.md for project conventions
- Explicit success criteria within `<success_criteria>` or `<verification>` tags

Conditionally Include (based on analysis):

- **Extended thinking triggers** for complex reasoning:
  - Phrases like: "thoroughly analyze", "consider multiple approaches", "deeply consider", "explore multiple solutions"
  - Don't use for simple, straightforward tasks
- **"Go beyond basics" language** for creative/ambitious tasks:
  - Example: "Include as many relevant features as possible. Go beyond the basics to create a fully-featured implementation."
- **WHY explanations** for constraints and requirements:
  - In generated prompts, explain WHY constraints matter, not just what they are
  - Example: Instead of "Never use ellipses", write "Your response will be read aloud, so never use ellipses since text-to-speech can't pronounce them"
- **Parallel tool calling** for agentic/multi-step workflows:
  - "For maximum efficiency, whenever you need to perform multiple independent operations, invoke all relevant tools simultaneously rather than sequentially."
- **Reflection after tool use** for complex agentic tasks:
  - "After receiving tool results, carefully reflect on their quality and determine optimal next steps before proceeding."
- `<research>` tags when codebase exploration is needed
- `<validation>` tags for tasks requiring verification
- `<examples>` tags for complex or ambiguous requirements - ensure examples demonstrate desired behavior and avoid undesired patterns
- Bash command execution with "!" prefix when system state matters
- MCP server references when specifically requested or obviously beneficial

Output Format:

1. Generate prompt content with XML structure
2. Save to the path `prompt-id alloc <slug>` printed — it is `<main-checkout>/.prompts/NNN-<slug>.md`
   - Number format: 001, 002, 003 — allocated by the tool, NEVER composed by hand and never read off the local directory (see the numbering note in `<context>`)
   - Name format: lowercase, hyphen-separated, max 5 words describing the task
   - Example: `.prompts/001-implement-user-authentication.md`
3. File should contain ONLY the prompt, no explanations or metadata

<prompt_patterns>

For Coding Tasks:

```xml
<objective>
[Clear statement of what needs to be built/fixed/refactored]
Explain the end goal and why this matters.
</objective>

<context>
[Project type, tech stack, relevant constraints]
[Who will use this, what it's for]
@[relevant files to examine]
</context>

<requirements>
[Specific functional requirements]
[Performance or quality requirements]
Be explicit about what Claude should do.
</requirements>

<implementation>
[Any specific approaches or patterns to follow]
[What to avoid and WHY - explain the reasoning behind constraints]
</implementation>

<output>
Create/modify files with relative paths:
- `./path/to/file.ext` - [what this file should contain]
</output>

<verification>
Before declaring complete, verify your work:
- [Specific test or check to perform]
- [How to confirm the solution works]
</verification>

<success_criteria>
[Clear, measurable criteria for success]
</success_criteria>
```

For Analysis Tasks:

```xml
<objective>
[What needs to be analyzed and why]
[What the analysis will be used for]
</objective>

<data_sources>
@[files or data to analyze]
![relevant commands to gather data]
</data_sources>

<analysis_requirements>
[Specific metrics or patterns to identify]
[Depth of analysis needed - use "thoroughly analyze" for complex tasks]
[Any comparisons or benchmarks]
</analysis_requirements>

<output_format>
[How results should be structured]
Save analysis to: `./analyses/[descriptive-name].md`
</output_format>

<verification>
[How to validate the analysis is complete and accurate]
</verification>
```

For Research Tasks:

```xml
<research_objective>
[What information needs to be gathered]
[Intended use of the research]
For complex research, include: "Thoroughly explore multiple sources and consider various perspectives"
</research_objective>

<scope>
[Boundaries of the research]
[Sources to prioritize or avoid]
[Time period or version constraints]
</scope>

<deliverables>
[Format of research output]
[Level of detail needed]
Save findings to: `./research/[topic].md`
</deliverables>

<evaluation_criteria>
[How to assess quality/relevance of sources]
[Key questions that must be answered]
</evaluation_criteria>

<verification>
Before completing, verify:
- [All key questions are answered]
- [Sources are credible and relevant]
</verification>
```
</prompt_patterns>
</step_1_generate_and_save>

<step_1_5_index_in_prime>
<title>Register Prompt in Prime Graph</title>

After each file is written, register it as a node in the per-repo `.prime/` graph so future `/create-prompt` invocations can recall and `/run-prompt` can attach outcomes.

For each saved `.prompts/NNN-name.md`:

1. `mcp__prime__prime_add_node`:
   - type: `"prompt"`
   - properties: `{ name: "NNN-name", file: ".prompts/NNN-name.md", domain: <inferred>, task_type: <"coding"|"analysis"|"research">, topic: <kebab-topic from filename>, created_at: <today YYYY-MM-DD> }`
   - Capture the returned `entity_id`.

2. `mcp__prime__prime_embed { id: <entity_id>, text: <user's task description + inferred topic/domain, ~1–2 sentences> }` — server embeds via in-process fastembed; this is what makes the prompt findable by the recall gate on future invocations.

3. For each `@path/to/file` reference in the generated prompt body, ensure a `file` node exists:
   - `mcp__prime__prime_search { type: "file" }` to find one with matching `path` property; if missing, `mcp__prime__prime_add_node { type: "file", properties: { name: <basename>, path: <relative>, domain: <inferred> } }`.
   - `mcp__prime__prime_add_edge { source: <prompt entity_id>, target: <file entity_id>, relation: "references" }`.

4. For multi-prompt outputs with sequential dependencies (e.g., 005 → 006 → 007):
   - `mcp__prime__prime_add_edge { source: <later prompt>, target: <earlier prompt>, relation: "depends_on" }`.

If any `mcp__prime__*` call errors (server unreachable, version mismatch), skip silently and continue — the markdown file is the source of truth; Prime indexing is best-effort. Requires `allsource-prime ≥ 0.21.3` for text-on-embed (`cargo install allsource-prime --force` if older).
</step_1_5_index_in_prime>

<intelligence_rules>

1. **Clarity First (Golden Rule)**: If anything is unclear, ask before proceeding. A few clarifying questions save time. Test: Would a colleague with minimal context understand this prompt?

2. **Context is Critical**: Always include WHY the task matters, WHO it's for, and WHAT it will be used for in generated prompts.

3. **Be Explicit**: Generate prompts with explicit, specific instructions. For ambitious results, include "go beyond the basics." For specific formats, state exactly what format is needed.

4. **Scope Assessment**: Simple tasks get concise prompts. Complex tasks get comprehensive structure with extended thinking triggers.

5. **Context Loading**: Only request file reading when the task explicitly requires understanding existing code. Use patterns like:

   - "Examine @package.json for dependencies" (when adding new packages)
   - "Review @src/database/\* for schema" (when modifying data layer)
   - Skip file reading for greenfield features

6. **Precision vs Brevity**: Default to precision. A longer, clear prompt beats a short, ambiguous one.

7. **Tool Integration**:

   - Include MCP servers only when explicitly mentioned or obviously needed
   - Use bash commands for environment checking when state matters
   - File references should be specific, not broad wildcards
   - For multi-step agentic tasks, include parallel tool calling guidance

8. **Output Clarity**: Every prompt must specify exactly where to save outputs using relative paths

9. **Verification Always**: Every prompt should include clear success criteria and verification steps
</intelligence_rules>

<decision_tree>
After saving the prompt(s), present this decision tree to the user:

---

**Prompt(s) created successfully!**

<single_prompt_scenario>
If you created ONE prompt (e.g., `.prompts/005-implement-feature.md`):

<presentation>
✓ Saved prompt to .prompts/005-implement-feature.md

What's next?

1. Run prompt now
2. Review/edit prompt first
3. Save for later
4. Other

Choose (1-4): \_
</presentation>

<action>
If user chooses #1, invoke via SlashCommand tool: `/run-prompt 005`
</action>
</single_prompt_scenario>

<parallel_scenario>
If you created MULTIPLE prompts that CAN run in parallel (e.g., independent modules, no shared files):

<presentation>
✓ Saved prompts:
  - .prompts/005-implement-auth.md
  - .prompts/006-implement-api.md
  - .prompts/007-implement-ui.md

Execution strategy: These prompts can run in PARALLEL (independent tasks, no shared files)
Trunk-based: for ONE feature the sub-agents converge on ONE feature branch → ONE PR (each in its own worktree/detached HEAD), never a branch or PR per prompt.

What's next?

1. Run all prompts in parallel now (launches 3 sub-agents simultaneously, all on the one feature branch)
2. Run prompts sequentially instead
3. Review/edit prompts first
4. Other

Choose (1-4): \_
</presentation>

<actions>
If user chooses #1, invoke via SlashCommand tool: `/run-prompt 005 006 007 --parallel`
If user chooses #2, invoke via SlashCommand tool: `/run-prompt 005 006 007 --sequential`
</actions>
</parallel_scenario>

<sequential_scenario>
If you created MULTIPLE prompts that MUST run sequentially (e.g., dependencies, shared files):

<presentation>
✓ Saved prompts:
  - .prompts/005-setup-database.md
  - .prompts/006-create-migrations.md
  - .prompts/007-seed-data.md

Execution strategy: These prompts must run SEQUENTIALLY (dependencies: 005 → 006 → 007)

What's next?

1. Run prompts sequentially now (one completes before next starts)
2. Run first prompt only (005-setup-database.md)
3. Review/edit prompts first
4. Other

Choose (1-4): \_
</presentation>

<actions>
If user chooses #1, invoke via SlashCommand tool: `/run-prompt 005 006 007 --sequential`
If user chooses #2, invoke via SlashCommand tool: `/run-prompt 005`
</actions>
</sequential_scenario>

---

</decision_tree>
</process>

<success_criteria>
- Intake gate completed (AskUserQuestion used for clarification if needed)
- User selected "Proceed" from decision gate
- Appropriate depth, structure, and execution strategy determined
- Prompt(s) generated with proper XML structure following patterns
- Files saved to .prompts/[number]-[name].md with correct sequential numbering
- Decision tree presented to user based on single/parallel/sequential scenario
- User choice executed (SlashCommand invoked if user selects run option)
- Each saved prompt registered as a `prompt` node in the `.prime/` graph (if Prime MCP is reachable; silently skipped otherwise)
- `scope-beads` applied: multi-prompt outputs for one feature share ONE feature branch → ONE PR, and each prompt carries its `<work_organization>` stamp
</success_criteria>

<meta_instructions>

- **Intake first**: Complete step_0_intake_gate before generating. Use AskUserQuestion for structured clarification.
- **Decision gate loop**: Keep asking questions until user selects "Proceed"
- Allocate the number with `~/.claude/bin/prompt-id alloc <slug>` and write into the path it prints — a Glob of the local `.prompts/` sees only this worktree's copy, which is exactly how numbers collide
- If .prompts/ doesn't exist, use Write tool to create the first prompt (Write will create parent directories)
- Keep prompt filenames descriptive but concise
- Adapt the XML structure to fit the task - not every tag is needed every time
- Consider the user's working directory as the root for all relative paths
- Each prompt file should contain ONLY the prompt content, no preamble or explanation
- After saving, present the decision tree as inline text (not AskUserQuestion)
- Use the SlashCommand tool to invoke /run-prompt when user makes their choice
- **Trunk-based (see `<trunk_based_work_organization>`)**: invoke `scope-beads`; multi-prompt outputs for one feature = ONE branch → ONE PR (agents converge, never a branch/PR per prompt); stamp every prompt with `<work_organization>`
</meta_instructions>
