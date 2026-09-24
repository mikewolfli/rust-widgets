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

# Which face, and why not the small one

The upstream repository publishes `2D/fonts/NotoColorEmoji-flagsonly.ttf` at 872 KB, which looks like
the obvious choice for a demo library. It is not usable: its cmap maps *36 codepoints*, and 10 of
them are `U+FE4E5..U+FE4EE` — private-use codepoints pointing at rows of an **emoji picker grid**
atlas. Rendered as text, U+1F1E6 draws an 136x128 image of an entire row of 25 flags, 3 px per flag.
Verified with `tools/emoji_font_probe.py`, and it is the reason this generator reads the full
`2D/fonts/NotoColorEmoji.ttf` (10,730,124 bytes) and *subsets* it down.

# The codepoint list is a file, not a filter

`tools/emoji_subset_codepoints.txt` holds the chosen codepoints and ranges with a rationale. It is
read here rather than hard-coded so the decision is reviewable and diffable on its own, and so the
"which emoji do we ship" question has one answer in one place.

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
CODEPOINT_LIST = REPO_ROOT / "tools" / "emoji_subset_codepoints.txt"

# The full face, not the 872 KB `flagsonly` one: see the module docs. The subset is what keeps the
# shipped artifact small.
FACES = {
    "emoji": {
        "project": "Noto Color Emoji",
        "note": "colour emoji as CBDT PNG bitmaps, one strike at 109 ppem",
        "url": "https://raw.githubusercontent.com/googlefonts/noto-emoji/main/2D/fonts/"
               "NotoColorEmoji.ttf",
        # SHA-256 of the *upstream* download, which is what NOTICE repeats.
        "sha256": "15671215ab769fdc7162a045d56fd7d7e477c51b04e6b3c761d914d8fdd6cc44",
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


def read_codepoint_list(path):
    """Parses `tools/emoji_subset_codepoints.txt` into a sorted list of codepoints.

    A missing list is a hard error rather than a fallback to "everything": silently shipping the
    whole 10.6 MB face because a file was renamed is the kind of mistake that only shows up as a
    binary size regression months later.
    """
    if not path.exists():
        raise SystemExit(f"REFUSED: {path} is missing; there is no default codepoint set.")
    codepoints = set()
    for lineno, raw in enumerate(path.read_text().splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        for part in line.split(","):
            part = part.strip()
            if not part:
                continue
            if ".." in part:
                lo, _, hi = part.partition("..")
                try:
                    lo_i, hi_i = int(lo, 16), int(hi, 16)
                except ValueError:
                    raise SystemExit(f"REFUSED: {path}:{lineno}: bad range {part!r}")
                if hi_i < lo_i:
                    raise SystemExit(f"REFUSED: {path}:{lineno}: reversed range {part!r}")
                codepoints.update(range(lo_i, hi_i + 1))
            else:
                try:
                    codepoints.add(int(part, 16))
                except ValueError:
                    raise SystemExit(f"REFUSED: {path}:{lineno}: bad codepoint {part!r}")
    if not codepoints:
        raise SystemExit(f"REFUSED: {path} parsed to an empty set.")
    return sorted(codepoints)


def subset_bytes(face, source_path, codepoints):
    """The subset, with the colour tables kept.

    `CBDT`/`CBLC` are retained by name; without them the subset would carry the cmap and no pixels.

    The requested codepoints are intersected with the face's own cmap first. fontTools refuses a
    request for a codepoint the face does not have, and the curated list is a superset of some
    upstream releases — so the intersection is what makes the list robust to that, while the count
    printed below still shows how much of the list was actually satisfied.
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
    available = set(font.getBestCmap().keys())
    wanted = [cp for cp in codepoints if cp in available]
    missing = sorted(set(codepoints) - available)
    if not wanted:
        raise SystemExit("REFUSED: none of the requested codepoints exist in the face.")
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=wanted)
    subsetter.subset(font)

    import io

    buffer = io.BytesIO()
    font.save(buffer, reorderTables=False)
    return buffer.getvalue(), len(wanted), missing


def header(face_id, face, payload_len, glyph_count, digest):
    return f'''// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_emoji_subset.py`. Regenerate with:
//     python3 tools/gen_emoji_subset.py --license={face["license"].lower()}
//
// Glyph source: {face["project"]}, {face["note"]}:
//     {face["url"]}
//     SHA-256 (upstream .ttf): {digest}
// Licence of the glyph data: {face["license"]} (SIL Open Font License 1.1). See the
// repository-root `NOTICE`. This subset is a Modified Version under the OFL: it retains
// the upstream colour bitmaps verbatim, and it does not use the upstream Reserved Font
// Name.
//
//     N = {glyph_count} codepoints; {payload_len} bytes of bitmap data.
// The codepoints shipped are listed in `tools/emoji_subset_codepoints.txt`, which is the
// input this generator read — the list is a decision, not a scrape.
//
// The payload is a binary `include_bytes!` rather than a Rust array: {payload_len} bytes written
// as `0x00,` literals would be several times this file's size, which is a worse artifact than the
// binary it describes.

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

    codepoints = read_codepoint_list(CODEPOINT_LIST)
    payload, glyph_count, missing = subset_bytes(face, source_path, codepoints)
    text = header(args.face, face, len(payload), glyph_count, digest)

    ttf_path = OUT_DIR / f"{args.face}.ttf"
    rs_path = OUT_DIR / f"{args.face}.rs"

    print(f"face:       {args.face} ({face['project']})")
    print(f"licence:    {face['license']}")
    print(f"upstream:   {source_path} ({source_path.stat().st_size} bytes)")
    print(f"sha256:     {digest}")
    print(f"requested:  {len(codepoints)} codepoints ({len(missing)} absent upstream)")
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
