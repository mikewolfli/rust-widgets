#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_cjk_shards_cover_the_face — the CJK shards must partition the `fonts-cjk` face.
#
#     **The union of the shards' codepoints must equal `tools/cjk_vector_codepoints.txt`.**
#
# # Why this gate exists
#
# `fonts-cjk` (one face) and `fonts-cjk-shards` (the same coverage, split on Unicode block
# boundaries) are two packagings of one capability. A host chooses between them by whether it wants
# all of CJK resident or one script at a time — and that choice is only honest if the shards cover
# **the same characters**. A shard set that quietly dropped a range would make the sharded build
# render tofu where the one-face build renders a glyph, and nothing else in the tree would notice:
# both build, both pass every test, and the difference only shows as a missing character on screen.
#
# # What this checks (four assertions)
#
#   1. `tools/cjk_vector_shards.txt` parses to a non-empty, disjoint shard set.
#   2. The union of the shards equals `tools/cjk_vector_codepoints.txt`, codepoint for codepoint.
#   3. `tools/gen_cjk_shards.py --check` agrees that the committed shard files match the list, so
#      the list and the shipped bytes cannot drift.
#   4. The crate builds with the shards on.
#
# # Reverse injection
#
# Deleting a range from `tools/cjk_vector_shards.txt` must make step 2 report the missing
# codepoints by name. See the round's report for the exact output.
#
# # What this gate does NOT prove
#
# That the shards are *useful* as shards — that loading one script does not pull in another. That is
# a build-size property, and it is measured by `--features desktop,fonts-cjk-shard-han` producing a
# smaller binary than `--features desktop,fonts-cjk-shards`; this gate owns coverage, not size.
#
# Principle #58: every command runs under a timeout, via `rw_run_bounded`.

set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

# shellcheck source=tools/lib_timeout.sh
source tools/lib_timeout.sh

SHARD_LIST="tools/cjk_vector_shards.txt"
COVERAGE_LIST="tools/cjk_vector_codepoints.txt"
GEN="tools/gen_cjk_shards.py"

for required in "$SHARD_LIST" "$COVERAGE_LIST" "$GEN"; do
    if [ ! -f "$required" ]; then
        echo "check_cjk_shards_cover_the_face: FAIL"
        echo "  missing $required"
        exit 1
    fi
done

echo "[1/4] the shard list parses and is disjoint"
if ! python3 - "$SHARD_LIST" <<'PY'
import re, sys
from pathlib import Path

path = Path(sys.argv[1])
shards = {}
for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
    line = raw.split("#", 1)[0].strip()
    if not line:
        continue
    shard_id, sep, ranges = line.partition(":")
    if not sep:
        print(f"  line {lineno}: expected `id: ranges`, got {line!r}", file=sys.stderr)
        sys.exit(1)
    shard_id = shard_id.strip()
    if shard_id in shards:
        print(f"  line {lineno}: duplicate shard id {shard_id!r}", file=sys.stderr)
        sys.exit(1)
    codepoints = set()
    for part in ranges.split(","):
        part = part.strip()
        if ".." in part:
            lo, _, hi = part.partition("..")
            codepoints.update(range(int(lo, 16), int(hi, 16) + 1))
        elif part:
            codepoints.add(int(part, 16))
    shards[shard_id] = codepoints

if not shards:
    print("  the shard list parses to an empty set", file=sys.stderr)
    sys.exit(1)

claimed = {}
for shard_id, codepoints in shards.items():
    for cp in codepoints:
        if cp in claimed:
            print(f"  U+{cp:04X} is in both {claimed[cp]!r} and {shard_id!r}", file=sys.stderr)
            sys.exit(1)
        claimed[cp] = shard_id
print(f"  {len(shards)} shards, {len(claimed)} codepoints, disjoint")
PY
then
    echo "  FAIL  the shard list is malformed or overlapping"
    exit 1
fi
echo "  PASS"

echo "[2/4] the shards partition the fonts-cjk coverage"
if ! python3 - "$SHARD_LIST" "$COVERAGE_LIST" <<'PY'
import sys
from pathlib import Path


def parse(path, inline):
    codepoints = set()
    for raw in Path(path).read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        for part in line.split(","):
            part = part.strip()
            if not part:
                continue
            if inline:
                part = part.split(":", 1)[1] if ":" in part else part
            if ".." in part:
                lo, _, hi = part.partition("..")
                codepoints.update(range(int(lo, 16), int(hi, 16) + 1))
            else:
                codepoints.add(int(part, 16))
    return codepoints


shards = parse(sys.argv[1], inline=True)
whole = parse(sys.argv[2], inline=False)

missing = sorted(whole - shards)
extra = sorted(shards - whole)
if missing or extra:
    if missing:
        preview = ", ".join(f"U+{cp:04X}" for cp in missing[:10])
        print(f"  {len(missing)} codepoint(s) in the face but in no shard: {preview}", file=sys.stderr)
    if extra:
        preview = ", ".join(f"U+{cp:04X}" for cp in extra[:10])
        print(f"  {len(extra)} codepoint(s) in a shard but not the face: {preview}", file=sys.stderr)
    sys.exit(1)
print(f"  the {len(shards)} shard codepoints equal the face's {len(whole)}")
PY
then
    echo "  FAIL  the shards do not partition the face"
    exit 1
fi
echo "  PASS"

echo "[3/4] the committed shard files match the list"
if ! rw_run_bounded 600 python3 "$GEN" --license=ofl-1.1 --check; then
    echo "  FAIL  the committed shards are stale; run"
    echo "        python3 tools/gen_cjk_shards.py --license=ofl-1.1"
    exit 1
fi
echo "  PASS"

echo "[4/4] the crate builds with the shards on"
if ! rw_run_bounded 900 cargo check --no-default-features \
        --features desktop,fonts-cjk-shards,runtime-fonts --lib; then
    echo "  FAIL  the sharded build does not compile"
    exit 1
fi
echo "  PASS"

echo
echo "check_cjk_shards_cover_the_face: OK"
