#!/usr/bin/env python3
"""Reports table rows whose cell count disagrees with their own header.

A markdown table with a short row still *renders*, but the trailing cells shift left — a
priority lands in the 「改进方式方法」 column and the row reads as if it had been adjudicated
when it has merely been mis-joined. That is worse than a stale row, so it is worth a check.

Usage:
  python3 tools/check_plan_tables.py            # summary
  python3 tools/check_plan_tables.py --list     # every offending row number
"""

import glob
import re
import sys

PLAN_GLOB = "docs/plans/*.md"
SEPARATOR = re.compile(r"^\|[-: |]+\|$")


def cells(row):
    text = row.strip()
    if text.endswith("|"):
        text = text[:-1]
    text = text.strip("|")
    # A `\|` inside a cell is an escaped pipe, not a separator — these tables quote shell
    # pipelines in their command column, so splitting naively counts `\| wc -l` as two cells
    # and reports a well-formed row as short.
    parts = re.split(r"(?<!\\)\|", text)
    return [part.strip() for part in parts]


def scan(path):
    """Every row in `path` whose cell count disagrees with its own header."""
    lines = open(path, encoding="utf-8").read().splitlines()
    problems = []
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
                got = len(cells(lines[row]))
                if got < want:
                    problems.append((path, row + 1, want, got, lines[row][:80]))
                row += 1
            index = row
        else:
            index += 1
    return problems


def main(argv):
    problems = []
    # Every **active** plan is scanned, not only the one archived file this started
    # from: a short row in an active plan is the table a reader is acting on (W8-12).
    # `archive/` is excluded on purpose — it is retained history, not current guidance,
    # and its older formatting is not something a current edit should have to rewrite.
    for path in sorted(glob.glob(PLAN_GLOB, recursive=True)):
        problems.extend(scan(path))

    print("rows whose cell count disagrees with their header: %d" % len(problems))
    for path, line_number, want, got, preview in problems:
        if "--list" in argv:
            print("  %s:%-5d header=%d row=%d  %s" % (path, line_number, want, got, preview))
        else:
            print("  %s:%d header=%d row=%d" % (path, line_number, want, got))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
