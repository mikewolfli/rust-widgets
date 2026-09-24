#!/usr/bin/env python3
"""Measure how much of a control's **appearance** is verified versus only declared.

# Why this exists

The BLUE19 gate set checks declarations: does every published event have a typed payload,
does every control have a test, does the count in the docs match the code. All of those can
be green while a control renders invisibly — which is what happened (BLUE20 §1.3).

The two numbers that matter are printed here:

* how many `impl Draw` files read **no** style colour at all (so they cannot respond to a
  theme), and
* how many places hardcode a colour, i.e. how many ways a control can ignore the theme.

They are the sizing evidence for the plan's P3 judgement ("light and dark must render
differently"), which turns both numbers into something a gate can assert.

# Usage

    python3 tools/audit_appearance.py

Exit status is 0: this measures, it does not gate. The gate is
`tools/check_control_rendering.sh`.
"""

import glob
import os
import re
import sys

WIDGET_GLOB = "src/widget/**/*.rs"
COLOR_LITERAL = re.compile(r"Color::rgb\(|Color::rgba\(")
# The style fields a `Draw` impl would read to honour the active theme.
#
# The pattern must match **every** way a control reaches its style record:
#
#   * `style.background_color`         -- a local `let style = self.style();` binding
#   * `self.style().background_color`  -- read inline, with no binding
#   * `self.base.style().text_color`   -- read via the base directly
#
# It matched only the first until this was fixed, so a file that read its style inline
# (`textarea.rs`, `inplace_editor.rs`) was counted among the "Draw files reading no style
# colour" even though it read all three. The count is the sizing evidence for the
# light/dark judgement, so an over-count here is not cosmetic: it makes the number report
# a defect that is not there, and a fix aimed at it has nothing to change.
STYLE_READ = re.compile(
    r"\.style\(\s*\)\s*\.\s*(?:background_color|text_color|border_color)"
    r"|\bstyle\s*\.\s*(?:background_color|text_color|border_color)",
    re.MULTILINE,
)
# The ways a test can look at rendered pixels rather than at widget state.
PIXEL_READ = re.compile(r"render_frame_tree|frame_rgba|blit_rgba|render_widget_to_svg")


def main() -> int:
    # `glob` yields host-native separators on Windows (`src\\widget\\...`); the coverage
    # comparison below matches those paths against the ones collected from the same walk,
    # so both sides are normalised to `/` first. Without this the count reads 0 on
    # Windows even when a test does render the file, which is a measurement artifact,
    # not a finding.
    def normalise(path: str) -> str:
        return path.replace(os.sep, "/")

    draw_files = []
    for path in sorted(glob.glob(WIDGET_GLOB, recursive=True)):
        text = open(path, encoding="utf-8", errors="ignore").read()
        if "impl Draw for" in text:
            draw_files.append((normalise(path), text))

    print(f"=== files with a Draw impl: {len(draw_files)} ===")

    total_literals = 0
    hardcoded_only = []
    for path, text in draw_files:
        literals = len(COLOR_LITERAL.findall(text))
        total_literals += literals
        if literals > 0 and not STYLE_READ.search(text):
            hardcoded_only.append((literals, path))

    print(f"=== total colour literals in Draw files: {total_literals} ===")
    print(
        f"=== Draw files reading no style colour, only literals: {len(hardcoded_only)} "
        f"({100 * len(hardcoded_only) // max(len(draw_files), 1)}%) ==="
    )
    for literals, path in sorted(hardcoded_only, reverse=True):
        print(f"  {literals:3d} literals  {path}")

    # How much of the drawing code any test actually looks at as pixels.
    pixel_files = set()
    for pattern in ("tests/**/*.rs", "src/**/*.rs"):
        for path in glob.glob(pattern, recursive=True):
            text = open(path, encoding="utf-8", errors="ignore").read()
            if PIXEL_READ.search(text):
                pixel_files.add(normalise(path))

    covered = sum(1 for path, _ in draw_files if path in pixel_files)
    print(f"\n=== Draw files a test renders as pixels: {covered} / {len(draw_files)} ===")

    return 0


if __name__ == "__main__":
    sys.exit(main())
