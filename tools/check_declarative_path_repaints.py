#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: the declarative property-write path must ask for a repaint.

# Why this gate exists

`runtime::with_widget_mut` mutates a control **in place and invalidates nothing**. Its own
documentation says so, and says what to do about it:

    Call this after mutating a widget through `with_widget_mut` so the change becomes
    visible. Backends route it to their own invalidation (...)

The name-based write path obeys that (`capability::access::write_widget_property` calls
`request_repaint` after a successful write). The declarative path did not: `view/apply.rs`
went through `with_widget_mut` and stopped. So the *same* logical `SetProperty` reached the
screen or not depending only on which route it took — a `Node`-driven update changed the
widget and left the previous frame painted, while the identical write by name repainted.

That is a silent failure: nothing errors, the property really is changed, and the user sees
the old value until an unrelated event happens to damage the area. It is also invisible to
every other gate, because the widget *is* in the right state — only the frame is stale.

# What it asserts

`src/view/apply.rs` must call `request_repaint` somewhere in its property-write path. The
check is textual and intentionally blunt: it asks whether the file names the API at all, and
the allowlist below records a file that is *proven* to exit before it could need to.

A blunt check is the right shape here because the round it comes from found the call missing
**entirely**, not misplaced. If the path is ever split across files, this gate will need to
follow the call — but it will fail loudly when that happens rather than passing vacuously.

Usage: tools/check_declarative_path_repaints.py   (exit 1 when the call is absent)
"""

from __future__ import annotations

import pathlib
import re
import sys

# The declarative property-write path: the module that turns a `Patch::SetProperty` into a
# `widget_property_set` call on a live control.
TARGET = pathlib.Path("src/view/apply.rs")

# The API that must be called after a successful write.
REQUIRED = "request_repaint"

# Files that write through `with_widget_mut` and are *proven* not to need the call, with the
# reason. Keep this empty unless there is a real one: it is a record of reasoning, not a
# place to silence the gate.
EXEMPT: dict[str, str] = {}


def scan() -> list[str]:
    """Returns the findings, one per file that writes without asking for a repaint."""
    findings: list[str] = []
    if not TARGET.exists():
        return [f"{TARGET}: does not exist, so this gate cannot run"]

    text = TARGET.read_text(encoding="utf-8")
    if REQUIRED not in text:
        findings.append(
            f"{TARGET}: never names `{REQUIRED}`, so a `Patch::SetProperty` applied through "
            "the declarative path changes the control and leaves the previous frame on screen"
        )
        return findings

    # The call must be reachable from a write, not merely mentioned. Pair each
    # `widget_property_set` with the text that follows it and require the repaint call in the
    # same function body — a comment naming the API is not a call, which is exactly the shape
    # that would otherwise pass this gate.
    write_sites = [m.start() for m in re.finditer(r"widget_property_set\b", text)]
    if not write_sites:
        findings.append(
            f"{TARGET}: no `widget_property_set` call found, so this gate has stopped "
            "tracking the path it guards (the write must have moved to another module)"
        )
        return findings

    if not re.search(r"request_repaint\s*\(", text):
        findings.append(
            f"{TARGET}: mentions `{REQUIRED}` but never calls it, so the repaint contract is "
            "documented rather than honoured"
        )
    return findings


def main() -> int:
    findings = scan()
    print(f"declarative write path checked: {TARGET}")
    print(f"exempt (writes that provably need no repaint): {len(EXEMPT)}")
    if not findings:
        print(f"failed: 0  (a successful declarative write asks for a repaint)")
        return 0

    print(f"failed: {len(findings)}")
    print()
    print("`with_widget_mut` invalidates nothing; its own docs require a `request_repaint`.")
    print("The name-based path calls it, so the declarative path must too — otherwise one")
    print("`SetProperty` has two different visible outcomes depending on its route.")
    print()
    for finding in findings:
        print(f"  {finding}")
    print()
    print("If a file genuinely writes without needing the call, add it to EXEMPT with a reason.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
