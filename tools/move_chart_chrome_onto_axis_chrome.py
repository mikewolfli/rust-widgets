#!/usr/bin/env python3
"""Moves the remaining chart-engine chrome literals onto `axis_chrome`/`axis_chrome_color`.

# The pattern being replaced

`charts.rs` painted its plot background, title and axis labels with three struct literals
written for a **light** chart:

    Color { r: 240, g: 240, b: 240, a: 255 }   plot background
    Color { r: 20,  g: 20,  b: 20,  a: 255 }   titles
    Color { r: 40,  g: 40,  b: 40,  a: 255 }   labels

On a dark appearance those are a near-white plate carrying near-black text — the chart reads as
a light-mode artifact pasted onto a dark window. The file already had the correct derivation
(`axis_chrome`, used by `draw_y_ticks`/`draw_x_ticks`) and the four `Chart::draw` bodies simply
did not call it.

# Why the strengths are these

`axis_chrome_color(strength)` steps from the surface toward the ink, so one number states "how
loud is this". The mapping keeps the *relationships* the literals encoded (a title is the
loudest, a label mid, the plate is the surface) without keeping the light-mode values:

    plot background  -> axis_chrome_color(0.0)   the surface itself
    title            -> axis_chrome_color(0.70)  same as `axis_chrome`'s label tone
    label            -> axis_chrome_color(0.45)  same as `axis_chrome`'s axis tone

The `PALETTE` entries and the `Color::GREEN`/`RED`/`YELLOW` semantic fallbacks are **data**, not
chrome, and are deliberately left alone.

Usage:
  python3 tools/move_chart_chrome_onto_axis_chrome.py --check
  python3 tools/move_chart_chrome_onto_axis_chrome.py
"""

import re
import sys

SOURCE = "src/widget/chart_widgets/charts.rs"

# (literal text, replacement). Longest first so the tuple form is not partially matched.
REPLACEMENTS = [
    ("Color { r: 240, g: 240, b: 240, a: 255 }", "axis_chrome_color(0.0)"),
    ("Color { r: 230, g: 230, b: 230, a: 255 }", "axis_chrome_color(0.0)"),
    ("Color { r: 20, g: 20, b: 20, a: 255 }", "axis_chrome_color(0.70)"),
    ("Color { r: 40, g: 40, b: 40, a: 255 }", "axis_chrome_color(0.45)"),
]

# The data palette and semantic fallbacks, which are not chrome.
KEEP = re.compile(
    r"Color \{ r: (255, g: |54, g: |75, g: |153, g: |46, g: |231, g: |90, g: 90)"
)


def main(argv):
    check_only = "--check" in argv
    body = open(SOURCE, encoding="utf-8").read()
    original = body

    changed = {}
    for literal, replacement in REPLACEMENTS:
        if literal in body:
            changed[literal] = body.count(literal)
            body = body.replace(literal, replacement)

    print("literals replaced:")
    for literal, count in changed.items():
        print("  %-46s x%d" % (literal, count))
    remaining = len(re.findall(r"Color \{ r: \d+, g: \d+, b: \d+, a: 255 \}", body))
    print("struct-literal colours remaining in the file: %d" % remaining)
    print("  (expected: the data PALETTE and the axis_chrome fallbacks)")

    if not check_only and body != original:
        open(SOURCE, "w", encoding="utf-8").write(body)
        print("wrote %s" % SOURCE)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
