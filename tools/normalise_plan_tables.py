#!/usr/bin/env python3
"""Normalises every appendix table row to its own header's cell count.

# Why this exists

`docs/plans/blue23.md` accumulated rows with six cells under a seven-column header over many
rounds of hand editing: the §A.3 input table's rows were written without the 「改进点」 column
that its header declares. A short row still *renders*, but the trailing cells shift left, so a
priority lands under 「改进方式方法」 and the row reads as adjudicated when it has merely been
mis-joined. Measured at HEAD: 121 such rows.

The repair inserts an empty cell at the missing position rather than inventing content — the
column is 「改进点」, which the plan's own prose says is a summary of the methods column, so an
empty cell is the honest statement that nobody has written it yet. Guessing would put words in
the plan's mouth.

Usage:
  python3 tools/normalise_plan_tables.py --check
  python3 tools/normalise_plan_tables.py
"""

import re
import sys

PLAN = "docs/plans/blue23.md"
SEPARATOR = re.compile(r"^\|[-: |]+\|$")


def cells(row):
    text = row.strip()
    if text.endswith("|"):
        text = text[:-1]
    return [part.strip() for part in text.strip("|").split("|")]


def rebuild(parts):
    return "| " + " | ".join(parts) + " |\n"


def main(argv):
    check_only = "--check" in argv
    lines = open(PLAN, encoding="utf-8").read().splitlines(keepends=True)

    fixed = 0
    skipped = []
    index = 0
    while index < len(lines):
        header = lines[index]
        if (
            header.startswith("|")
            and index + 1 < len(lines)
            and SEPARATOR.match(lines[index + 1].strip())
        ):
            want = len(cells(header))
            row = index + 2
            while row < len(lines) and lines[row].startswith("|"):
                parts = cells(lines[row])
                if len(parts) != want:
                    if len(parts) == want - 1:
                        # Insert the missing column just before the trailing priority, which is
                        # the position the header's 「改进点」 occupies. An empty cell says
                        # "not written yet"; inventing text here would fabricate a verdict.
                        parts.insert(want - 2, "")
                        lines[row] = rebuild(parts)
                        fixed += 1
                    else:
                        skipped.append((row + 1, want, len(parts)))
                row += 1
            index = row
        else:
            index += 1

    print("normalised: %d" % fixed)
    if skipped:
        print("left alone (off by more than one, needs a human): %d" % len(skipped))
        for line_number, want, got in skipped:
            print("  line %-5d header=%d row=%d" % (line_number, want, got))
    if not check_only and fixed:
        open(PLAN, "w", encoding="utf-8").writelines(lines)
        print("wrote %s" % PLAN)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
