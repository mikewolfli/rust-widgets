#!/usr/bin/env python3
"""List every registered control, grouped, so a gap can be argued from a list rather than recalled."""
import pathlib
import re

PROPERTIES_DIR = pathlib.Path('src/widget/capability/properties')
PROPERTIES = PROPERTIES_DIR / 'mod.rs'
children = sorted(path for path in PROPERTIES_DIR.glob('*.rs') if path != PROPERTIES)
s = "\n".join(
    [PROPERTIES.read_text(encoding='utf-8')]
    + [path.read_text(encoding='utf-8') for path in children]
)
names = sorted(set(re.findall(r'canonical_name:\s*"([^"]+)"', s)))
print("total:", len(names))

groups = {}
for n in names:
    groups.setdefault(n.split('_')[0], []).append(n)

for k in sorted(groups):
    print(f"{k:20} {len(groups[k]):3}  {', '.join(groups[k])}")
