#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: a single-line label must not be positioned at `band.y + band.height / 2`.

# Why this gate exists

`RenderContext`'s text origin is the glyph box's **top-left** edge, not its baseline (see
`check_text_origin_is_a_top_edge.py`, which guards the other half of the same contract).
So the expression everyone reaches for when they mean "centre this label in its band" —

    y = band.y + band.height / 2

— does the opposite of centring: it puts the box's *top edge* on the band's middle line and
draws the whole label half a line low. Centring is

    y = band.y + (band.height - line_height) / 2

That single wrong shape was the largest class of visual defect in this crate: **76 sites across
40-odd files**, spanning buttons, checkboxes, radio buttons, combo boxes, date/time editors,
status bars, banners, ratings, tool buttons, six dialog button rows, `mdi_area` titles, the
properties panel, five data tables, the menu bar, the toolbar, tabs, keyboard key caps, tag
input, popovers, toasts and the chart empty state. Every one of them satisfied every existing
gate: it is valid Rust, it compiles, the ink stays inside the control, and the SVG is
well-formed. `tools/audit_text_y.py` can *see* the result but cannot prove intent — it infers
the band from neighbouring rectangles, which is why it is documented as an audit aid.

This gate asks the exact, source-local question instead: **is a text origin's `y` derived by
halving a height that is not the line height?** That is decidable without knowing the band,
because the correct expression always subtracts a *measured line height*.

# What is allowed

The `h / 2` form is correct — and must not be flagged — when the thing being halved is not the
text's own band:

  * a **non-text** primitive: a circle centre, a track midline, an arc anchor, a divider. This
    is the majority of the ~100 remaining matches in the crate and is exactly the false-positive
    class principle #108 ③ warns about.
  * a **polar anchor**: a label centred on a point on an arc or circle, e.g.
    `label_pos.y - metrics.height / 2` (the subtraction is already there).
  * a **midpoint that is then fed to a centring helper**: `draw_text_fitted(bounds, ..)` with
    `HorizontalAlignment::Center` still takes `bounds.y` verbatim for the vertical axis, so it
    is only correct when `bounds` is a line box. This gate therefore checks the *argument*, not
    the call.

The detection is textual and deliberately narrow: it looks at the `y` coordinate of a
`Point::new` / `Point { y: .. }` that is passed to (or assigned for) a `draw_text*` call, and
reports it when the expression contains both a `height` term and a `/ 2` term but no measured
line height. Sites that are genuinely correct go in `ALLOWED` with a stated reason.

Usage: tools/check_text_vertically_centred.py   (exit 1 on a new unlisted site)
"""

from __future__ import annotations

import pathlib
import re
import sys

SRC = pathlib.Path("src")

# The wrong shape, in the spellings the crate actually uses:
#   `rect.y + rect.height as i32 / 2`
#   `band.y + (band.height as i32 / 2)`
#   `y + row_height / 2`
#   `geom.y + geom.height as f32 / 2.0`
# A `height` term followed (possibly through casts/parens) by a division by two.
HALVED_HEIGHT = re.compile(
    r"\.height\b[^;,)\n]*?/\s*2(?:\.0)?\b"
    r"|\.height\s*as\s+\w+[^;,)\n]*?/\s*2(?:\.0)?\b"
)

# A measured line height is the marker of a correct centring: the arithmetic has to subtract
# something the renderer measured. Recognising it here is what keeps the gate from demanding a
# change at every site that already does the right thing with a differently-spelled band.
MEASURED_LINE = re.compile(
    r"(measure_text|text_line|line_height|metrics\.height|glyph_height|TAG_HEIGHT|ROW_HEIGHT)"
)

# A text draw, and the assignment feeding it. The window is bounded so an unrelated `height`
# elsewhere in the function cannot be attributed to the call.
DRAW = re.compile(r"\b(draw_text|draw_text_fitted|draw_text_line)\s*\(")

LINE_COMMENT = re.compile(r"//[^\n]*")

# Sites that are correct and why. Keyed `relative/path.rs::<needle>`, where the needle is a
# distinctive fragment of the reported line. Keep this list short and justified: it is not a
# mute button, it is a record of the reasoning.
ALLOWED: dict[str, str] = {
    # `draw_text_fitted` is called with `bounds` already built as a one-line band whose
    # `height` IS the measured line height, so the `y` term is the band's own top edge.
    "src/widget/chart_widgets/pie_chart.rs::pct_y":
        "polar anchor: the label is centred on a point on the arc; `- metrics.height / 2` is "
        "the centring term, and the enclosing band is incidental",
}


def strip_comments(text: str) -> str:
    """Blanks line comments while preserving byte offsets, so line numbers stay exact."""
    return LINE_COMMENT.sub(lambda m: " " * len(m.group(0)), text)


def y_expression(text: str, call_start: int) -> str:
    """The `y` coordinate expression of the call beginning at `call_start`, plus its locals.

    Text is positioned two ways in this crate: an inline `Point::new(x, <expr>)` in the call's
    own arguments, or a `let text_y = <expr>;` a few lines above that the call then names. Both
    are collected, because an early version of the sibling `ascent` gate read only the call's
    own arguments and stayed green against exactly that shape.
    """
    scope = text[max(0, call_start - 3000) : call_start]
    window = text[call_start : call_start + 300]
    for name in set(re.findall(r"\b([a-z_][a-z0-9_]*)\b", window)):
        pattern = r"\blet\s+(?:mut\s+)?" + re.escape(name) + r"\s*(?::[^=]+)?=\s*([^;]*);"
        for match in re.finditer(pattern, scope):
            window += "\n" + match.group(1)
    return window


def scan() -> tuple[int, list[tuple[str, int, str]]]:
    checked = 0
    offenders: list[tuple[str, int, str]] = []
    for path in sorted(SRC.rglob("*.rs")):
        raw = path.read_text(encoding="utf-8")
        text = strip_comments(raw)
        for match in DRAW.finditer(text):
            checked += 1
            window = y_expression(text, match.start())
            if not HALVED_HEIGHT.search(window):
                continue
            if MEASURED_LINE.search(window):
                continue
            line_no = text.count("\n", 0, match.start()) + 1
            line = raw.splitlines()[line_no - 1].strip()
            rel = path.as_posix()
            if any(
                rel.endswith(key.split("::")[0]) and needle in line
                for key in ALLOWED
                for needle in [key.split("::", 1)[1]]
            ):
                continue
            offenders.append((rel, line_no, line))
    return checked, offenders


def main() -> int:
    if not SRC.is_dir():
        print(f"no source directory at {SRC}", file=sys.stderr)
        return 1

    checked, offenders = scan()
    print(f"text draw calls checked: {checked}")
    print(f"allowlisted (correct with a stated reason): {len(ALLOWED)}")
    if not offenders:
        print("failed: 0  (no text origin is a halved band height)")
        return 0

    print(f"failed: {len(offenders)}")
    print()
    print("A text origin is the glyph box's TOP edge, so `band.y + band.height / 2` puts that")
    print("edge on the band's middle line and draws the label half a line low. Centre with")
    print("`context.text_line(band, font)` — or, if the band is genuinely taller than one line,")
    print("`band.y + (band.height - measured_line_height) / 2`.")
    print()
    for path, line_no, line in offenders:
        print(f"  {path}:{line_no}\n      {line}")
    print()
    print("If a site is genuinely correct, add it to ALLOWED in this file with a reason.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
