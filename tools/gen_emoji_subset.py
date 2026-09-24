#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate the opt-in colour-emoji subset this crate ships (BLUE23 §0A.4, G-6).

# What this produces

    src/render/text/font_assets/emoji.ttf   the subset, byte-for-byte from fontTools
    src/render/text/font_assets/emoji.rs    a one-line `include_bytes!` with a provenance header

The header repeats the upstream project, the upstream URL, the SHA-256 of the *upstream* file and
the licence, because `tools/font_license_scan.py` requires exactly those facts and `NOTICE` must
repeat them. Generating the header rather than hand-writing it is what makes the two impossible to
disagree.

# Why the payload is a PNG-in-`CBDT` subset and not an outline face

Colour emoji in the shipped upstream face are **bitmap** glyphs: `CBDT` holds one PNG per glyph and
`CBLC` indexes them. That is why G-6 needs a second pixel format in `GlyphSource` rather than more
outline code, and it is why this generator keeps `CBDT`/`CBLC` instead of converting to outlines
(which would lose the colour entirely).

# The licence gate, and why it is first

The script refuses to run unless the caller states, with `--license`, which licence they are relying
on. It cannot verify that assertion — that is the caller's job, and it is a legal assertion rather
than a technical one — but it makes "we shipped this because someone ran a script" impossible
without a recorded decision. `tools/check_font_licenses.sh` asserts the gate still refuses.

# Determinism

Output is deterministic for a given fontTools version: the source is pinned by SHA-256, and the
subsetter is invoked with `recalcTimestamp=False` so `head.modified` keeps the upstream value. Without
that, fontTools stamps the save time into the file and two runs produce two different artifacts, so
`--check` could never pass.
"""

import argparse
import hashlib
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = REPO_ROOT / "src" / "render" / "text" / "font_assets"
CACHE_DIR = REPO_ROOT / "target" / "font-cache"

# `2D/fonts/NotoColorEmoji-flagsonly.ttf` is the smallest CBDT release upstream publishes (872 KB
# against 10.7 MB for the full face). A *subset* of the full face is what a real host wants; this
# entry point exists so the subset can be regenerated rather than hand-copied.
FACES = {
    "emoji": {
        "project": "Noto Color Emoji",
        "note": "colour emoji as CBDT PNG bitmaps, one strike at 109 ppem",
        "url": "https://raw.githubusercontent.com/googlefonts/noto-emoji/main/2D/fonts/"
               "NotoColorEmoji-flagsonly.ttf",
        # SHA-256 of the *upstream* download, which is what NOTICE repeats.
        "sha256": "6d3b266ee9631119cd6fea9eea7ea2226874a441532e18908c5b839322f3c5dd",
        "license": "OFL-1.1",
        # A colour bitmap face is not a variable font, so there is no instance to pin.
        "instance": None,
    },
}


class GateRefused(Exception):
    """The licence gate refused the run. Named so `main` maps it to exit status 2."""


def enforce_license_argument(license_id):
    """Refuses a run that does not declare the licence it is relying on.

    The set is deliberately a closed list: accepting any string would let a caller satisfy the gate
    with a typo, which defeats the point of requiring the assertion.
    """
    allowed = {"ofl-1.1", "gpl-2.0-with-font-exception"}
    if license_id is None:
        raise GateRefused(
            "REFUSED: --license is required. This generator ships third-party glyph data, and the "
            "caller must state which licence they are relying on. One of: "
            + ", ".join(sorted(allowed))
        )
    normalised = license_id.strip().lower()
    if normalised not in allowed:
        raise GateRefused(
            f"REFUSED: --license={license_id} is not one of {sorted(allowed)}. Guessing a licence "
            "is not something this script will do."
        )
    return normalised


def download(url, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url, timeout=600) as response:
        dest.write_bytes(response.read())


def upstream_file(face_id, face, refresh=False):
    """The upstream file for `face`, downloading it once into the cache directory."""
    cached = CACHE_DIR / f"{face_id}-{Path(face['url']).name}"
    if refresh or not cached.exists():
        print(f"download: {face['url']}")
        download(face["url"], cached)
    digest = hashlib.sha256(cached.read_bytes()).hexdigest()
    if face["sha256"] != "PENDING" and digest != face["sha256"]:
        raise SystemExit(
            f"REFUSED: {cached} is not the pinned upstream file.\n"
            f"  expected sha256 {face['sha256']}\n"
            f"  found    sha256 {digest}\n"
            "Either the upstream file changed (in which case re-pin it deliberately, after "
            "reviewing the licence and the NOTICE text) or the cache is corrupt."
        )
    return cached, digest


def subset_bytes(face, source_path):
    """The subset, with the colour tables kept.

    `CBDT`/`CBLC` are retained by name; without them the subset would carry the cmap and no pixels.
    """
    from fontTools.ttLib import TTFont
    from fontTools import subset

    options = subset.Options()
    options.layout_features = ["*"]
    # The colour bitmap tables, which are the entire point of this face.
    options.retain_gids = True
    options.notdef_outline = True
    options.recalc_bounds = False
    # Do not stamp the save time into `head`: see the module docs on determinism.
    options.recalc_timestamp = False
    options.canonical_order = True
    options.drop_tables += ["DSIG"]

    font = TTFont(str(source_path))
    unicodes = sorted(int(u) for u in font.getBestCmap().keys())
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=unicodes)
    subsetter.subset(font)

    import io

    buffer = io.BytesIO()
    font.save(buffer, reorderTables=False)
    return buffer.getvalue(), len(unicodes)


def header(face_id, face, payload_len, glyph_count, digest):
    return f'''// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED by tools/gen_emoji_subset.py -- do not edit by hand.
//
// {face["project"]} -- colour-emoji subset ({face["note"]})
//     Project:  {face["url"].rsplit("/", 4)[0]}
//     Source:   {face["url"]}
//     SHA-256 (upstream .ttf): {digest}
//     Licence:  {face["license"]}
//     Subset:   {glyph_count} codepoints, {payload_len} bytes
//
// This file is a **Modified Version** within the meaning of the SIL Open Font License, Version
// 1.1: it retains the upstream colour bitmaps verbatim, as a subset. It does not use the
// upstream Reserved Font Name.
//
// The payload is a binary `include_bytes!` rather than a Rust array: {payload_len} bytes written
// as `0x00,` literals would be several times this file's size, which is a worse artifact than the
// binary it describes. See the repository-root `NOTICE`.

/// The generated subset's bytes, in the binary's read-only section.
pub const FONT: &[u8] = include_bytes!("{face_id}.ttf");
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--license", help="REQUIRED. The licence you are relying on for the glyph data.")
    parser.add_argument("--face", default="emoji", choices=sorted(FACES))
    parser.add_argument("--refresh", action="store_true", help="re-download the upstream file")
    parser.add_argument("--source", default=None, help="use this upstream file instead of the cache")
    parser.add_argument("--check", action="store_true", help="verify the output matches; write nothing")
    args = parser.parse_args()

    try:
        enforce_license_argument(args.license)
    except GateRefused as refusal:
        print(refusal, file=sys.stderr)
        return 2

    face = FACES[args.face]
    if args.source:
        source_path = Path(args.source)
        digest = hashlib.sha256(source_path.read_bytes()).hexdigest()
    else:
        source_path, digest = upstream_file(args.face, face, refresh=args.refresh)

    payload, glyph_count = subset_bytes(face, source_path)
    text = header(args.face, face, len(payload), glyph_count, digest)

    ttf_path = OUT_DIR / f"{args.face}.ttf"
    rs_path = OUT_DIR / f"{args.face}.rs"

    print(f"face:       {args.face} ({face['project']})")
    print(f"licence:    {face['license']}")
    print(f"upstream:   {source_path} ({source_path.stat().st_size} bytes)")
    print(f"sha256:     {digest}")
    print(f"subset:     {glyph_count} codepoints, {len(payload)} bytes")
    print(f"output:     {ttf_path}")
    print(f"            {rs_path}")

    if args.check:
        problems = []
        if not ttf_path.exists() or ttf_path.read_bytes() != payload:
            problems.append(f"{ttf_path} is not what would be generated")
        if not rs_path.exists() or rs_path.read_text() != text:
            problems.append(f"{rs_path} is not what would be generated")
        if problems:
            for problem in problems:
                print(f"check:      FAIL {problem}", file=sys.stderr)
            return 1
        print("check:      both files are up to date")
        return 0

    ttf_path.write_bytes(payload)
    rs_path.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
