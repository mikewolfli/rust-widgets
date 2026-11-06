#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `tools/icon_catalog.txt` — the full Material Symbols name set, as declarations.

# What this produces

One `token | Variant | doc` line for every upstream icon name that `tools/icon_tokens.txt` does
not already declare. The result is the **catalogue**: the part of `IconName` a human did not
write. `gen_icon_names.py` concatenates the two files, curated first, which is `IconName`'s
declaration order.

# Why it is a separate file rather than more lines in `icon_tokens.txt`

`icon_tokens.txt` is hand-maintained: its lines carry a reviewed doc sentence each. The
catalogue is 4,248 entries derived from upstream's codepoint file, and mixing generated lines
into the curated file would make `git diff` on a human edit indistinguishable from a refresh.
Two files also keep the curated discriminants fixed: they are declared first, so a catalogue
refresh never moves an existing `IconName`.

# The naming rule

`icon_sources.variant_name` — `pascal(token)`, prefixed with `Num` when the result would start
with a digit (`10k` -> `Num10k`). It lives in that module rather than here because
`gen_icon_names.py`, `gen_icon_data.py` and `gen_icon_fallback.py` must apply the same rule;
four copies would drift.

# Where the names come from

`tools/material_symbols/upstream_codepoints.txt` — upstream's own codepoint list, vendored
verbatim and pinned by digest (`vendor_material_symbols.UPSTREAM_CODEPOINTS_SHA256`). This
script is **offline**: it reads that file and the curated token list, and nothing else.

Usage:
    python3 tools/gen_icon_catalog.py            # write the catalogue
    python3 tools/gen_icon_catalog.py --check    # exit 1 if stale
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from icon_sources import (  # noqa: E402
    CATALOG_PATH,
    DECLARATION_RE,
    REPO_ROOT,
    curated_declarations,
    upstream_codepoints,
    variant_name,
)


def rows() -> list[tuple[str, str, str]]:
    """`(token, variant, doc)` for every upstream name the curated list does not declare."""
    curated = curated_declarations()
    curated_tokens = {token for token, _variant, _doc in curated}
    curated_variants = {variant for _token, variant, _doc in curated}
    out: list[tuple[str, str, str]] = []
    for name, codepoint in upstream_codepoints():
        if name in curated_tokens:
            continue
        if not DECLARATION_RE.match(name):
            raise SystemExit(f"REFUSED: upstream name {name!r} is not a lower_case_underscore token")
        variant = variant_name(name)
        if variant in curated_variants:
            raise SystemExit(
                f"REFUSED: upstream name {name!r} would generate the variant {variant!r}, which "
                "the curated list already declares"
            )
        out.append((name, variant, f"Material Symbols `{name}` (U+{codepoint.upper()})."))
    if not out:
        raise SystemExit("REFUSED: the catalogue came out empty; the codepoint list is wrong")
    return out


def render(entries: list[tuple[str, str, str]]) -> str:
    lines: list[str] = []
    lines.append("# The Material Symbols catalogue — every upstream icon name this crate ships,")
    lines.append("# minus the tokens `icon_tokens.txt` already declares by hand.")
    lines.append("#")
    lines.append("# GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("# Produced by `tools/gen_icon_catalog.py` from")
    lines.append("# `tools/material_symbols/upstream_codepoints.txt` (upstream's own codepoint list,")
    lines.append("# vendored verbatim at the pinned revision). Regenerate with:")
    lines.append("#     python3 tools/gen_icon_catalog.py")
    lines.append("#")
    lines.append("# # Format")
    lines.append("#")
    lines.append("#   <token> | <Variant> | <doc>")
    lines.append("#")
    lines.append("# The same three columns as `icon_tokens.txt`, so the two files concatenate into one")
    lines.append("# declaration list. Order is upstream's codepoint order, which is stable across")
    lines.append("# refreshes that do not add or remove a name — a refresh that does appears as a diff")
    lines.append("# here, which is what a reviewer needs to see.")
    lines.append("#")
    lines.append("# The doc line states the upstream name and its codepoint: the codepoint is what")
    lines.append("# upstream's variable font maps a ligature to, so it is the one fact that ties this")
    lines.append("# line to a specific upstream entry.")
    lines.append("#")
    lines.append(f"# {len(entries)} entries.")
    lines.append("")
    for token, variant, doc in entries:
        lines.append(f"{token} | {variant} | {doc}")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="exit 1 if the file is stale")
    args = parser.parse_args()

    entries = rows()
    rendered = render(entries)

    if args.check:
        if not CATALOG_PATH.exists():
            print(f"FAIL  {CATALOG_PATH.relative_to(REPO_ROOT)} does not exist", file=sys.stderr)
            return 1
        if CATALOG_PATH.read_text(encoding="utf-8") != rendered:
            print(
                f"FAIL  {CATALOG_PATH.relative_to(REPO_ROOT)} is stale; run\n"
                f"    python3 tools/gen_icon_catalog.py",
                file=sys.stderr,
            )
            return 1
        print(f"icon catalogue OK: {len(entries)} entries, matches the codepoint list")
        return 0

    CATALOG_PATH.write_text(rendered, encoding="utf-8")
    print(f"wrote {CATALOG_PATH.relative_to(REPO_ROOT)}: {len(entries)} entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
