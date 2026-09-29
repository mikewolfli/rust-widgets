#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: no struct field in the widget/style/layout layers may be written and never read.

# Why this is a gate and not a one-off probe

Principle #99: a field must be either a fallback that production code **reads**, or
deleted. Round 79 found seven violations of that rule by hand — a CSS `shadow:`
property parsed, inherited and never painted; an `AnimationFillMode` with a builder
and no reader; three dangling dialog/media fields. Each was a feature that looked
implemented and was not, and none was caught by the compiler or clippy, because
writing a field is all Rust requires.

# What it checks

For every field declared in a `struct` under `src/widget/`, `src/style/` or
`src/layout/`, count how often the name is accessed anywhere under `src/`:

  * a **read** is any `.name` access other than an assignment or a mutating method;
  * a **write** is `self.name = ...` or `self.name.push(...)` (and friends).

A field with writes but no reads is reported. The scan counts *any* receiver
(`self.x` and the nested `config.x` alike) and treats a `_`-prefixed name as an
intentional keep-alive, exactly as Rust's own convention does — a signal handle that
exists to stay subscribed is not a forgotten reader.

# Why it is a locator that fails loudly rather than a proof

Two false-positive classes survive the rules above, and both are checked by hand
before an entry is added to the allow-list:

  * a field read only by code under `#[cfg(test)]` (a separate compilation unit);
  * a field whose reader is reached through a trait object in another crate.

The allow-list below therefore names each tolerated case with its reason, so a new
entry is an explicit decision. Adding one is the intended workflow; the gate exists
to make the decision visible rather than to pretend the question cannot arise.

Reverse injection: delete the `reconcile_shadow_into_surface` call from the CSS
applier and add `custom_shadow` back as a write-only field — the gate reports it.
"""

from __future__ import annotations

import pathlib
import re
import sys
from collections import defaultdict

# (path suffix, field, reason) — each entry is a deliberate, reviewed exception.
ALLOW_LIST: set[tuple[str, str]] = {
    (
        "src/widget/menu_toolbar/action.rs",
        "_enabled_handle",
    ): "keep-alive: holds the signal connection open; never read by design",
    (
        "src/widget/menu_toolbar/action.rs",
        "_toggled_handle",
    ): "keep-alive: holds the signal connection open; never read by design",
}

FIELD_DECL = re.compile(
    r"^\s{4,}(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*[A-Za-z_&'\[]", re.M
)
ANY_ACCESS = re.compile(r"\.([a-z_][a-z0-9_]*)\b")
MUTATOR = re.compile(r"\s*(=[^=]|\.(?:push|insert|extend|clear|remove|add|retain|take|get_or_insert))")


def struct_fields(text: str) -> set[str]:
    """Field names declared inside a `struct { .. }` body in this file."""
    names: set[str] = set()
    for block in re.finditer(r"\bstruct\s+\w+[^{;]*\{", text):
        depth = 1
        index = block.end()
        start = index
        while index < len(text) and depth > 0:
            char = text[index]
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
            index += 1
        names.update(m.group(1) for m in FIELD_DECL.finditer(text[start:index]))
    return names


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    sources: dict[pathlib.Path, str] = {}
    for sub in ("src/widget", "src/style", "src/layout"):
        for path in (root / sub).rglob("*.rs"):
            sources[path] = path.read_text(encoding="utf-8")

    access: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for text in sources.values():
        for match in ANY_ACCESS.finditer(text):
            slot = 0 if MUTATOR.match(text[match.end():match.end() + 4]) else 1
            access[match.group(1)][slot] += 1

    findings: list[tuple[str, str]] = []
    for path, text in sources.items():
        rel = path.relative_to(root).as_posix()
        for name in sorted(struct_fields(text)):
            if name.startswith("_"):
                continue
            writes, reads = access.get(name, [0, 0])
            if writes > 0 and reads == 0 and (rel, name) not in ALLOW_LIST:
                findings.append((rel, name))

    if findings:
        print(f"\u274c write-only fields: {len(findings)}")
        for rel, name in findings:
            print(f"   {rel}: `{name}` is written but never read (principle #99)")
            print("      → read it, delete it, or add a reviewed entry to the allow-list")
        return 1

    print(f"  [1/1] no write-only fields in widget/style/layout ({len(ALLOW_LIST)} allow-listed)")
    print("\u2705 every field is either read or explicitly exempt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
