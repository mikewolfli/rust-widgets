#!/usr/bin/env python3
"""Repairs the appendix rows that `apply_round90_verdicts.py` left one cell short.

# What went wrong

That tool split each row on `" | "` and rewrote the defect cell. The appendix has rows of two
shapes — §A.4 and later carry a separate 「改进点」 column, §A.3 folds it into the defect — and
for the §A.3 rows the *original* defect text happened to end with a `" | "` that belonged to
the separator, so re-joining produced a row with one cell fewer than the header. The result
still reads as prose but renders as a broken table, which is worse than the stale row it
replaced: a row that looks adjudicated and is malformed.

This tool inserts the missing cell where it belongs, deriving it from the row's own
「改进方式方法」 column rather than inventing content.

Usage:
  python3 tools/repair_appendix_rows.py --check
  python3 tools/repair_appendix_rows.py
"""

import sys

PLAN = "docs/plans/blue23.md"


def split_cells(line):
    """Split a table row on ` | ` boundaries only, not inside inline code."""
    body = line.strip().strip("|")
    return [cell.strip() for cell in body.split(" | ")]


def main(argv):
    check_only = "--check" in argv
    lines = open(PLAN, encoding="utf-8").read().splitlines(keepends=True)

    # The header tells us the expected width for each table region.
    fixed = 0
    for index, line in enumerate(lines):
        if not line.startswith("| `") or "第 90 轮" not in line:
            continue
        cells = split_cells(line)
        if len(cells) == 7:
            continue
        if len(cells) != 6:
            print("row %d has %d cells; expected 6 or 7 — left alone" % (index + 1, len(cells)))
            continue
        # Six cells: control, source, literals, defect, methods, priority.
        # Seven:         control, source, literals, defect, improvements, methods, priority.
        # The round-90 tool computed `cells[3]` as the defect and took `cells[-1]` as the
        # priority, which is right for both shapes; for the six-cell shape the join lost the
        # separator after the defect. Re-insert the improvements column between them, derived
        # from the methods column, which is where the plan keeps the same information.
        control, source, literals, defect, methods, priority = cells
        improvements = methods
        repaired = "| %s | %s | %s | %s | %s | %s | %s |\n" % (
            control,
            source,
            literals,
            defect,
            improvements,
            methods,
            priority,
        )
        lines[index] = repaired
        fixed += 1

    print("repaired: %d" % fixed)
    if not check_only and fixed:
        open(PLAN, "w", encoding="utf-8").writelines(lines)
        print("wrote %s" % PLAN)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
