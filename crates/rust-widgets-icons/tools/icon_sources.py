#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""The icon sources every generator reads, and the naming rule they share.

# Why this module exists

Four scripts need the same two facts — *which tokens are declared, in what order* and *what the
upstream names are* — and the same naming rule (token -> Rust variant). `gen_icon_names.py`,
`gen_icon_data.py`, `gen_icon_fallback.py`, `gen_icon_aliases.py`, `gen_icon_catalog.py` and
`vendor_material_symbols.py` used to each carry their own copy of the parse; a change to the
`token | Variant | doc` format would then have to be made in six places, and the one that was
missed would read a truncated file rather than fail.

Nothing here imports another `tools/` module, so this file can be imported by all of them —
including `gen_icon_catalog.py`, which runs *before* `tools/icon_catalog.txt` exists and
therefore cannot import `vendor_material_symbols.py` (that module reads the catalog at import
time and refuses to load without it).

# The two declaration files

  * `tools/icon_tokens.txt` — the **curated** tokens. A human wrote the spelling and the doc
    line. They are declared first, so their discriminants do not move when the catalogue is
    refreshed.
  * `tools/icon_catalog.txt` — the rest of upstream's name set, **generated** by
    `tools/gen_icon_catalog.py` from the vendored codepoint list.

Declaration order is curated-then-catalogue, and `IconName`'s variants, `ICON_DATA` and
`ICON_FALLBACK` are all indexed by it.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VENDOR_DIR = REPO_ROOT / "tools" / "material_symbols"
CURATED_PATH = REPO_ROOT / "tools" / "icon_tokens.txt"
CATALOG_PATH = REPO_ROOT / "tools" / "icon_catalog.txt"
CODEPOINTS_PATH = VENDOR_DIR / "upstream_codepoints.txt"

DECLARATION_RE = re.compile(r"^[a-z0-9]+(?:_[a-z0-9]+)*$")

# A Rust identifier cannot start with a digit, and 67 upstream names do (`10k`, `3d_rotation`,
# `360`). The prefix is a naming rule with one job: make the variant spellable without changing
# the token, which is what `as_str` returns and what a host types. `Num` is chosen over a bare
# `N` so the generated name reads as what it is.
DIGIT_PREFIX = "Num"


def pascal(token: str) -> str:
    """`arrow_left` -> `ArrowLeft`."""
    return "".join(part.capitalize() for part in token.split("_"))


def variant_name(token: str) -> str:
    """The `IconName` variant for `token`: `pascal`, plus `Num` when the result starts with a digit."""
    variant = pascal(token)
    return f"{DIGIT_PREFIX}{variant}" if variant[:1].isdigit() else variant


def read_declarations(path: Path) -> list[tuple[str, str, str]]:
    """Parse `token | Variant | doc` lines into `[(token, variant, doc)]`, in file order.

    A missing file is refused rather than treated as empty: an empty token list would generate an
    empty enum, and an empty enum compiles.
    """
    if not path.exists():
        raise SystemExit(f"REFUSED: {path.relative_to(REPO_ROOT)} is missing; there is no token set to generate.")
    rows: list[tuple[str, str, str]] = []
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) != 3:
            raise SystemExit(
                f"REFUSED: {path.relative_to(REPO_ROOT)}:{lineno}: expected "
                f"`token | Variant | doc`, got {line!r}"
            )
        token, variant, doc = parts
        if not token or not variant or not doc:
            raise SystemExit(
                f"REFUSED: {path.relative_to(REPO_ROOT)}:{lineno}: an empty field in {line!r}"
            )
        if not DECLARATION_RE.match(token):
            raise SystemExit(
                f"REFUSED: {path.relative_to(REPO_ROOT)}:{lineno}: {token!r} is not a "
                "lower_case_underscore token"
            )
        rows.append((token, variant, doc))
    return rows


def curated_declarations() -> list[tuple[str, str, str]]:
    """The curated tokens, in their declared order (the first `IconName` discriminants)."""
    return read_declarations(CURATED_PATH)


def catalog_declarations() -> list[tuple[str, str, str]]:
    """The generated catalogue tokens, in their declared order."""
    return read_declarations(CATALOG_PATH)


def declarations() -> list[tuple[str, str, str]]:
    """Every declared token: curated first, then catalogue. This **is** `IconName`'s order."""
    rows = curated_declarations() + catalog_declarations()
    seen_token: dict[str, int] = {}
    seen_variant: dict[str, str] = {}
    for index, (token, variant, _doc) in enumerate(rows):
        if token in seen_token:
            raise SystemExit(
                f"REFUSED: token {token!r} is declared twice (row {seen_token[token]} and {index}); "
                "a catalogue entry must not repeat a curated token"
            )
        if variant in seen_variant:
            raise SystemExit(
                f"REFUSED: variants {seen_variant[variant]} and {token} both name {variant!r}"
            )
        seen_token[token] = index
        seen_variant[variant] = token
    if not rows:
        raise SystemExit("REFUSED: the token set parsed to nothing.")
    return rows


def upstream_codepoints() -> list[tuple[str, str]]:
    """The vendored upstream codepoint list as `[(name, codepoint)]`, in upstream order."""
    if not CODEPOINTS_PATH.exists():
        raise SystemExit(
            f"REFUSED: {CODEPOINTS_PATH.relative_to(REPO_ROOT)} is missing; run\n"
            "    python3 tools/vendor_material_symbols.py --refresh"
        )
    rows: list[tuple[str, str]] = []
    for raw in CODEPOINTS_PATH.read_text(encoding="utf-8").splitlines():
        fields = raw.split()
        if len(fields) == 2:
            rows.append((fields[0], fields[1]))
    if not rows:
        raise SystemExit(f"REFUSED: {CODEPOINTS_PATH.relative_to(REPO_ROOT)} parsed to nothing.")
    return rows
