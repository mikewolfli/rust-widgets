#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# The rule this implements is stated in full — what it proves, what it does not, and the
# reverse injection that keeps it honest — in tools/check_generated_font_table_integrity.sh,
# which is the only caller. That file is the documentation of record.
#
# The generated CJK table is read by a binary search for the codepoint, then a fixed-size window
# into a flat row array. Both of those are *assumptions about the data*, and neither is checked
# by the compiler: an out-of-order table makes `binary_search` silently miss glyphs, and a short
# row array makes some glyphs read blanks. This scan asserts the assumptions.
#
# Prints one `finding: ...` line per problem, then a `glyphs=N rows=M failed=K` summary line.
# Exit status is always 0; the shell wrapper makes the pass/fail judgement.

import argparse
import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_TABLE = "src/render/text/cjk_bitmap_data.rs"

# 16 rows of 2 bytes, per glyph. The reader derives the window from this; the two must agree.
BYTES_PER_GLYPH = 32

CODEPOINTS = re.compile(r"CODEPOINTS:\s*\[u32;\s*(\d+)\]\s*=\s*\[(.*?)\];", re.S)
ROWS = re.compile(r"ROWS:\s*\[u8;\s*(\d+)\]\s*=\s*\[(.*?)\];", re.S)
HEX = re.compile(r"0x[0-9A-Fa-f]+")


def load(path: pathlib.Path):
    text = path.read_text(encoding="utf-8")
    codepoints = CODEPOINTS.search(text)
    rows = ROWS.search(text)
    if codepoints is None or rows is None:
        return None
    return (
        int(codepoints.group(1)),
        [int(value, 16) for value in HEX.findall(codepoints.group(2))],
        int(rows.group(1)),
        [int(value, 16) for value in HEX.findall(rows.group(2))],
    )


def check(path: pathlib.Path, label: str, findings: list):
    loaded = load(path)
    if loaded is None:
        findings.append(f"{label}: could not find the CODEPOINTS/ROWS arrays")
        return 0, 0
    declared_glyphs, codepoints, declared_rows, rows = loaded

    if declared_glyphs != len(codepoints):
        findings.append(
            f"{label}: declares {declared_glyphs} codepoints but lists {len(codepoints)}"
        )
    if declared_rows != len(rows):
        findings.append(f"{label}: declares {declared_rows} row bytes but lists {len(rows)}")
    if len(rows) != len(codepoints) * BYTES_PER_GLYPH:
        findings.append(
            f"{label}: {len(rows)} row bytes for {len(codepoints)} glyphs; expected "
            f"{len(codepoints) * BYTES_PER_GLYPH}"
        )
    # The binary search's precondition, both halves. Duplicates would make the lookup ambiguous
    # rather than wrong, which is worse: which of two glyphs a label gets would depend on the
    # table's order.
    for index in range(1, len(codepoints)):
        if codepoints[index] <= codepoints[index - 1]:
            findings.append(
                f"{label}: codepoints not strictly ascending at {index} "
                f"(0x{codepoints[index - 1]:04X} then 0x{codepoints[index]:04X})"
            )
            break
    # ASCII is deliberately absent: the crate's 8x8 face covers it, and including it would
    # change the default build's output.
    ascii_hits = [value for value in codepoints if value <= 0x7F]
    if ascii_hits:
        findings.append(f"{label}: contains {len(ascii_hits)} ASCII codepoints, which must not be here")
    return len(codepoints), len(rows)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--table", default=str(REPO_ROOT / DEFAULT_TABLE))
    parser.add_argument("--inject", default=None,
                        help="check this file instead, to prove the checks can fail")
    args = parser.parse_args(argv)

    findings = []
    if args.inject:
        path = pathlib.Path(args.inject)
        glyphs, rows = check(path, pathlib.Path(args.inject).as_posix(), findings)
    else:
        path = pathlib.Path(args.table)
        if not path.exists():
            print(f"finding: {args.table} is missing")
            print("glyphs=0 rows=0 failed=1")
            return 0
        glyphs, rows = check(path, DEFAULT_TABLE, findings)

        # The table must not be compiled in unless its feature is on, or a `mini` build pays
        # ~85 KB for nothing. Checked here rather than in the opt-in gate because this is the
        # file that knows how much it is worth.
        mod = REPO_ROOT / "src/render/text/mod.rs"
        lines = mod.read_text(encoding="utf-8").splitlines() if mod.exists() else []
        declaration = "mod cjk_bitmap_data;"
        gated = any(
            line.strip() == declaration
            and any(
                previous.strip() == '#[cfg(feature = "fonts-cjk-bitmap")]'
                for previous in lines[max(0, index - 2) : index]
            )
            for index, line in enumerate(lines)
        )
        if not gated:
            findings.append(
                f"{DEFAULT_TABLE}: not gated by #[cfg(feature = \"fonts-cjk-bitmap\")] "
                f"in src/render/text/mod.rs"
            )

    for finding in findings:
        print(f"finding: {finding}")
    print(f"glyphs={glyphs} rows={rows} failed={len(findings)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
