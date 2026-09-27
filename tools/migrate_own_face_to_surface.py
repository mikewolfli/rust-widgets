#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE24 §10A A1: route every control's own face through `RenderContext::face`.

# What it replaces, and what it refuses to

A control's **own face** is the fill whose target is the control's own rectangle --
`fill_rect(rect, X)` or `fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), X)`.
Only that call is replaced, with `context.face(rect, X, style, Color::BLACK)`; every other
`fill_rect` (an inner swatch, a tab bar, a viewfinder overlay) is left alone, because those
are not the control's face and must keep their own drawing.

A file is migrated only when it has **exactly one** such call. A file with several candidates
is reported and skipped: two faces in one `draw` need a human to say which is the control's
own, and guessing would silently repaint an inner element.

# Why the outline is left alone

The border is frequently a semantic colour (a validation error, a checked state) that lives in
the control, not in the face. `face` draws the shadow and the fill; the control keeps its own
`draw_rect_stroke`. That is also what keeps this migration a **radius-and-depth** change and
nothing else.

# Usage

    python3 tools/migrate_own_face_to_surface.py --dry-run   # the review list
    python3 tools/migrate_own_face_to_surface.py --apply
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
WIDGET = ROOT / "src" / "widget"

# `context.fill_rect(rect, <expr>);`  -- the common spelling.
FILL_RECT_SHORT = re.compile(r"(?P<indent>[ \t]*)context\.fill_rect\(rect,\s*(?P<fill>[^\n]+?)\);")
# `context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), <expr>);`
FILL_RECT_LONG = re.compile(
    r"(?P<indent>[ \t]*)context\.fill_rect\(\s*"
    r"Rect::new\(rect\.x,\s*rect\.y,\s*rect\.width,\s*rect\.height\),\s*(?P<fill>[^\n]+?)\);"
)


def production_body(text: str) -> tuple[str, str]:
    """Splits `text` at its trailing test module (fixtures are not drawing).

    The split is at the **last** `#[cfg(test)]`, not the first: a file may carry a
    `#[cfg(test)]` on an inner helper long before its trailing test module, and splitting there
    would discard the production code this tool exists to rewrite. Measured: `lineedit.rs` has
    one at byte 281 and its `impl Draw` at 36898, so a first-match split reported "no own face"
    for a control that has one -- a silent miss, which is worse than a wrong rewrite because
    nothing reports it.
    """
    marker = text.rfind("\n#[cfg(test)]")
    if marker == -1:
        return (text, "")
    return (text[:marker], text[marker:])


def plan(text: str) -> list[re.Match[str]]:
    body, _ = production_body(text)
    if "impl Draw" not in body:
        return []
    hits = [*FILL_RECT_SHORT.finditer(body), *FILL_RECT_LONG.finditer(body)]
    return hits


def migrate(text: str) -> str | None:
    """Rewrites the single own-face fill, or `None` when there is not exactly one.

    The emitted call passes the two values the call site already has -- the surface and the radius
    -- rather than a `&WidgetStyle`: some `draw` bodies hold the style by value, some by reference,
    and some only in an inner scope, so naming the style directly would not borrow-check at every
    site. Both `SurfaceStyle` and `u32` are `Copy`, which is what makes this mechanical.
    """
    body, tests = production_body(text)
    hits = plan(text)
    if len(hits) != 1:
        return None
    hit = hits[0]
    target = "rect" if hit.re is FILL_RECT_SHORT else "Rect::new(rect.x, rect.y, rect.width, rect.height)"
    ind = hit.group("indent")
    replacement = (
        f"{ind}context.face(\n"
        f"{ind}    {target},\n"
        f"{ind}    {hit.group('fill')},\n"
        f"{ind}    self.style().surface.unwrap_or_default(),\n"
        f"{ind}    self.style().border_radius.unwrap_or(0),\n"
        f"{ind}    Color::BLACK,\n"
        f"{ind});"
    )
    new_body = body[: hit.start()] + replacement + body[hit.end() :]
    return new_body + tests


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    migrated: list[str] = []
    skipped: list[str] = []
    for path in sorted(WIDGET.rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        hits = plan(text)
        if not hits:
            continue
        rel = str(path.relative_to(ROOT))
        if len(hits) != 1:
            skipped.append(f"{rel} ({len(hits)} own-face candidates)")
            continue
        new_text = migrate(text)
        if new_text is None or new_text == text:
            skipped.append(f"{rel} (no change)")
            continue
        migrated.append(rel)
        if args.apply:
            path.write_text(new_text, encoding="utf-8")

    verb = "migrated" if args.apply else "would migrate"
    print(f"{verb}: {len(migrated)}")
    for rel in migrated:
        print(f"  {rel}")
    if skipped:
        print(f"\nneeds a human ({len(skipped)}):")
        for rel in skipped:
            print(f"  {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
