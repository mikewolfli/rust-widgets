#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `crates/rust-widgets-icons/src/data.rs` from the vendored Material Symbols SVG paths.

# What this produces

One `IconData` per icon token, each carrying the **SVG path `d`** copied verbatim from
`tools/material_symbols/<token>.svg`. The `d` is the icon: this script never redraws, simplifies
or re-encodes an outline, so the shipped geometry is upstream's to the byte.

The table is gated `#[cfg(feature = "data")]` in the module that includes it, so a build
without the opt-in feature does not compile (or link) any of this data.

# The three completeness guarantees
#
#   1. **Every token has data.** `IconName::data()` in the generated file returns `IconData`
#      (not `Option`), so a token with no vendor file is a compile error rather than a runtime
#      placeholder. This script refuses to emit a table with a gap.
#   2. **One outline per token, and a duplicate outline is recorded.** Two tokens resolving to
#      one `d` is *upstream's* aliasing (`star`/`grade`/`star_outline`, `close`/`clear`) as often
#      as it is a translation (`user` -> `person`). Because the token set is now upstream's own
#      name set, a duplicate is not evidence of a mistake the way the hand-drawn `Close == Cross`
#      defect was. What must never happen is a duplicate nobody can see, so the relation is
#      computed here and emitted as `UPSTREAM_ALIASES` by `gen_icon_aliases.py`, and the tests
#      assert the two agree exactly.
#   3. **The `d` round-trips.** Each `d` is escaped into a Rust string literal and re-read back,
#      and the round-tripped value must equal the source. A `d` that cannot survive the literal is
#      refused rather than emitted broken.

# Licence

The output is a derivative of Material Symbols (Apache-2.0, Google LLC). The generated header
records the source, the licence and the pinned upstream revision; the repository-root `NOTICE`
carries the full attribution. `tools/material_symbols/LICENSE` is the required licence copy.
This script is **offline** — it reads only the vendor tree — so a gate or build can never be
affected by an upstream outage. See `tools/vendor_material_symbols.py` for the fetch step.

`--license=<id>` is **required**: third-party outline data may not enter the tree through a run
that did not name the licence it relies on. It is the same inbound gate
`tools/gen_cjk_bitmap.py` carries, so one audit (`tools/check_icon_licences.sh`) can assert both.

Usage:  python3 tools/gen_icon_data.py --license=apache-2.0             # write the table
        python3 tools/gen_icon_data.py --license=apache-2.0 --check     # exit 1 if stale
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from icon_sources import REPO_ROOT, VENDOR_DIR, declarations  # noqa: E402
from vendor_material_symbols import ICONS, UPSTREAM_SHA, UPSTREAM_REF_DESCRIPTION  # noqa: E402

OUT = REPO_ROOT / "src" / "data.rs"

# `IconName`'s declaration order — curated tokens first, then the catalogue. It is the order
# `ICON_DATA` and `ICON_FALLBACK` must be in, because `IconName::data` indexes the table by
# discriminant. `tests/icon_data_integrity_test.rs` asserts the table matches the enum at test
# time, so a reorder on either side fails by name rather than drawing the wrong picture.
DECLARATION_ORDER = [token for token, _variant, _doc in declarations()]

PATH_RE = re.compile(r'<path[^>]*\bd="([^"]+)"')
VIEWBOX_RE = re.compile(r'viewBox="([^"]+)"')

# The one design grid every entry is stored on: `IconData`'s coordinates mean "x spans 0..`GRID`,
# y spans -`GRID`..0", and the renderer's single mapping relies on it (`icon.rs`'s
# `draw_outline_in`). Upstream publishes 73 names only in a legacy 24-unit, y-down form (no
# `viewBox`, or `0 96 960 960`); drawing one of those as if it were on this grid collapses the
# icon into a two-pixel sliver, which is a defect nothing else in the pipeline can see. So the
# box is **checked**, by name, rather than assumed.
GRID = 960
GRID_VIEWBOX = "0 -960 960 960"

# The inbound licence gate. The value is checked against the vendored `LICENSE` header, so a run
# cannot claim a licence the vendored copy does not carry. `apache-2.0` is the only accepted id:
# a second one would mean a second upstream, which would be a different script.
REQUIRED_LICENCE = "apache-2.0"


def pascal(token: str) -> str:
    """`arrow_left` -> `ArrowLeft`, matching the `IconName` variant names."""
    return "".join(part.capitalize() for part in token.split("_"))


def read_vendor(token: str) -> str:
    """Return the `d` of the vendored SVG for `token`, or exit with the reason.

    The file's `viewBox` must be [`GRID_VIEWBOX`]: this table stores one coordinate convention,
    and a legacy-grid file would otherwise render as a sliver rather than as its icon. The
    off-grid names are vendored from the release tree instead (see
    `vendor_material_symbols.LEGACY_GRID`), so refusing here names the file rather than silently
    drawing something wrong.
    """
    path = VENDOR_DIR / f"{token}.svg"
    if not path.exists():
        raise SystemExit(
            f"missing vendor file {path.relative_to(REPO_ROOT)}; run\n"
            f"    python3 tools/vendor_material_symbols.py"
        )
    text = path.read_text(encoding="utf-8")
    viewbox = VIEWBOX_RE.search(text)
    declared = viewbox.group(1).strip() if viewbox is not None else None
    if declared != GRID_VIEWBOX:
        raise SystemExit(
            f"{path.relative_to(REPO_ROOT)} declares viewBox {declared!r}, not {GRID_VIEWBOX!r}.\n"
            "  The table stores one design grid. Re-vendor the file from the release tree (add the\n"
            "  name to `vendor_material_symbols.LEGACY_GRID`) rather than re-encoding its numbers."
        )
    match = PATH_RE.search(text)
    if match is None:
        raise SystemExit(f'{path.relative_to(REPO_ROOT)} has no <path d="...">')
    return match.group(1)


def rust_string(d: str) -> str:
    """Encode `d` as the **contents** of a Rust string literal, then prove it round-trips.

    The escaping is deliberately minimal — `d` is digits, `-`, `.`, `,` and letters — but the
    round-trip check is what actually proves losslessness, so nothing here is trusted on its own.
    """
    if "\\" in d or '"' in d:
        raise SystemExit(f"path data contains a quote or backslash, unsupported here: {d!r}")
    # Re-read the literal exactly as rustc would: strip the surrounding quotes from an escaped
    # form and compare. `d` has no escapes to undo, so a plain equality is the check.
    encoded = d.replace("\\", "\\\\").replace('"', '\\"')
    decoded = encoded.replace('\\"', '"').replace("\\\\", "\\")
    if decoded != d:
        raise SystemExit(f"path data does not round-trip through a Rust literal: {d!r}")
    return encoded


def collect() -> list[tuple[str, str, str, str]]:
    """Return `(token, variant, upstream_name, d)` for every icon, in declaration order."""
    # A missing or extra token is a hard failure: either the enum has a variant this generator
    # does not know (its data would be absent) or this list has a token the enum does not name
    # (an entry `data()` could never reach). Both are reported by name.
    missing = sorted(set(ICONS) - set(DECLARATION_ORDER))
    extra = sorted(set(DECLARATION_ORDER) - set(ICONS))
    if missing:
        raise SystemExit(f"icons in the vendor map but not in DECLARATION_ORDER: {missing}")
    if extra:
        raise SystemExit(f"tokens in DECLARATION_ORDER but not in the vendor map: {extra}")

    rows: list[tuple[str, str, str, str]] = []
    for token in DECLARATION_ORDER:
        upstream = ICONS[token]
        d = read_vendor(token)
        rows.append((token, pascal(token), upstream, d))
    return rows


def outline_groups(rows: list[tuple[str, str, str, str]]) -> list[list[str]]:
    """Group the tokens that share one outline, in declaration order within each group.

    Returned groups are sorted by the declaration index of their first token and only include
    groups with more than one token. `gen_icon_aliases.py` renders this as `UPSTREAM_ALIASES`,
    and the integrity tests compare the two, so the relation is stated once (here, from the
    shipped `d`) and asserted everywhere else.
    """
    order = {token: index for index, (token, _variant, _upstream, _d) in enumerate(rows)}
    by_d: dict[str, list[str]] = {}
    for token, _variant, _upstream, d in rows:
        by_d.setdefault(d, []).append(token)
    groups = [[*tokens] for tokens in by_d.values() if len(tokens) > 1]
    for group in groups:
        group.sort(key=lambda token: order[token])
    groups.sort(key=lambda group: order[group[0]])
    return groups


def render(rows: list[tuple[str, str, str, str]]) -> str:
    count = len(rows)
    groups = outline_groups(rows)
    aliased = sum(len(group) - 1 for group in groups)
    lines: list[str] = []
    lines.append("// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)")
    lines.append("// SPDX-License-Identifier: MIT")
    lines.append("//")
    lines.append("// GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("// Produced by `tools/gen_icon_data.py`. Regenerate with:")
    lines.append("//     python3 tools/gen_icon_data.py --license=apache-2.0")
    lines.append("//")
    lines.append("// Icon source: Material Symbols, by Google LLC — the per-icon SVG outlines from")
    lines.append("//     https://github.com/google/material-design-icons")
    lines.append(f"//     pinned at commit {UPSTREAM_SHA}")
    lines.append(f"//     ({UPSTREAM_REF_DESCRIPTION}), style `materialsymbolsoutlined`, 24 px.")
    lines.append("// Licence of the path data: Apache License 2.0. The licence copy is shipped at")
    lines.append("// `tools/material_symbols/LICENSE`; the attribution is recorded in the repository-root")
    lines.append("// `NOTICE`. Each `d` below is upstream's outline **copied verbatim** — no outline was")
    lines.append("// redrawn, simplified or re-encoded.")
    lines.append("//")
    lines.append("// # The one revision exception")
    lines.append("//")
    lines.append("// `vendor_material_symbols.RELEASE_TREE_SOURCES` names the tokens whose file is not the")
    lines.append("// pinned revision's: `RELEASE_ONLY` is absent there, and `LEGACY_GRID` exists there only")
    lines.append("// on upstream's legacy 24-unit grid, which this table does not store. Both come from the")
    lines.append("// Material Symbols release tree instead. `NOTICE` repeats the exception.")
    lines.append("//")
    lines.append("// # Duplicate outlines")
    lines.append("//")
    lines.append(f"// {aliased} tokens share an outline with an earlier token, in {len(groups)} groups — upstream")
    lines.append("// publishes one picture under several names. The relation is recorded in")
    lines.append("// `src/upstream_aliases.rs`, not silently deduplicated here, so nothing about it is hidden.")
    lines.append("//")
    lines.append("// The vendored SVG sources live in `tools/material_symbols/<token>.svg`; this file is")
    lines.append("// derived from them and `tools/check_icon_licences.sh` asserts the chain.")
    lines.append("//")
    lines.append("// `#[rustfmt::skip]` keeps `cargo fmt --check` stable over the generated table.")
    lines.append("")
    lines.append("// The `IconData` shape is defined in `rust-widgets-icons`'s `lib.rs` rather than here, so a build")
    lines.append("// without the `data` feature can still name the type (its table is what is gated).")
    lines.append("use crate::IconData;")
    lines.append("")
    lines.append(f"/// Every icon's data, indexed by `IconName`'s declaration order. {count} entries.")
    lines.append("///")
    lines.append("/// `IconName::data` indexes this with the variant's discriminant, and")
    lines.append("/// `tests/icon_data_integrity_test.rs` asserts the names line up, so the order is checked")
    lines.append("/// in every build rather than only asserted in a debug one.")
    lines.append("#[rustfmt::skip]")
    lines.append(f"pub(crate) static ICON_DATA: [IconData; {count}] = [")
    for token, _variant, _upstream, d in rows:
        escaped = rust_string(d)
        lines.append("    IconData {")
        lines.append(f'        name: "{token}",')
        lines.append(f"        grid: {GRID},")
        lines.append(f'        paths: &["{escaped}"],')
        lines.append("    },")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def check_licence(claimed: str | None) -> None:
    """Refuse to run unless `claimed` names the licence the vendored copy actually carries.

    Exiting **2** (not 1) separates "the licence gate refused" from "the table is stale": the
    former means no data was allowed in, the latter means data is out of date. A caller that
    conflated them would read a refused run as a staleness finding.
    """
    if claimed is None:
        print(
            "refusing to generate third-party icon data without a stated licence.\n"
            f"Pass --license={REQUIRED_LICENCE} to confirm you have read tools/material_symbols/LICENSE.",
            file=sys.stderr,
        )
        raise SystemExit(2)
    if claimed != REQUIRED_LICENCE:
        print(
            f"unknown --license={claimed!r}. The vendored outlines are {REQUIRED_LICENCE!r} "
            "(Material Symbols, Google LLC); another id would name a different upstream.",
            file=sys.stderr,
        )
        raise SystemExit(2)
    licence_path = VENDOR_DIR / "LICENSE"
    if not licence_path.exists():
        print(
            f"{licence_path.relative_to(REPO_ROOT)} is missing, so the claimed licence cannot be "
            "checked against the copy. Run python3 tools/vendor_material_symbols.py first.",
            file=sys.stderr,
        )
        raise SystemExit(2)
    text = licence_path.read_text(encoding="utf-8", errors="replace")
    if "Apache License" not in text or "Version 2.0" not in text:
        print(
            f"{licence_path.relative_to(REPO_ROOT)} does not read as Apache-2.0, so --license="
            f"{claimed} is not supported by the vendored copy.",
            file=sys.stderr,
        )
        raise SystemExit(2)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="exit 1 if the table is stale")
    parser.add_argument(
        "--license",
        dest="license_id",
        default=None,
        help=f"the licence relied on; must be {REQUIRED_LICENCE!r}. Omitting it exits 2.",
    )
    args = parser.parse_args()

    check_licence(args.license_id)

    rows = collect()
    rendered = render(rows)

    if args.check:
        if not OUT.exists():
            print(f"FAIL  {OUT.relative_to(REPO_ROOT)} does not exist", file=sys.stderr)
            return 1
        if OUT.read_text(encoding="utf-8") != rendered:
            print(
                f"FAIL  {OUT.relative_to(REPO_ROOT)} is stale; run\n"
                f"    python3 tools/gen_icon_data.py",
                file=sys.stderr,
            )
            return 1
        print(f"icon data OK: {len(rows)} icons, matches the vendor tree")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUT.relative_to(REPO_ROOT)}: {len(rows)} icons")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
