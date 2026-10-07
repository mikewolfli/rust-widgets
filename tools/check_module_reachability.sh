#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# Module reachability gate (principle #72). Runs `tools/check_module_reachability.py`
# under a wall-clock bound and asserts its result contract, so a silent zero exit
# (an interpreter that never really ran the validator) is reported as a failure
# rather than a pass.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

OUTPUT=""
if ! OUTPUT="$(rw_run_bounded 600 "$PYTHON" tools/check_module_reachability.py "$@")"; then
    printf '%s\n' "$OUTPUT"
    exit 1
fi

# `--report` prints one line per module, not the validator's own summary, so only
# assert the result contract on a normal (summarising) run.
if [ "$#" -eq 0 ] && ! printf '%s\n' "$OUTPUT" | grep -q 'module reachability:'; then
    printf '%s\n' "$OUTPUT"
    echo "FAIL: check_module_reachability.py exited 0 without its result summary"
    exit 1
fi

printf '%s\n' "$OUTPUT"
