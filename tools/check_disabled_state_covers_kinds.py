#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Every registered control carries a generated `disabled` state key.

# Why this gate exists

`preset_state_overrides` used to name nine controls by hand. A control registered later had no
`"<kind>:disabled"` key, so disabling it changed only whether it accepted input — the exact
defect W5-03 closed per control. The framework extension makes the list generated from the
registry's own export (`docs/capability_schema.json`); this gate keeps the two in step.

# What it asserts

  [1] the schema exports canonical control names;
  [2] `DISABLED_STATE_KINDS` in `src/theme/preset_states.rs` contains every exported name;
  [3] it contains no name the schema does not export (a stale hand edit).

Usage: tools/check_disabled_state_covers_kinds.sh
Exit 0 = the generated list matches the registry export.
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

SCHEMA = pathlib.Path("docs/capability_schema.json")
PRESETS = pathlib.Path("src/theme/preset_states.rs")
LIST = re.compile(
    r"DISABLED_STATE_KINDS\s*:\s*&\[&str\]\s*=\s*&\[(.*?)\];", re.S
)
NAME = re.compile(r'"([a-z][a-z0-9_]*)"')


def self_test() -> list[str]:
    sample = 'pub(super) const DISABLED_STATE_KINDS: &[&str] = &["a_b", "c"];'
    hit = LIST.search(sample)
    if not hit or NAME.findall(hit.group(1)) != ["a_b", "c"]:
        return ["self-test: the list pattern does not read a real declaration"]
    return []


def main() -> int:
    problems = self_test()
    if problems:
        for problem in problems:
            print(f"FAIL: {problem}")
        return 1

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    controls = {entry["control"] for entry in schema if entry.get("control")}
    text = PRESETS.read_text(encoding="utf-8")
    match = LIST.search(text)
    if not match:
        print(f"FAIL: {PRESETS} no longer declares DISABLED_STATE_KINDS")
        return 1
    listed = set(NAME.findall(match.group(1)))

    missing = sorted(controls - listed)
    stale = sorted(listed - controls)
    for name in missing:
        print(f"{PRESETS}: registered control `{name}` has no disabled state key")
    for name in stale:
        print(f"{PRESETS}: `{name}` is listed but the registry does not export it")
    print(f"checked={len(controls)} listed={len(listed)} missing={len(missing)} stale={len(stale)}")
    if missing or stale:
        print("  Regenerate the list from docs/capability_schema.json.")
        return 1
    print("✅ every registered control carries a disabled state key")
    return 0


if __name__ == "__main__":
    sys.exit(main())
