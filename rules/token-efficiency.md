# Token efficiency

Rule form, for inclusion in a project's agent instructions. Deep dive:
[../docs/token-efficiency.md](../docs/token-efficiency.md).

- **Targeted searches first** — grep/glob with specific patterns instead of reading
  whole files. Read only the relevant sections with `offset`/`limit`. If grep
  misses, try glob for file patterns. A full file read is the last resort.
- **No `sed`/`awk`/`cat`/`head`/`tail` for reading** — use the file-read tool with
  `offset`/`limit`, never a shell text dump into context. For inline edits use the
  edit tool, not `sed -i`. If you must pipe a large command's output, filter it to
  errors and warnings.
- **Cache mentally** — don't re-read files you've already seen this session.
  Reference line numbers from prior reads. Retain: file structure, exports,
  function signatures, import patterns, config values.
- **Concise responses** — state what you'll do, do it, report the result.
- **Batch parallel calls** — read 3 independent files in one message, not three.
  Run grep and glob together.
- **Scope strictly** — only what was asked. No added comments, docstrings, or
  refactors to code you didn't change. No speculative improvements. Fewer lines
  changed = fewer tokens on review.
- **Minimal diffs** — prefer a small `old_string` edit over rewriting a file. Never
  rewrite a file to change 3 lines.

## Output filtering (optional tooling)

A proxy that filters command output before it reaches the context window
(compilers, test runners, linters, `git`) saves 60–90% on dev operations. Two
things to know if you adopt one:

- **A summarizing filter can hide a FAILED build.** The summary reads as success.
  Read the tool's own tee log, or the exit code, before believing it.
- **A rewriting hook can break a tool's own flags.** Verify the rewritten command
  is the one you meant; keep an escape hatch, and never disable the proxy globally
  to work around one command.

## Caveman output — always on

Every assistant message is caveman (`full`): no articles, filler, pleasantries
or hedging; fragments fine; technical terms, code, paths and error text verbatim.
Status lines, progress updates, findings, final reports and questions included.
No session, task size or "this one is mostly code" turns it off; only the user
saying "stop caveman" or "normal mode" does.

- **Subagents and workflow agents too.** Their final text lands in the parent's
  context. Every delegation prompt ends with: `Return caveman: terse fragments,
  file:line, no prose.` Prefer the `cavecrew-*` agents where one fits.
- **Enforced for subagents by a `SubagentStart` hook**, because `SessionStart`
  never fires for them. It prints
  `{"hookSpecificOutput":{"hookEventName":"SubagentStart","additionalContext":"CAVEMAN MODE ACTIVE (full). …"}}`.
- **Write normal only for artifacts a third party reads**: commit messages, PR
  bodies, files under `docs/`, code comments, user-facing product copy.
- **Suspend for one message, then resume**: security warnings, confirming an
  irreversible action, ordered steps where a dropped word changes the meaning.
