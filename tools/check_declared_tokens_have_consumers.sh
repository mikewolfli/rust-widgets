#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_declared_tokens_have_consumers.sh -- BLUE23 §5.5 (P1-11)
# ============================================================================
# The rule this guards (BLUE23 §5.5):
#
#   **Every colour role declared in `Colors` has at least one production consumer.**
#
# # Why this is a gate and not a test
#
# The crate has repeatedly added a colour role, written it into every preset, documented it
# -- and read it nowhere. At the point this gate landed, seven layering roles
# (`scrim`, `surface_container`, `surface_container_high`, `inverse_surface`,
# `on_inverse_surface`, `outline_variant`) and `outline` sat in exactly that state: the
# theme file said they existed, and no control changed when they did. That is the same
# defect class as BLUE21's "seven islands", one layer down: a *token* with no consumer.
#
# `check_mechanism_has_a_consumer` covers the source level (does anything call this
# function?). This covers the data level (does anything read this role?). Both are needed:
# a control can read a *mechanism* that never touches the palette, and it can touch the
# palette through a mechanism the other gate does not name.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan. For each field of `Colors`, its name must appear in
# a path expression (`.field`) somewhere under `src/` outside the theme module, or be
# listed in the scan's `ALLOWED` table with the reason it is reserved. The table is the
# ratchet: a new unused role fails immediately, so it can only shrink.
#
# # What this gate does NOT prove
#
#   * It is **lexical**: it proves the name is *read*, not that the read reaches a control's
#     pixels. The SVG snapshot gates (`check_svg_snapshots`, `check_control_rendering`)
#     cover the visible half.
#   * `ALLOWED` names structural roles (`background`, `foreground`, `secondary`) that are
#     read indirectly through role resolution rather than by field access; that indirection
#     is what makes a lexical read impossible for them.
#
# # Reverse injection
#
# Step 2 pretends a new role (`definitely_unconsumed_role`) was declared and requires the
# scan to report it, so a search that matched nothing cannot pass.
#
# Usage: tools/check_declared_tokens_have_consumers.sh
# Exit 0 = every declared role has a consumer (or an allowlist entry with a reason).
# Exit 1 = an unconsumed role (each offender is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

if ! "$PYTHON" tools/declared_tokens_scan.py; then
    echo ""
    echo "  A colour role that nothing reads is a promise the theme cannot keep: the file"
    echo "  says the role exists, and no control changes when it does. Consume it, or record"
    echo "  it as reserved in the scan's ALLOWED table."
    exit 1
fi

if "$PYTHON" tools/declared_tokens_scan.py --inject >/dev/null 2>&1; then
    echo "FAIL: injecting an unconsumed role did not change the result, so the check is not"
    echo "      comparing against the declared fields"
    exit 1
fi

echo "declared-token checks passed."
