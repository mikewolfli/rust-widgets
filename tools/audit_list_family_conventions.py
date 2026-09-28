#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""How far the list/table family has got on the two judgements appendix A asks of it.

# Why this exists

Appendix A's M6 ("separators read `outline_variant`") and M1 ("rows have a hover") are the two
highest-yield items in the plan: they apply to a whole family of controls that share their
drawing, so one fix covers several rows. Measuring the family by hand — opening fifteen files
to see which has a hover and which still strokes its rows in `border_color` — is the same
"read 150 rows by eye" that this round is trying to stop doing.

The tool prints, per control in the list/table family, three facts:

  * whether the file names `outline_variant` (the M6 separator role);
  * whether it tracks a **row/cell** hover (a field or a base-hover read), which is M1;
  * whether it draws row separator lines at all.

Reading it needs one caveat the tool cannot encode: a control that delegates its rows to a
shared renderer (an engine, a `layout.rs`) will show zeros here while the shared file carries
the work. `data_view` and `virtual_list` are that shape, and so is anything drawing through
`CompositeBuilder`. A zero is a prompt to look, not a finding — which is the same rule the
other two audits in this directory follow.

Usage: python3 tools/audit_list_family_conventions.py
"""

import glob
import os
import re
import sys

FAMILIES = ("src/widget/view_widgets/*.rs", "src/widget/container_widgets/*.rs")
ROW_SEPARATOR = re.compile(r"draw_line|draw_line_stroke|row_separator|separator")
HOVER = re.compile(r"hovered_row|hovered_index|hovered_cell|hovered_item|base\.is_hovered")


def main() -> int:
    rows = []
    for pattern in FAMILIES:
        for path in sorted(glob.glob(pattern)):
            text = open(path, encoding="utf-8", errors="ignore").read()
            if "fn draw" not in text:
                continue
            name = os.path.basename(path)
            rows.append(
                (
                    name,
                    len(re.findall(r"outline_variant", text)),
                    bool(HOVER.search(text)),
                    len(ROW_SEPARATOR.findall(text)) > 0,
                )
            )

    print(f"{'control':28s} {'outline_variant':>15s} {'row-hover':>10s} {'draws lines':>12s}")
    for name, outline, hover, lines in rows:
        print(f"{name:28s} {outline:>15d} {'yes' if hover else 'NO':>10s} {'yes' if lines else 'no':>12s}")

    missing_outline = [name for name, outline, _, _ in rows if outline == 0]
    missing_hover = [name for name, _, hover, _ in rows if not hover]
    print(f"\n=== files naming no `outline_variant`: {len(missing_outline)} ===")
    for name in missing_outline:
        print(f"    {name}")
    print(f"=== files tracking no row/cell hover: {len(missing_hover)} ===")
    for name in missing_hover:
        print(f"    {name}")
    print(
        "\nA zero may mean the work lives in a shared renderer the file delegates to "
        "(`data_view`/`virtual_list` draw through shared code). Verify before acting."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
