#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: the two `WidgetKind` alias tables must name the same pairs (BLUE25 C-7).

# The rule

`src/widget/capability.rs` resolves an alias kind two ways:

  * `alias_factory_name` — a `match` on `WidgetKind` variants, compiled `full_widgets`;
  * `alias_for_name`     — a `match` on the **canonical spelling** string, compiled
                           `all(widgets_unstripped, not(full_widgets))`.

A `cfg` boundary separates the two, so **no single build sees both**. That is exactly how a
copy drifts: adding a row to one table and not the other compiles everywhere and silently
gives one profile a different alias than the other. The module comment used to claim the two
"cannot disagree"; they could, because they were two spellings with nothing comparing them.

# What this checks

Parse both tables from the source and compare their `(spelling, target)` pair sets. The
variant-keyed table gives its spelling by snake_casing the variant name (the same transform
`kind_canonical_name_into` performs at runtime), so the comparison is between the two
*spellings of the same pair*.

# What it does NOT prove

    * It is lexical, and reads the tables by their `match` shape. If either is rewritten to a
      table-driven form, the extractor must be updated with it — which is why it names the
      tables it could not find rather than reporting an empty success.

# Reverse injection

Deleting one row from either table must make this fail with that pair named. See the round's
report for the exact output.

Usage:  python3 tools/check_alias_tables_agree.py
Exit 0 = the two alias tables name the same pairs.
Exit 1 = a finding (each disagreeing pair is named, with which table holds it).
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SOURCE = REPO / "src/widget/capability.rs"


def snake_case(name: str) -> str:
    """`ActivityIndicator` -> `activity_indicator`, matching `kind_canonical_name_into`."""
    out = []
    for i, ch in enumerate(name):
        if ch.isupper() and i > 0 and not name[i - 1].isupper():
            out.append("_")
        out.append(ch.lower())
    return "".join(out)


def extract_variant_table(text: str) -> dict[str, str]:
    """The `(WidgetKind::X, "target")` rows of `alias_factory_name`."""
    start = text.index("fn alias_factory_name(")
    # The rows end at the first `other =>` arm, which is the wildcard.
    end = text.index("other =>", start)
    body = text[start:end]
    pairs: dict[str, str] = {}
    for variant, target in re.findall(
        r"WidgetKind::(\w+)\s*=>\s*\"([^\"]+)\"", body
    ):
        pairs[snake_case(variant)] = target
    return pairs


def extract_name_table(text: str) -> dict[str, str]:
    """The `("spelling" => "target")` rows of `alias_for_name`."""
    start = text.index("fn alias_for_name(")
    body = text[start:]
    pairs: dict[str, str] = {}
    for spelling, target in re.findall(r"\"([^\"]+)\"\s*=>\s*\"([^\"]+)\"", body):
        pairs[spelling] = target
    return pairs


def main() -> int:
    text = SOURCE.read_text(encoding="utf-8")

    try:
        variants = extract_variant_table(text)
    except ValueError:
        print("FAIL  could not locate `alias_factory_name` or its wildcard arm")
        return 1
    try:
        names = extract_name_table(text)
    except ValueError:
        print("FAIL  could not locate `alias_for_name`")
        return 1

    if not variants:
        print("FAIL  `alias_factory_name` yielded no rows; the extractor no longer matches")
        return 1
    if not names:
        print("FAIL  `alias_for_name` yielded no rows; the extractor no longer matches")
        return 1

    findings: list[str] = []
    for spelling, target in variants.items():
        # An `X -> x` identity row needs no entry in the name table: `canonical_name_for_kind`
        # already returns the spelling for such a kind, so `alias_for_name` correctly answers
        # `None` and the caller falls through to that table. The identity rows are exactly
        # `panel` and `menu_item`.
        if spelling == target:
            continue
        if spelling not in names:
            findings.append(
                f"`{spelling}` -> `{target}` is in alias_factory_name but not alias_for_name"
            )
        elif names[spelling] != target:
            findings.append(
                f"`{spelling}` targets `{target}` (variant table) but `{names[spelling]}` (name table)"
            )
    for spelling, target in names.items():
        if spelling not in variants:
            findings.append(
                f"`{spelling}` -> `{target}` is in alias_for_name but not alias_factory_name"
            )

    print(
        f"alias tables: {len(variants)} variant rows, {len(names)} name rows, "
        f"{len(findings)} disagreement(s)"
    )
    if findings:
        for finding in findings:
            print(f"  FAIL  {finding}")
        print(
            "        The two alias tables are compiled under different `cfg`s, so only a source\n"
            "        comparison can keep them in step. Add the row to both, or to neither."
        )
        return 1

    print("  PASS  both tables name the same pairs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
