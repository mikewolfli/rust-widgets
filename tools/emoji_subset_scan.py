#!/usr/bin/env python3
"""Scan the full NotoColorEmoji face and report what a given subset would cover.

This exists because the shipped face is 10.7 MB with ~1.2k colour glyphs, and a demo library
cannot ship that. The question "how many emoji can be had for how many bytes" has to be answered
from the actual PNG sizes rather than guessed, so this prints it.

Usage: python3 tools/emoji_subset_scan.py [--source PATH] [--max-bytes N]
"""

import argparse
import collections
import struct
import sys
from pathlib import Path

DEFAULT = Path("target/font-cache/emoji-NotoColorEmoji.ttf")


def tables(d):
    count = struct.unpack_from(">H", d, 4)[0]
    out = {}
    for i in range(count):
        off = 12 + i * 16
        tag = d[off:off + 4]
        toff, tlen = struct.unpack_from(">II", d, off + 8)
        out[tag] = (toff, tlen)
    return out


def cblc_glyph_sizes(d, cbdt, cblc):
    """Every glyph's PNG length, from CBLC's index subtables against CBDT."""
    num_sizes = struct.unpack_from(">I", d, cblc + 4)[0]
    sizes = {}
    for s in range(num_sizes):
        base = cblc + 8 + s * 48
        array_off = struct.unpack_from(">I", d, base)[0]
        count = struct.unpack_from(">I", d, base + 8)[0]
        start_g = struct.unpack_from(">H", d, base + 40)[0]
        end_g = struct.unpack_from(">H", d, base + 42)[0]
        ppem_x, ppem_y, depth, flags = struct.unpack_from(">BBBB", d, base + 44)
        print(f"strike {s}: glyphs {start_g}..{end_g} ppem={ppem_x}x{ppem_y} depth={depth}")
        lst = cblc + array_off
        for r in range(count):
            ent = lst + r * 8
            first, last, sub_off = struct.unpack_from(">HHI", d, ent)
            sub = lst + sub_off
            fmt, img_fmt = struct.unpack_from(">HH", d, sub)
            data_off = struct.unpack_from(">I", d, sub + 4)[0]
            if fmt != 1:
                print(f"  record {r}: glyphs {first}..{last} indexFormat={fmt} (skipped)")
                continue
            n = last - first + 2
            offs = struct.unpack_from(f">{n}I", d, sub + 8)
            hdr = {17: 9, 18: 12, 19: 4}.get(img_fmt)
            if hdr is None:
                print(f"  record {r}: imageFormat={img_fmt} (not PNG, skipped)")
                continue
            for i in range(n - 1):
                gid = first + i
                if gid > end_g:
                    break
                img = cbdt + data_off + offs[i]
                if img + hdr > len(d):
                    continue
                data_len = struct.unpack_from(">I", d, img + hdr - 4)[0]
                sizes[gid] = data_len
    return sizes


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source", default=str(DEFAULT))
    ap.add_argument("--max-bytes", type=int, default=400_000)
    args = ap.parse_args()

    path = Path(args.source)
    if not path.exists():
        print(f"missing {path}", file=sys.stderr)
        return 1
    d = path.read_bytes()
    t = tables(d)
    cbdt, _ = t[b"CBDT"]
    cblc, _ = t[b"CBLC"]
    print(f"{path} ({len(d)} bytes) CBDT@{cbdt} CBLC@{cblc}")
    sizes = cblc_glyph_sizes(d, cbdt, cblc)
    print(f"glyphs with images: {len(sizes)}")
    total = sum(sizes.values())
    print(f"total PNG bytes: {total}")
    ordered = sorted(sizes.items(), key=lambda kv: kv[1])
    running = 0
    for n, (gid, sz) in enumerate(ordered, 1):
        running += sz
        if running > args.max_bytes:
            print(f"first {n - 1} glyphs (smallest first) = {running - sz} bytes")
            break
    # Distribution by size bucket, to show what "the cheapest N" costs.
    buckets = collections.Counter()
    for sz in sizes.values():
        buckets[min(sz // 1000, 20)] += 1
    print("size buckets (KB->count):", dict(sorted(buckets.items())))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
