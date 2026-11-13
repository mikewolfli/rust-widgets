#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_large_file_ratchet.sh — issue §8 direction 6 / R-28
#
# The >2000-line census is a debt tracker: it may only shrink. A file that
# crosses the line fails by name; a file that drops below requires updating the
# recorded budget, so progress is recorded rather than forgotten.
#
# Usage: tools/check_large_file_ratchet.sh

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

STATUS=0
OUTPUT="$("$PYTHON" tools/check_large_file_ratchet.py)" || STATUS=$?
printf '%s\n' "$OUTPUT"
[[ "$STATUS" -eq 0 ]] || exit "$STATUS"
echo "check_large_file_ratchet: OK"
