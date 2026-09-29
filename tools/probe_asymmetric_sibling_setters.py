#!/usr/bin/env python3
"""Iceberg scan: setters that are *siblings* of a repainting setter but do not repaint themselves.

`tab_bar` had `set_tab_min_width` (repaints) beside `set_tab_max_width` (silent). The two are the
same fact from either end, so the omission is visible as an asymmetry inside one impl block rather
than as a missing line in isolation. That is a much stronger signal than "does not call
request_redraw", which is true of many correct methods.

This looks for exactly that shape: within one `impl <Control>` block, a `set_X` that repaints and a
`set_Y` that does not, where `Y` differs from `X` only by an antonym or a suffix pair.
"""
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "src"

PAIRS = [
    ("min", "max"),
    ("minimum", "maximum"),
    ("top", "bottom"),
    ("left", "right"),
    ("start", "end"),
    ("horizontal", "vertical"),
    ("show", "hide"),
    ("enable", "disable"),
    ("enabled", "disabled"),
    ("visible", "hidden"),
]


def strip_comments(line: str) -> str:
    out, in_str, i = [], False, 0
    while i < len(line):
        ch = line[i]
        if ch == '"' and (i == 0 or line[i - 1] != "\\"):
            in_str = not in_str
        if not in_str and line.startswith("//", i):
            break
        out.append(ch)
        i += 1
    return "".join(out)


def body_of(lines, start):
    depth, seen, body, i = 0, False, [], start
    while i < len(lines):
        raw = strip_comments(lines[i])
        depth += raw.count("{") - raw.count("}")
        if "{" in raw:
            seen = True
        body.append(raw)
        if seen and depth <= 0:
            break
        i += 1
    return "\n".join(body)


def main() -> None:
    findings = []
    for path in sorted(SRC.rglob("*.rs")):
        lines = path.read_text(encoding="utf-8").split("\n")
        # Group setters by impl block.
        blocks = []
        current = None
        for i, line in enumerate(lines):
            m = re.match(r"\s*impl(?:<[^>]*>)?\s+(\w+)", line)
            if m:
                current = (m.group(1), [])
                blocks.append(current)
                continue
            d = re.match(r"^\s{4}(?:pub(?:\([^)]*\))?\s+)?fn\s+(set_[a-z0-9_]+)\s*[(<]", line)
            if d and current is not None:
                current[1].append((d.group(1), i))

        for type_name, setters in blocks:
            by_name = {n: i for n, i in setters}
            for name, index in setters:
                redraws = "request_redraw" in body_of(lines, index)
                for a, b in PAIRS:
                    for other in (
                        name.replace(a, b) if a in name else None,
                        name.replace(b, a) if b in name else None,
                    ):
                        if not other or other not in by_name:
                            continue
                        other_redraws = "request_redraw" in body_of(lines, by_name[other])
                        if redraws and not other_redraws:
                            findings.append(
                                f"{path.relative_to(ROOT)}  {type_name}::{other} is silent "
                                f"while {name} repaints"
                            )
    seen = set()
    for row in findings:
        if row in seen:
            continue
        seen.add(row)
        print(row)
    print()
    print(f"{len(seen)} asymmetric sibling pair(s)")


if __name__ == "__main__":
    main()
