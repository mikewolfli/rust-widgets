#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# A text origin is the glyph box's TOP edge, never a baseline (principle #18, BLUE20 rules
# #102/#103).
#
# # Why this gate exists
#
# `draw_text`'s `origin` is the box's top-left: the SVG backend pairs the value with
# `dominant-baseline="text-before-edge"` and the rasteriser blits downward from it. So an
# `ascent` term added to that origin is always a placement error. Round 61 caught the eight
# instances that overflowed their *control* (P5's bound is the control rectangle); the ones
# that stayed inside it were invisible to every gate, and round 62 found ~40 more across
# 20-odd files — mis-centred labels in `app_bar`, `video_player`, `number_picker`,
# `search_bar`, `bottom_navigation_bar`, `mobile_date_picker` and others.
#
# The check is `tools/check_text_origin_is_a_top_edge.py`; its docstring records the
# reasoning, the allowlist rationale, and the reverse-injection this gate was verified with
# (an early version looked only at the call's own arguments and stayed green against a
# deliberately injected defect — the variable-definition resolution in `origin_sources` is
# the fix for that false green).
#
# Usage: tools/check_text_origin_is_a_top_edge.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_text_origin_is_a_top_edge.py; then
    echo "FAIL: a text origin carries an \`ascent\` term"
    exit 1
fi
