#!/usr/bin/env python3
"""Report maintained source size without generated SDKs, dependencies, or assets."""
from pathlib import Path

root = Path(__file__).resolve().parent.parent
rows = [
    ('BAML (includes inline tests)', sorted((root / 'baml_src').rglob('*.baml'))),
    ('Rust application', sorted((root / 'src').rglob('*.rs'))),
    ('Rust tests', sorted((root / 'tests').rglob('*.rs'))),
    ('JavaScript / JSX dashboard', [root / 'dashboard' / name for name in ['app.jsx', 'metrics.mjs', 'build.mjs']]),
    ('JavaScript tests', [root / 'dashboard/metrics.test.mjs']),
    ('TypeScript', []),
    ('CSS', [root / 'dashboard/style.css']),
    ('Python tooling', sorted((root / 'scripts').glob('*.py'))),
    ('Shell tooling', sorted((root / 'scripts').glob('*.sh')) + [root / 'switch-app']),
]
print('# Maintained source size\n')
print('Run `python3 scripts/code_stats.py > docs/code-stats.md` to refresh.\n')
print('| Language / role | Files | Nonblank lines | KiB |')
print('| --- | ---: | ---: | ---: |')
for label, paths in rows:
    lines = sum(sum(bool(line.strip()) for line in path.read_text().splitlines()) for path in paths)
    size = sum(path.stat().st_size for path in paths) / 1024
    print(f'| {label} | {len(paths)} | {lines:,} | {size:.1f} |')
print('\nCounts are physical nonblank lines, including comments. Formatting affects line')
print('counts, so bytes are shown too. BAML tests live beside the policy they test. The')
print('generated Rust SDK, dependencies, build output, docs, lockfiles, CI configuration,')
print('and the embedded app-icon asset module are excluded. There is no TypeScript in')
print('this version; the dashboard uses JavaScript/JSX. Every authored Rust function has')
print('a documented reason in the boundary audit.')
