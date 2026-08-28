# Rust SEO category design

Date: 2026-08-28

## Goal

Add two directly installable Claude skills under one documented **SEO** category:

- `claude-seo`: evidence-led SEO workflow backed by a safe Rust audit CLI.
- `aso-lint`: App Store Optimization workflow backed by a deterministic Rust metadata and experiment linter.

Skill directories remain flat under `skills/` because Claude discovers direct skill folders. The SEO category exists in the repository inventory and README, not as an extra filesystem nesting level.

## Source and migration boundary

`claude-seo` is a Rust-first adaptation of AgriciDaniel/claude-seo at commit `a1480c7e590b16001bd9dc1627eacdcd44d580f9` (MIT). That upstream contains 25 skills and 53 Python scripts (21,102 lines). This migration consolidates the workflow into one skill and ports its deterministic local audit core to Rust. Credentialed Google, DataForSEO, Moz, Bing, maps, and image-generation integrations remain evidence inputs: Claude must use available tools or user-provided exports and label missing data `setup required`. It must never invent API output.

`aso-lint` is a Rust-first adaptation of alirezarezvani/claude-code-aso-skill at commit `94148561f173a917b45f8fd125e3025fa25cba85` (MIT). It keeps strategy in `SKILL.md` and moves repeatable validation and experiment math into Rust.

Each adapted skill carries its upstream license and notice. Root `THIRD_PARTY_NOTICES.md` records both sources.

## `claude-seo` architecture

`skills/claude-seo/scripts/` is an independent Cargo workspace producing binary `claude-seo`.

Commands:

- `audit`: read local HTML or fetch one public HTTP(S) page, then emit structured JSON or Markdown.
- `sitemap`: validate a local or public XML sitemap and report URL/count/shape findings.
- `drift`: compare two saved audit JSON files and report high-risk field changes.
- `doctor`: print runtime capability and integration setup state without probing credentials.

Audit evidence includes status/final URL, title, description, canonical, robots directives, language, headings, links, images/alt text, word count, Open Graph, Twitter metadata, JSON-LD types, and hreflang. Findings include severity, observed evidence, recommendation, and verification step. Scores cover only executed checks; output also reports coverage so a partial crawl cannot masquerade as a full SEO health score.

Network safety is default-deny:

- only `http` and `https`;
- reject URL credentials;
- resolve host and reject loopback, private, link-local, multicast, and unspecified IPs;
- disable automatic redirects, then revalidate every redirect target;
- cap redirects, response bytes, and request time;
- accept only expected HTML/XML content types.

No JavaScript rendering, recursive crawl, backlink index, ranking data, or search-console truth is inferred from one HTTP response. Skill workflow labels such evidence gaps explicitly and routes visual/rendered checks to browser tooling when available.

## `aso-lint` architecture

`skills/aso-lint/scripts/` is an independent Cargo workspace producing binary `aso-lint`.

Commands:

- `lint`: validate Apple App Store or Google Play listing JSON against current official field constraints.
- `experiment`: compute a two-proportion z-test for control and variant conversion counts.
- `template`: emit platform-specific starter JSON.

Validation reports stable rule IDs, severity, field, observed value/length, limit, and repair advice. Apple keyword limits use UTF-8 bytes. Character-limited fields use Unicode scalar counts and explain that store-side rendering remains final authority. Store constraints live in one Rust rules module and are paired with official-source URLs in skill references.

## Quality gates

Both Rust workspaces:

- commit `Cargo.lock`;
- inherit workspace lint policy with Clippy `all` and `pedantic`, `unsafe_code = deny`, and broken rustdoc links denied;
- pass `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`;
- include fixture-driven CLI smoke tests in repository `tests/`;
- run in `test-skills` CI with Rust cache support.

Existing gate workspace remains separate. New skill runtimes are tested through their own manifests, avoiding dependency pollution of `gates/rust`.

## Documentation and release

README and `skills-guide/overview.md` gain an SEO category containing exactly `claude-seo` and `aso-lint`. README explains executable Rust commands, coverage boundaries, and install examples. `CLAUDE.md` changes from documentation-only wording to documentation plus bundled Rust tooling.

Completion requires Chronis tasks closed, license audit, skill-package validation, both Rust gate suites, repository skill tests, and an install-from-archive smoke test.
