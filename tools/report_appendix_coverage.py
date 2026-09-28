#!/usr/bin/env python3
"""Reports which appendix-A control rows still lack a round-90/91 verdict.

The appendix is the plan's per-control table. A row that has not been adjudicated against the
current code is indistinguishable from one that has, which is how the plan drifted (see
`log-20260924-1.md` §91.3). This prints the gap so the batch write-back can be measured rather
than estimated.

Usage:
  python3 tools/report_appendix_coverage.py            # counts by section
  python3 tools/report_appendix_coverage.py --list     # every un-adjudicated row
"""

import re
import sys

PLAN = "docs/plans/archive/blue23.md"
SECTION = re.compile(r"^#{2,4} A\.(\d+)")
CONTROL_ROW = re.compile(r"^\| `")
# A row counts as adjudicated when its defect cell names the round it was checked in, or
# carries a ✅/⚠️ verdict marker. Both are how this plan records "somebody compared this row
# against the code".
VERDICT = re.compile(r"第 \d+ 轮|✅|⚠️")


def defect_cell(row):
    text = row.strip()
    if text.endswith("|"):
        text = text[:-1]
    parts = re.split(r"(?<!\\)\|", text.strip("|"))
    return parts[3].strip() if len(parts) > 3 else ""


def main(argv):
    lines = open(PLAN, encoding="utf-8").read().splitlines()

    section = None
    in_appendix = False
    per_section = {}
    uncovered = []
    for number, line in enumerate(lines, start=1):
        match = SECTION.match(line)
        if match:
            section = "A.%s" % match.group(1)
            in_appendix = True
            continue
        if line.startswith("## ") and not line.startswith("## A."):
            in_appendix = False
        if line.startswith("## A.10") or line.startswith("## A.11"):
            in_appendix = False
        if not in_appendix or not CONTROL_ROW.match(line):
            continue
        total, done = per_section.get(section, (0, 0))
        total += 1
        if VERDICT.search(defect_cell(line)):
            done += 1
        else:
            uncovered.append((number, section, line))
        per_section[section] = (total, done)

    grand_total = sum(total for total, _ in per_section.values())
    grand_done = sum(done for _, done in per_section.values())
    print("%-8s %6s %6s %6s" % ("section", "rows", "done", "gap"))
    for name in sorted(per_section, key=lambda s: int(s.split(".")[1])):
        total, done = per_section[name]
        print("%-8s %6d %6d %6d" % (name, total, done, total - done))
    print("%-8s %6d %6d %6d" % ("TOTAL", grand_total, grand_done, grand_total - grand_done))

    if "--list" in argv:
        print()
        for number, name, line in uncovered:
            label = line.split("|")[1].strip()
            print("  line %-5d %-6s %s" % (number, name, label))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
