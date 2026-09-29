# Jev with Claude Code

Jev is TypeSafe's "System One" model: it reads a state and answers typed
questions (a yes/no probability, a choice among named options, a score on a
rubric) with a confidence. It generates no text, so it cannot replace the model
behind Claude Code. It is for the decisions an agent or an app would otherwise
pay a frontier model to make: route, gate, rank, classify, check.

There are three places to use it, from least to most invasive.

## 1. Teach Claude to build with it

The official plugin gives Claude the API, the primitives and the patterns, so
code it writes for you uses Jev correctly:

```sh
claude plugin marketplace add typesafe-ai/skills
claude plugin install typesafe@typesafe-ai
```

In a session, `/typesafe:typesafe-ai` loads it; otherwise it triggers when a
feature needs a structured decision or a prompt-and-parse step could become one.
It adds only its description to context until used.

## 2. Use it in the apps you build

The SDKs are project dependencies, not machine installs:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install --upgrade typesafe-sdk
npm install @typesafe-ai/sdk
```

Keep `TYPESAFE_API_KEY` in a file only you can read and export it per shell;
never commit it. Three habits from TypeSafe's docs and early write-ups:

- **Batch every question that reads the same state into one call.** Price is per
  input token, so one state with five questions costs about one call.
- **Route on the answer AND its confidence.** Send low-confidence answers to a
  person or a stronger model; a typed answer can still be wrong.
- **Pin the model version** (`jev-1.13.0`, not `jev-latest`) and log the version
  that answered.

## 3. Let Claude ask Jev at its own decision points

Claude's own loop has forks a small model can decide: whether to fan out
subagents, how much research a question needs, whether a failing approach should
stop, whether an earlier result can be reused, whether an irreversible step
needs a person. The pattern:

- **A router you run locally**, which takes a small JSON state (a one-line
  goal, the kind of task, whether a cached result exists, the last error and its
  count) and returns an action (`reuse_cache`, `stop_retry`, `research_capped`,
  `allow_subagent`, `ask_human`, `proceed_full`, …).
- **A user skill** (`~/.claude/skills/<name>/SKILL.md`) whose description names
  those moments, so Claude calls the router there and skips it for routine edits.
- **Shadow mode first.** The router's answer is advice; Claude states when it
  acts differently, and the decisions go to a log you read before letting the
  answer bind.

Claude Code's auto mode refuses to run a third-party router until you allow it,
and refuses to grant that permission itself. Add it to `~/.claude/settings.json`
yourself:

```json
"permissions": { "allow": ["Bash(/path/to/router/route:*)"] }
```

**What leaves the machine:** the state you send. Keep the goal to one line with
no secrets, customer data or client names.

## Optional: choosing which rules load

A `UserPromptSubmit` hook can ask Jev which task-triggered rules apply to each
prompt and inject only those, shrinking the always-loaded instruction set further
than `paths:` scoping can (see [rules/README.md](../rules/README.md)). The cost is
that the start of **every prompt, in every repo**, goes to TypeSafe. Decide that
per machine; client repositories may rule it out.
