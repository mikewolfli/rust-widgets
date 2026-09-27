#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Dry-run tool: find the "the control's own face" block in a widget's `draw`.

A control's own face is the fill+stroke pair whose rectangle is the control's
**own** rectangle (`rect` / `geom`), not an inner decoration. Only that pair is
what `SurfaceStyle::paint` replaces; an inner swatch, a tab indicator or a
letterbox bar must keep its own drawing.

This reports what it would replace and what it cannot, so the migration is a
reviewed list rather than a hope. It never writes.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent


def scan(path: pathlib.Path) -> tuple[int, int, int]:
    """Returns (fills of the control's own rect, other fills, strokes of the own rect)."""
    text = path.read_text(encoding="utf-8")
    # Strip the test module: fixtures are not production drawing.
    marker = text.find("\n#[cfg(test)]")
    body = text[:marker] if marker != -1 else text
    if "impl Draw" not in body:
        return (0, 0, 0)
    # A fill/stroke on the control's **own** rectangle: the target is `rect`, or
    # `Rect::new(rect.x, rect.y, rect.width, rect.height)`.
    own = len(re.findall(r"fill_rect\(\s*Rect::new\(\s*rect\.x, rect\.y, rect\.width, rect\.height\s*\)", body))
    own += len(re.findall(r"fill_rect\(rect,", body))
    all_fills = len(re.findall(r"fill_rect\(", body)) + len(re.findall(r"fill_rounded_rect\(", body))
    strokes = len(re.findall(r"draw_rect_stroke\(\s*(?:Rect::new\(\s*rect\.x|rect,)", body))
    return (own, all_fills - own, strokes)


def main(argv: list[str]) -> int:
    files = sorted((ROOT / "src" / "widget").rglob("*.rs"))
    print(f"{'file':60} own other strokes  verdict")
    for path in files:
        rel = str(path.relative_to(ROOT))
        own, other, strokes = scan(path)
        if own == 0:
            continue
        verdict = "SIMPLE" if own == 1 else "REVIEW"
        print(f"{rel:60} {own:3} {other:5} {strokes:7}  {verdict}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
