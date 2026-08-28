# Upstream migration ledger

Source: `AgriciDaniel/claude-seo` revision
`a1480c7e590b16001bd9dc1627eacdcd44d580f9` (v2.2.5). This repository
consolidates upstream's 25 skills and replaces its 53 Python scripts with one
bounded Rust core plus evidence/tool routing.

Status meanings:

- **Rust core**: deterministic local behavior exists in `scripts/`.
- **Workflow**: Claude performs evidence-led analysis using available sources.
- **Setup required**: result needs credentials, external service, browser, or
  user export; no result is synthesized.

| Upstream capability | Migration | Notes |
|---|---|---|
| `seo` orchestration | Workflow | Consolidated in `SKILL.md` and playbooks |
| `seo-audit` | Rust core + workflow | Static page evidence plus full audit routing |
| `seo-page` | Rust core + workflow | Single-page HTML audit |
| `seo-technical` | Rust core + workflow | Static/status subset; crawl/render/index APIs need evidence |
| `seo-content` | Rust core + workflow | Extracted text/headings plus human quality review |
| `seo-schema` | Rust core + workflow | Extracts JSON-LD types; eligibility/content validation remains contextual |
| `seo-sitemap` | Rust core + workflow | XML shape, URL syntax, duplicates, count; fetch/index truth separate |
| `seo-hreflang` | Rust core + workflow | Page declarations extracted; reciprocal graph needs crawl/export |
| `seo-drift` | Rust core + workflow | High-risk audit JSON comparison |
| `seo-images` | Rust core + workflow | Missing-alt evidence; format/size/render review needs files/browser |
| `seo-performance` | Setup required | PageSpeed, CrUX, Lighthouse, or RUM evidence |
| `seo-google` | Setup required | GSC, GA4, URL Inspection, Indexing API, Keyword Planner |
| `seo-dataforseo` | Setup required | DataForSEO credentials and explicit cost consent |
| `seo-backlinks` | Setup required | Moz/Common Crawl/Bing/export plus verification |
| `seo-firecrawl` | Setup required | External crawler integration |
| `seo-visual` | Setup required | Browser/screenshot tooling |
| `seo-image-gen` | Setup required | Image generation tooling and rights/provenance review |
| `seo-maps` | Setup required | Maps/GBP/geo-grid provider evidence |
| `seo-local` | Workflow + setup required | Site/entity review; GBP/rank/reviews need data |
| `seo-ecommerce` | Workflow + setup required | HTML/schema subset; merchant/feed data needs integration |
| `seo-cluster` | Workflow + setup required | Requires frozen live SERP/query evidence |
| `seo-competitor-pages` | Workflow + setup required | Requires current sourced competitor facts |
| `seo-plan` | Workflow | Evidence-based roadmap and KPI definitions |
| `seo-programmatic` | Workflow | Template sampling and rollout gates |
| `seo-sxo` | Workflow + setup required | Page experience plus search/analytics/conversion evidence |
| `seo-geo` | Workflow + setup required | Passage citability plus observed AI-platform testing |
| `seo-flow` | Workflow | Evidence labels and verification-first reporting |

## Intentional removals

- No autonomous indexing or IndexNow submission: external mutation requires
  explicit user scope and credentials.
- No PDF/XLSX report generation in skill runtime: structured JSON/Markdown stays
  composable and keeps dependency surface small.
- No bundled browser/JavaScript runtime: use installed browser skill/tool.
- No generated API or rank values: unavailable evidence remains unavailable.
- No recursive crawler in v1: bounded single-page fetch prevents accidental
  load and keeps SSRF controls auditable.
