#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Locates setter methods that assign to a field without asking for a redraw.

# What this is, and what it is not

This is a **locator, not a gate**. It was 233 hits on its first run and 75 after the two
corrections below, and the surviving hits still need a person to read them: "did not call
request_redraw" is a fact, while "should have" depends on whether the field is read by `draw`,
whether a delegate down the chain redraws, and whether the value is *supposed* to be invisible.

Marking it a gate would have two failure modes, and both have already happened on this project:

  * a gate nobody can satisfy is a gate nobody runs (BLUE22 §F.3 lesson 4);
  * a gate that is satisfied by weakening the code it protects measures nothing.

# The two scope corrections, both of which are the same lesson

1. **Only real controls.** A method on `impl Date` cannot redraw anything: `Date` is a value, not
   a widget. Requiring the `impl` block's type to hold a `BaseWidget` field took 233 -> ~90, the
   same "the measuring range decides the conclusion" defect the round-77 notes record.
2. **One hop of delegation.** `set_date` is `self.set_datetime(..)` in its entirety; the redraw
   lives in the delegate. Following one hop to the real implementation removes the false
   positives that made the list unusable.

Usage: tools/probe_setters_without_repaint.py [--verbose]
Exit 0 always -- this is a locator. The findings are printed for a person to rule on.
"""

from __future__ import annotations

import pathlib
import re
import sys
from typing import Dict, List, Set, Tuple

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "src"

# A field holding this type is what makes a struct a *control* rather than a value.
WIDGET_MARKER = "BaseWidget"

# A field that is a *handle* to shared state, or a size the control reads from elsewhere, rather
# than painted content of its own. Assigning one changes what the control can reach, not what it
# looks like, so no repaint is owed. Reviewed one by one -- these are not a dumping ground.
HANDLE_FIELDS = ("registry", "delegate", "target_widget", "content_size", "viewport")

FN_DECL = re.compile(r"^\s{4}(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z_][a-z0-9_]*)\s*[(<]")
ASSIGNS_FIELD = re.compile(r"self\s*\.\s*([a-z_][a-z0-9_]*)\s*(?:=[^=]|\+=|-=|\*=|/=)")
REDRAW = re.compile(r"request_redraw\s*\(")
DELEGATE = re.compile(r"^\s*self\s*\.\s*([a-z_][a-z0-9_]*)\s*\([^;]*\)\s*;\s*$")


REDRAW_HELPERS = (
    "request_redraw",
    "reapply_active_theme_state",
    "touch_activity",
    "announce_after",
    "sync_color_from_hsva",
    "set_scroll_position",
    "scroll_to",
    "ensure_visible",
    "update_geometry",
    "invalidate",
    "mark_dirty",
)

def strip_comments(line: str) -> str:
    """Drop a trailing `//` comment, so a doc sentence is not read as code."""
    out = []
    in_str = False
    i = 0
    while i < len(line):
        ch = line[i]
        if ch == '"' and (i == 0 or line[i - 1] != "\\"):
            in_str = not in_str
        if not in_str and line.startswith("//", i):
            break
        out.append(ch)
        i += 1
    return "".join(out)


def brace_block(lines: List[str], start: int) -> Tuple[str, int]:
    """The body of a `fn` starting at `start`, by brace balance."""
    depth = 0
    seen = False
    body: List[str] = []
    index = start
    while index < len(lines):
        raw = strip_comments(lines[index])
        depth += raw.count("{") - raw.count("}")
        if "{" in raw:
            seen = True
        body.append(raw)
        if seen and depth <= 0:
            break
        index += 1
    return "\n".join(body), index


def struct_fields_holding(text: str, marker: str) -> Set[str]:
    """Field names whose declaration mentions `marker`."""
    names: Set[str] = set()
    for block in re.finditer(r"\bstruct\s+\w+[^{;]*\{", text):
        depth = 1
        index = block.end()
        churn = text[index:]
        for raw in churn.split("\n"):
            depth += raw.count("{") - raw.count("}")
            if depth <= 0:
                break
            decl = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*(.+)", raw)
            if decl and marker in decl.group(2):
                names.add(decl.group(1))
    return names


def control_types(text: str) -> Set[str]:
    """The names of the structs in `text` that hold a widget field.

    # Why the unit is the *type* and not the file

    A file commonly holds both: `date_edit.rs` declares the `DateEdit` control **and** the plain
    `Date` value it edits. Deciding per file let `Date::set_year` through, and a method on a value
    cannot repaint anything — that is the 233-hit scope error again, one level down. Naming the
    control structs lets each `impl` block be judged by the type it is for.
    """
    names: Set[str] = set()
    for block in re.finditer(r"\bstruct\s+(\w+)[^{;]*\{", text):
        depth = 1
        index = block.end()
        holds = False
        for raw in text[index:].split("\n"):
            depth += raw.count("{") - raw.count("}")
            if depth <= 0:
                break
            decl = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*(.+)", raw)
            if decl and WIDGET_MARKER in decl.group(2):
                holds = True
        if holds:
            names.add(block.group(1))
    return names


def setters_without_redraw(path: pathlib.Path) -> List[Tuple[str, str, str]]:
    """`(type, method, field)` for every assigning setter that never asks for a repaint."""
    text = path.read_text(encoding="utf-8")
    controls = control_types(text)
    if not controls:
        return []
    lines = text.split("\n")
    findings: List[Tuple[str, str, str]] = []
    current_type = ""

    for index, line in enumerate(lines):
        type_decl = re.match(r"\s*impl(?:<[^>]*>)?\s+(\w+)", line)
        if type_decl:
            current_type = type_decl.group(1)
        # Only a *control*'s inherent methods can repaint; a value type's `set_*` cannot, and
        # listing them was the scope error this file's docstring describes.
        if current_type not in controls:
            continue

        decl = FN_DECL.match(line)
        if not decl:
            continue
        name = decl.group(1)
        if not name.startswith("set_"):
            continue
        body, _ = brace_block(lines, index)
        if REDRAW.search(body):
            continue
        assigned = {m.group(1) for m in ASSIGNS_FIELD.finditer(body)}
        if not assigned:
            continue
        # One hop of delegation: a body that is only `self.other(..);` repaints inside `other`.
        delegate = None
        statements = [s.strip() for s in body.split(";") if s.strip() and not s.strip().startswith("fn ")]
        if len(statements) == 2:  # the signature line, then the single call
            call = DELEGATE.match(statements[1] + ";")
            if call:
                delegate = call.group(1)
        if delegate:
            if delegate_redraws(lines, delegate):
                continue
        # A helper that redraws *inside the body itself* is a redraw one call away, which is the
        # same fact as a delegate: the repaint happens, just not on this line. Counting it as a
        # finding is what made the first two runs report shapes that were already correct.
        if any(helper in body for helper in REDRAW_HELPERS):
            continue
        for field in sorted(assigned):
            if field in HANDLE_FIELDS:
                continue
            findings.append((current_type, name, field))
    return findings


def delegate_redraws(lines: List[str], method: str) -> bool:
    """Whether `method` itself, anywhere in the tree, asks for a repaint."""
    pattern = re.compile(r"^\s{4}(?:pub(?:\([^)]*\))?\s+)?fn\s+" + re.escape(method) + r"\s*[(<]")
    for index, line in enumerate(lines):
        if pattern.match(line):
            body, _ = brace_block(lines, index)
            if REDRAW.search(body):
                return True
    return False


def main() -> int:
    verbose = "--verbose" in sys.argv
    total = 0
    by_file: Dict[str, List[Tuple[str, str, str]]] = {}

    for path in sorted(SRC.rglob("*.rs")):
        found = setters_without_redraw(path)
        if found:
            by_file[str(path.relative_to(ROOT))] = found
            total += len(found)

    print(f"setters that assign a field without requesting a redraw: {total}")
    print("(each still needs a human ruling -- see this file's docstring)")
    print()
    for name, found in by_file.items():
        print(f"  {name}")
        for type_name, method, field in found:
            print(f"      {type_name}::{method}  ->  {field}")
    if verbose:
        print()
        print(f"scanned {len(list(SRC.rglob('*.rs')))} files under src/")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
