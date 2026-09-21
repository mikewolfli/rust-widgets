#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Semantic-colour census (BLUE20 layer 1, rule #109).

Answers one question with evidence: for each of the four semantic tokens the theme
declares (`error` / `warning` / `success` / `info`), **which control reads it**.

# Why this is a gate and not a note

Before BLUE20 the answer was "none". `Theme::colors` declared four tokens and the
control layer referenced them **zero** times, so a theme author could change
`error` and nothing on screen moved. That is the same defect class as "an event is
published but never emitted": a declaration with no consumer. It is invisible
because nothing is broken in isolation — only the *pair* (declaration, consumer) is
meaningful, and only a census can see the pair is missing a side.

# What counts as a consumer

A control reads a token when its drawing path goes through
`theme::semantic_color(...)` (directly, or through `BannerSeverity::semantic()`).
Finding the *mapping* is not enough — a mapping that is never called is still a
declaration with no consumer — so the checker requires both:

  1. the token appears in some control's severity/role mapping, **and**
  2. that mapping reaches `semantic_color`.

Both are source-text facts, which is honest: the alternative (render every control
and diff the token's colour into the output) cannot attribute a pixel to a token.

Exit 0 = every token has a consumer. Exit 1 = a token is unread.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src"

TOKENS = ("info", "success", "warning", "error")

# The call that turns a token into a colour. A control must reach this.
CONSUMER_CALL = "semantic_color"


def control_sources() -> list[Path]:
    """Every Rust file under `src/widget/`, the control layer."""
    widget_dir = SRC / "widget"
    if not widget_dir.is_dir():
        return []
    return sorted(widget_dir.rglob("*.rs"))


def token_referencing_files(token: str) -> list[Path]:
    """Files that name `SemanticColor::<Token>`."""
    pattern = re.compile(rf"SemanticColor::{token}\b", re.IGNORECASE)
    return [path for path in control_sources() if pattern.search(path.read_text(encoding="utf-8", errors="replace"))]


def files_calling_the_consumer() -> list[Path]:
    """Files that actually call `semantic_color(...)`."""
    return [
        path
        for path in control_sources()
        if CONSUMER_CALL in path.read_text(encoding="utf-8", errors="replace")
    ]


def main() -> int:
    consumers = set(files_calling_the_consumer())
    if not consumers:
        print("FAIL  no control calls `semantic_color`: every semantic token is an empty declaration")
        print("      the theme declares the tokens and nothing reads them (rule #109)")
        return 1

    print("token     consumers")
    print("--------  ------------------------------------------------")
    missing: list[str] = []
    for token in TOKENS:
        referencing = token_referencing_files(token)
        # A consumer must both *name* the token and *call* the accessor: naming it
        # in a match arm that returns a literal would look like a consumer and be
        # none.
        readers = [path for path in referencing if path in consumers]
        if not readers:
            missing.append(token)
            print(f"{token:<9} (none)")
            continue
        names = ", ".join(str(path.relative_to(ROOT)) for path in readers)
        print(f"{token:<9} {names}")

    if missing:
        print("")
        print(f"FAIL  these semantic tokens have no consumer: {', '.join(missing)}")
        print("      a theme that changes them would change nothing on screen")
        return 1

    print("")
    print(f"checked={len(TOKENS)} skipped=0 failed=0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
