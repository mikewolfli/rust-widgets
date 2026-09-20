#!/usr/bin/env bash
# Every input-handling control must consult its `enabled` state (principle #37, round 52 carry-over).
#
# # Why this gate exists
#
# `set_enabled(false)` is a host's way to take a control out of play, and that promise is kept only
# where the control's handler checks it. Nothing enforced the check, and the misses clustered in
# controls nobody had audited:
#
#   * `MiniCanvas` emitted `mouse_pressed`/`mouse_released`/`clicked` while disabled — and for a
#     press anywhere in the window, since it also had no hit test.
#   * `ImePreedit` appended keystrokes to its buffer while disabled.
#   * `MaterialSnackbar`, `CupertinoAlertDialog`, `CupertinoSlider`, `MaterialNavigationRail`,
#     `MdiArea`, `CandlestickChart` and `VolumeChart` handled input with no check at all.
#
# The round-52 pass reported "147/187 widgets ignore enabled" and was wrong — most widgets are
# decoration or containers and need no check. That is why the gate is an explicit allowlist with a
# stated reason per entry rather than a blanket requirement: the signal is a *new* file that is
# neither guarded nor listed, not a count.
#
# The check is `tools/check_enabled_is_honoured.py`; see its docstring for the allowlist rationale.
#
# Usage: tools/check_enabled_is_honoured.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_enabled_is_honoured.py; then
    echo "FAIL: a control ignores its `enabled` state"
    exit 1
fi

echo "enabled contract checks passed."
