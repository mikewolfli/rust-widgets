#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Generate the unicode-range shards of the CJK vector face (BLUE23 §0A.4 follow-up).

# What this produces

For each shard in `tools/cjk_vector_shards.txt`, two files:

    src/render/text/font_assets/cjk_shard_<id>.ttf   the subset itself
    src/render/text/font_assets/cjk_shard_<id>.rs     an `include_bytes!` with a provenance header

and one index module:

    src/render/text/font_assets/cjk_shards.rs          the shard table `shard_for(ch)` reads

# Why this is a separate script from `gen_font_subset.py`

`gen_font_subset.py` produces the **one-face** payloads (`latin`, `arabic`, `cjk`). This one
produces a **partition** of one of them and an index that must agree with `gen_font_subset.py`'s
`cjk` output — a different job with a different invariant (the partition equals the whole), and
folding it in would put a partition and a whole behind one `--face` argument.

# The invariant this script enforces

The union of the shards' codepoints must equal `tools/cjk_vector_codepoints.txt` (the list
`gen_font_subset.py --face=cjk` reads). That is the whole point of sharding — the shards are the
same coverage, split — and this script refuses to emit if the two lists disagree, so a shard edit
that quietly drops a character fails here rather than as a glyph that stopped rendering.

Usage:
    python3 tools/gen_cjk_shards.py --license=ofl-1.1
    python3 tools/gen_cjk_shards.py --license=ofl-1.1 --check     # exit 1 if stale

Exit status: 0 = written (or `--check` agrees), 2 = licence gate refused, 1 = other.
"""

import argparse
import hashlib
import io
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
ASSET_DIR = REPO_ROOT / "src" / "render" / "text" / "font_assets"
SHARD_LIST = REPO_ROOT / "tools" / "cjk_vector_shards.txt"
COVERAGE_LIST = REPO_ROOT / "tools" / "cjk_vector_codepoints.txt"

# The upstream the shards are cut from. Pinned to the same revision as `CJK_VECTOR_FACE` in
# `gen_font_subset.py`: a shard set cut from a different revision would not be a partition of the
# face it claims to partition.
UPSTREAM = {
    "project": "Noto Sans SC",
    "url": "https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/SubsetOTF/SC/"
           "NotoSansSC-Regular.otf",
    "sha256": "faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9",
    "instance": None,
}

# The licence the outlines are relied on under. Same gate as `gen_font_subset.py`, and the same
# reason: third-party outlines may not enter the tree through a run that did not name a licence.
LICENSES = {"ofl-1.1": "SIL Open Font License 1.1"}

# A per-shard budget, so one shard cannot grow into "the whole face, renamed". The largest shard
# (`han`, 800 ideographs) is expected around 330 KB; this is a ceiling, not a target.
SHARD_BUDGET_BYTES = 400_000


def read_shard_list(path):
    """Parse `id: A..B, C..D` lines into `[(id, [codepoints])]`, in file order."""
    if not path.exists():
        raise SystemExit(f"REFUSED: {path} is missing; there is no default shard set.")
    shards = []
    seen_ids = set()
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        shard_id, sep, ranges = line.partition(":")
        if not sep:
            raise SystemExit(f"REFUSED: {path}:{lineno}: expected `id: ranges`, got {line!r}")
        shard_id = shard_id.strip()
        if not shard_id:
            raise SystemExit(f"REFUSED: {path}:{lineno}: empty shard id")
        if shard_id in seen_ids:
            raise SystemExit(f"REFUSED: {path}:{lineno}: duplicate shard id {shard_id!r}")
        seen_ids.add(shard_id)
        codepoints = set()
        for part in ranges.split(","):
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
            raise SystemExit(f"REFUSED: {path}:{lineno}: shard {shard_id!r} covers nothing")
        shards.append((shard_id, sorted(codepoints)))
    if not shards:
        raise SystemExit(f"REFUSED: {path} parsed to an empty shard set.")

    # The partition must be disjoint: two shards claiming one codepoint would make "which file is
    # this character in?" ambiguous, and `shard_for` would answer with whichever came first.
    claimed = {}
    for shard_id, codepoints in shards:
        for cp in codepoints:
            if cp in claimed:
                raise SystemExit(
                    f"REFUSED: U+{cp:04X} is in both {claimed[cp]!r} and {shard_id!r}; shards must "
                    "partition the coverage, not overlap it."
                )
            claimed[cp] = shard_id
    return shards


def read_coverage(path):
    """The same parser `gen_font_subset.py` uses, so the two lists are read the same way."""
    if not path.exists():
        raise SystemExit(f"REFUSED: {path} is missing; there is no coverage to partition.")
    codepoints = set()
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        for part in line.split(","):
            part = part.strip()
            if not part:
                continue
            if ".." in part:
                lo, _, hi = part.partition("..")
                lo_i, hi_i = int(lo, 16), int(hi, 16)
                codepoints.update(range(lo_i, hi_i + 1))
            else:
                codepoints.add(int(part, 16))
    return codepoints


def resolve_source(refresh):
    """Noto Sans SC from the cache (or downloaded), verified against the pinned SHA-256."""
    import urllib.request

    cache_dir = REPO_ROOT / "target" / "font-cache"
    cached = cache_dir / "cjk-NotoSansSC-Regular.otf"
    if refresh or not cached.exists():
        cache_dir.mkdir(parents=True, exist_ok=True)
        print(f"downloading {UPSTREAM['url']}")
        request = urllib.request.Request(
            UPSTREAM["url"], headers={"User-Agent": "rust-widgets/gen_cjk_shards.py"}
        )
        with urllib.request.urlopen(request, timeout=120) as response:
            cached.write_bytes(response.read())
    digest = hashlib.sha256(cached.read_bytes()).hexdigest()
    if digest != UPSTREAM["sha256"]:
        raise SystemExit(
            f"REFUSED: {cached} is not the pinned upstream file.\n"
            f"  expected SHA-256 {UPSTREAM['sha256']}\n  found    SHA-256 {digest}\n"
            "  A different revision is not a partition of the shipped `cjk` face."
        )
    return cached


def subset_bytes(source, codepoints):
    """One shard's bytes. Mirrors `gen_font_subset.py`'s options so the shards partition cleanly."""
    try:
        from fontTools import subset
        from fontTools.ttLib import TTFont
    except ImportError as missing:  # pragma: no cover - environment-dependent
        print(f"fontTools is required ({missing}).\n  python3 -m pip install fonttools", file=sys.stderr)
        raise SystemExit(1)

    font = TTFont(source, recalcTimestamp=False)
    options = subset.Options()
    options.layout_features = ["*"]
    options.notdef_outline = True
    options.drop_tables += ["DSIG"]

    available = set(font.getBestCmap().keys())
    wanted = [cp for cp in codepoints if cp in available]
    absent = sorted(set(codepoints) - available)
    if not wanted:
        raise SystemExit("REFUSED: none of the shard's codepoints exist in the face.")

    subsetter = subset.Subsetter(options=options)
    subsetter.populate(unicodes=wanted)
    subsetter.subset(font)
    buffer = io.BytesIO()
    font.save(buffer)
    return buffer.getvalue(), absent


def shard_module(shard_id, license_id, payload_len, glyphs, requested, absent, ranges_text):
    """The `include_bytes!` module for one shard, with the provenance header the gate requires."""
    return f'''// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//
// GENERATED FILE — DO NOT EDIT BY HAND.
// Produced by `tools/gen_cjk_shards.py`. Regenerate with:
//     python3 tools/gen_cjk_shards.py --license={license_id}
//
// Glyph source: {UPSTREAM["project"]}, shard `{shard_id}` — a partition of the `fonts-cjk` face.
//     {UPSTREAM["url"]}
//     SHA-256 (upstream .ttf): {UPSTREAM["sha256"]}
//
// Ranges: {ranges_text}
//
// Licence of the glyph outlines: {LICENSES[license_id]}. See the repository-root `NOTICE`, which
// records this file by name together with the facts above.
//
// The shard is {payload_len} bytes ({glyphs} glyphs), covering {requested} codepoints
// ({absent} of those requested are absent from this upstream revision and were skipped).
// The shard set is defined in `tools/cjk_vector_shards.txt`; the union of the shards equals
// `tools/cjk_vector_codepoints.txt`, which `tools/check_cjk_shards_cover_the_face.sh` asserts.

/// This shard's bytes. Parsed on first use, never copied into a heap allocation.
pub const FONT: &[u8] = include_bytes!("cjk_shard_{shard_id}.ttf");
'''


def index_module(shards, sizes):
    """The shard table `shard_for(ch)` reads, plus the feature-gated list of active shards."""
    lines = [
        "// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)",
        "// SPDX-License-Identifier: MIT",
        "//",
        "// GENERATED FILE — DO NOT EDIT BY HAND.",
        "// Produced by `tools/gen_cjk_shards.py`. Regenerate with:",
        "//     python3 tools/gen_cjk_shards.py",
        "//",
        "// The CJK vector face, partitioned on Unicode block boundaries. Each shard is one",
        "// `include_bytes!` payload behind its own `fonts-cjk-shard-<id>` feature, so a host that",
        "// draws one script loads one script's glyphs. `shard_for` names the file a character lives",
        "// in; `active_shards` lists the shards this build carries.",
        "//",
        "// Glyph source: Noto Sans SC, by the Noto project (Adobe and Google). The shards are a",
        "// partition of the `fonts-cjk` face's coverage, cut from the same upstream revision.",
        f"//     {UPSTREAM['url']}",
        f"//     SHA-256 (upstream .otf): {UPSTREAM['sha256']}",
        "//",
        "// Licence of the glyph outlines: SIL Open Font License, Version 1.1. The repository-root",
        "// `NOTICE` records this file by name together with the facts above.",
        "//",
        "// The union of the shards equals `tools/cjk_vector_codepoints.txt` (the `fonts-cjk` face's",
        "// coverage), asserted by `tools/check_cjk_shards_cover_the_face.sh`.",
        "",
    ]
    # One module per shard, each behind its own feature. The `#[path]` is required: this index is
    # a module file, so a bare `mod cjk_shard_x;` would be looked for under a `cjk_shards/`
    # directory, while the generated shard modules live beside this file.
    for shard_id, _codepoints, ranges_text in shards:
        feature = f"fonts-cjk-shard-{shard_id}"
        lines.append(f'#[cfg(feature = "{feature}")]')
        lines.append(f'#[path = "cjk_shard_{shard_id}.rs"]')
        lines.append(f"mod cjk_shard_{shard_id};")
        lines.append(f'#[cfg(feature = "{feature}")]')
        lines.append(f"pub use cjk_shard_{shard_id}::FONT as {shard_id.upper()};")
        lines.append("")
    lines.append("/// One shard: its id, its bytes (when this build carries it), and its coverage.")
    lines.append("pub struct Shard {")
    lines.append("    /// The shard id, as spelled in `tools/cjk_vector_shards.txt`.")
    lines.append("    pub id: &'static str,")
    lines.append("    /// The family name a diagnostic prints for this shard.")
    lines.append("    pub name: &'static str,")
    lines.append("    /// The shard's bytes, or an empty slice when no feature carries it.")
    lines.append("    pub bytes: &'static [u8],")
    lines.append("    /// The range of codepoints this shard covers, inclusive at both ends.")
    lines.append("    pub range: (u32, u32),")
    lines.append("}")
    lines.append("")
    lines.append("/// Every shard, in file order, with the bytes of the ones this build carries.")
    lines.append("///")
    lines.append("/// A shard whose feature is off has an empty `bytes` and is skipped by the loaders, so the")
    lines.append("/// table's shape does not depend on the feature set — a test can enumerate every shard on")
    lines.append("/// any build and assert which ones are resident.")
    lines.append("pub const SHARDS: &[Shard] = &[")
    for shard_id, codepoints, _ranges_text in shards:
        name = f"Noto Sans SC ({shard_id})"
        feature = f"fonts-cjk-shard-{shard_id}"
        lines.append("    Shard {")
        lines.append(f'        id: "{shard_id}",')
        lines.append(f'        name: "{name}",')
        lines.append(f"        range: (0x{codepoints[0]:04X}, 0x{codepoints[-1]:04X}),")
        lines.append(f'        #[cfg(feature = "{feature}")]')
        lines.append(f"        bytes: {shard_id.upper()},")
        lines.append(f'        #[cfg(not(feature = "{feature}"))]')
        lines.append("        bytes: &[],")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    lines.append("/// The shard that covers `ch`, or `None` when no shard's range contains it.")
    lines.append("///")
    lines.append("/// A range test rather than a glyph-table probe: the ranges partition the coverage, so a")
    lines.append("/// character is in at most one shard and the answer is a comparison, not a parse. Whether")
    lines.append("/// this **build** carries that shard is a separate question — see `Shard::bytes`.")
    lines.append("pub fn shard_for(ch: char) -> Option<&'static Shard> {")
    lines.append("    let codepoint = ch as u32;")
    lines.append("    SHARDS.iter().find(|shard| {")
    lines.append("        let (low, high) = shard.range;")
    lines.append("        codepoint >= low && codepoint <= high")
    lines.append("    })")
    lines.append("}")
    lines.append("")
    lines.append("/// The shards this build carries (a non-empty `bytes`), in file order.")
    lines.append("pub fn active_shards() -> impl Iterator<Item = &'static Shard> {")
    lines.append("    SHARDS.iter().filter(|shard| !shard.bytes.is_empty())")
    lines.append("}")
    lines.append("")
    lines.append(f"/// The total bytes this build's shards weigh. {sum(sizes)} bytes when every shard is on.")
    lines.append("pub const fn total_bytes() -> usize {")
    lines.append("    let mut total = 0;")
    lines.append("    let mut index = 0;")
    lines.append("    while index < SHARDS.len() {")
    lines.append("        total += SHARDS[index].bytes.len();")
    lines.append("        index += 1;")
    lines.append("    }")
    lines.append("    total")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--license", dest="license_id", default=None,
                        help=f"the licence the caller relies on; one of {sorted(LICENSES)}")
    parser.add_argument("--refresh", action="store_true", help="re-download the upstream file")
    parser.add_argument("--check", action="store_true", help="verify the committed files, write nothing")
    args = parser.parse_args(argv)

    if args.license_id is None:
        print(
            "REFUSED: no --license given.\n"
            "These outlines are third-party material; the run must record the licence it relies on.\n"
            "  e.g.  python3 tools/gen_cjk_shards.py --license=ofl-1.1",
            file=sys.stderr,
        )
        return 2
    if args.license_id not in LICENSES:
        print(f"REFUSED: unknown --license={args.license_id!r}; known: {sorted(LICENSES)}",
              file=sys.stderr)
        return 2

    shards = read_shard_list(SHARD_LIST)
    covered = sorted(cp for _id, cps in shards for cp in cps)
    whole = sorted(read_coverage(COVERAGE_LIST))

    # The partition invariant: the shards must be exactly the `fonts-cjk` face's coverage. A shard
    # set that is a strict subset would silently lose characters the one-face build renders.
    if covered != whole:
        missing = sorted(set(whole) - set(covered))
        extra = sorted(set(covered) - set(whole))
        print("REFUSED: the shards do not partition `cjk_vector_codepoints.txt`.", file=sys.stderr)
        if missing:
            preview = ", ".join(f"U+{cp:04X}" for cp in missing[:10])
            print(f"  missing from the shards: {len(missing)} codepoints ({preview}...)", file=sys.stderr)
        if extra:
            preview = ", ".join(f"U+{cp:04X}" for cp in extra[:10])
            print(f"  in the shards but not the face: {len(extra)} codepoints ({preview}...)", file=sys.stderr)
        return 1

    source = resolve_source(args.refresh)

    # Ids and their ranges, for the generated index's `range` field and the header text.
    rendered_shards = []
    sizes = []
    stale = False
    # The table is emitted in **codepoint order**, not file order. The file is written for a human
    # (one script per line, grouped); the table is read by `shard_for`, whose `find` is only
    # unambiguous when ranges are ascending and disjoint. Emitting in file order would put `han`
    # (U+4E00) after `fullwidth` (U+FF00) and make the linear scan's correctness depend on the file
    # layout rather than on the ranges.
    shards = sorted(shards, key=lambda entry: entry[1][0])
    for shard_id, codepoints in shards:
        payload, absent = subset_bytes(source, codepoints)
        glyphs, _has_gsub, _has_gpos = _metrics(payload)
        if len(payload) > SHARD_BUDGET_BYTES:
            print(
                f"REFUSED: shard {shard_id!r} is {len(payload)} bytes, over the "
                f"{SHARD_BUDGET_BYTES}-byte budget.\n"
                "  Trim `tools/cjk_vector_shards.txt` deliberately, or raise the budget here *and*\n"
                "  update the NOTICE text to match.",
                file=sys.stderr,
            )
            return 1
        ranges_text = ", ".join(f"U+{cp:04X}" for cp in (codepoints[0], codepoints[-1]))
        print(f"{shard_id}: {len(payload)} bytes, {glyphs} glyphs "
              f"({len(codepoints)} codepoints requested, {len(absent)} absent upstream)")
        sizes.append(len(payload))
        rendered_shards.append((shard_id, codepoints, ranges_text))

        ttf_path = ASSET_DIR / f"cjk_shard_{shard_id}.ttf"
        rs_path = ASSET_DIR / f"cjk_shard_{shard_id}.rs"
        module = shard_module(shard_id, args.license_id, len(payload), glyphs,
                              len(codepoints), len(absent), ranges_text)
        if args.check:
            for path, expected, label in (
                (ttf_path, payload, "subset bytes"),
                (rs_path, module.encode("utf-8"), "wrapper header"),
            ):
                actual = path.read_bytes() if path.exists() else b""
                if actual != expected:
                    print(f"CHECK FAILED: {path} does not match ({label})", file=sys.stderr)
                    stale = True
        else:
            ttf_path.write_bytes(payload)
            rs_path.write_text(module, encoding="utf-8")

    index_path = ASSET_DIR / "cjk_shards.rs"
    index = index_module(rendered_shards, sizes)
    if args.check:
        actual = index_path.read_text(encoding="utf-8") if index_path.exists() else ""
        if actual != index:
            print(f"CHECK FAILED: {index_path} is stale", file=sys.stderr)
            stale = True
    else:
        index_path.write_text(index, encoding="utf-8")

    if args.check:
        if stale:
            return 1
        print("check: every committed shard matches the generator.")
    else:
        print(f"wrote {len(rendered_shards)} shards + index; total {sum(sizes)} bytes")
    return 0


def _metrics(payload):
    """`(glyph count, has GSUB, has GPOS)` — the same report `gen_font_subset.py` prints."""
    try:
        from fontTools.ttLib import TTFont
    except ImportError:  # pragma: no cover
        return ("?", False, False)
    font = TTFont(io.BytesIO(payload))
    return (font["maxp"].numGlyphs, "GSUB" in font, "GPOS" in font)


if __name__ == "__main__":
    sys.exit(main())
