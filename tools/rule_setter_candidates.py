#!/usr/bin/env python3
"""Ruling aid: for each candidate setter, is the assigned field *read by a draw path*?

A setter that assigns a field nothing paints does not need a repaint: there is nothing to repaint.
The interesting cases are the ones where the field **is** read by `draw` yet the setter is silent —
those are the real findings, because the change is invisible until something else forces a frame.
"""
from __future__ import annotations

import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "src"


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


def find_body(lines, start):
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


def draw_bodies(text: str) -> str:
    """Every `fn draw(..)` body in the file, concatenated."""
    lines = text.split("\n")
    out = []
    for i, line in enumerate(lines):
        if re.match(r"^\s{4}fn draw\s*\(", line):
            out.append(find_body(lines, i))
    return "\n".join(out)


def main() -> None:
    listing = subprocess.run(
        ["python3", str(ROOT / "tools/probe_setters_without_repaint.py")],
        capture_output=True, text=True, check=True,
    ).stdout.split("\n")

    current_file = None
    painted, unpainted = [], []
    for raw in listing:
        if raw.startswith("  src/"):
            current_file = raw.strip()
        elif raw.strip() and "::" in raw and "->" in raw:
            head, field = raw.split("->")
            type_name, method = head.strip().split("::")
            text = (ROOT / current_file).read_text(encoding="utf-8")
            drawn = draw_bodies(text)
            # `self.field` read *anywhere* in a draw body, including nested closures/blocks.
            read = re.search(r"self\s*\.\s*" + re.escape(field.strip()) + r"\b", drawn) is not None
            row = f"{current_file}  {type_name}::{method}  ->  {field.strip()}"
            (painted if read else unpainted).append(row)

    print(f"ASSIGNED AND READ BY A DRAW PATH  ({len(painted)})  <- these need a ruling")
    for row in painted:
        print("   " + row)
    print()
    print(f"ASSIGNED BUT NOT READ BY DRAW  ({len(unpainted)})  <- nothing to repaint")
    for row in unpainted:
        print("   " + row)


if __name__ == "__main__":
    main()
