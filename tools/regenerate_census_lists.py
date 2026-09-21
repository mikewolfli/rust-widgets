#!/usr/bin/env python3
"""Regenerate the KNOWN_* allowlists inside the census test from live output.

Run after changing a control's rendering: the lists are the *work list*, so a fix
must shrink them. Doing it by hand is exactly the manual step that drifts.
"""
import re
from pathlib import Path

inv = [l.strip() for l in open('/tmp/inv4.txt') if l.strip()]
tb = [l.strip() for l in open('/tmp/tb4.txt') if l.strip()]

p = Path('tests/control_rendering_census_test.rs')
s = p.read_text()


def block(names):
    return '\n'.join(f'    "{n}",' for n in names)


s = re.sub(
    r'(const KNOWN_INVISIBLE: &\[&str\] = &\[\n).*?(\n\];)',
    lambda m: m.group(1) + block(inv) + m.group(2),
    s, flags=re.S)
s = re.sub(
    r'(const KNOWN_THEME_BLIND: &\[&str\] = &\[\n).*?(\n\];)',
    lambda m: m.group(1) + block(tb) + m.group(2),
    s, flags=re.S)
p.write_text(s)
print(f'invisible={len(inv)} theme_blind={len(tb)}')
