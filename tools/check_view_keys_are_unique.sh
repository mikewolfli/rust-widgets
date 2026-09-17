#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Shell wrapper for `check_view_keys_are_unique.py` (BLUE18 rule #88).
#
# It exists so the gate is invoked the same way as every other one in `tools/`
# (through `tools/check_*.sh`), and so the interpreter resolution in
# `tools/lib_python.sh` — which refuses to trust `python3` on PATH — applies here
# too.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

"$PYTHON" tools/check_view_keys_are_unique.py "$@"
