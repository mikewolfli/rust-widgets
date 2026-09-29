#!/usr/bin/env python3
"""Probe: which `Event` variants have no production construction site anywhere?

Read-only diagnostic used to size the fix; not a gate.
"""
from __future__ import annotations

import pathlib
import re
import sys

sys.path.insert(0, "tools")
from check_event_producers import (  # noqa: E402
    event_enum_variants,
    production_construction_sites,
    EVENT_FILE,
)

declared = event_enum_variants()
produced: dict[str, set[str]] = {}
for path in pathlib.Path("src").rglob("*.rs"):
    if path == EVENT_FILE:
        continue
    for variant in production_construction_sites(path):
        produced.setdefault(variant, set()).add(str(path))

# Also count real construction sites inside types.rs itself, minus the enum decl
# and the constructors helper impl.
print(f"declared variants: {len(declared)}")
orphans = sorted(v for v in declared if v not in produced)
print(f"variants with NO production site outside types.rs: {len(orphans)}")
for v in orphans:
    print(f"  {v}")

print()
print("variants produced only by their own constructor in types.rs:")
text = re.sub(r"#\[cfg\(test\)\][\s\S]*$", "", EVENT_FILE.read_text())
self_made: set[str] = set()
for m in re.finditer(r"Event::(\w+)\s*\{", text):
    self_made.add(m.group(1))
for v in sorted(declared):
    if v not in produced and v in self_made:
        print(f"  {v}  (constructor only)")
