#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
"""Gate: every raster embedded in a control snapshot is a picture, not a placeholder.

# Why a size check on top of the byte comparison

`check_svg_snapshots.sh` proves two things about an embedded raster: that a committed snapshot
equals what its producer writes today, and (step 6) that the payload is a format every SVG renderer
understands. Neither can see a *regression in the sample data*: if an arm goes back to a 1×1
placeholder or a flat two-pixel swatch, the producer and the committed file still agree, the format
is still PNG, and every gate stays green while the control's picture has stopped showing anything.

That is not hypothetical. W7-02 replaced the image family's sample arms precisely because a 32×32
checkerboard in the corner of a 240×120 frame read as "no image"; the next regression of the same
kind would be equally invisible.

# What is asserted

  * every `data:image/...;base64,…` payload in a snapshot decodes to a raster whose header the
    gate can read;
  * every such raster is at least `MIN_SIDE` pixels on each side and `MIN_PIXELS` pixels in area.

The numbers are a floor, not a target. Two real measurements set them:

  * the smallest raster in a **single-control** snapshot is a 37×28 gallery thumbnail (1036 px);
  * a **companion-pair** snapshot composes the same gallery much smaller, and its thumbnail is
    16×12 (192 px) — a legitimate picture, not a placeholder.

A placeholder is 1×1 (1 px) or a flat 2×1 (2 px), so a floor of 128 px clears the smallest real
raster with room to spare and still rejects both. The gate also prints the smallest raster it saw,
so a sample that slowly shrinks shows up in the output before it trips the floor.

# PNG and BMP

PNG is what the SVG backend writes (`encode_raster`); BMP is the documented fallback for a build
with no image encoder. Both headers are read here so the gate keeps working in either build.

Usage:
    python3 tools/check_snapshot_rasters.py [--dir snapshots/svg]
"""

from __future__ import annotations

import base64
import pathlib
import re
import struct
import sys

EMBEDDED = re.compile(rb"data:image/([a-z]+);base64,([A-Za-z0-9+/=]+)")
# A raster smaller than this on either side, or smaller than this in area, is a placeholder rather
# than a picture. The smallest real raster in the tree is a 16×12 (192 px) thumbnail inside a
# companion-pair snapshot; the historical placeholders were 1×1 and 2×1.
MIN_SIDE = 8
MIN_PIXELS = 128


def raster_size(mime: bytes, payload: bytes) -> tuple[int, int] | None:
    """The raster's `(width, height)` from its own header, or `None` when unreadable."""
    try:
        raw = base64.b64decode(payload, validate=True)
    except Exception:
        return None
    if mime == b"png":
        # 8-byte signature, 4-byte length, 4-byte "IHDR", then width and height as big-endian u32.
        if len(raw) < 24 or raw[12:16] != b"IHDR":
            return None
        width, height = struct.unpack(">II", raw[16:24])
        return width, height
    if mime == b"bmp":
        # BITMAPINFOHEADER: width at 18 (i32), height at 22 (i32, negative for top-down rows).
        if len(raw) < 26:
            return None
        width, height = struct.unpack("<ii", raw[18:26])
        return abs(width), abs(height)
    return None


def main(argv: list[str]) -> int:
    directory = pathlib.Path("snapshots/svg")
    if "--dir" in argv:
        directory = pathlib.Path(argv[argv.index("--dir") + 1])
    if not directory.is_dir():
        print(f"FAIL {directory} is not a directory")
        return 2

    files = sorted(directory.glob("*.svg"))
    if not files:
        print(f"FAIL {directory} holds no .svg files")
        return 2

    findings: list[str] = []
    scanned = 0
    smallest: tuple[int, str] = (1 << 30, "")

    for path in files:
        data = path.read_bytes()
        for mime, payload in EMBEDDED.findall(data):
            scanned += 1
            size = raster_size(mime, payload)
            if size is None:
                findings.append(f"{path.name}: an embedded `data:image/{mime.decode()}` has no readable header")
                continue
            width, height = size
            area = width * height
            if width < MIN_SIDE or height < MIN_SIDE or area < MIN_PIXELS:
                findings.append(
                    f"{path.name}: embedded raster is {width}x{height} ({area} px), "
                    f"smaller than the {MIN_SIDE}px / {MIN_PIXELS}px placeholder floor"
                )
            if area < smallest[0]:
                smallest = (area, f"{path.name} ({width}x{height})")

    if scanned == 0:
        print(f"FAIL no embedded raster found under {directory}; the gate measured nothing")
        return 1

    if findings:
        print("FAIL these snapshots embed a placeholder instead of a picture:")
        for finding in findings:
            print(f"  {finding}")
        print("  A sample arm regressed: the byte comparison and the format check both stay green")
        print("  when a control's sample data shrinks to a placeholder. Fix the arm in")
        print("  src/widget/sample_fill/ and regenerate the snapshots.")
        return 1

    print(
        f"snapshot rasters: {scanned} embedded raster(s) across {len(files)} snapshot(s), "
        f"smallest {smallest[0]} px ({smallest[1]})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
