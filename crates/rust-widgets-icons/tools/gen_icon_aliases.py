#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `crates/rust-widgets-icons/src/upstream_aliases.rs` — the same-picture relation.

# What this produces

One `(alias, canonical)` pair for every token whose outline is byte-identical to an earlier
token's outline. `canonical` is the first-declared token of its group, so the relation is
deterministic given the declaration order.

# Why the relation is data rather than a refused condition

`gen_icon_data.py` used to **refuse** two tokens sharing one `d`, because the hand-drawn set
had a real `Close == Cross` defect: `icon.rs` drew two distinct names with one method. That
guard was right for a hand-translated table. It is the wrong shape now: the token set is
upstream's own name set, and upstream publishes one picture under several names on purpose
(`star` / `grade` / `star_outline`; `close` / `clear`; `phone` / `call` / `local_phone`).
Refusing the whole catalogue because upstream aliases its own icons would force this crate to
drop real names.

What the guard existed to prevent — *a duplicate nobody can see* — is still prevented, by
publishing the relation instead of permitting it quietly:

  * this file is generated from the shipped `d`, so it cannot disagree with the data;
  * `tests/icon_data_integrity_test.rs` asserts that the set of duplicate outlines is **exactly**
    the set listed here, so a new duplicate that this file does not name fails the test;
  * a host may read [`UPSTREAM_ALIASES`] to group an icon picker, which is what makes the
    relation useful rather than merely tolerated.

# Licence

Emits *code* naming upstream's own relation; the geometry it describes is vendored by
`vendor_material_symbols.py` and attributed in `NOTICE`. Offline: reads the vendor tree through
`gen_icon_data.py`, nothing else.

Usage:
    python3 tools/gen_icon_aliases.py            # write the file
    python3 tools/gen_icon_aliases.py --check    # exit 1 if stale
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_icon_data import collect, outline_groups  # noqa: E402
from icon_sources import REPO_ROOT, declarations, variant_name  # noqa: E402

OUT = REPO_ROOT / "src" / "upstream_aliases.rs"


def pairs() -> list[tuple[str, str]]:
    """`(alias_token, canonical_token)` for every token that shares an outline, in order."""
    rows = collect()
    variants = {token: variant for token, variant, _doc in declarations()}
    out: list[tuple[str, str]] = []
    for group in outline_groups(rows):
        canonical = group[0]
        for alias in group[1:]:
            out.append((alias, canonical))
        if canonical not in variants:
            raise SystemExit(f"REFUSED: {canonical!r} has no IconName variant")
    return out


def render(entries: list[tuple[str, str]]) -> str:
    count = len(entries)
    lines: list[str] = []
    lines.append("// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)")
    lines.append("// SPDX-License-Identifier: MIT")
    lines.append("//")
    lines.append("// GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("// Produced by `tools/gen_icon_aliases.py` from the vendored outlines. Regenerate with:")
    lines.append("//     python3 tools/gen_icon_aliases.py")
    lines.append("//")
    lines.append("// Every token whose outline is byte-identical to an earlier token's outline. Material")
    lines.append("// Symbols publishes one picture under several names (`star`/`grade`/`star_outline`,")
    lines.append("// `close`/`clear`, `phone`/`call`), and this crate's curated spellings add a few more")
    lines.append("// (`user`/`person`, `cross`/`cancel`). The relation is listed rather than deduplicated,")
    lines.append("// so nothing about it is implicit; `tests/icon_data_integrity_test.rs` asserts this table")
    lines.append("// is exactly the set of duplicate outlines the data has.")
    lines.append("")
    lines.append("use crate::IconName;")
    lines.append("")
    lines.append("/// `(alias, canonical)` pairs: `alias` draws exactly the same outline as `canonical`.")
    lines.append("///")
    lines.append("/// `canonical` is the first-declared token of its outline group, so the pairing is stable")
    lines.append("/// across regenerations. Both members are real tokens — an alias is not a spelling of the")
    lines.append("/// canonical name, it is its own name for the same picture.")
    lines.append(f"pub const UPSTREAM_ALIASES: [(&str, IconName); {count}] = [")
    for alias, canonical in entries:
        lines.append(f'    ("{alias}", IconName::{variant_name(canonical)}),')
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="exit 1 if the file is stale")
    args = parser.parse_args()

    rendered = render(pairs())

    if args.check:
        if not OUT.exists():
            print(f"FAIL  {OUT.relative_to(REPO_ROOT)} does not exist", file=sys.stderr)
            return 1
        if OUT.read_text(encoding="utf-8") != rendered:
            print(
                f"FAIL  {OUT.relative_to(REPO_ROOT)} is stale; run\n"
                f"    python3 tools/gen_icon_aliases.py",
                file=sys.stderr,
            )
            return 1
        print("icon aliases OK: matches the duplicate outlines in the vendor tree")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUT.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
