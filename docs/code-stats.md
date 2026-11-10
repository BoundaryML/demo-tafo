# Maintained source size

Run `python3 scripts/code_stats.py > docs/code-stats.md` to refresh.

| Language / role | Files | Nonblank lines | KiB |
| --- | ---: | ---: | ---: |
| BAML (includes inline tests) | 21 | 3,027 | 101.5 |
| Rust application | 8 | 3,147 | 115.0 |
| Rust tests | 2 | 644 | 21.7 |
| JavaScript / JSX dashboard | 3 | 903 | 30.0 |
| JavaScript tests | 1 | 76 | 3.0 |
| TypeScript | 0 | 0 | 0.0 |
| CSS | 1 | 882 | 14.5 |
| Python tooling | 3 | 168 | 9.0 |
| Shell tooling | 4 | 85 | 3.1 |

Counts are physical nonblank lines, including comments. Formatting affects line
counts, so bytes are shown too. BAML tests live beside the policy they test. The
generated Rust SDK, dependencies, build output, docs, lockfiles, CI configuration,
and the embedded app-icon asset module are excluded. There is no TypeScript in
this version; the dashboard uses JavaScript/JSX. Every authored Rust function has
a documented reason in the boundary audit.
