#!/usr/bin/env python3
"""Check that each text run in a `quote_board` snapshot sits inside a column.

`quote_board.svg` paints each heading as its own glyph `<path>` run. The board's columns
are an equal division of the control's width by the column count, so the runs' x extents
can be compared against those boundaries without the board telling us the boundaries.

Reports the five (or however many) runs and which column each one's ink falls into.

Usage:
  python3 tools/probe_quote_board_columns.py <svg-file> [svg-file ...]
"""

import re
import sys

PATH_RE = re.compile(r'<path d="([^"]*)"')
GLYPH_RE = re.compile(r"M(-?\d+) -?\d+h1v")


def runs(body):
    out = []
    for match in PATH_RE.finditer(body):
        xs = [int(x) for x in GLYPH_RE.findall(match.group(1))]
        if xs:
            out.append((min(xs), max(xs)))
    return out


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    failed = False
    for path in argv[1:]:
        body = open(path, "r", encoding="utf-8").read()
        width_match = re.search(r'<svg[^>]*width="(\d+)"', body)
        if not width_match:
            print("%s: no width" % path)
            continue
        width = int(width_match.group(1))
        rs = runs(body)
        if not rs:
            print("%s: no text runs" % path)
            continue
        columns = 5  # `QuoteBoard::new` ships five default columns.
        slot = width // columns
        print("%s  width=%d  columns=%d  slot=%d" % (path, width, columns, slot))
        for index, (left, right) in enumerate(rs):
            column = left // slot
            ok = 0 <= left < width and right < width
            verdict = "ok" if ok else "ESCAPES THE CONTROL"
            if not ok:
                failed = True
            print(
                "  run%d ink=%3d..%-3d  column=%d [%d..%d)  %s"
                % (index, left, right, column, column * slot, (column + 1) * slot, verdict)
            )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
