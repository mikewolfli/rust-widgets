#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT

"""Prints the exact assertion each remaining vector-build failure makes.

Run from the repo root after `cargo test` with the vector face enabled. The point is to
show that the failures are one class -- a bitmap glyph's ink fills its box corner and an
outline's does not -- rather than a list of unrelated problems to patch one by one.
"""

import pathlib
import re
import subprocess
import sys

TESTS = [
    ("widget::svg::tests::the_emitted_path_reproduces_the_glyph_box_exactly", "src/widget/svg.rs"),
    ("widget::display_widgets::arc::tests::arc_value_is_drawn_only_when_it_fits_in_the_ring_hole", "src/widget/display_widgets/arc.rs"),
    ("widget::input_widgets::lineedit::tests::a_prefix_is_drawn_before_the_value_and_shifts_it", "src/widget/input_widgets/lineedit.rs"),
]


def failures() -> list[str]:
    out = subprocess.run(
        ["cargo", "test", "--lib", "--no-default-features",
         "--features", "desktop,fonts-vector-latin"],
        capture_output=True, text=True,
    ).stdout
    return re.findall(r"^\s+(widget::\S+|render::\S+)$", out, re.M)


def main() -> int:
    names = failures()
    print(f"{len(names)} failing tests\n")
    for name in names:
        print(name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
