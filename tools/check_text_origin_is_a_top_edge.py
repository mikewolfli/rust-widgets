#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: a text origin must never carry an `ascent` term.

# Why this gate exists

`RenderContext::draw_text(origin, ...)` takes `origin` to be the glyph box's **top-left**
edge. That is not an inference: the SVG backend emits the same value as `<text y=...>`
*paired with* `dominant-baseline="text-before-edge"`, which is the attribute that
redefines SVG's `y` to mean "text box top"; and the software rasteriser blits downward
from it (`GlyphDrawConfig { y: origin.y }` then `y0 = config.y + (gy * height) / 8`).

Because the origin is a top edge and **not** a baseline, adding `metrics.ascent` to it is
always wrong, and it fails in two visible ways:

* **centring** — `(box - height) / 2 + ascent` starts the box a full ascent below the
  middle, so the label sits half a line low. Round 61 caught the eight instances that
  overflowed their control (`badge`, `date_range_picker`, …); the instances that stayed
  *inside* their control were invisible to P5, whose bound is the control rectangle.
  Round 62 found ~40 more surviving across 20-odd files.
* **top alignment** — `top + ascent` starts the box a full ascent below the edge it was
  aligned to, so the label no longer sits where the surrounding layout put it.

Both were live defects that every existing gate read past: P5 only asks whether the ink
stays inside the *control*, and `tools/audit_text_y.py` is a heuristic that infers the
band from neighbouring rects (it cannot tell a deliberate top-aligned label from a
mis-centred one, which is why it is an audit aid and not a check). This gate asks the
only question that is exact and source-local: does the origin arithmetic contain an
`ascent` term at all?

# What is allowed

`ascent` is a legitimate **field of `TextMetrics`** and is still read for genuine
baseline arithmetic such as `ascent / 2 - descent / 2` used to place a *middle* of the
line box relative to something that is not the line box (a spinner centre, an arc anchor).
It is also legitimate to **read** it for measurement (`avatar` computes a text height
from `ascent + descent`).

So the gate is deliberately narrow: it flags an `ascent` term in the **`y` expression of a
`draw_text` / `draw_text_fitted` origin**, not every use of the word. The detection is
textual and conservative — a site the checker cannot prove safe is reported, and the
allowlist below records the ones that are genuinely correct with a stated reason.

Usage: tools/check_text_origin_is_a_top_edge.py   (exit 1 on a new unlisted site)
"""

from __future__ import annotations

import pathlib
import re
import sys

SRC = pathlib.Path("src")

# An `ascent` term, however it is spelled: `metrics.ascent`, `x.ascent as i32`,
# `metrics.ascent as i32 / 2`, `+ ascent`. `descent` is not matched: unlike ascent, a
# descent term in a centring expression is the *other* half of the line box and is not
# itself evidence of a baseline assumption.
ASCENT = re.compile(r"\bascent\b")

# A draw call's window: from the `draw_text`/`draw_text_fitted` token to the end of that
# statement. The origin expression is always inside it, and the window is bounded so a
# nearby-but-unrelated `ascent` cannot be attributed to the call.
CALL = re.compile(r"\b(draw_text|draw_text_fitted)\s*\(")

# Comments are stripped before scanning. A site that was *fixed* explains the removed
# term in a comment ("the old `+ ascent` began the box half a line low"), and flagging the
# documentation of a fix would make this gate unusable — the checker asks what the code
# does, not what its comment says about what it used to do.
LINE_COMMENT = re.compile(r"//[^\n]*")

# Sites that are correct and why. Keyed `relative/path.rs::<needle>`, where the needle is
# a distinctive fragment of the reported line. Keep this list short and justified: it is
# not a mute button, it is a record of the reasoning.
ALLOWED: dict[str, str] = {
    # Reading the metric is not placing a baseline: these consume `ascent` as a number.
    "src/widget/misc_widgets/avatar.rs::text_height":
        "reads `ascent + descent` to obtain a text height; no origin arithmetic",
}


def strip_comments(text: str) -> str:
    """Blanks line comments while preserving byte offsets, so line numbers stay exact."""
    return LINE_COMMENT.sub(lambda m: " " * len(m.group(0)), text)


def call_windows(text: str, start: int) -> str:
    """Returns the `draw_text`-family call beginning at `start`, up to a balanced close."""
    depth = 0
    for index in range(start, min(len(text), start + 1200)):
        ch = text[index]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    return text[start : start + 1200]


# An identifier used as an argument to the call, e.g. `text_y` in `Point::new(x, text_y)`.
IDENT = re.compile(r"\b([a-z_][a-z0-9_]*)\b")


def origin_sources(text: str, call: str, call_start: int) -> str:
    """The call plus the definitions of every local it names.

    The origin is frequently computed one line above the call (`let text_y = ...;` then
    `Point::new(x, text_y)`), so inspecting only the call's own arguments misses most of
    the class — an early version of this checker was reverse-injected against exactly
    that shape and stayed green, which is the failure this function exists to prevent.
    """
    window = call
    # Identifiers the call mentions, resolved against the text before it. Bounded to 4000
    # bytes, which covers a function's locals without dragging in unrelated definitions.
    scope = text[max(0, call_start - 4000) : call_start]
    for name in set(IDENT.findall(call)):
        pattern = r"\blet\s+(?:mut\s+)?" + re.escape(name) + r"\s*=\s*([^;]*);"
        for match in re.finditer(pattern, scope):
            window += "\n" + match.group(1)
    return window


def scan() -> tuple[int, list[tuple[str, int, str]]]:
    checked = 0
    offenders: list[tuple[str, int, str]] = []
    for path in sorted(SRC.rglob("*.rs")):
        raw = path.read_text(encoding="utf-8")
        text = strip_comments(raw)
        for match in CALL.finditer(text):
            window = origin_sources(text, call_windows(text, match.start()), match.start())
            checked += 1
            if not ASCENT.search(window):
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
            offenders.append((path.as_posix(), line_no, line))
    return checked, offenders


def main() -> int:
    if not SRC.is_dir():
        print(f"no source directory at {SRC}", file=sys.stderr)
        return 1

    checked, offenders = scan()
    print(f"text-origin calls checked: {checked}")
    print(f"allowlisted (ascent read for a non-origin reason): {len(ALLOWED)}")
    if not offenders:
        print("failed: 0  (no text origin carries an `ascent` term)")
        return 0

    print(f"failed: {len(offenders)}")
    print()
    print("A text origin is the glyph box's TOP edge, not a baseline, so an `ascent` term")
    print("in it is always a placement error. Centre by `(box - metrics.height) / 2`;")
    print("top-align by dropping the term entirely. See the gate's docstring.")
    print()
    for path, line_no, line in offenders:
        print(f"  {path}:{line_no}\n      {line}")
    print()
    print("If a site is genuinely correct, add it to ALLOWED in this file with a reason.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
