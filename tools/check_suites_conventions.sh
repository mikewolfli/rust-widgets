#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_suites_conventions.sh — issue §8 extension direction 1
#
# Four main-package conventions must also hold in `crates/rust-widgets-suites`:
# key codes, disabled visual, focus ring, RTL column mirroring. The Python
# scanner documents the exact criteria and strips test modules.
#
# Usage: tools/check_suites_conventions.sh
# Exit 0 = every suites control follows the four conventions.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

STATUS=0
OUTPUT="$("$PYTHON" tools/check_suites_conventions.py)" || STATUS=$?
printf '%s\n' "$OUTPUT"
[[ "$STATUS" -eq 0 ]] || exit "$STATUS"
echo "check_suites_conventions: OK"
