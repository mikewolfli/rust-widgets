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

# A struct-literal initializer: `field: value` inside a `Braces` expression. This is the shape the
# gate used to miss entirely, and it is the **most common** way a field is first written —
# `Self { field: .. }`, `WidgetStyle { field: .. }`. Classifying only `.field` accesses counted such
# a write as zero, so a field written *only* in initializers looked like it had no writes at all and
# the "written but never read" rule never fired on it. A faithful probe (declare + initialise +
# never read) passed the gate before this pattern existed.
#
# It requires the field to be followed by `:` and then something that is not `:` (so a type
# annotation or a `::` path is not a write) and not `=` (so `field == x` is not a write).
INIT_FIELD = re.compile(r"\b([a-z_][a-z0-9_]*)\s*:\s*[^:=]")

# A shorthand initializer: `Self { field }` / `Self { .., field }`, i.e. the field named alone. That
# is both a write (into the new value) and a read (from the enclosing scope), so it cannot make a
# field "write-only" on its own; it is recorded as a **read** so an otherwise-unread field that is
# merely forwarded through shorthand is not reported as write-only.
INIT_SHORTHAND = re.compile(r"(?<![.\w])([a-z_][a-z0-9_]*)\s*[,}]")


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
    # The fields this gate owns. A declaration here is what makes a field *subject* to the rule.
    owned: dict[pathlib.Path, str] = {}
    for sub in ("src/widget", "src/style", "src/layout"):
        for path in (root / sub).rglob("*.rs"):
            owned[path] = path.read_text(encoding="utf-8")

    # Every `.rs` file, for the **access** tally. This is deliberately wider than `owned`.
    #
    # A field declared in this layer is routinely read by a consumer that lives outside it —
    # `Gradient::start_point` is written in `src/style/gradient.rs` and read in
    # `src/render/svg/backend.rs`. Counting accesses only inside `owned` reported that field as
    # write-only, which is a false positive: the reader exists, it is simply one layer down. The
    # rule being enforced is "written but never read", and "never" has to include the code that
    # does not declare the field too.
    all_sources: list[str] = [
        path.read_text(encoding="utf-8") for path in (root / "src").rglob("*.rs")
    ]

    access: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for text in all_sources:
        for match in ANY_ACCESS.finditer(text):
            slot = 0 if MUTATOR.match(text[match.end():match.end() + 4]) else 1
            access[match.group(1)][slot] += 1
        # Struct-literal initializers count as writes; see `INIT_FIELD` for why this had to be
        # added and what it cost when it was missing.
        for match in INIT_FIELD.finditer(text):
            access[match.group(1)][0] += 1
        # Shorthand initializers count as reads (the value is taken from a binding in scope).
        for match in INIT_SHORTHAND.finditer(text):
            access[match.group(1)][1] += 1

    findings: list[tuple[str, str]] = []
    for path, text in owned.items():
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
