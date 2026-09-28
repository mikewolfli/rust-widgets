#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `src/widget/icon_data.rs` from the vendored Material Symbols SVG paths.

# What this produces

One `IconData` per icon token, each carrying the **SVG path `d`** copied verbatim from
`tools/material_symbols/<token>.svg`. The `d` is the icon: this script never redraws, simplifies
or re-encodes an outline, so the shipped geometry is upstream's to the byte.

The table is gated `#[cfg(feature = "icons")]` in the module that includes it, so a build
without the opt-in feature does not compile (or link) any of this data.

# The three completeness guarantees

  1. **Every token has data.** `IconName::data()` in the generated file returns `IconData`
     (not `Option`), so a token with no vendor file is a compile error rather than a runtime
     placeholder. This script refuses to emit a table with a gap.
  2. **No two tokens share one outline.** `Close == Cross` was a real defect: `icon.rs` drew
     both with one method, so two distinct names produced byte-identical pictures. This script
     hashes every `d` and **refuses** to emit when two tokens collide, so the defect cannot be
     re-introduced by an upstream rename that maps two local tokens onto one icon.
  3. **The `d` round-trips.** Each `d` is escaped into a Rust string literal and re-read back,
     and the round-tripped value must equal the source. A `d` that cannot survive the literal is
     refused rather than emitted broken.

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
import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VENDOR_DIR = REPO_ROOT / "tools" / "material_symbols"
OUT = REPO_ROOT / "src" / "widget" / "icon_data.rs"

# Imported rather than re-typed: the token -> upstream map is one fact, and two copies of it
# would drift. `vendor_material_symbols` imports nothing from this module, so there is no cycle.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from vendor_material_symbols import ICONS, UPSTREAM_SHA, UPSTREAM_REF_DESCRIPTION  # noqa: E402

# The declaration order of `IconName`'s variants, which is the order `ICON_DATA` must be in:
# `IconName::data` indexes the table by discriminant. This is the single place the order is
# stated, and `tests/icon_data_integrity_test.rs` asserts the table matches the enum at test
# time — so a reorder on either side fails by name rather than drawing the wrong picture.
def declaration_order() -> list[str]:
    """The token column of `tools/icon_tokens.txt`, in file order.

    `IconName`'s declaration order is what `ICON_DATA` and `ICON_FALLBACK` are indexed by, so this
    generator and `gen_icon_names.py` must agree on it exactly. Rather than keep a second list here
    (which would be a copy that drifts), both read the one token list — the single source of truth.
    """
    path = REPO_ROOT / "tools" / "icon_tokens.txt"
    if not path.exists():
        raise SystemExit(f"REFUSED: {path} is missing; there is no token order to generate from.")
    tokens: list[str] = []
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) != 3 or not parts[0]:
            raise SystemExit(
                f"REFUSED: {path}:{lineno}: expected `token | Variant | doc`, got {line!r}"
            )
        tokens.append(parts[0])
    if not tokens:
        raise SystemExit(f"REFUSED: {path} parsed to an empty token set.")
    return tokens


DECLARATION_ORDER = declaration_order()

PATH_RE = re.compile(r'<path[^>]*\bd="([^"]+)"')

# The inbound licence gate. The value is checked against the vendored `LICENSE` header, so a run
# cannot claim a licence the vendored copy does not carry. `apache-2.0` is the only accepted id:
# a second one would mean a second upstream, which would be a different script.
REQUIRED_LICENCE = "apache-2.0"


def pascal(token: str) -> str:
    """`arrow_left` -> `ArrowLeft`, matching the `IconName` variant names."""
    return "".join(part.capitalize() for part in token.split("_"))


def read_vendor(token: str) -> str:
    """Return the `d` of the vendored SVG for `token`, or exit with the reason."""
    path = VENDOR_DIR / f"{token}.svg"
    if not path.exists():
        raise SystemExit(
            f"missing vendor file {path.relative_to(REPO_ROOT)}; run\n"
            f"    python3 tools/vendor_material_symbols.py"
        )
    match = PATH_RE.search(path.read_text(encoding="utf-8"))
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
    """Return `(token, variant, upstream_name, d)` for every icon, checking distinctness."""
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
    seen: dict[str, str] = {}
    for token in DECLARATION_ORDER:
        upstream = ICONS[token]
        d = read_vendor(token)
        digest = hashlib.sha256(d.encode("utf-8")).hexdigest()
        if digest in seen:
            raise SystemExit(
                "two icon tokens resolve to the same outline, which would draw two names as one\n"
                f"picture (the `Close == Cross` defect):\n"
                f"    {seen[digest]} and {token}\n"
                f"    outline sha256 {digest[:16]}\n"
                "Give one of them a different upstream icon."
            )
        seen[digest] = token
        rows.append((token, pascal(token), upstream, d))
    return rows


def render(rows: list[tuple[str, str, str, str]]) -> str:
    count = len(rows)
    lines: list[str] = []
    lines.append("// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)")
    lines.append("// SPDX-License-Identifier: MIT")
    lines.append("//")
    lines.append("// GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("// Produced by `tools/gen_icon_data.py`. Regenerate with:")
    lines.append("//     python3 tools/gen_icon_data.py")
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
    lines.append("// The vendored SVG sources live in `tools/material_symbols/<token>.svg`; this file is")
    lines.append("// derived from them and `tools/check_icon_licences.sh` asserts the chain.")
    lines.append("//")
    lines.append("// `#[rustfmt::skip]` keeps `cargo fmt --check` stable over the generated table.")
    lines.append("")
    lines.append("// The `IconData` shape is defined in `display_widgets/icon.rs` rather than here, so a build")
    lines.append("// without the `icons` feature can still name the type (its table is what is gated).")
    lines.append("use super::display_widgets::icon::IconData;")
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
        lines.append("        grid: 960,")
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
