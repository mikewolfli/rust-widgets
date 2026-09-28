#!/usr/bin/env python3
"""Measure where a snapshot's text ink actually starts, in device pixels.

The SVG backend paints text as a run of one-pixel `<rect>`s (one per lit glyph pixel),
so the leftmost `x` over the whole drawing *is* the left edge of the ink. That is the
quantity `fitted_origin` decides, and it can be compared between two snapshots of the
same label without rendering anything.

Also reports the control's own extent, taken from the non-black `rect`s, so the ink can
be expressed as a distance from the control's left edge.

Usage:
  python3 tools/probe_snapshot_ink_left.py <svg-file> [svg-file ...]
"""

import re
import sys

RECT_RE = re.compile(
    r"<rect\s+x=\"(-?\d+)\"\s+y=\"(-?\d+)\"\s+width=\"(\d+)\"\s+height=\"(\d+)\"([^/]*)/>"
)
# A run of one-pixel squares is how the backend rasterises a glyph: `M96 53h1v1h-1zM97 53h1v1h-1z…`.
GLYPH_RUN_RE = re.compile(r"M(-?\d+) (-?\d+)h1v\d")


def scan(path):
    with open(path, "r", encoding="utf-8") as handle:
        body = handle.read()
    ink_xs = [int(m.group(1)) for m in GLYPH_RUN_RE.finditer(body)]
    chrome_xs = []
    for match in RECT_RE.finditer(body):
        x, y, w, h, rest = match.groups()
        x, y, w, h = int(x), int(y), int(w), int(h)
        # A one-pixel square belongs to a glyph run; anything with real extent is chrome.
        if w == 1 and h in (1, 2):
            continue
        if w > 2 or h > 8:
            chrome_xs.append(x)
    return ink_xs, chrome_xs


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    print("%-46s %8s %8s %8s" % ("file", "ink_left", "ink_right", "chrome_x"))
    for path in argv[1:]:
        ink, chrome = scan(path)
        if not ink:
            print("%-46s %8s %8s %8s" % (path, "-", "-", min(chrome) if chrome else "-"))
            continue
        print(
            "%-46s %8d %8d %8s"
            % (path, min(ink), max(ink), min(chrome) if chrome else "-")
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
