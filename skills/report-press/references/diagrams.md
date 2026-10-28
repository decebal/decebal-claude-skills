# Diagrams

Figures are [Markdy](https://markdy.com) scenes, rendered by the Markdy 1.5.7
bundle compiled into report-press. The grammar moves fast between releases; this
page is what was **measured against 1.5.7**, not the latest spec.

## Pick the type from the section's claim

| The section says | Scene | Node kinds |
|---|---|---|
| X moves through states; here is where it can go wrong | `type=state` | `start`, `state`, `decision`, `end` |
| These parts exist and this is how they connect | `type=architecture` + `group` = C4 container view | `user`, `client`, `service`, `database`, `security`, `runtime`, `repo`, `policy`, `container`, `desktop`, `branch`, `configmap`, `pipeline`, `release`, `hub`, `step` |

A C4 container view in Markdy: one node per container (app, store, tool, repo),
`group` for each system boundary, a `user` for each person, edges labelled with
what flows. Keep code-level detail out.

## State diagram

```markdy State diagram of a bead in the task tracker
scene type=state
layout LR
start Created "created"
state Ready "ready"
decision Gates "gates green?"
end Done "done"
beat reveal "States":
  show $nodes stagger=60ms
beat life "One bead":
  Created -> Ready "no blockers"
  Ready -> Gates "implement"
  Gates -> Done "yes"
beat fail "Gates fail":
  Ready <- Gates "no: fix"
  glow Done color=#FF8300
```

## C4 container view

```markdy C4 container view of the service and what it talks to
scene type=architecture
layout LR
user Dev "Engineer"
service Api "API"
database Db "Postgres"
runtime Worker "Worker"
group app "The application": Api Db Worker
beat reveal "Containers":
  show $nodes stagger=50ms
beat flow "Requests":
  Dev -> Api "HTTPS"
  Api -> Db "SQL"
  Api ~> Worker "jobs"
  glow app color=#023047
```

## Grammar, the subset that works

- Line 1: `scene type=state|architecture`. Leave `theme=` out: the page sets
  `editorial` (light) or `midnight` (dark).
- `layout LR` (default for reports) or `layout TB`.
- Nodes, unindented: `<kind> <Id> "Label"`. Ids are single words.
- Groups, unindented: `group <id> "Label": Id1 Id2 …`.
- Beats: `beat <id> "Caption":` then two-space-indented lines.
  - edges: `A -> B "label"` (forward, sets rank), `A ~> B "label"` (async,
    dashed), `A <- B "label"` (a return path; use it for back-edges so the
    layout does not fold)
  - cues: `show $nodes stagger=60ms`, `glow <Id|group> color=#hex`
- Edges belong inside beats.

## Size limits, measured

Width grows with the number of left-to-right ranks and with label length. The
page is 1,180 px wide and the renderer never draws below 0.75 scale, so about
1,430 px of natural width fits. Measured with 1.5.7:

| Longest forward chain | Labels | Natural width | On a desktop page |
|---|---|---|---|
| 4 ranks | ≤ 15 characters | 1,264 px | fits |
| 4 ranks | about 20 characters | 1,392 px | fits |
| 4 ranks | about 25 characters | 1,504 px | scrolls |
| 6 ranks | short | 1,872 px | scrolls |
| 8 ranks | short | 2,480 px | scrolls |

A `group` box adds padding: the same 4-rank chain with short labels measured
1,472 px once two of its nodes sat in a group.

`check` warns above 4 ranks. At 4 ranks keep labels under about 20 characters
and avoid a group on the main chain; the browser check in the skill catches the
rest (a figure that scrolls gets class `wide`).

To get a long lifecycle under 4 ranks: merge the start into the first state
(`start Queued "spawned, queued"`), fan outcomes out from one `decision`
instead of chaining them, or split it into two figures. Keep a strip when the
sequence itself is the point.

## Traps already hit

- `exportDiagramAsVectorSvg` is **synchronous** in 1.5.7. The renderer handles
  it; do not write page JS against it.
- Without `fit: true, fitMode: "contain"` the animated player follows the
  active node with the camera and a narrow figure never shows the whole
  diagram. The renderer sets both.
- A caption cannot contain backticks: it is the fence's info string.
