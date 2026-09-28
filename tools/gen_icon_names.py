#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate `src/widget/display_widgets/icon_names.rs` from `tools/icon_tokens.txt`.

# What this produces

The whole `IconName` type: the enum and its variants with their doc comments, `as_str`, `from_name`,
`ALL`, `all_tokens`, and `data` / `data_opt`. One generated file, one source of truth.

# Why the type is generated

Adding an icon used to mean editing six hand-written places — the variant, the `as_str` arm, the
`from_name` arm, `ALL`, `all_tokens`, and three separate `31` length literals. A change that missed
one of the six is a bug no compiler can see (`ALL` shorter than the enum compiles; a missing
`as_str` arm does not, but a missing `ALL` entry does). Generating the type from one list makes that
class of drift unrepresentable: the file is a function of the token list and cannot disagree with
itself.

# The two cross-checks (and why both are here)

  1. **The vendor map must agree.** Every token here must have an entry in
     `tools/vendor_material_symbols.py`'s `ICONS`, and vice versa. A token with no vendored SVG
     would draw the "unknown" placeholder while claiming to be a real icon; a vendored SVG with no
     token is data nothing can reach. Both are refused by name.
  2. **The variant names must be valid and unique.** A duplicate variant is a compile error in the
     generated file, but it is refused here so the message names the offending line rather than a
     line in generated code.

# Licence

This script emits *code*, not third-party data — the geometry it names is vendored by
`vendor_material_symbols.py` and attributed in `NOTICE`. It is therefore offline: it reads only
`tools/icon_tokens.txt` and the vendor map's keys. No network, no licence gate (there is nothing
inbound here to license).

Usage:
    python3 tools/gen_icon_names.py            # write the file
    python3 tools/gen_icon_names.py --check    # exit 1 if stale
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TOKEN_LIST = REPO_ROOT / "tools" / "icon_tokens.txt"
OUT = REPO_ROOT / "src" / "widget" / "display_widgets" / "icon_names.rs"

sys.path.insert(0, str(Path(__file__).resolve().parent))
from vendor_material_symbols import ICONS  # noqa: E402

VARIANT_RE = re.compile(r"^[A-Z][A-Za-z0-9]*$")


def read_tokens() -> list[tuple[str, str, str]]:
    """Parse `token | Variant | doc` lines into `[(token, variant, doc)]`, in file order."""
    if not TOKEN_LIST.exists():
        raise SystemExit(f"REFUSED: {TOKEN_LIST} is missing; there is no token set to generate.")
    rows: list[tuple[str, str, str]] = []
    seen_tokens: dict[str, int] = {}
    seen_variants: dict[str, int] = {}
    for lineno, raw in enumerate(TOKEN_LIST.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) != 3:
            raise SystemExit(f"REFUSED: {TOKEN_LIST}:{lineno}: expected `token | Variant | doc`, got {line!r}")
        token, variant, doc = parts
        if not token or not variant or not doc:
            raise SystemExit(f"REFUSED: {TOKEN_LIST}:{lineno}: an empty field in {line!r}")
        if not VARIANT_RE.match(variant):
            raise SystemExit(
                f"REFUSED: {TOKEN_LIST}:{lineno}: {variant!r} is not a valid UpperCamelCase variant name"
            )
        if token in seen_tokens:
            raise SystemExit(
                f"REFUSED: {TOKEN_LIST}:{lineno}: token {token!r} already declared on line {seen_tokens[token]}"
            )
        if variant in seen_variants:
            raise SystemExit(
                f"REFUSED: {TOKEN_LIST}:{lineno}: variant {variant!r} already declared on line "
                f"{seen_variants[variant]}"
            )
        seen_tokens[token] = lineno
        seen_variants[variant] = lineno
        rows.append((token, variant, doc))
    if not rows:
        raise SystemExit(f"REFUSED: {TOKEN_LIST} parsed to an empty token set.")

    # Cross-check against the vendor map: the two lists are one fact told twice, so they must agree.
    tokens = {token for token, _variant, _doc in rows}
    vendored = set(ICONS)
    missing_svg = sorted(tokens - vendored)
    unreachable = sorted(vendored - tokens)
    if missing_svg:
        raise SystemExit(
            "REFUSED: these tokens have no entry in `tools/vendor_material_symbols.py`'s ICONS, so\n"
            f"  no SVG is vendored for them: {missing_svg}\n"
            "  Add them to the vendor map (and run the vendor script), or remove them here."
        )
    if unreachable:
        raise SystemExit(
            "REFUSED: these vendored tokens are not declared here, so nothing can reach them:\n"
            f"  {unreachable}\n"
            "  Add them to `tools/icon_tokens.txt` or remove them from the vendor map."
        )
    return rows


def render(rows: list[tuple[str, str, str]]) -> str:
    count = len(rows)
    lines: list[str] = []
    lines.append("// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)")
    lines.append("// SPDX-License-Identifier: MIT")
    lines.append("//")
    lines.append("// GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("// Produced by `tools/gen_icon_names.py` from `tools/icon_tokens.txt`. Regenerate with:")
    lines.append("//     python3 tools/gen_icon_names.py")
    lines.append("//")
    lines.append("// The `IconName` type: the enum, its token spellings, and the accessors the rest of the")
    lines.append("// crate uses. The token list is the single source of truth; the vendor map")
    lines.append("// (`tools/vendor_material_symbols.py`) is cross-checked against it, so a token that")
    lines.append("// cannot be drawn or a vendored icon nothing can name is a generation failure.")
    lines.append("//")
    lines.append("// The geometry itself lives in `icon_data.rs` (opt-in `icons` feature) and")
    lines.append("// `icon_fallback_data.rs` (always), both indexed by this enum's declaration order.")
    lines.append("")
    lines.append("// `IconData` is defined once, in `icon.rs`, so a build without the `icons` feature can still")
    lines.append("// name the type; this module only borrows it for the two accessors that return it.")
    lines.append("use super::IconData;")
    lines.append("")
    lines.append("/// Common icon names for use with the Icon widget.")
    lines.append("///")
    lines.append("/// Every variant names an icon this crate ships. With the opt-in `icons` feature on it")
    lines.append("/// resolves to a real Material Symbols outline; with the feature off it resolves to")
    lines.append("/// generated fallback geometry derived from the same outline. Either way the picture is a")
    lines.append("/// faithful shape rather than an alias of another icon — every variant round-trips through")
    lines.append("/// [`IconName::as_str`] and [`IconName::from_name`], and the integrity tests assert no two")
    lines.append("/// variants draw the same picture.")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    lines.append("pub enum IconName {")
    for _token, variant, doc in rows:
        lines.append(f"    /// {doc}")
        lines.append(f"    {variant},")
    lines.append("}")
    lines.append("")
    lines.append("impl IconName {")
    lines.append("    /// Returns the string representation of this icon name.")
    lines.append("    ///")
    lines.append("    /// The tokens are lower-case and underscore-separated (`\"arrow_left\"`), and are the")
    lines.append("    /// exact spellings accepted by [`IconName::from_name`] and by the `icon` property. They")
    lines.append("    /// are also the names used by [`Icon::set_icon`].")
    lines.append("    pub fn as_str(&self) -> &'static str {")
    lines.append("        match self {")
    for token, variant, _doc in rows:
        lines.append(f'            Self::{variant} => "{token}",')
    lines.append("        }")
    lines.append("    }")
    lines.append("")
    lines.append("    /// Parses a token back to its variant, or `None` when it is not one.")
    lines.append("    ///")
    lines.append("    /// The match is exact and case-sensitive: only the tokens produced by")
    lines.append("    /// [`IconName::as_str`] are accepted, so `\"ArrowLeft\"` and `\"arrow left\"` both return")
    lines.append("    /// `None`. Use [`Icon::set_icon`] when an unrecognised name should fall back to a")
    lines.append("    /// placeholder rather than being rejected.")
    lines.append("    pub fn from_name(name: &str) -> Option<Self> {")
    lines.append("        match name {")
    for token, variant, _doc in rows:
        lines.append(f'            "{token}" => Some(Self::{variant}),')
    lines.append("            _ => None,")
    lines.append("        }")
    lines.append("    }")
    lines.append("")
    lines.append("    /// The canonical token of every variant, in declaration order.")
    lines.append("    ///")
    lines.append("    /// The table's own view of [`IconName::ALL`], so a test can compare the two lists rather")
    lines.append("    /// than compare each against a third copy.")
    lines.append(f"    pub fn all_tokens() -> [&'static str; {count}] {{")
    lines.append(f'        let mut tokens = [""; {count}];')
    lines.append("        let mut index = 0;")
    lines.append("        let mut variant_index = 0;")
    lines.append("        while variant_index < Self::ALL.len() {")
    lines.append("            tokens[index] = Self::ALL[variant_index].as_str();")
    lines.append("            index += 1;")
    lines.append("            variant_index += 1;")
    lines.append("        }")
    lines.append("        tokens")
    lines.append("    }")
    lines.append("")
    lines.append("    /// Every variant, in declaration order.")
    lines.append("    ///")
    lines.append("    /// Indexed directly by the enum's discriminant — `data()` and `data_opt()` rely on that")
    lines.append("    /// order matching the generated tables, which the integrity tests assert by name.")
    lines.append(f"    pub const ALL: [IconName; {count}] = [")
    for _token, variant, _doc in rows:
        lines.append(f"        Self::{variant},")
    lines.append("    ];")
    lines.append("")
    lines.append("    /// This icon's bundled outline data, indexed by declaration order.")
    lines.append("    ///")
    lines.append("    /// # Why the return is not an `Option`")
    lines.append("    ///")
    lines.append("    /// With the `icons` feature on, every variant has data — the generator refuses to emit a")
    lines.append("    /// table with a gap — so a token with no outline is a compile error rather than a runtime")
    lines.append("    /// placeholder. A `None` here would be a lie about a value that cannot be absent.")
    lines.append('    #[cfg(feature = "icons")]')
    lines.append("    pub fn data(self) -> IconData {")
    lines.append("        use crate::widget::icon_data::ICON_DATA;")
    lines.append("        // Indexed by the enum's declaration order, which is also `ICON_DATA`'s order: both")
    lines.append("        // come from the same token list. The `debug_assert!` catches a reorder in a debug")
    lines.append("        // build; `tests/icon_data_integrity_test.rs` checks the names in every build.")
    lines.append("        let index = self as usize;")
    lines.append("        debug_assert_eq!(")
    lines.append("            ICON_DATA[index].name,")
    lines.append("            self.as_str(),")
    lines.append('            "ICON_DATA order must match IconName declaration order"')
    lines.append("        );")
    lines.append("        ICON_DATA[index]")
    lines.append("    }")
    lines.append("")
    lines.append("    /// [`Self::data`], available whether or not the feature is on.")
    lines.append("    ///")
    lines.append("    /// # Why two accessors rather than one returning `Option`")
    lines.append("    ///")
    lines.append("    /// The always-available spelling, so a draw path compiles in both states without a `cfg`")
    lines.append("    /// at the call site. It is `Some` exactly when `icons` is on, and `None` otherwise, since")
    lines.append("    /// without the feature there is no table to index.")
    lines.append('    #[cfg(feature = "icons")]')
    lines.append("    pub fn data_opt(self) -> Option<IconData> {")
    lines.append("        Some(self.data())")
    lines.append("    }")
    lines.append("")
    lines.append("    /// [`Self::data`] for a build without the icon data: there is none, which is the honest")
    lines.append("    /// answer rather than a fabricated entry.")
    lines.append('    #[cfg(not(feature = "icons"))]')
    lines.append("    pub fn data_opt(self) -> Option<IconData> {")
    lines.append("        None")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="exit 1 if the file is stale")
    args = parser.parse_args()

    rows = read_tokens()
    rendered = render(rows)

    if args.check:
        if not OUT.exists():
            print(f"FAIL  {OUT.relative_to(REPO_ROOT)} does not exist", file=sys.stderr)
            return 1
        if OUT.read_text(encoding="utf-8") != rendered:
            print(
                f"FAIL  {OUT.relative_to(REPO_ROOT)} is stale; run\n"
                f"    python3 tools/gen_icon_names.py",
                file=sys.stderr,
            )
            return 1
        print(f"icon names OK: {len(rows)} tokens, matches the token list and the vendor map")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUT.relative_to(REPO_ROOT)}: {len(rows)} tokens")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
