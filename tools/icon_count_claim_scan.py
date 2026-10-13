#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Check that every documented icon count matches `tools/icon_tokens.txt`.

# Why this exists

The token set has one declaration — `tools/icon_tokens.txt` — and several prose copies. Those copies
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
    """Every spelling the tree uses for "the number of `IconName` tokens"."""
    return [
        # 69 `IconName` tokens  /  **69** `IconName` tokens
        re.compile(r"(\d{2,3})\D{0,4}`?IconName`?\s+tokens?\b"),
        # 69 个 `IconName` token  /  69 個 `IconName` token
        re.compile(r"(\d{2,3})\D{0,4}(?:个|個)\s*`?IconName`?\s*token"),
        # `Icon` names 69 icons through `IconName`.
        re.compile(r"(\d{2,3})\s+icons?\b(?=[^.]*IconName)"),
        # ENUM["IconName<br/>(69 variants)"]
        re.compile(r"IconName\D{0,30}\((\d{2,3})\s+variants?\)"),
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
            continue
        checked += 1
        text = path.read_text(encoding="utf-8")
        lines = text.splitlines()
        for pattern in patterns:
            for match in pattern.finditer(text):
                if match.group(1) == actual:
                    continue
                lineno = text.count("\n", 0, match.start()) + 1
                snippet = lines[lineno - 1].strip() if lineno <= len(lines) else ""
                print(
                    f"❌ {name}:{lineno} states '{match.group(1)}' icons but "
                    f"tools/icon_tokens.txt declares {actual}: {snippet}",
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
