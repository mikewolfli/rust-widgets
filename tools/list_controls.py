#!/usr/bin/env python3
"""List every registered control, grouped, so a gap can be argued from a list rather than recalled."""
import re

s = open('src/widget/capability/properties.rs').read()
names = sorted(set(re.findall(r'canonical_name:\s*"([^"]+)"', s)))
print("total:", len(names))

groups = {}
for n in names:
    groups.setdefault(n.split('_')[0], []).append(n)

for k in sorted(groups):
    print(f"{k:20} {len(groups[k]):3}  {', '.join(groups[k])}")
