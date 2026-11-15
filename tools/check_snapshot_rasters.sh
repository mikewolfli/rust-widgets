#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_snapshot_rasters.sh — issue-20261115 §10.3 (sample-asset placeholder detection)
#
# Every raster embedded in a control snapshot must be a picture, not a placeholder. The byte
# comparison in `check_svg_snapshots.sh` cannot see this class of regression: a sample arm that
# shrinks back to a 1×1 swatch still produces committed bytes equal to its own producer.
#
# Usage: tools/check_snapshot_rasters.sh [--dir <snapshots dir>]

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

STATUS=0
OUTPUT="$("$PYTHON" tools/check_snapshot_rasters.py "$@")" || STATUS=$?
printf '%s\n' "$OUTPUT"
[[ "$STATUS" -eq 0 ]] || exit "$STATUS"
echo "check_snapshot_rasters: OK"
