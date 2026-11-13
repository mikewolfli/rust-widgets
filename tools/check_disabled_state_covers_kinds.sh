#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_disabled_state_covers_kinds.sh — issue §8 extension direction 2
#
# `DISABLED_STATE_KINDS` in `src/theme/preset_states.rs` is generated from the
# registry export (`docs/capability_schema.json`). A newly registered control
# must appear there, or disabling it changes only its input gate.
#
# Usage: tools/check_disabled_state_covers_kinds.sh
# Exit 0 = the generated list matches every exported canonical control.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

STATUS=0
OUTPUT="$("$PYTHON" tools/check_disabled_state_covers_kinds.py)" || STATUS=$?
printf '%s\n' "$OUTPUT"
[[ "$STATUS" -eq 0 ]] || exit "$STATUS"
echo "check_disabled_state_covers_kinds: OK"
