#!/usr/bin/env python3
"""Inspect a colour-emoji face: table directory, `CBLC`/`CBDT` sizes, and the cmap.

Kept as a tool rather than a throwaway because the facts it prints are what the
`fonts-emoji-color` generator's `--check` and the `NOTICE` entry are written from, and a
later face swap has to be re-measured the same way.

Usage:
    python3 tools/emoji_font_probe.py <path-to.ttf>
"""

import struct
import sys
from pathlib import Path


def u16(data, off):
    return struct.unpack_from(">H", data, off)[0]


def u32(data, off):
    return struct.unpack_from(">I", data, off)[0]


def table_directory(data):
    count = u16(data, 4)
    tables = {}
    for i in range(count):
        off = 12 + 16 * i
        tag = data[off:off + 4].decode("latin1")
        offset = u32(data, off + 8)
        length = u32(data, off + 12)
        tables[tag] = (offset, length)
    return tables


def cblc_sizes(data, off):
    """`CBLC` v2/v3 header, then one `BitmapSizeTable` per strike.

    # The two offsets whose base has to be verified, not assumed

    The `CBLC` header is `major`/`minor` (2+2) plus `numSizes` (4) bytes, so the first
    `BitmapSizeTable` begins at `CBLC + 8`.

    `BitmapSizeTable.indexSubTableArrayOffset` then measures from **`CBLC + 8`**, verified against
    the strike's own `startGlyphIndex`/`endGlyphIndex`. It is not a file offset and not a
    `CBLC`-start offset: the two candidates read

    | base | first index record | verdict |
    |---|---|---|
    | `CBLC + 56` | `(65562, 16, 1769750)` | plausible numbers, nonsense |
    | `CBLC + 8 + 56` | `(1, 278, 56)` | matches the strike's glyph range |

    Reading a bitmap table at the wrong base does not fault -- it returns integers -- so the base
    was *verified* by cross-checking the strike header, not assumed. `BitmapSizeTable` is 48 bytes
    (`indexSubTableArrayOffset`, `indexTablesSize`, `numberOfIndexSubTables`, `colorRef`, then two
    12-byte `SbitLineMetrics`), so the array follows the size table at exactly 56.

    `BitmapSizeTable` layout, all big-endian:

    | offset | field |
    |---|---|
    | +0 | `indexSubTableArrayOffset` u32 |
    | +4 | `indexTablesSize` u32 |
    | +8 | `numberOfIndexSubTables` u32 |
    | +12 | `colorRef` u32 |
    | +16..+28 | `hori` `SbitLineMetrics` (12 bytes) |
    | +28..+40 | `vert` `SbitLineMetrics` (12 bytes) |
    | +40 | `startGlyphIndex` u16 |
    | +42 | `endGlyphIndex` u16 |
    | +44 | `ppemX` u8, `ppemY` u8, `bitDepth` u8, `flags` i8 |

    An earlier version of this probe read the header as a flat `>IIIIBBBB` starting at `CBLC+8`,
    which puts `glyphs 2..0 ppem=101x229 bitDepth=136` -- values that pass every sanity check
    except being true.
    """
    major, minor = u16(data, off), u16(data, off + 2)
    print(f"CBLC version {major}.{minor}")
    num_sizes = u32(data, off + 4)
    print(f"numSizes {num_sizes}")
    o = off + 8
    for i in range(num_sizes):
        idx_sub = u32(data, o)
        idx_tables = u32(data, o + 8)
        start_glyph = u16(data, o + 40)
        end_glyph = u16(data, o + 42)
        ppem_x = data[o + 44]
        ppem_y = data[o + 45]
        bit_depth = data[o + 46]
        flags = struct.unpack_from(">b", data, o + 47)[0]
        print(
            f"  strike {i}: indexSubTableArrayOffset={idx_sub} "
            f"numberOfIndexSubTables={idx_tables} glyphs {start_glyph}..{end_glyph} "
            f"ppem={ppem_x}x{ppem_y} bitDepth={bit_depth} flags={flags}"
        )
        o += 48
        a = off + 8 + idx_sub
        for t in range(idx_tables):
            first, last, add = struct.unpack_from(">III", data, a + 12 * t)
            print(
                f"    index {t}: firstGlyph={first} lastGlyph={last} "
                f"additionalOffset={add}"
            )
        first, last, add = struct.unpack_from(">III", data, a)
        # `additionalOffsetToIndexSubtable` is measured from the same base as the array.
        sub = off + 8 + add
        fmt = u16(data, sub)
        print(f"    first index subtable format={fmt}")
        if fmt in (1, 2, 3):
            image_format = u16(data, sub + 2)
            image_data_offset = u32(data, sub + 4)
            print(
                f"    imageFormat={image_format} imageDataOffset={image_data_offset} "
                "(imageFormat 17 = small metrics + PNG, 18 = big metrics + PNG, "
                "19 = PNG without metrics)"
            )


def cmap_summary(data, off):
    n = u16(data, off + 2)
    print(f"cmap: {n} subtable(s)")
    for i in range(n):
        pid = u16(data, off + 4 + 8 * i)
        eid = u16(data, off + 6 + 8 * i)
        sub = off + u32(data, off + 8 + 8 * i)
        fmt = u16(data, sub)
        print(f"  platform={pid} encoding={eid} format={fmt}")
        if fmt == 4:
            seg_x2 = u16(data, sub + 6)
            print(f"    format 4: segCountX2={seg_x2} -> {seg_x2 // 2} segments")
            print("    (a format-4-only cmap means the face is BMP-only)")
        elif fmt == 12:
            groups = u32(data, sub + 12)
            print(f"    format 12: {groups} groups")
            start = u32(data, sub + 16)
            end = u32(data, sub + 20)
            print(f"    first group: U+{start:04X}..U+{end:04X}")


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    path = Path(sys.argv[1])
    data = path.read_bytes()
    print(f"file: {path} ({len(data)} bytes)")
    print(f"sfnt version: {data[:4]!r}")
    tables = table_directory(data)
    print(f"tables: {len(tables)}")
    for tag, (off, length) in sorted(tables.items()):
        print(f"  {tag} offset={off} length={length}")
    if "CBLC" in tables:
        cblc_sizes(data, tables["CBLC"][0])
    if "CBDT" in tables:
        print(f"CBDT: {tables['CBDT'][1]} bytes of bitmap data")
    if "cmap" in tables:
        cmap_summary(data, tables["cmap"][0])
    for tag in ("glyf", "CFF ", "CFF2", "sbix", "COLR", "CPAL", "SVG "):
        if tag in tables:
            print(f"outline/colour table present: {tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
