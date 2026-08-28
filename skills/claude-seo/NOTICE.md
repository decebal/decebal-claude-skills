# Modification notice

This skill is adapted from
[`AgriciDaniel/claude-seo`](https://github.com/AgriciDaniel/claude-seo), revision
`a1480c7e590b16001bd9dc1627eacdcd44d580f9` (v2.2.5).

Copyright (c) 2026 agricidaniel. Used under MIT license in
[`LICENSE.txt`](LICENSE.txt).

Changes:

- consolidated 25 upstream skills into one progressive Claude workflow;
- replaced 53 Python scripts with a focused Rust static audit runtime;
- added default-deny URL validation, DNS pinning, redirect revalidation, byte
  limits, and timeouts;
- separated observed, inferred, and setup-required evidence;
- removed autonomous external mutations and fabricated integration fallback;
- documented capability parity and intentional omissions.
