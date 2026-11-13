#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""The large-file census may only shrink (issue §8 direction 6 / R-28).

# Why a ratchet and not a hard ceiling

Principle #9 sets the ceiling at 5000 production lines, and `check_source_structure` enforces
it. R-28's 2000-line census is a *debt tracker*, not a build rule: 115 files are above it and
splitting them in one round would be a rewrite. What must not happen is the count growing while
the work waits. This ratchet stores the exact file list; a file that crosses 2000 is a failure
that names it, and a file that drops below is a failure until the budget is updated to record
the progress (so the number can only move down).

Two measures are stored because they answer different questions:

  * **total lines** — R-28's original metric, includes inline tests;
  * **production lines** — principle #9's metric, excludes inline/child test modules.

Usage:
    python3 tools/check_large_file_ratchet.py --update   # record the current census
    python3 tools/check_large_file_ratchet.py            # verify it has not grown
"""

from __future__ import annotations

import json
import pathlib
import re
import sys

BUDGET = pathlib.Path("tools/large_file_ratchet.json")
LIMIT = 2000

CFG_TEST = re.compile(r"^#\[cfg\([^\]]*\btest\b[^\]]*\)\]\s*$")
TEST_MOD = re.compile(r"^#{0,1}mod [A-Za-z0-9_]+ \{\s*$")


def production_lines(path: pathlib.Path) -> int:
    lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
    starts = [
        index
        for index in range(len(lines) - 1)
        if CFG_TEST.match(lines[index]) and TEST_MOD.match(lines[index + 1])
    ]
    return starts[0] if starts else len(lines)


def sources() -> list[pathlib.Path]:
    out: list[pathlib.Path] = []
    for root in (pathlib.Path("src"), pathlib.Path("crates")):
        for path in root.rglob("*.rs"):
            if "target" in path.parts:
                continue
            if path.name == "tests.rs" or path.name.endswith("_tests.rs"):
                continue
            out.append(path)
    return sorted(out)


def census() -> dict[str, list[str]]:
    total: list[str] = []
    production: list[str] = []
    for path in sources():
        text_lines = path.read_text(encoding="utf-8", errors="ignore").splitlines()
        if len(text_lines) > LIMIT:
            total.append(path.as_posix())
        if production_lines(path) > LIMIT:
            production.append(path.as_posix())
    return {"total_lines_over_2000": total, "production_lines_over_2000": production}


def family(path: str) -> str:
    parts = pathlib.PurePosixPath(path).parts
    if len(parts) >= 6 and parts[0] == "src" and parts[1] == "widget":
        return "widget/" + parts[2]
    if len(parts) >= 3 and parts[0] == "src":
        return parts[1]
    if len(parts) >= 3 and parts[0] == "crates":
        return "crates/" + parts[1]
    return "other"


def main() -> int:
    current = census()
    if "--update" in sys.argv[1:]:
        BUDGET.write_text(json.dumps(current, indent=2) + "\n", encoding="utf-8")
        print(
            f"wrote {BUDGET}: total={len(current['total_lines_over_2000'])} "
            f"production={len(current['production_lines_over_2000'])}"
        )
        return 0

    if not BUDGET.is_file():
        print(f"FAIL: {BUDGET} is missing; record the census with --update")
        return 1
    budget = json.loads(BUDGET.read_text(encoding="utf-8"))
    findings: list[str] = []
    for measure in ("total_lines_over_2000", "production_lines_over_2000"):
        before = set(budget.get(measure, []))
        after = set(current[measure])
        for path in sorted(after - before):
            findings.append(f"{path}: crossed {LIMIT} lines (`{measure}`); split it or record why")
        for path in sorted(before - after):
            findings.append(
                f"{path}: dropped below {LIMIT} lines; update {BUDGET} to record the progress"
            )

    counts = {
        measure: len(current[measure])
        for measure in ("total_lines_over_2000", "production_lines_over_2000")
    }
    families: dict[str, int] = {}
    for path in current["production_lines_over_2000"]:
        families[family(path)] = families.get(family(path), 0) + 1
    print(
        "large files: total_lines={total_lines_over_2000} production_lines={production_lines_over_2000}".format(
            **counts
        )
    )
    print("production census by family: " + ", ".join(
        f"{name}={count}" for name, count in sorted(families.items())
    ))
    for finding in findings:
        print(finding)
    if findings:
        return 1
    print("✅ the >2000-line census holds (it may only shrink)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
