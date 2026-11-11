#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Check that every documented icon count matches `crates/rust-widgets-icons/tools/icon_tokens.txt`.

# Why this exists

The token set has one declaration — `crates/rust-widgets-icons/tools/icon_tokens.txt` — and several prose copies. Those copies
drifted: the two READMEs and the cookbook all said **68** while the file held **69**, and nothing
noticed, because no gate looked at the number. The widget-kind count has exactly this gate
(`tools/check_widget_kind_count.sh`), which records the rule in its own header: *a documented number
needs a check tied to the single source of truth*.

# Why the pattern set is the point

A count check is only as wide as the spellings it knows. The widget-kind gate learned this the hard
way — it matched `181 kinds` and missed `180 controls`, `180 个控件` and `180 Widget Kinds`, so three
documents sat stale while the gate reported success. This file therefore lists the spellings the tree
actually uses, and adding a new one is the expected maintenance step: a spelling that is not listed is
silently unchecked, which is worse than not having the gate at all, because the gate prints a pass.

# Usage

    tools/icon_count_claim_scan.py <declared-count>

Exit 0 = every documented count agrees. Exit 1 = a finding is printed for each disagreement.
"""

import re
import sys
from pathlib import Path

DOCS = [
    "README.md",
    "README.zh-CN.md",
    "cookbook/en/src/chapters/architecture.md",
    "cookbook/zh-CN/src/chapters/architecture.md",
    "cookbook/zh-TW/src/chapters/architecture.md",
]


def build_patterns() -> list[re.Pattern]:
    """Every spelling the tree uses for "the number of `IconName` tokens".

    The number is past a thousand now, so the digit width is `1..6` and a thousands separator is
    part of the spelling: `4,323` is what the documents write and `4323` is what
    `icon_tokens.txt` counts. `main` normalises the separator before comparing, so a document that
    writes `4,323` and one that writes `4323` are both accepted.
    """
    count = r"(\d{1,6}(?:,\d{3})*)"
    return [
        # 75 `IconName` tokens  /  4,323 `IconName` tokens  /  **69** `IconName` tokens
        re.compile(rf"{count}\D{{0,4}}`?IconName`?\s+tokens?\b"),
        # 75 个 `IconName` token  /  4,323 個 `IconName` token
        re.compile(rf"{count}\D{{0,4}}(?:个|個)\s*`?IconName`?\s*token"),
        # `Icon` names 4,323 icons through `IconName`.
        re.compile(rf"{count}\s+icons?\b(?=[^.]*IconName)"),
        # ENUM["IconName<br/>(4,323 variants)"]
        re.compile(rf"IconName\D{{0,30}}\({count}\s+variants?\)"),
    ]


def main(argv: list[str]) -> int:
    if len(argv) != 2 or not argv[1].isdigit():
        print("usage: icon_count_claim_scan.py <declared-count>", file=sys.stderr)
        return 2
    actual = argv[1]
    patterns = build_patterns()

    errors = 0
    checked = 0
    for name in DOCS:
        path = Path(name)
        if not path.is_file():
            # A missing document must fail the gate: silently skipping it shrinks coverage while
            # still printing a pass, which is the failure mode this script exists to prevent.
            print(
                f"❌ {name}: listed in DOCS but missing; "
                "deleting a chapter, or moving it, must fail this gate instead of shrinking coverage",
                file=sys.stderr,
            )
            errors += 1
            continue
        checked += 1
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        for pattern in patterns:
            for match in pattern.finditer(text):
                if match.group(1).replace(",", "") == actual:
                    continue
                lineno = text.count("\n", 0, match.start()) + 1
                snippet = lines[lineno - 1].strip() if lineno <= len(lines) else ""
                print(
                    f"❌ {name}:{lineno} states '{match.group(1)}' icons but "
                    f"crates/rust-widgets-icons/tools/icon_tokens.txt declares {actual}: {snippet}",
                    file=sys.stderr,
                )
                errors += 1

    if checked == 0:
        print("❌ no document was scanned; the list in DOCS is stale", file=sys.stderr)
        return 1
    if errors:
        return 1
    # Printed only on a real pass: a "5 document(s) agree" line beside a finding is exactly the kind
    # of output that teaches a reader to skim past the failure.
    print(f"icon-count claim scan: {checked} document(s) agree on {actual}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
