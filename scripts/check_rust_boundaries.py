#!/usr/bin/env python3
"""Require an explicit host-boundary rationale for every Rust function/ABI declaration."""
import json
from pathlib import Path
import re
import sys

root = Path(__file__).resolve().parent.parent
manifest = json.loads((root / 'docs/rust-boundaries.json').read_text())
pattern = re.compile(r'\bfn\s+([A-Za-z_]\w*)\s*(?:<[^{};]*>)?\s*\(')
actual = {
    str(path.relative_to(root)): set(pattern.findall(path.read_text()))
    for path in sorted((root / 'src').rglob('*.rs'))
}
documented = {}
for file, groups in manifest.items():
    documented[file] = set()
    for group in groups:
        if len(group['reason'].strip()) < 30:
            sys.exit(f'Missing substantive reason in {file}: {group["functions"]}')
        for name in group['functions']:
            if name in documented[file]:
                sys.exit(f'Duplicate rationale: {file}:{name}')
            documented[file].add(name)
errors = []
for file in sorted(actual.keys() | documented.keys()):
    missing = actual.get(file, set()) - documented.get(file, set())
    stale = documented.get(file, set()) - actual.get(file, set())
    errors.extend(f'Undocumented Rust function: {file}:{name}' for name in sorted(missing))
    errors.extend(f'Stale Rust rationale: {file}:{name}' for name in sorted(stale))
if errors:
    sys.exit('\n'.join(errors))
print(f'Rust boundary audit passed: {sum(map(len, actual.values()))} functions/ABI declarations documented.')
