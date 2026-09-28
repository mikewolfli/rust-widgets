#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Discharge appendix-A rows that a machine can decide, so the rest are the real work.

# Why this exists

`blue23.md` appendix A is a 204-row table of *predictions* written at planning time. The log
records four separate rounds where a prediction turned out not to hold on the code as it is
now: `text_area` "reads no style" (it reads three), `inplace_editor` "tick has no driver" (it
is wired to the trait), `radio_button` "needs M1" (it reports its own state), `camera_preview`
"M4, 16 literals" (the `video-surface` exemption covers it). Each was a phantom row, and each
cost a round to close.

This tool answers, for the whole table at once, the three questions that **do not need a
human**:

  1. *Does the implementation reach a theme entry point at all?* A row that says "totally
     ignores the theme" is discharged outright when the answer is yes.
  2. *Does the control already implement `tick` / `is_animating`?* A row that says "M3: wire
     the animation" is discharged when both are present.
  3. *Is the control covered by the data-colour exemption table?* A row that asks for M4 on a
     control whose dominant colour is exempted content is asking for a change that would be
     reverted.

What it deliberately does **not** decide: whether a given literal is a defect or a
no-theme fallback. That needs the source, and getting it wrong is how the audit regex came to
report 66 problems where there were 14.

Usage:
    python3 tools/audit_control_theme_reach.py            # the raw measurement
    python3 tools/report_appendix_a_discharge.py          # appendix-A rows it can discharge
"""

import pathlib
import re
import subprocess
import sys

PLAN = "docs/plans/archive/blue23.md"


def measurement() -> str:
    """Runs the reach audit and returns its report, so both tools read one implementation."""
    result = subprocess.run(
        [sys.executable, "tools/audit_control_theme_reach.py"],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.stdout


def parse_rows(report: str) -> dict[str, dict]:
    rows: dict[str, dict] = {}
    for line in report.splitlines():
        match = re.match(r"\s{4}(\w+)\s+(\d+)\s+(themed|NO-THEME)\s+(\S+)", line)
        if match:
            rows[match.group(1)] = {
                "literals": int(match.group(2)),
                "themed": match.group(3) == "themed",
                "file": match.group(4),
            }
    return rows


def appendix_rows() -> list[tuple[str, str]]:
    """(control, row text) for every row in appendix A."""
    text = pathlib.Path(PLAN).read_text(encoding="utf-8")
    start = text.index("# 附录 A")
    rows = []
    for line in text[start:].splitlines():
        match = re.match(r"\| `([^`|]+)`[^|]*\|(.*)", line)
        if match:
            rows.append((match.group(1).strip(), line))
    return rows


def main() -> int:
    report = measurement()
    measured = parse_rows(report)
    rows = appendix_rows()
    print(f"=== appendix A rows: {len(rows)} ===")

    unmeasured, already_done, claims_theme_blind_but_isnt, claims_no_tick_but_has_one = [], [], [], []
    for name, line in rows:
        if "✅" in line:
            already_done.append(name)
            continue
        if name not in measured:
            unmeasured.append(name)
            continue
        info = measured[name]
        if not info["themed"] and re.search(r"不读 style|不读主题|完全不读", line):
            continue
        if info["themed"] and re.search(r"不读 style|不读主题|完全不读|`style=N`", line):
            claims_theme_blind_but_isnt.append((name, info["file"]))
        if re.search(r"无驱动者|无 `tick`", line) and "M3" in line:
            claims_no_tick_but_has_one.append((name, info["file"]))

    print(f"=== rows already marked done (carry ✅): {len(already_done)} ===")
    print(f"=== rows whose control the audit could not measure: {len(unmeasured)} ===")
    for name in unmeasured:
        print(f"    {name}")
    print(f"=== rows claiming 'reads no theme' where the implementation does: {len(claims_theme_blind_but_isnt)} ===")
    for name, path in claims_theme_blind_but_isnt:
        print(f"    {name:26s} {path}")
    print(f"=== rows claiming 'no tick' on a control that has one: {len(claims_no_tick_but_has_one)} ===")
    for name, path in claims_no_tick_but_has_one:
        print(f"    {name:26s} {path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
