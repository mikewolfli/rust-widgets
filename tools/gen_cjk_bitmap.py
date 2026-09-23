#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate the 16x16 bitmap CJK subset `src/render/text/cjk_bitmap_data.rs`.

# What this produces

Two Rust arrays for a subset of GNU Unifont 16.0.01:

  * `CODEPOINTS: [u32; N]` — ascending codepoints, one per glyph;
  * `ROWS: [u8; N * 32]`   — 32 bytes per glyph: 16 rows of 2 bytes, row-major,
    MSB-first within each byte (bit `0x8000` = leftmost pixel of the row).

The source is Unifont's **bitmap** build, the `.hex` file, never the OTF. The `.hex`
bytes *are* the pixels, so nothing is rasterised and no outline hinter can move a pixel
between runs — a subset of the bitmap source is reproducible byte-for-byte.

# Why a licence gate (BLUE23 G-4a "许可门禁")

Unifont is dual-licensed SIL OFL 1.1 / GPL-2.0-or-later-with-font-embedding-exception.
Shipping glyph data is a licensing decision, not a build step, so this script refuses to
run unless the caller states the licence they are relying on with `--license`, and it
refuses any payload that is not the exact Unifont release it claims to be. The gate has
two halves, and the second is the one with teeth:

  1. the source label (path or URL) must contain the expected version string, so a caller
     cannot quietly point the script at some other font; and
  2. the SHA-256 of the decompressed `.hex` must equal the digest recorded for 16.0.01.

Step 1 alone would pass against a file merely *named* `unifont-16.0.01`; step 2 is what
actually proves the bytes are that release.

A subset is a "Modified Version" under the OFL, so the Reserved Font Name "Unifont" must
not be used for it — the output is named `cjk_bitmap_data.rs`. The repository-root
`NOTICE` records the derivation and the licence.

# Why the codepoint set is trimmed

The full requested set — CJK `U+4E00..=U+56FF`, Hiragana+Katakana `U+3040..=U+30FF`,
CJK symbols `U+3000..=U+303F`, halfwidth/fullwidth forms `U+FF00..=U+FFEF` — is 2800
glyphs, i.e. 100800 bytes of array data. That is over the ~100 KB ceiling, and BLUE23
G-4b budgets ~85 KB for the `mini`/`embedded` profiles, so the CJK upper bound is trimmed
(U+4E00 stays the lower bound) until the combined size lands just under 85 KB. The exact
final ranges and byte count are written into the generated header, so the file describes
itself and the trimmed range is never a silent surprise.

# Usage

    python3 tools/gen_cjk_bitmap.py --license=ofl-1.1              # download if uncached
    python3 tools/gen_cjk_bitmap.py --license=ofl-1.1 --refresh    # force re-download
    python3 tools/gen_cjk_bitmap.py --license=ofl-1.1 --source F.hex.gz
    python3 tools/gen_cjk_bitmap.py --license=ofl-1.1 --check      # verify, write nothing

Exit status: 0 = written (or `--check` agrees), 2 = licensing gate refused, 1 = other.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

UNIFONT_VERSION = "16.0.01"
UNIFONT_URL = (
    "https://unifoundry.com/pub/unifont/unifont-16.0.01/font-builds/"
    "unifont-16.0.01.hex.gz"
)
# Content identity of the two halves of the canonical download. The decompressed digest is the
# authoritative one (it survives a re-gzip); the `.gz` digest pins the published archive itself.
UNIFONT_HEX_SHA256 = "3b9d881d534c4144cd9865a47a7667760e33a571a45794a5334c16a72cb8f839"
UNIFONT_GZ_SHA256 = "2ce5ba84af1b4606391d06255073b5f14e146f3e1fc0c781f2f18d4c0a6c8f13"

DEFAULT_CACHE = REPO_ROOT / "target" / "unifont-cache" / "unifont-16.0.01.hex.gz"
DEFAULT_OUT = REPO_ROOT / "src" / "render" / "text" / "cjk_bitmap_data.rs"

# `--license` values this script will accept. Each is a licence the caller asserts covers the
# glyph data; the script's job is to refuse everything it has not been told, not to guess.
LICENSES = {
    "ofl-1.1": "SIL Open Font License 1.1",
    "gpl-2.0-with-font-exception": "GPL-2.0-or-later with the GNU font embedding exception",
}

# Ranges taken in full. ASCII is deliberately absent: the crate already ships an 8x8 face for
# U+0000..=U+007F and the default build's SVG snapshots must stay byte-identical.
FIXED_RANGES = (
    (0x3000, 0x303F, "CJK Symbols and Punctuation"),
    (0x3040, 0x30FF, "Hiragana and Katakana"),
    (0xFF00, 0xFFEF, "Halfwidth and Fullwidth Forms"),
)
# The one range that may be trimmed, and only at its upper bound.
CJK_RANGE = (0x4E00, 0x56FF, "CJK Unified Ideographs")

# 32 bytes of bitmap + 4 bytes of u32 codepoint, per glyph.
BYTES_PER_GLYPH = 32 + 4
# Over this, trim. Under it, the ~85 KB target is what we aim for.
SIZE_HARD_LIMIT = 100_000
SIZE_TARGET = 85_000

ROWS_PER_GLYPH = 16
BYTES_PER_ROW = 2


class GateRefused(Exception):
    """The licence gate refused the run. Named so `main` can map it to exit status 2."""


# --------------------------------------------------------------------------------------
# Source resolution
# --------------------------------------------------------------------------------------


def download(url: str, dest: Path) -> None:
    """Fetch `url` to `dest`, creating the parent directory. Raises on any HTTP failure."""
    dest.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(
        url, headers={"User-Agent": "rust-widgets/gen_cjk_bitmap.py"}
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        dest.write_bytes(response.read())


def resolve_source(args: argparse.Namespace) -> tuple[str, Path]:
    """Return `(label, path)` for the `.hex.gz` to read.

    `label` is what the version-string half of the gate inspects: the original URL or the
    caller-supplied path, never the cache path a download happened to land in.
    """
    if args.source:
        if args.source.startswith(("http://", "https://")):
            dest = Path(args.cache)
            download(args.source, dest)
            return args.source, dest
        return args.source, Path(args.source)

    cache = Path(args.cache)
    if args.refresh or not cache.exists():
        download(UNIFONT_URL, cache)
    return UNIFONT_URL, cache


def read_payload(path: Path) -> bytes:
    """Read `path`, transparently gunzipping when it is a gzip stream."""
    raw = path.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        return gzip.decompress(raw)
    return raw


# --------------------------------------------------------------------------------------
# Licence gate
# --------------------------------------------------------------------------------------


def enforce_license_argument(license_id: str | None) -> None:
    """Refuse the run unless the caller named a licence this script is allowed to emit under."""
    if license_id is None:
        raise GateRefused(
            "no font licence declared. This script embeds GNU Unifont glyph data, whose\n"
            "licence is a decision the caller must make, not one this script may assume.\n"
            f"Re-run with --license=<id>, where <id> is one of: {', '.join(sorted(LICENSES))}.\n"
            f"  e.g.  python3 {Path(__file__).name} --license=ofl-1.1"
        )
    if license_id not in LICENSES:
        raise GateRefused(
            f"unknown --license={license_id!r}.\n"
            f"Accepted values: {', '.join(sorted(LICENSES))}.\n"
            f"  {LICENSES.get(license_id, '')}".rstrip()
        )


def enforce_source_identity(label: str, payload: bytes) -> None:
    """Refuse a payload that is not the exact Unifont release this script was written for."""
    if UNIFONT_VERSION not in label:
        raise GateRefused(
            f"the source label does not name GNU Unifont {UNIFONT_VERSION}:\n"
            f"    {label}\n"
            "The version string must be present so a run cannot silently target a different\n"
            f"font release. Expected it to contain {UNIFONT_VERSION!r}."
        )
    digest = hashlib.sha256(payload).hexdigest()
    if digest != UNIFONT_HEX_SHA256:
        raise GateRefused(
            "the source is not GNU Unifont "
            f"{UNIFONT_VERSION} as this script knows it.\n"
            f"    expected SHA-256 (decompressed .hex): {UNIFONT_HEX_SHA256}\n"
            f"    actual   SHA-256 (decompressed .hex): {digest}\n"
            "A file merely *named* for the version is not proof of its contents; the digest is.\n"
            "If you intended a different release, update UNIFONT_VERSION and the two digests\n"
            "in this script together."
        )


# --------------------------------------------------------------------------------------
# `.hex` parsing and glyph selection
# --------------------------------------------------------------------------------------


def parse_hex(text: str) -> dict[int, bytes]:
    """Parse a Unifont `.hex` file into `{codepoint: bitmap bytes}`.

    One glyph per line, `<HEXCODE>:<hex row bytes…>`, top row first. Rejects malformed
    lines rather than skipping them, so a truncated download cannot pass as a small font.
    """
    glyphs: dict[int, bytes] = {}
    for lineno, line in enumerate(text.splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        code, sep, data = line.partition(":")
        if sep != ":":
            raise ValueError(f"line {lineno}: no ':' separator: {line[:40]!r}")
        try:
            cp = int(code, 16)
        except ValueError as exc:
            raise ValueError(f"line {lineno}: bad codepoint {code!r}") from exc
        if len(data) % 2:
            raise ValueError(f"line {lineno}: odd number of hex digits in U+{cp:04X}")
        try:
            glyphs[cp] = bytes.fromhex(data)
        except ValueError as exc:
            raise ValueError(f"line {lineno}: non-hex bitmap for U+{cp:04X}") from exc
    return glyphs


def to_16x16(raw: bytes) -> bytes:
    """Normalise a `.hex` glyph to 16 rows of 2 bytes, MSB-first within each byte.

    Unifont stores a 16-px glyph as 32 bytes and an 8-px (half-width) glyph as 16. The
    half-width form is left-aligned in the 16-px cell — high byte holds the pixels, low
    byte is blank — so `0x8000` is the leftmost pixel in both cases, as `ROWS` documents.
    """
    if len(raw) == ROWS_PER_GLYPH * BYTES_PER_ROW:
        return raw
    if len(raw) == ROWS_PER_GLYPH:
        wide = bytearray(ROWS_PER_GLYPH * BYTES_PER_ROW)
        for row in range(ROWS_PER_GLYPH):
            wide[row * BYTES_PER_ROW] = raw[row]
        return bytes(wide)
    raise ValueError(
        f"glyph is {len(raw)} bytes; expected {ROWS_PER_GLYPH} (8 px) or "
        f"{ROWS_PER_GLYPH * BYTES_PER_ROW} (16 px)"
    )


def select_glyphs(glyphs: dict[int, bytes]) -> list[tuple[int, bytes]]:
    """Choose `(codepoint, bitmap)` pairs, trimming the CJK upper bound to meet the budget.

    Returns ascending pairs. Assumes the CJK range is contiguous in the source; a gap only
    makes the result smaller, never invalid.
    """
    fixed = [
        (cp, glyphs[cp])
        for lo, hi, _ in FIXED_RANGES
        for cp in range(lo, hi + 1)
        if cp in glyphs
    ]
    cjk = [
        (cp, glyphs[cp]) for cp in range(CJK_RANGE[0], CJK_RANGE[1] + 1) if cp in glyphs
    ]

    combined = (len(fixed) + len(cjk)) * BYTES_PER_GLYPH
    if combined > SIZE_HARD_LIMIT:
        allowed = (SIZE_TARGET - len(fixed) * BYTES_PER_GLYPH) // BYTES_PER_GLYPH
        if allowed < 1:
            raise ValueError(
                "the fixed ranges alone exceed the size budget; nothing can be trimmed from CJK"
            )
        cjk = cjk[:allowed]

    chosen = sorted([*fixed, *cjk])
    for (prev_cp, _), (cp, _) in zip(chosen, chosen[1:]):
        if cp <= prev_cp:
            raise ValueError(f"codepoints are not strictly ascending at U+{cp:04X}")
    return chosen


# --------------------------------------------------------------------------------------
# Rendering
# --------------------------------------------------------------------------------------


def _wrap(values: list[str], per_line: int) -> str:
    """Join literals, `per_line` to a line, each line indented and trailing-comma terminated."""
    lines = [
        "    " + ", ".join(values[i : i + per_line]) + ","
        for i in range(0, len(values), per_line)
    ]
    return "\n".join(lines)


def _header(chosen: list[tuple[int, bytes]], codepoint_bytes: int) -> str:
    present = {cp for cp, _ in chosen}
    row_bytes = len(chosen) * ROWS_PER_GLYPH * BYTES_PER_ROW
    combined = row_bytes + codepoint_bytes

    lines = [
        "// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)",
        "// SPDX-License-Identifier: MIT",
        "//",
        "// GENERATED FILE — DO NOT EDIT BY HAND.",
        "// Produced by `tools/gen_cjk_bitmap.py`. Regenerate with:",
        "//     python3 tools/gen_cjk_bitmap.py --license=ofl-1.1",
        "//",
        "// Glyph source: GNU Unifont 16.0.01, the bitmap `.hex` build (not the OTF):",
        f"//     {UNIFONT_URL}",
        f"//     SHA-256 (decompressed .hex): {UNIFONT_HEX_SHA256}",
        "// Licence of the glyph data: dual-licensed SIL Open Font License 1.1, or",
        "// GPL-2.0-or-later with the GNU font embedding exception. See the repository-root",
        "// `NOTICE`. The subset is a Modified Version under the OFL and is renamed: it does",
        '// not use the Reserved Font Name "Unifont".',
        "//",
        "// ASCII (U+0000..=U+007F) is deliberately absent — the crate's 8x8 face covers it.",
        "// Included codepoint ranges:",
    ]
    # Emitted in codepoint order so the table reads top-down, and sorted rather than taken
    # from the unordered `present` set — `set` iteration order is not the codepoint order.
    cjk_cps = sorted(cp for cp in present if CJK_RANGE[0] <= cp <= CJK_RANGE[1])
    ranges = [
        (lo, hi, name, sum(1 for cp in range(lo, hi + 1) if cp in present))
        for lo, hi, name in FIXED_RANGES
    ]
    ranges.append((cjk_cps[0], cjk_cps[-1], CJK_RANGE[2] + " (subset)", len(cjk_cps)))
    for lo, hi, name, count in sorted(ranges):
        lines.append(f"//     U+{lo:04X}..=U+{hi:04X}  {name:<32} {count:>4} glyphs")
    lines += [
        "//",
        f"//     N = {len(chosen)} glyphs; {combined} bytes of array data "
        f"(ROWS {row_bytes} + CODEPOINTS {codepoint_bytes}).",
        "// Half-width (8 px) source glyphs are left-aligned in the 16 px cell.",
        "// `#[rustfmt::skip]` keeps `cargo fmt --check` stable over the generated table.",
    ]
    return "\n".join(lines)


def render(chosen: list[tuple[int, bytes]]) -> str:
    """Render the whole Rust source file."""
    n = len(chosen)
    rows = b"".join(to_16x16(glyph) for _, glyph in chosen)
    assert len(rows) == n * ROWS_PER_GLYPH * BYTES_PER_ROW, "row buffer size drifted"

    parts = [
        _header(chosen, n * 4),
        "",
        "/// Sorted codepoints, one per glyph in `ROWS`. Ascending order (binary search relies on it).",
        "#[rustfmt::skip]",
        f"pub static CODEPOINTS: [u32; {n}] = [",
        _wrap([f"0x{cp:04X}" for cp, _ in chosen], 8),
        "];",
        "",
        "/// `32` bytes per glyph: 16 rows of 2 bytes, row-major, **MSB-first within each byte**,",
        "/// i.e. bit `0x8000` = leftmost pixel of the row.",
        "///",
        "/// A source glyph that is only 8 px wide is left-aligned in the 16 px cell, so `0x8000`",
        "/// is the leftmost pixel there too.",
        "#[rustfmt::skip]",
        f"pub static ROWS: [u8; {n * 32}] = [",
        _wrap([f"0x{byte:02X}" for byte in rows], 16),
        "];",
        "",
    ]
    return "\n".join(parts)


# --------------------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Generate the 16x16 bitmap CJK subset (GNU Unifont 16.0.01).",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--license",
        metavar="ID",
        help=(
            "REQUIRED. The licence you are relying on for the glyph data. "
            f"One of: {', '.join(sorted(LICENSES))}."
        ),
    )
    parser.add_argument(
        "--source",
        help="A `.hex`/`.hex.gz` path or URL. Defaults to the canonical Unifont release.",
    )
    parser.add_argument(
        "--cache",
        default=str(DEFAULT_CACHE),
        help=f"Where a download is stored (default: {DEFAULT_CACHE.relative_to(REPO_ROOT)}).",
    )
    parser.add_argument(
        "--refresh", action="store_true", help="Re-download even when the cache exists."
    )
    parser.add_argument(
        "--out",
        default=str(DEFAULT_OUT),
        help=f"Output file (default: {DEFAULT_OUT.relative_to(REPO_ROOT)}).",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the output file matches what would be generated; write nothing.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)

    try:
        # Half one of the gate: the caller must have named a licence, before any I/O happens.
        enforce_license_argument(args.license)

        label, path = resolve_source(args)
        if not path.exists():
            raise FileNotFoundError(f"source not found: {path}")
        payload = read_payload(path)

        # Half two: the source must be the release it claims to be.
        enforce_source_identity(label, payload)
    except GateRefused as refusal:
        print(f"LICENCE GATE REFUSED:\n{refusal}", file=sys.stderr)
        return 2

    glyphs = parse_hex(payload.decode("ascii"))
    chosen = select_glyphs(glyphs)
    rendered = render(chosen)

    n = len(chosen)
    fixed = sum(1 for cp, _ in chosen if not (CJK_RANGE[0] <= cp <= CJK_RANGE[1]))
    cjk = n - fixed
    print(f"source:     {label}")
    print(f"licence:    {LICENSES[args.license]}")
    print(f"glyphs:     N={n} (fixed ranges {fixed}, CJK {cjk})")
    print(
        f"array data: {n * BYTES_PER_GLYPH} bytes (ROWS {n * 32} + CODEPOINTS {n * 4})"
    )
    print(f"codepoints: U+{chosen[0][0]:04X}..=U+{chosen[-1][0]:04X}")

    out = Path(args.out)
    if args.check:
        if not out.exists():
            print(f"CHECK FAILED: {out} does not exist", file=sys.stderr)
            return 1
        if out.read_text(encoding="utf-8") != rendered:
            print(
                f"CHECK FAILED: {out} is stale; re-run without --check", file=sys.stderr
            )
            return 1
        print(f"check:      {out} is up to date")
        return 0

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(rendered, encoding="utf-8")
    print(f"wrote:      {out} ({len(rendered.encode('utf-8'))} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
