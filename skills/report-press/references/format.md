# Story format

A story is Markdown (CommonMark plus GFM tables) with four additions:
frontmatter, heading directives, heading attributes and two inline marks.

## Frontmatter

```markdown
---
title: The Harness Edition        # masthead; the last word is set in the accent colour
date: 2026-10-28                  # ISO; printed as "Wednesday 28 October 2026"
desk: Engineering · Web & SDK     # masthead, centre
strap: For peer discussion | Not a decision record    # " | "-separated, under the title
footer: my-repo · `docs/reports/…`                    # inline Markdown
publisher: Field Notes            # optional; replaces the logo with a text wordmark
---
```

`title` and `date` are required. Unknown keys are errors.

## Sections

| Heading | Becomes |
|---|---|
| `# Headline` | the lead: left column, drop cap on the first paragraph, its figures full width below, the "In this edition" index on the right. Exactly one. |
| `## Headline {#id}` | a story section: prose set in two columns, tables and figures full width |
| `## Headline {.back}` | a back-page box (questions, follow-ups) |
| `## What I have not checked {.notchecked}` | the last back-page box. Required, and must be last |

`{#id}` sets the anchor (default: a slug of the headline). `### …` inside a
section is a subhead.

## Directives

Directly under a heading, one per line:

| Directive | Where | Shows as |
|---|---|---|
| `@kicker Desk` | any | small capitals above the headline |
| `@deck One sentence.` | any | italic standfirst under the headline |
| `@byline By …` | lead | rule-boxed byline |
| `@short Index label` | story, back | the entry in "In this edition" (default: the headline) |

## Blocks

| Markdown | Becomes |
|---|---|
| `> text` | pull quote |
| `> [!NOTE]`<br>`> **Limit.** text` | the limit box (left rule, muted) |
| a GFM table | full-width table |
| ```` ```markdy Caption text ```` | a numbered figure (see [diagrams.md](diagrams.md)). The caption is plain text: a fence's info string cannot hold backticks |
| any other fence | a code block |

## Inline marks

| Mark | Becomes |
|---|---|
| `{{observed: \`file:12\`}}` | a source label pill; Markdown inside works |
| `[[have]]` `[[part]]` `[[gap]]` | green / amber / red status tag |
| `[[rock]]` `[[sand]]` `[[water]]` | confidence tag |

Marks inside code spans and fences are left alone.

## What `check` enforces

Errors (build refuses):

- frontmatter: `title`, valid `date`, known keys; one `#` lead
- every lead and story section has a `markdy` figure
- the last section is `{.notchecked}`, and there is only one
- unknown directives, heading attributes, tags; unbalanced `{{ }}`
- figures: a caption, no backtick in it, a `scene` first line, no `</`, edges
  and cues that name real nodes or groups

Warnings:

- a story section with no `{{source label}}`
- a left-to-right figure longer than 4 ranks (it will scroll sideways)
- a scene type other than `state` / `architecture`, or a node kind not verified
  against the vendored Markdy
- filler vocabulary, number + time unit ("3 days"), more than 2 em dashes per 100 words
