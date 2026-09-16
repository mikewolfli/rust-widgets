#!/usr/bin/env python3
"""Verify the native window actually shows the CodeEditor's rendered pixels.

Reads a PNG screenshot crop using only the standard library (`zlib`), then checks
whether the editor's distinctive chrome colours are present. A blank window
contains a single colour and fails.

Usage:
    python3 tools/verify_window_pixels.py <crop.png>

Exit code 0 when editor colours are found, 1 otherwise.
"""

import struct
import sys
import zlib


def read_png(path):
    """Decodes a non-interlaced 8-bit RGB/RGBA PNG into (w, h, bpp, rows)."""
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit("not a PNG")

    pos = 8
    width = height = bit_depth = color_type = interlace = None
    idat = bytearray()
    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        ctype = data[pos + 4 : pos + 8]
        payload = data[pos + 8 : pos + 8 + length]
        if ctype == b"IHDR":
            width, height, bit_depth, color_type, _c, _f, interlace = struct.unpack(
                ">IIBBBBB", payload
            )
        elif ctype == b"IDAT":
            idat += payload
        elif ctype == b"IEND":
            break
        pos += 12 + length

    if bit_depth != 8:
        raise SystemExit(f"unsupported bit depth {bit_depth}")
    if interlace != 0:
        raise SystemExit("interlaced PNG not supported")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color_type]

    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    rows = []
    previous = bytearray(stride)
    offset = 0
    for _ in range(height):
        filter_type = raw[offset]
        offset += 1
        line = bytearray(raw[offset : offset + stride])
        offset += stride
        # Undo the per-scanline filter (PNG spec section 9).
        if filter_type == 1:
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif filter_type == 2:
            for i in range(stride):
                line[i] = (line[i] + previous[i]) & 0xFF
        elif filter_type == 3:
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif filter_type == 4:
            for i in range(stride):
                a = line[i - channels] if i >= channels else 0
                b = previous[i]
                c = previous[i - channels] if i >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        rows.append(bytes(line))
        previous = line
    return width, height, channels, rows


# Editor chrome colours that must appear in a real render.
SIGNATURES = {
    "editor background": (252, 253, 255),
    "gutter background": (244, 247, 252),
    "tab strip": (238, 242, 248),
    "editor border": (188, 197, 211),
    "caret blue": (24, 99, 190),
}


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    width, height, channels, rows = read_png(sys.argv[1])
    print(f"crop: {width}x{height}, {channels} channels")

    seen = set()
    for row in rows:
        for x in range(0, len(row) - channels + 1, channels):
            seen.add((row[x], row[x + 1], row[x + 2]))

    print(f"distinct colours in window region: {len(seen)}")
    missing = []
    for name, rgb in SIGNATURES.items():
        found = rgb in seen
        print(f"  {name:20} {str(rgb):18} {'FOUND' if found else 'MISSING'}")
        if not found:
            missing.append(name)

    if missing:
        print(f"\nFAIL: window does not show the editor ({', '.join(missing)} absent)")
        sample = sorted(seen)[:6]
        print(f"      colours actually present (sample): {sample}")
        return 1
    print("\nOK: the native window contains the editor's rendered pixels")
    return 0


if __name__ == "__main__":
    sys.exit(main())
