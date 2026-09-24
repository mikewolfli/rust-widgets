#!/usr/bin/env python3
"""Dump the codepoints the full NotoColorEmoji face covers, as a starting point for a subset list.

Prints `U+XXXX  name-ish` lines so a hand-curated list can be assembled without a GUI font browser.
"""

import argparse
import struct
import sys
import unicodedata
from pathlib import Path

DEFAULT = Path("target/font-cache/emoji-NotoColorEmoji.ttf")


def read_table(d, tag):
    count = struct.unpack_from(">H", d, 4)[0]
    for i in range(count):
        off = 12 + i * 16
        if d[off:off + 4] == tag:
            return struct.unpack_from(">II", d, off + 8)
    return None


def cmap_codepoints(d):
    off, _ = read_table(d, b"cmap")
    n = struct.unpack_from(">H", d, off + 2)[0]
    best = None
    for i in range(n):
        pid, eid, sub = struct.unpack_from(">HHI", d, off + 4 + i * 8)
        fmt = struct.unpack_from(">H", d, off + sub)[0]
        if fmt == 12:
            best = off + sub
    if best is None:
        return {}
    ngroups = struct.unpack_from(">I", d, best + 12)[0]
    out = {}
    for g in range(ngroups):
        s, e, gid = struct.unpack_from(">III", d, best + 16 + g * 12)
        for cp in range(s, e + 1):
            out[cp] = gid + (cp - s)
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--source", default=str(DEFAULT))
    ap.add_argument("--ranges", action="store_true", help="print only the codepoint ranges")
    args = ap.parse_args()
    d = Path(args.source).read_bytes()
    cmap = cmap_codepoints(d)
    print(f"{len(cmap)} codepoints", file=sys.stderr)
    if args.ranges:
        cps = sorted(cmap)
        start = prev = cps[0]
        for cp in cps[1:]:
            if cp == prev + 1:
                prev = cp
                continue
            print(f"{start:04X}..{prev:04X}")
            start = prev = cp
        print(f"{start:04X}..{prev:04X}")
        return 0
    for cp, _gid in sorted(cmap.items()):
        if cp > 0x10FFFF:
            continue
        try:
            name = unicodedata.name(chr(cp))
        except ValueError:
            name = "<unnamed>"
        print(f"U+{cp:04X} {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
