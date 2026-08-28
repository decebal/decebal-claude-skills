# Modification notice

This skill is adapted from
[`alirezarezvani/claude-code-aso-skill`](https://github.com/alirezarezvani/claude-code-aso-skill),
revision `94148561f173a917b45f8fd125e3025fa25cba85`.

Copyright (c) 2025 Alireza Rezvani. Used under MIT license in
[`LICENSE.txt`](LICENSE.txt).

Changes:

- consolidated upstream workflows for progressive Claude skill loading;
- replaced Python tooling with focused Rust metadata and experiment checks;
- corrected Google Play app-name limit to current 30 characters;
- enforced Apple's keyword limit as 100 UTF-8 bytes;
- added stable rule IDs, platform templates, locked dependencies, and boundary
  tests;
- removed sample performance claims and unsupported volume/ranking assumptions;
- composed `claude-seo` for evidence-led research instead of duplicating agents.
