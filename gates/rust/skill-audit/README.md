# skill-audit

Every Claude Code session starts with a listing of the skills, agents and MCP
instructions it can use, and that listing is paid for on every turn. On one
machine it measured 69,932 bytes (~17,500 tokens), and the largest plugin in it,
39% of the bytes, had never been called once. `skill-audit` measures that cost
from a real session and counts how often each entry was actually used, so a
removal can cite a number instead of a feeling.

It is read-only and prints names, byte counts, use counts and dates. No
transcript text leaves the tool.

```sh
cargo install --path gates/rust/skill-audit
```

## The five steps

```sh
# 1. What one session's listing cost, entry by entry (bytes and whether the
#    budget kept its description).
skill-audit listing ~/.claude/projects/<project>/<session>.jsonl > listing.tsv

# 2. How often each skill, slash command, agent and MCP server was used, across
#    every project. Tool calls are deduplicated by id, because resumed sessions
#    copy earlier records into new files.
skill-audit usage ~/.claude/projects --since 2026-09-12 > usage.tsv

# 3. Typed /commands from history.jsonl, which outlives transcript retention.
skill-audit history ~/.claude/history.jsonl --since 2026-09-12 >> usage.tsv

# 4. One row per listing entry: set (repo, personal, plugin, built-in), cost,
#    uses, referrers among other skills and hooks, and protection (named in a
#    CLAUDE.md, a rule or MEMORY.md, or changed in the last 30 days).
skill-audit join listing.tsv usage.tsv <repo> "$HOME" > joined.tsv

# 5. An HTML table: <!--ROWS--> in your template is replaced with one row per
#    entry and a proposal. Plugin decisions are yours to pass; the tool holds
#    none.
skill-audit render joined.tsv template.html report.html --disable p1,p2 --keep p3
```

## Reading the numbers

- A **zero** is only as good as the scan. Grep the transcripts by hand for a
  name the scan says is unused, alongside a positive control you know is used,
  before acting on it.
- The skill listing has a size budget. When it is full, entries are listed by
  name only and removing one saves only its name; the agent and MCP lists are
  not capped, so removals there pay in full.
