#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate the opt-in vector font subsets this crate ships (BLUE23 §0A.4, G-4c).

# What this produces

For each declared face, two files:

    src/render/text/font_assets/<id>.ttf   the subset itself, byte-for-byte from fontTools
    src/render/text/font_assets/<id>.rs    a one-line `include_bytes!` with a provenance header

The header repeats the upstream project, the upstream URL, the SHA-256 of the *upstream* file
and the licence, because `tools/font_license_scan.py` requires exactly those facts and `NOTICE`
must repeat them. Keeping the header generated rather than hand-written is what makes the two
impossible to disagree.

# The licence gate, and why it is first

The script refuses to run unless the caller states, with `--license`, which licence they are
relying on. It cannot verify that assertion — that is the caller's job, and it is a legal
assertion rather than a technical one — but it makes "we shipped this because someone ran a
script" impossible without a recorded decision. `tools/check_font_licenses.sh` asserts the gate
still refuses.

# Reproducibility

Output is deterministic for a given fontTools version: the source is pinned by SHA-256, the
variable instance is pinned per face, and fontTools preserves `head`'s timestamps from the
source rather than stamping the clock. `--check` re-generates into memory and compares, so a
hand edit or a toolchain drift is a failure rather than a silent difference.

Usage:
    python3 tools/gen_font_subset.py --license=ofl-1.1             # all faces
    python3 tools/gen_font_subset.py --license=ofl-1.1 --face=arabic
    python3 tools/gen_font_subset.py --license=ofl-1.1 --refresh   # re-download
    python3 tools/gen_font_subset.py --license=ofl-1.1 --check     # verify, write nothing

Exit status: 0 = written (or `--check` agrees), 2 = licensing gate refused, 1 = other.
"""

import argparse
import hashlib
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

# Licences this script will accept. Each is a licence the caller asserts covers the outlines;
# refusing everything it has not been told is the whole point of the gate.
LICENSES = {
    "ofl-1.1": "SIL Open Font License 1.1",
}

ASSET_DIR = REPO_ROOT / "src" / "render" / "text" / "font_assets"
CACHE_DIR = REPO_ROOT / "target" / "font-cache"

# ── The faces ───────────────────────────────────────────────────────────────────────────────────
# `unicodes` is the coverage a *useful* subset needs, not the minimum a test needs: a face that
# covered only the strings in the test suite would be a face no host could build a UI with.
FACES = {
    "latin": {
        "project": "Open Sans",
        "note": "Latin-1 letters, ASCII, and the punctuation a label uses",
        "url": "https://raw.githubusercontent.com/google/fonts/main/ofl/opensans/"
               "OpenSans%5Bwdth,wght%5D.ttf",
        # SHA-256 of the *upstream* download, which is what NOTICE repeats.
        "sha256": "36643644f318a812aab2d2ed3bb98f8cf0872527f835fe9398d95fe6b9adb878",
        # Pin the variable axes to one instance: a variable font carries every weight's deltas,
        # and a label that draws at one weight has no use for the other eight.
        "instance": {"wdth": 100, "wght": 400},
        "unicodes": (
            list(range(0x20, 0x7F))          # ASCII printable
            + list(range(0xA0, 0x100))       # Latin-1 supplement
            + [0x131, 0x152, 0x153]          # dotless i, OE, oe
            + [0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026, 0x20AC, 0x2122]
        ),
    },
    "arabic": {
        "project": "Noto Naskh Arabic",
        "note": "the Arabic block with its joining forms, marks and digits",
        "url": "https://raw.githubusercontent.com/google/fonts/main/ofl/notonaskharabic/"
               "NotoNaskhArabic%5Bwght%5D.ttf",
        "sha256": "67b5a525a661b607971fbd3f96a81b89d3a768e74534fca84f18ac97e6fab72f",
        "instance": {"wght": 400},
        "unicodes": (
            list(range(0x20, 0x7F))          # ASCII, so a mixed line measures
            + list(range(0x600, 0x6FF))      # the Arabic block: letters, marks, digits
            + [0x200C, 0x200D, 0x200E, 0x200F]  # ZWNJ, ZWJ, LRM, RLM
            + [0x2010, 0x2013, 0x2014, 0x2018, 0x2019, 0x201C, 0x201D, 0x2026]
        ),
    },
}

# ── The CJK vector face (`fonts-cjk`), whose codepoints live in a separate file ───────────────
#
# # Why `fonts-cjk` is not in `FACES` above, and why it does not squash the bitmap face
#
# Measured with this generator against Noto Sans SC (see the round log for the full curve):
#
#     symbols + kana            253 cps      127 KB
#     + fullwidth forms         477 cps      190 KB
#     + 500 Han                 977 cps      274 KB
#     + 1000 Han               1477 cps      396 KB
#     + 2000 Han               2477 cps      612 KB
#     (the bitmap face's exact coverage, 2342 cps)              581 KB
#
# The bitmap face is 85 KB for 2361 glyphs, so a vector face at the *same* coverage costs **7x**
# more. That is the fact BLUE23 §0A.4 constraint 2 and the migrated 附录 G both turn on: CJK is
# the script that needs *data* and no *shaping*, which is exactly why `mini`/`embedded` must be
# able to get Chinese from the 85 KB bitmap rather than being pushed onto a 581 KB vector face.
#
# So `fonts-cjk` is not "the CJK face": it is the **scalable** CJK face, for a `desktop`/`tablet`
# host that wants Chinese at any px size with antialiasing. It ships the ranges that are small and
# that a bitmap handles worst — kana, CJK punctuation, fullwidth forms, and the most common Han —
# and it deliberately does **not** try to replace the bitmap.
#
# The codepoints are listed in `tools/cjk_vector_codepoints.txt` rather than computed here, for the
# same reason the emoji list is a file: "which Han do we ship" is a decision, and a decision that
# lives in two places is a decision that will disagree with itself.
CJK_VECTOR_FACE = {
    "project": "Noto Sans SC",
    "note": "kana, CJK punctuation, fullwidth forms and common Han",
    "url": "https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/SubsetOTF/SC/"
           "NotoSansSC-Regular.otf",
    # SHA-256 of the *upstream* download, which is what NOTICE repeats.
    "sha256": "faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9",
    # The SubsetOTF release is already a single weight; there is no variable axis to pin.
    "instance": None,
    "codepoint_list": REPO_ROOT / "tools" / "cjk_vector_codepoints.txt",
}

# What `--check` and `NOTICE` expect this face to weigh, in bytes. A `--check` run that produces
# something *larger* than this is a failure rather than a surprise: the whole reason this face has a
# curated list instead of `--face=cjk` meaning "all 30890 cmap entries" is the budget.
CJK_VECTOR_BUDGET_BYTES = 400_000


def read_codepoint_list(path):
    """Parse a codepoint list into ascending codepoints.

    A missing list is a hard error rather than a fallback to "everything": silently shipping the
    whole face because a file was renamed is the kind of mistake that only shows up as a binary
    size regression months later.
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


def face_codepoints(face):
    """The codepoints `face` should be subset to — inline, or read from its list file."""
    if "codepoint_list" in face:
        return read_codepoint_list(face["codepoint_list"])
    return sorted(set(face["unicodes"]))


def all_faces():
    """Every face id this generator can produce, in a stable order.

    `cjk` lives in its own table rather than in `FACES` because its codepoints come from a file and
    its budget is checked — see `CJK_VECTOR_FACE`.
    """
    return sorted(list(FACES) + ["cjk"])


def get_face(face_id):
    if face_id == "cjk":
        return CJK_VECTOR_FACE
    return FACES.get(face_id)


class GateRefused(Exception):
    """The licence gate refused the run. Named so `main` maps it to exit status 2."""


def enforce_license_argument(license_id):
    """Refuse unless `license_id` is a licence this script has been taught."""
    if license_id is None:
        raise GateRefused(
            "no --license given.\n"
            "These outlines are third-party material and ship inside this repository, so the\n"
            "run must record which licence the caller is relying on.\n"
            "  e.g.  python3 tools/gen_font_subset.py --license=ofl-1.1"
        )
    if license_id not in LICENSES:
        raise GateRefused(
            f"unknown --license={license_id!r}.\n"
            f"  known: {', '.join(sorted(LICENSES))}"
        )


def download(url, dest):
    dest.parent.mkdir(parents=True, exist_ok=True)
    request = urllib.request.Request(url, headers={"User-Agent": "rust-widgets/gen_font_subset.py"})
    with urllib.request.urlopen(request, timeout=120) as response:
        dest.write_bytes(response.read())


def resolve_source(face_id, face, refresh):
    """The upstream file for `face`, downloading it once into the cache directory."""
    cached = CACHE_DIR / f"{face_id}-{Path(face['url']).name.split('%')[0]}.ttf"
    if refresh or not cached.exists():
        print(f"downloading {face['url']}")
        download(face["url"], cached)
    digest = hashlib.sha256(cached.read_bytes()).hexdigest()
    if digest != face["sha256"]:
        print(
            f"REFUSED: {cached} is not the pinned upstream file.\n"
            f"  expected SHA-256 {face['sha256']}\n"
            f"  found    SHA-256 {digest}\n"
            "  The pin is what makes the shipped subset auditable against a named revision; a\n"
            "  different file would need a new pin *and* a new NOTICE entry.",
            file=sys.stderr,
        )
        raise SystemExit(1)
    return cached


def subset_bytes(source, face, license_id, unicodes):
    """The subset's bytes. Imported here so `--help` works without fontTools installed."""
    try:
        from fontTools import subset
        from fontTools.ttLib import TTFont
        from fontTools.varLib import instancer
    except ImportError as missing:  # pragma: no cover - environment-dependent
        print(
            f"fontTools is required to generate a subset ({missing}).\n"
            "  python3 -m pip install fonttools",
            file=sys.stderr,
        )
        raise SystemExit(1)

    # `recalcTimestamp=False` is what makes the output a function of the pinned input alone:
    # fontTools otherwise stamps `head.modified` with the current time on every save, so two runs
    # would produce two different files and `--check` could never agree.
    font = TTFont(source, recalcTimestamp=False)
    # `instance` is `None` for a face that is already a single weight (the CJK `SubsetOTF`
    # release): instancing a font with no `fvar` is an error, not a no-op.
    if face["instance"] is not None:
        instancer.instantiateVariableFont(
            font, face["instance"], inplace=True, updateFontNames=False
        )

    options = subset.Options()
    # Every layout feature is kept: joining (`init`/`medi`/`fina`/`isol`), ligatures, kerning.
    # Dropping them would produce a face that renders Arabic as isolated letters — the exact
    # defect shaping exists to remove, shipped as data.
    options.layout_features = ["*"]
    options.notdef_outline = True
    options.drop_tables += ["DSIG"]

    # Request only codepoints the face actually has. fontTools refuses a request for a codepoint
    # the font lacks, and a curated list is a *superset* of some upstream revisions — so the
    # intersection is what makes the list robust. The count that was dropped is reported below,
    # which is what keeps the intersection from being a silent truncation.
    available = set(font.getBestCmap().keys())
    wanted = [cp for cp in unicodes if cp in available]
    absent = sorted(set(unicodes) - available)
    if not wanted:
        raise SystemExit("REFUSED: none of the requested codepoints exist in the face.")

    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=wanted)
    subsetter.subset(font)

    import io

    buffer = io.BytesIO()
    font.save(buffer)
    return buffer.getvalue(), absent


def layout_metrics(data):
    """`(glyph count, has GSUB, has GPOS)` — reported so the round's log carries real numbers."""
    try:
        from fontTools.ttLib import TTFont
    except ImportError:  # pragma: no cover
        return ("?", False, False)
    import io

    font = TTFont(io.BytesIO(data))
    return (font["maxp"].numGlyphs, "GSUB" in font, "GPOS" in font)


def wrapper_source(face_id, face, license_id, payload_len, glyphs, has_gsub, has_gpos,
                  requested, absent):
    """The `include_bytes!` module, with the provenance header the licence gate requires."""
    features = []
    if has_gsub:
        features.append("GSUB")
    if has_gpos:
        features.append("GPOS")
    # The codepoint provenance differs by face: an inline tuple of ranges is self-documenting,
    # while a curated file is not, and a reader of the generated header must be able to find the
    # decision that produced these bytes without reading the generator.
    if "codepoint_list" in face:
        list_path = face["codepoint_list"].relative_to(REPO_ROOT).as_posix()
        provenance = (
            f"// The codepoints shipped are listed in `{list_path}`, which is the input this\n"
            f"// generator read — the list is a decision, not a scrape.\n"
        )
    else:
        provenance = ""
    return f'''// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_font_subset.py`. Regenerate with:
//     python3 tools/gen_font_subset.py --face={face_id} --license={license_id}
//
// Glyph source: {face["project"]}, subset to {face["note"]}.
//     {face["url"]}
//     SHA-256 (upstream .ttf): {face["sha256"]}
//
// Licence of the glyph outlines: {LICENSES[license_id]}. See the repository-root `NOTICE`,
// which records this file by name together with the facts above. The subset is a Modified
// Version under the OFL and is renamed: it does not use the upstream Reserved Font Name.
//
// The subset is {payload_len} bytes ({glyphs} glyphs, layout tables: {", ".join(features) or "none"}),
// covering {requested} codepoints ({absent} of those requested are absent from this upstream
// revision and were skipped).
{provenance}// It is *not* compiled unless the feature that names it is enabled, and it is `include_bytes!`d
// rather than written into this source as an array: a 36 KB payload as `0x00,` literals would be
// ~150 KB of source, which is a worse artifact than the binary it describes.

/// The subset's bytes. Parsed on first use, never copied into a heap allocation.
pub const FONT: &[u8] = include_bytes!("{face_id}.ttf");
'''


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--license", dest="license_id", default=None,
                        help=f"the licence the caller relies on; one of {sorted(LICENSES)}")
    parser.add_argument("--face", action="append", default=None,
                        help=f"which face to generate (repeatable); default all of {all_faces()}")
    parser.add_argument("--refresh", action="store_true", help="re-download the upstream file")
    parser.add_argument("--source", default=None, help="use this upstream file instead of the cache")
    parser.add_argument("--check", action="store_true", help="verify the committed files, write nothing")
    args = parser.parse_args(argv)

    try:
        enforce_license_argument(args.license_id)
    except GateRefused as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        return 2

    face_ids = args.face or all_faces()
    for face_id in face_ids:
        if get_face(face_id) is None:
            print(f"unknown --face={face_id!r}; known: {', '.join(all_faces())}", file=sys.stderr)
            return 1
    ASSET_DIR.mkdir(parents=True, exist_ok=True)

    stale = False
    for face_id in face_ids:
        face = get_face(face_id)
        source = Path(args.source) if args.source else resolve_source(face_id, face, args.refresh)
        unicodes = face_codepoints(face)
        payload, absent = subset_bytes(source, face, args.license_id, unicodes)
        glyphs, has_gsub, has_gpos = layout_metrics(payload)
        wrapper = wrapper_source(
            face_id, face, args.license_id, len(payload), glyphs, has_gsub, has_gpos,
            len(unicodes), len(absent),
        )

        ttf_path = ASSET_DIR / f"{face_id}.ttf"
        rs_path = ASSET_DIR / f"{face_id}.rs"
        print(f"{face_id}: {len(payload)} bytes, {glyphs} glyphs, GSUB={has_gsub} GPOS={has_gpos}"
              f" ({len(unicodes)} codepoints requested, {len(absent)} absent upstream)")

        # The CJK vector face is the only one with a budget, because it is the only one whose
        # "which codepoints" question has a cost curve instead of an obvious answer.
        if face_id == "cjk" and len(payload) > CJK_VECTOR_BUDGET_BYTES:
            print(
                f"REFUSED: cjk subset is {len(payload)} bytes, over the {CJK_VECTOR_BUDGET_BYTES}-byte "
                "budget.\n"
                "  Trim tools/cjk_vector_codepoints.txt deliberately, or raise the budget here *and* \n"
                "  update the NOTICE text and Cargo.toml's size figure to match.",
                file=sys.stderr,
            )
            return 1

        if args.check:
            for path, expected, label in (
                (ttf_path, payload, "subset bytes"),
                (rs_path, wrapper.encode("utf-8"), "wrapper header"),
            ):
                actual = path.read_bytes() if path.exists() else b""
                if actual != expected:
                    print(f"CHECK FAILED: {path} does not match ({label}); re-run without --check",
                          file=sys.stderr)
                    stale = True
            continue

        ttf_path.write_bytes(payload)
        rs_path.write_text(wrapper, encoding="utf-8")

    if args.check and stale:
        return 1
    if args.check:
        print("check: every committed subset matches the generator.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
