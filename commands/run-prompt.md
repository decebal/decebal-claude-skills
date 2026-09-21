---
name: run-prompt
description: Delegate one or more prompts to fresh sub-task contexts with parallel or sequential execution
argument-hint: <prompt-number(s)-or-name> [--parallel|--sequential]
allowed-tools: [Read, Task, Bash(ls:*), Bash(mv:*), Bash(git:*), Bash(~/.claude/bin/prompt-id:*), Bash(rustc:*), mcp__prime__prime_search, mcp__prime__prime_add_node, mcp__prime__prime_add_edge]
---

<context>
Git status: !`git status --short`
Recent prompts: !`ls -t "$(~/.claude/bin/prompt-id store 2>/dev/null || echo "$(git rev-parse --git-common-dir)/../.prompts")"/*.md 2>/dev/null | head -5`
Already shipped: !`~/.claude/bin/prompt-id shipped 2>/dev/null | tail -n +3`
</context>

The listing above comes from the SHARED store (the main checkout's `.prompts/`), not
from the current directory — in a worktree those are different, and the local one is
usually empty.

**Anything under "Already shipped" is finished work still sitting in the queue** — its
reserved branch has already merged. Do not run it. Archive it with
`~/.claude/bin/prompt-id archive <number>` and say so. Two prompts sat there for days
because nothing looked; re-running one burns a full agent run redoing merged work, on a
branch name a merged PR already owns.

<objective>
Execute one or more prompts from `.prompts/` as delegated sub-tasks with fresh context. Supports single prompt execution, parallel execution of multiple independent prompts, and sequential execution of dependent prompts.
</objective>

<input>
The user will specify which prompt(s) to run via $ARGUMENTS, which can be:

**Single prompt:**

- Empty (no arguments): Run the most recently created prompt (default behavior)
- A prompt number (e.g., "001", "5", "42")
- A partial filename (e.g., "user-auth", "dashboard")

**Multiple prompts:**

- Multiple numbers (e.g., "005 006 007")
- With execution flag: "005 006 007 --parallel" or "005 006 007 --sequential"
- If no flag specified with multiple prompts, default to --sequential for safety
  </input>

<process>
<step1_parse_arguments>
Parse $ARGUMENTS to extract:
- Prompt numbers/names (all arguments that are not flags)
- Execution strategy flag (--parallel or --sequential)

<examples>
- "005" → Single prompt: 005
- "005 006 007" → Multiple prompts: [005, 006, 007], strategy: sequential (default)
- "005 006 007 --parallel" → Multiple prompts: [005, 006, 007], strategy: parallel
- "005 006 007 --sequential" → Multiple prompts: [005, 006, 007], strategy: sequential
</examples>
</step1_parse_arguments>

<step2_resolve_files>
**Resolve against the shared store, not the directory you are standing in.**
`.prompts/` is gitignored in most repos, so every `git worktree` has its own copy
and a prompt written in one is invisible from another. Resolve the store first:

```bash
STORE=$(~/.claude/bin/prompt-id store)          # the main checkout's .prompts
STORE=${STORE:-"$(git rev-parse --git-common-dir)/../.prompts"}   # fallback if the tool is absent
```

Then, for each prompt number/name:

- If empty or "last": `ls -t "$STORE"/*.md | head -1`
- If a number: match that zero-padded number in `"$STORE"` (e.g. "5" matches `005-*.md`)
- If text: match the string in filenames under `"$STORE"`

<matching_rules>

- If exactly one match found: Use that file
- If multiple matches found: List them and ask user to choose. **Numbers collided
  historically** (a number can name two unrelated prompts), so never silently pick
  the first — and if a match is already in `completed/`, say so, because re-running
  shipped work is usually a mistake rather than the intent.
- If no matches found: Report error and list available prompts FROM THE STORE (a
  local listing will look empty in a worktree and read as "no such prompt")
  </matching_rules>
  </step2_resolve_files>

<step3_execute>
<single_prompt>

1. Read the complete contents of the prompt file
2. Delegate as sub-task using Task tool with subagent_type="general-purpose"
3. Wait for completion
4. Archive prompt with `prompt-id archive` — see `<archiving>`
5. Commit all work (INCLUDING the archive move):
   - Stage files YOU modified with `git add [file]` (never `git add .`)
   - Determine appropriate commit type based on changes (fix|feat|refactor|style|docs|test|chore)
   - Commit with format: `[type]: [description]` (lowercase, specific, concise)
6. Return results
   </single_prompt>

<parallel_execution>

1. Read all prompt files
2. **Spawn all Task tools in a SINGLE MESSAGE** (this is critical for parallel execution):
   <example>
   Use Task tool for prompt 005
   Use Task tool for prompt 006
   Use Task tool for prompt 007
   (All in one message with multiple tool calls)
   </example>
3. Wait for ALL to complete
4. Archive all prompts with `prompt-id archive` — see `<archiving>`
5. Commit all work (INCLUDING the archive moves):
   - Stage files YOU modified with `git add [file]` (never `git add .`)
   - Determine appropriate commit type based on changes (fix|feat|refactor|style|docs|test|chore)
   - Commit with format: `[type]: [description]` (lowercase, specific, concise)
6. Return consolidated results
   </parallel_execution>

<sequential_execution>

1. Read first prompt file
2. Spawn Task tool for first prompt
3. Wait for completion
4. Archive first prompt
5. Read second prompt file
6. Spawn Task tool for second prompt
7. Wait for completion
8. Archive second prompt
9. Repeat for remaining prompts
10. Archive all prompts with `prompt-id archive` — see `<archiving>`
11. Commit all work (INCLUDING the archive moves):
    - Stage files YOU modified with `git add [file]` (never `git add .`)
    - Determine appropriate commit type based on changes (fix|feat|refactor|style|docs|test|chore)
    - Commit with format: `[type]: [description]` (lowercase, specific, concise)
12. Return consolidated results
    </sequential_execution>
    </step3_execute>
    </process>

<archiving>
"Archive" means MOVE THE FILE IN GIT, in the same commit as the work. A prompt left
sitting in `.prompts/` after it shipped is indistinguishable from one nobody ran, and a
later audit will re-run finished work or waste a session proving it was already done.

```bash
~/.claude/bin/prompt-id archive NNN     # prints the archived path, or fails loudly
```

That one command does the move, the `git add -f`, and the verification, against the
SHARED store — never the worktree you ran from, which would leave the original sitting
in the main checkout looking unrun. It refuses a number two prompts share rather than
guessing; pass a distinguishing piece of the file name instead
(`prompt-id archive a-dead-queue`).

**Written down as three manual steps this was honoured 15 times out of 255.** The other
240 archives existed only on the machine that ran them, so every other clone of the repo
saw a store that looked almost empty. Do not hand-roll the steps again.

- If `prompt-id` is genuinely absent, the three steps it replaces are: `git mv`, falling
  back to `mv` + `git add -f` when git says `not under version control`; then confirm
  `git ls-files .prompts/completed/NNN-name.md` prints the path. **Never a bare `mv`** —
  `.prompts/` is gitignored, usually via the user's GLOBAL gitignore, so a plain `mv`
  records a DELETION and leaves the new path untracked: the prompt disappears from the
  repo instead of moving, and nothing says so.
- Commit the move **together with the work it produced**, so the archive cannot be
  forgotten as a separate step. If the work needed no commit (prompt was already
  satisfied by earlier work), commit the move alone as
  `chore(prompts): archive NNN — already shipped by <sha or PR>`.
</archiving>

<context_strategy>
By delegating to a sub-task, the actual implementation work happens in fresh context while the main conversation stays lean for orchestration and iteration.
</context_strategy>

<prime_outcome_recording>
After each prompt finishes (success, failure, or partial), record an outcome event in the per-repo `.prime/` graph. This is how future `/create-prompt` invocations learn which prompts worked.

For each completed prompt:

1. Find the prompt's existing node:
   - `mcp__prime__prime_search { type: "prompt" }`, filter the result for `properties.file === <relative path of the prompt being run>`. Capture its `entity_id`.
   - If no match (prompt predates the Prime integration), create one inline: `mcp__prime__prime_add_node { type: "prompt", properties: { name, file, created_at: <file mtime>, indexed_late: true } }`.

2. Create the outcome node:
   - `mcp__prime__prime_add_node`:
     - type: `"prompt_outcome"`
     - properties: `{ prompt_file: <relative path>, status: "success" | "failure" | "partial", commit_hash: <sha if committed, else null>, archived_to: ".prompts/completed/NNN-name.md", completed_at: <YYYY-MM-DD> }`
     - On failure, also set `error: <first 200 chars of error message>`.

3. Link outcome to prompt:
   - `mcp__prime__prime_add_edge { source: <outcome entity_id>, target: <prompt entity_id>, relation: "outcomes" }`.

For parallel runs, record one outcome per prompt. For sequential runs that aborted, record `status: "success"` for completed ones and `status: "failure"` for the one that stopped the chain; do not record nodes for prompts that never started.

If `mcp__prime__*` calls error, log and continue — the prompt has already executed; failing to index doesn't roll back work.
</prime_outcome_recording>

<output>
<single_prompt_output>
✓ Executed: .prompts/005-implement-feature.md
✓ Archived to: .prompts/completed/005-implement-feature.md

<results>
[Summary of what the sub-task accomplished]
</results>
</single_prompt_output>

<parallel_output>
✓ Executed in PARALLEL:

- .prompts/005-implement-auth.md
- .prompts/006-implement-api.md
- .prompts/007-implement-ui.md

✓ All archived to .prompts/completed/

<results>
[Consolidated summary of all sub-task results]
</results>
</parallel_output>

<sequential_output>
✓ Executed SEQUENTIALLY:

1. .prompts/005-setup-database.md → Success
2. .prompts/006-create-migrations.md → Success
3. .prompts/007-seed-data.md → Success

✓ All archived to .prompts/completed/

<results>
[Consolidated summary showing progression through each step]
</results>
</sequential_output>
</output>

<critical_notes>

- For parallel execution: ALL Task tool calls MUST be in a single message
- For sequential execution: Wait for each Task to complete before starting next
- Archive prompts only after successful completion, and archive with `prompt-id archive` in the
  work's own commit — never a bare `mv`, never a follow-up "I'll move it later"
- If any prompt fails, stop sequential execution and report error
- Provide clear, consolidated results for multiple prompt execution
  </critical_notes>
