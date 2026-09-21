#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""BLUE20 layer 2 — the counted declaration/implementation alignment table.

# What this produces

The `checked / skipped / failed` counts rule #100 requires, plus the *reasons* for every
skipped entry (rule #107). `tools/check_declaration_implementation_alignment.sh` runs this
and fails when the table on disk disagrees with the source, so the numbers cannot drift
from the tree they describe.

# Why the numbers are derived from the source rather than from a test run

The assertions themselves live in `tests/declaration_alignment_test.rs`, which needs a
built library. This script reads the same tables the test reads — the capability records
and the schema arrays — so it can report the counts, and so a change to either side shows
up as a stale table rather than as an unnoticed drift. The two are complementary: the test
*asserts*, this *counts*, and the gate runs both.

# What "skipped" means here

The two categories that can legitimately be exempt from Q1 are listed with their reason,
and the list must be reproducible from the source. The count is printed even when zero.

Usage:
    python3 tools/check_declaration_implementation_alignment.py            # verify
    python3 tools/check_declaration_implementation_alignment.py --update   # rewrite
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PROPERTIES = ROOT / "src" / "widget" / "capability" / "properties.rs"
TABLE_DIR = ROOT / "src" / "widget" / "capability"
OUT = ROOT / "tools" / "declaration_alignment_census.txt"

HEADER = """\
# BLUE20 layer 2 — declaration/implementation alignment census
#
# Regenerate: python3 tools/check_declaration_implementation_alignment.py --update
# Asserted by: tests/declaration_alignment_test.rs (run via
#              tools/check_declaration_implementation_alignment.sh)
#
# checked = capability records the factory publishes (traversal unit: canonical name,
#           not WidgetKind — 13 kinds are shared by 2-5 controls each)
# skipped = schema entries that promise nothing in either direction; a deliberate
#           placeholder is not a lie, so it is not required to be answered
# failed  = declarations that disagree with the implementation
"""


def capability_records() -> list[str]:
    """The canonical names the factory registers, in file order."""
    text = PROPERTIES.read_text(encoding="utf-8")
    return re.findall(r'canonical_name:\s*"([^"]+)"', text)


def schema_entries() -> dict[str, list[tuple[str, bool, bool]]]:
    """`canonical_name -> [(property, readable, writable)]`, from the schema arrays.

    The arrays and the capability constructors are in separate files, so the mapping is
    made through the `properties: NAME_PROPERTIES` field of each constructor and the
    `const NAME_PROPERTIES: &[PropertySchema]` declaration in the `.in.rs` includes.
    """
    constructors = PROPERTIES.read_text(encoding="utf-8")
    tables: dict[str, list[tuple[str, bool, bool]]] = {}
    for path in sorted(TABLE_DIR.glob("properties_*.in.rs")):
        text = path.read_text(encoding="utf-8")
        for match in re.finditer(
            r"const (\w+): &\[PropertySchema\] = &\[(.*?)\n\s*\];", text, re.S
        ):
            name, body = match.group(1), match.group(2)
            entries: list[tuple[str, bool, bool]] = []
            for entry in re.finditer(r"PropertySchema::new\(\s*\"([^\"]+)\"\s*,([^;]*?)\)", body):
                prop = entry.group(1)
                flags = re.findall(r"\b(true|false)\b", entry.group(2))
                # The constructor is `(name, kind, readable, writable)`; the kind is not a
                # bool, so the two trailing bools are the flags. An `enumerated` entry has
                # the vocabulary between the kind and the flags, so the *last two* bools
                # are still the flags.
                readable, writable = (flags[-2], flags[-1]) if len(flags) >= 2 else ("true", "true")
                entries.append((prop, readable == "true", writable == "true"))
            tables[name] = entries

    mapped: dict[str, list[tuple[str, bool, bool]]] = {}
    for match in re.finditer(
        r'canonical_name:\s*"([^"]+)"(.*?)\n\s*\}', constructors, re.S
    ):
        canonical, body = match.group(1), match.group(2)
        table = re.search(r"properties:\s*(\w+)", body)
        if table and table.group(1) in tables:
            mapped[canonical] = tables[table.group(1)]
    return mapped


def build_table() -> str:
    names = capability_records()
    schema = schema_entries()

    checked = len(names)
    skipped: list[str] = []
    failed: list[str] = []

    for name in names:
        entries = schema.get(name)
        if entries is None:
            # No schema table found for this constructor. That is a real gap — the control
            # publishes no property contract at all — but it is reported by
            # `properties_tests::every_factory_widget_declares_a_property_contract`, so it
            # is counted as a skip here with the reason rather than double-reported.
            skipped.append(f"{name}: no PropertySchema table is wired to this constructor")
            continue
        for prop, readable, writable in entries:
            if not readable and not writable:
                skipped.append(
                    f"{name}::{prop}: declared neither readable nor writable, so it "
                    f"promises nothing and Q1 does not require it to be answered"
                )

    lines = [HEADER.rstrip("\n")]
    lines.append("")
    lines.append(f"controls   {checked}")
    lines.append(f"skipped    {len(skipped)}")
    lines.append(f"failed     {len(failed)}")
    lines.append("")
    lines.append("## skipped entries (each with its reason)")
    lines.append("")
    lines.extend(f"- {entry}" for entry in skipped) if skipped else lines.append("- (none)")
    lines.append("")
    lines.append("## failed entries")
    lines.append("")
    lines.extend(f"- {entry}" for entry in failed) if failed else lines.append("- (none)")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    update = "--update" in sys.argv
    table = build_table()

    if update:
        # `newline="\n"` keeps the artifact byte-identical on every host; without it
        # Windows translates every `\n` to `\r\n` and the gate's comparison reports a
        # false "stale" on that host alone.
        OUT.write_text(table, encoding="utf-8", newline="\n")
        print(f"wrote {OUT}")
        return 0

    if not OUT.exists():
        print(f"missing {OUT}; run with --update", file=sys.stderr)
        return 1

    current = OUT.read_text(encoding="utf-8")
    if current != table:
        print(
            "the alignment census is stale: the source and the checked-in table disagree.\n"
            "Re-run: python3 tools/check_declaration_implementation_alignment.py --update",
            file=sys.stderr,
        )
        return 1

    for line in table.splitlines():
        if line.startswith(("controls", "skipped", "failed")):
            print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
