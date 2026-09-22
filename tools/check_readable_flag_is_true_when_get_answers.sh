#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# A schema row flagged `readable: false` must not be answered by its control's `get`
# (BLUE21 E1 / P3-1d / C3).
#
# # Why this gate exists
#
# `PropertySchema::readable` is not advisory: `WidgetFactory::read_property` returns
# `UnsupportedOnWidget` when the row says `false`, without ever asking the control. The write
# path checks the *other* flag, so a row declared `readable: false, writable: true` is settable
# and unreadable — the caller writes a value the control stores and the reflection layer refuses
# to hand back. "Wrote it, cannot read it" is exactly what the schema exists to prevent, and it
# is invisible everywhere else: the Rust compiles, the control is right, and only a boolean in a
# table is wrong.
#
# `candlestick_chart.overlay_count` was in that state (`CandlestickChart::get` returns
# `self.overlays.len()` while the row said `false, true`). It is fixed, and this gate is the
# thing that keeps it fixed. Its sibling is the runtime test
# `a_readable_false_row_must_not_be_answered_by_its_control` in `properties_tests.rs`, which asks
# every control directly; this one answers the same question at the source so it can name the row
# to edit and needs no build.
#
# # Reverse injection
#
# Step 2 runs the same check with one row pretended violating and requires a failure, so a run
# that compared nothing cannot pass.
#
# Usage: tools/check_readable_flag_is_true_when_get_answers.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_readable_flag_is_true_when_get_answers.py; then
    echo "FAIL: a property is flagged unreadable while its control's get answers it"
    exit 1
fi

# A non-zero exit is the expected outcome here, and it is the assertion rather than an error.
# The name is the real defect this gate was written for, not a synthetic one.
if "$PYTHON" tools/check_readable_flag_is_true_when_get_answers.py \
    --inject=candlestick_chart.overlay_count >/dev/null 2>&1; then
    echo "FAIL: injecting a known violation did not make the check fail, so it is not checking"
    exit 1
fi

echo "readable-flag checks passed."
