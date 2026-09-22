#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# A single-line label must not be positioned at `band.y + band.height / 2` (BLUE21 / P0-1).
#
# # Why this gate exists
#
# It is the other half of `check_text_origin_is_a_top_edge`: that gate forbids an `ascent`
# term in a text origin, this one forbids a *halved band height*. Both come from the same
# fact — the origin is the glyph box's TOP edge, not its baseline — and together they make
# both spellings of "I thought this centred the text" fail at the source rather than in a
# snapshot someone has to notice.
#
# The wrong shape was the largest single class of visual defect in this crate: 76 sites
# across 40-odd files, spanning buttons, checkboxes, radio buttons, combo boxes,
# date/time editors, status bars, banners, ratings, tool buttons, six dialog button rows,
# `mdi_area` titles, the properties panel, five data tables, the menu bar, the toolbar,
# tabs, keyboard key caps, tag input, popovers, toasts and the chart empty state. Every one
# passed every existing gate: it is valid Rust, it compiles, the ink stays inside the
# control, and the SVG is well-formed. `tools/audit_text_y.py` can see the *result* but
# cannot prove intent (it infers the band from neighbouring rectangles), which is why it is
# documented as an audit aid rather than a check.
#
# The reasoning, the allowlist and the reverse-injection record live in the Python checker's
# docstring, alongside its sibling's.
#
# Usage: tools/check_text_vertically_centred.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/check_text_vertically_centred.py; then
    echo "FAIL: a text origin is a halved band height"
    exit 1
fi
