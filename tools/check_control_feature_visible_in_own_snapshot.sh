#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# A control that exists to demonstrate a feature must show that feature in its own snapshot
# (BLUE21 E1 / P3-1h / A.3.9 / #107).
#
# # Why this gate exists
#
# Every other snapshot assertion asks something a machine can answer: did the control paint
# anything, did its chrome follow the theme, is the SVG well formed. None of them ask the question
# a person asks in one second — "this control is called `floating_label`; where is the label?".
#
# A control can be complete, theme-aware, non-empty and well-formed while failing to demonstrate
# the one thing it exists for, and that shipped three times:
#
#   * `floating_label` — a control whose entire purpose is Material's floating caption — produced
#     a snapshot with **no label in it at all**. Its `label` property was not even published, and
#     the shared label helper resolved "the label" to `text`, so the caption was written into the
#     input and `draw_label` returned early;
#   * `tab_widget` shipped with **zero tabs**, so its tab band — the chrome that *is* a tab widget
#     — was absent from its own picture;
#   * `badge`'s pill resolved to the window's own colour, so the pill was drawn and invisible.
#
# # What it asserts
#
# Each listed control's dark snapshot must contain its feature's markers and must not contain the
# spelling the defect produces. Both halves are needed: "a `<text>` exists" does not distinguish
# `floating_label`'s caption from its input text, while the forbidden pattern is the exact shape
# the bug had in the file.
#
# Every marker was obtained by inspecting the committed file and then reproducing the defect
# (revert the feature, re-export, diff), so each one is present now and verifiably absent when its
# feature regresses. The Python checker's docstring carries the per-marker reasoning.
#
# # Reverse injection
#
# Step 2 pretends one control's feature has regressed and requires a finding, so a comparison that
# never ran cannot pass.
#
# Usage: tools/check_control_feature_visible_in_own_snapshot.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_control_feature_visible_in_own_snapshot.py; then
    echo "FAIL: a control does not show its own feature in its snapshot"
    exit 1
fi

# A non-zero exit is the expected outcome here, and it is the assertion rather than an error.
if "$PYTHON" tools/check_control_feature_visible_in_own_snapshot.py \
    --inject=floating_label >/dev/null 2>&1; then
    echo "FAIL: pretending a feature regressed did not make the check fail, so it is not checking"
    exit 1
fi

echo "control-feature snapshot checks passed."
