---
title: The Press Edition
date: 2026-10-28
desk: Tools · how it works
strap: Explainer | report-press 0.1.0 | Markdy 1.5.7
publisher: Field Notes
footer: Built from `skills/report-press/examples/explainer.md`
---

# The model writes the story; a Rust binary prints the page
@kicker Explainer
@deck report-press turns one Markdown file into one self-contained HTML page, and refuses to print a story that breaks the rules.
@byline How report-press works · sources labelled inline

A report is two files. The story is Markdown that a person reviews in a pull request. The page is HTML that `report-press build` prints from it: about 350 KB, with the CSS, the figure renderer and the Markdy 1.5.7 bundle inlined, so it opens from `file://` with no network {{observed: `src/render.rs`, `vendor/markdy/README.md`}}.

The split is deliberate. Choosing what a report claims, and which diagram shows it, is judgement, so it stays with the model and the skill. Layout, typography, brand and figure sizing are mechanics, so they are compiled into the binary, and every page comes out the same.

> The model never writes HTML. Each defect the binary prevents was first hit by writing HTML by hand.

> [!NOTE]
> **Limit.** Checked in headless Chromium at 1280 px and 390 px, light and dark. Not checked in Safari, Firefox, print or a screen reader.

```markdy C4 container view: author, story, the press and its compiled-in parts, and the page
scene type=architecture
layout LR
repo Story "story.md, by author + skill"
service Press "report-press"
policy Assets "CSS · JS · Markdy · brand"
desktop Page "page.html"
group binary "Compiled into the binary": Press Assets
beat reveal "Containers":
  show $nodes stagger=60ms
beat print "From story to page":
  Story -> Press "check"
  Assets -> Press "inlined"
  Press -> Page "build"
  glow binary color=#FF8300
```

## A story must pass check before it can be printed {#s-check}
@kicker Lifecycle
@deck Errors stop the build; warnings are printed and left to the author.
@short Check before print

`report-press build` runs the same checks as `report-press check` and writes nothing if any error remains {{observed: `src/main.rs`}}. Errors cover structure: a frontmatter `title` and `date`, one `#` lead, a figure in every lead and story section, a `{.notchecked}` section placed last, and figures whose edges name real nodes {{observed: `src/lint.rs`}}.

Warnings cover style and size: a section with no source label, a figure longer than 4 ranks, filler words, a number followed by a time unit, more than 2 em dashes per 100 words. The author decides whether to fix each one.

```markdy State diagram of a story from draft to a page someone has looked at
scene type=state
layout LR
start Draft "story.md drafted"
decision Check "check: errors?"
state Built "page.html built"
end Seen "opened and read"
beat reveal "States":
  show $nodes stagger=60ms
beat happy "Clean story":
  Draft -> Check "report-press check"
  Check -> Built "no: build"
  Built -> Seen "agent-browser"
beat fix "Errors":
  Draft <- Check "yes: fix and re-run"
  glow Check color=#FEB306
```

## The binary is four small modules and three compiled-in assets {#s-binary}
@kicker Inside
@deck Parsing and checking never touch the disk; only main.rs reads and writes files.
@short Inside the binary

`story.rs` splits the file into frontmatter, sections and directives and records line numbers, so every finding points into the story. `markup.rs` uses pulldown-cmark to turn each body into blocks: prose for the columns, plus pull quotes, notes, tables and figures. `lint.rs` checks the story. `render.rs` assembles the page {{observed: `src/*.rs`}}. The one dependency is pulldown-cmark 0.13 with only the `html` feature {{observed: `Cargo.toml`}}.

```markdy C4 container view of the binary's modules and the assets compiled into it
scene type=architecture
layout LR
service Main "main.rs: read, write, exit code"
service Story "story.rs: sections, lines"
security Lint "lint.rs: errors, warnings"
service Render "render.rs: page"
policy Css "press.css + brand"
runtime Js "renderer.js + Markdy"
group logic "Pure: no I/O": Story Lint Render
group assets "include_str!": Css Js
beat reveal "Containers":
  show $nodes stagger=50ms
beat flow "One build":
  Main -> Story "source text"
  Story -> Lint "Story"
  Story -> Render "Story"
  Css -> Render "inlined"
  Js -> Render "inlined"
  glow logic color=#023047
```

## In the browser, each figure is measured before it animates {#s-figure}
@kicker Rendering
@deck Labels stay legible because no figure is drawn below 0.75 of its natural size.
@short How a figure is drawn

The renderer first asks Markdy for a static SVG and reads its `viewBox`, which is the diagram's natural size. It sizes the frame from that: full size if it fits, never below 0.75 scale, and wider diagrams scroll inside their frame with a caption hint {{observed: `assets/renderer.js`}}. Only then does it start the animated player, with `fit` set so the camera shows the whole diagram instead of following the active node.

A reader who has asked for reduced motion gets the static SVG. A change of colour scheme redraws every figure in the matching Markdy theme.

```markdy State diagram of one figure from source to screen
scene type=state
layout LR
start Measure "export SVG, read viewBox"
state Sized "frame sized, min 0.75"
decision Motion "reduced motion?"
end Player "animated player, fit"
end Static "static SVG"
beat reveal "States":
  show $nodes stagger=60ms
beat draw "Draw":
  Measure -> Sized "natural size"
  Sized -> Motion "check preference"
  Motion -> Player "no"
  Motion -> Static "yes"
  glow Sized color=#FF8300
```

## Seven defects from the first hand-built report are now impossible or caught {#s-defects}
@kicker Lessons
@deck Each row was a real failure while the first report was being built by hand.
@short Defects it prevents

| Defect seen | Effect | Now |
|---|---|---|
| `exportDiagramAsVectorSvg` treated as async | every figure blank | renderer wraps the synchronous call [[have]] |
| Player without `fit` | camera pans; a narrow figure never shows whole | renderer sets `fit` + `contain` [[have]] |
| Player resets the frame's inline width | sideways scroll lost | renderer sets min/max width instead [[have]] |
| Grid column sized to a figure | page 944 px wide on a 390 px phone | CSS uses `minmax(0, …)` [[have]] |
| 8-rank lifecycle squeezed to fit | labels about 6 px | check warns above 4 ranks; frame scrolls [[have]] |
| Backticks in a caption | the fence stops being a figure | check error [[have]] |
| `</` inside a diagram | closes the page's script | check error [[have]] |

All seven were observed while building the first report by hand on 2026-10-28 {{observed: that session}}. The rank limit comes from measured widths: 8 ranks drew 2,480 px wide; 4 ranks fit with labels under about 20 characters and scroll with 25-character ones.

```markdy C4 container view of where each defect is stopped
scene type=architecture
layout LR
repo Story "story.md"
security Check "report-press check"
policy Css "press.css"
runtime Js "renderer.js"
desktop Page "page.html"
beat reveal "Containers":
  show $nodes stagger=50ms
beat stop "Where each is stopped":
  Story -> Check "ranks · backticks · script close"
  Check -> Page "builds"
  Css -> Page "minmax(0, …)"
  Js -> Page "sync export · fit · min-width"
  glow Check color=#FEB306
```

## Follow-ups {.back}

1. Upgrade Markdy deliberately: rebuild the bundle, rebuild every example, look at each figure.
2. A second brand needs a second `brand.rs`, not a frontmatter field: stories cannot restyle the page.
3. Add a print stylesheet pass if reports get printed.

## What I have not checked {.notchecked}

- Safari, Firefox and print rendering.
- Screen-reader output of the figures beyond their `aria-label`.
- Markdy node kinds outside the list in `references/diagrams.md`.
- Scene types other than `state` and `architecture`.
