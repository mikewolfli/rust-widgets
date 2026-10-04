#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# _inject_probe.sh — run one gate against one injected edit, then restore.
#
# This is a *helper for authoring* `tools/gates_reverse_injection.md`, not a gate
# itself (it is deliberately named with a leading underscore so the
# `tools/check_*.sh` glob never picks it up). The record's own rule is that a gate
# is only trusted once it has been shown to go red; doing that by hand for ~76
# gates means 76 opportunities to forget the restore and leave the tree dirty.
#
# It does exactly four things, in order:
#   1. copy the target file to a scratch backup;
#   2. apply the caller's edit command;
#   3. run the gate under a timeout, capturing combined output;
#   4. restore the target file from the backup — **on every exit path**, including
#      Ctrl-C and a gate that hangs, via the EXIT trap.
#
# Usage:
#   tools/_inject_probe.sh <gate> <target-file> <edit-command...>
#
# Example (rename a real API so the cookbook scan must report it):
#   tools/_inject_probe.sh tools/check_cookbook.sh \
#     cookbook/en/src/chapters/performance-quality.md \
#     sed -i 's/BaseWidget::request_redraw/BaseWidget::request_redraw_x/'
#
# Prints the gate's own output between markers so the `reported` column of the
# record can quote it verbatim.

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR" || exit 1

# `timeout` is coreutils and is absent on a default macOS/BSD userland, so this helper
# uses the same portable bound as the gates: `rw_run_bounded` (GNU `timeout` when present,
# a pure-bash watchdog otherwise).
. "$ROOT_DIR/tools/lib_timeout.sh"

if [ "$#" -lt 3 ]; then
    echo "usage: $0 <gate> <target-file> <edit-command...>" >&2
    exit 2
fi

GATE="$1"
TARGET="$2"
shift 2

if [ ! -f "$GATE" ]; then
    echo "FAIL  no such gate: $GATE" >&2
    exit 2
fi
if [ ! -f "$TARGET" ]; then
    echo "FAIL  no such target: $TARGET" >&2
    exit 2
fi

BACKUP="$(mktemp)"
cp "$TARGET" "$BACKUP"

restore() {
    cp "$BACKUP" "$TARGET"
    rm -f "$BACKUP"
}
trap restore EXIT INT TERM

echo "[probe] target=$TARGET"
echo "[probe] edit: $*"
# Append the target path so a `sed`/`perl` in-place command does not have to repeat
# it (and so a relative path is interpreted from the repo root this script cd'd to).
"$@" "$TARGET" || {
    echo "[probe] the edit command itself failed; nothing was run" >&2
    exit 2
}

echo "----- gate output begin -----"
rw_run_bounded "${RW_GATE_TIMEOUT:-900}" bash "$GATE" 2>&1
GATE_RC=$?
echo "----- gate output end (exit=$GATE_RC) -----"

if [ "$GATE_RC" -eq 0 ]; then
    echo "[probe] the gate PASSED after the injection: the injection is wrong," >&2
    echo "        or the gate is too narrow. Both are findings." >&2
else
    echo "[probe] the gate went red as required (exit=$GATE_RC)."
fi
