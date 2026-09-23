#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Text/background contrast audit over the committed SVG snapshots.

The snapshots in `snapshots/svg/` are the human-reviewable half of BLUE20 layer 3.
A person reading 376 files by hand will miss the one control whose label is black
on a dark fill, so this script does the arithmetic instead: for every `<text>`
element it finds the element painted immediately underneath the glyph origin and
computes the WCAG relative-luminance contrast ratio.

Two things this is NOT
----------------------
* It is not a gate. A low ratio can be correct: a disabled label is supposed to be
  faint, and a watermark is supposed to recede. `tools/check_control_rendering.sh`
  owns the assertions; this is the evidence generator that tells a reviewer which
  of the 188 controls deserve a look.
* It does not try to model antialiasing or font hinting. The snapshot is a vector
  stream, so the colours are exact and the comparison is exact.

Reading the output
------------------
`ratio` is `(L_lighter + 0.05) / (L_darker + 0.05)` per WCAG 2.1, where `L` is the
relative luminance. 4.5 is the AA floor for body text, 3.0 the floor for large
text and for non-text UI. `ink` is the dominant colour painted under the text
origin, which is what the eye actually compares the glyph against.
"""

from __future__ import annotations

import pathlib
import re
import sys

SNAPSHOT_DIR = pathlib.Path("snapshots/svg")
# The AA floor for text below 18.66 px (or 24 px regular). Every snapshot renders at
# 14 pt, which is "small text" in the WCAG sense, so 4.5 is the correct bar.
AA_SMALL_TEXT = 4.5
# Non-text UI (icons, borders, indicators) may sit at 3.0. Reported for completeness.
AA_LARGE_TEXT = 3.0

RGBA = re.compile(r"rgba\((\d+),\s*(\d+),\s*(\d+),\s*([0-9.]+)\)")
TEXT = re.compile(r"<text\b(?P<attrs>[^>]*)>(?P<body>[^<]*)</text>")


def channel_luminance(value: int) -> float:
    """One sRGB channel to linear light, per WCAG 2.1."""
    srgb = value / 255.0
    if srgb <= 0.04045:
        return srgb / 12.92
    return ((srgb + 0.055) / 1.055) ** 2.4


def luminance(rgb: tuple[int, int, int]) -> float:
    r, g, b = rgb
    return (
        0.2126 * channel_luminance(r)
        + 0.7152 * channel_luminance(g)
        + 0.0722 * channel_luminance(b)
    )


def contrast(a: tuple[int, int, int], b: tuple[int, int, int]) -> float:
    la, lb = luminance(a), luminance(b)
    lighter, darker = max(la, lb), min(la, lb)
    return (lighter + 0.05) / (darker + 0.05)


def flatten(rgba: tuple[int, int, int, float], backdrop: tuple[int, int, int]) -> tuple[int, int, int]:
    """Composite a possibly-translucent fill over the backdrop, like the rasteriser does."""
    r, g, b, a = rgba
    return (
        round(r * a + backdrop[0] * (1 - a)),
        round(g * a + backdrop[1] * (1 - a)),
        round(b * a + backdrop[2] * (1 - a)),
    )


def parse_colour(attrs: str, key: str) -> tuple[int, int, int, float] | None:
    match = re.search(key + r'="rgba\(([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\)"', attrs)
    if not match:
        return None
    return (
        int(float(match.group(1))),
        int(float(match.group(2))),
        int(float(match.group(3))),
        float(match.group(4)),
    )


def element_contains(tag: str, x: float, y: float) -> bool:
    """Whether a point falls inside a `<rect>`, the only shape that paints a text backdrop.

    The point passed in is the text element's `(x, y)`. The backend emits a **baseline**
    (`origin.y + ascent`) and no `dominant-baseline` keyword, while the *trigger* — the edge
    a glyph is blitted from — is the glyph box's top edge. The caller converts between the
    two (`baseline_y - ascent`); this function only answers "is that point inside the rect".
    Asking about the box's top edge is asking about the surface the glyph sits on.
    """
    if not tag.startswith("<rect"):
        return False
    rx = re.search(r'\bx="([\d.-]+)"', tag)
    ry = re.search(r'\by="([\d.-]+)"', tag)
    rw = re.search(r'\bwidth="([\d.-]+)"', tag)
    rh = re.search(r'\bheight="([\d.-]+)"', tag)
    if not (rx and ry and rw and rh):
        return False
    left, top = float(rx.group(1)), float(ry.group(1))
    width, height = float(rw.group(1)), float(rh.group(1))
    # Half-open on the bottom and right edges. A fill and the element below it routinely share
    # a boundary pixel (a palette ending at y=76, a label starting at y=76), and a closed test
    # attributes the label to the fill *above* it — which reported a correctly placed readout
    # as 2.65:1 against a colour it no longer touches. The element that starts at `y` is the
    # one whose surface the glyph sits on.
    return left <= x < left + width and top <= y < top + height


def audit(path: pathlib.Path) -> list[tuple[float, str, tuple[int, int, int], tuple[int, int, int]]]:
    """Returns `(ratio, label, ink, backdrop)` for every text element in one snapshot."""
    lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]

    # The frame fill is the outermost backdrop; every later fill composes onto it.
    backdrop = (255, 255, 255)
    for line in lines:
        if line.startswith("<rect") and "width=\"240\"" in line and "height=\"120\"" in line:
            colour = parse_colour(line, "fill")
            if colour and colour[3] > 0:
                backdrop = flatten(colour, backdrop)
                break

    findings = []
    for line in lines:
        match = TEXT.search(line)
        if not match:
            continue
        body = match.group("body")
        if not body:
            continue
        attrs = match.group("attrs")
        origin_x = re.search(r'\bx="([\d.-]+)"', attrs)
        origin_y = re.search(r'\by="([\d.-]+)"', attrs)
        if not (origin_x and origin_y):
            continue
        x, y = float(origin_x.group(1)), float(origin_y.group(1))

        ink_rgba = parse_colour(attrs, "fill")
        if ink_rgba is None:
            continue

        # The backdrop is the last opaque-or-not fill drawn before this text that
        # contains the glyph origin — SVG paints in document order.
        surface = (255, 255, 255)
        for candidate in lines[: lines.index(line)]:
            if element_contains(candidate, x, y):
                colour = parse_colour(candidate, "fill")
                if colour and colour[3] > 0:
                    surface = flatten(colour, backdrop)

        ink = (ink_rgba[0], ink_rgba[1], ink_rgba[2])
        # The glyph itself is composited over the surface at its own alpha.
        if ink_rgba[3] < 1.0:
            ink = flatten(ink_rgba, surface)
        findings.append((contrast(ink, surface), body, ink, surface))
    return findings


def main() -> int:
    if not SNAPSHOT_DIR.is_dir():
        print(f"no snapshot directory at {SNAPSHOT_DIR}", file=sys.stderr)
        return 1

    rows = []
    for path in sorted(SNAPSHOT_DIR.glob("*.svg")):
        for ratio, label, ink, surface in audit(path):
            if ratio < AA_SMALL_TEXT:
                rows.append((ratio, path.name, label, ink, surface))

    rows.sort(key=lambda row: row[0])
    if not rows:
        print("text contrast: every glyph meets the 4.5:1 AA floor")
        return 0

    print(f"text contrast below {AA_SMALL_TEXT}:1 — {len(rows)} occurrence(s)")
    print()
    print(f"{'ratio':>6}  {'snapshot':<34} {'ink':<16} {'on':<16} label")
    for ratio, name, label, ink, surface in rows:
        flag = "AA-large" if ratio >= AA_LARGE_TEXT else "below-3"
        print(
            f"{ratio:6.2f}  {name:<34} "
            f"{f'{ink[0]},{ink[1]},{ink[2]}':<16} "
            f"{f'{surface[0]},{surface[1]},{surface[2]}':<16} "
            f"{label!r} [{flag}]"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
