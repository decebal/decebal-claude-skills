# Markdy 1.5.7, vendored

[Markdy](https://markdy.com) (MIT, `LICENSE` here) draws the report's figures.
`markdy-1.5.7.iife.min.js` is one IIFE (global `Markdy`) built from
`@markdy/core@1.5.7` and `@markdy/renderer-dom@1.5.7`, because the npm
packages ship ESM only and the CLI's HTML loads from a CDN. sha256
`43c8f93d5ab202f8d3e1f8dfec37c6a645e3d4cab7b60f89568c95c0f2a5ca17`.

```bash
mkdir -p /tmp/markdy-build
cd /tmp/markdy-build
npm init -y
npm i @markdy/core@1.5.7 @markdy/renderer-dom@1.5.7 esbuild
printf 'export { createDiagram, exportDiagramAsVectorSvg } from "@markdy/renderer-dom";\n' > entry.js
npx esbuild entry.js --bundle --minify --format=iife --global-name=Markdy --platform=browser --outfile=markdy.iife.min.js
```

What report-press relies on, verified in headless Chromium from `file://` on
2026-10-28:

| Call | Behaviour in 1.5.7 |
|---|---|
| `exportDiagramAsVectorSvg(code)` | **synchronous**, returns an SVG string whose `viewBox` gives the diagram's natural size |
| `createDiagram({container, code, autoplay, loop, fit: true, fitMode: "contain"})` | animated player; without `fit` it pans the camera onto the active node and a narrow figure never shows the whole diagram |
| scene types | `state`, `architecture` (with `group`) |
| node kinds | the `VERIFIED_KINDS` list in `src/lint.rs` |

A left-to-right diagram's width grows with its ranks and its label lengths:
4 ranks measured 1,264 px with short labels and 1,504 px with 25-character
ones; 8 ranks measured 2,480 px. About 1,430 px fits a 1,180 px page at 0.75
scale; `report-press check` warns above 4 ranks.

Upgrade deliberately: the grammar changes often. Rebuild the bundle, then
rebuild `skills/report-press/examples/*.md` and look at every figure.
