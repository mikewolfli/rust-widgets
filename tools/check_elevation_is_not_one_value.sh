#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_elevation_is_not_one_value.sh — BLUE24 §10A.6 criterion 4
# ============================================================================
# The rule this guards:
#
#   **A face's height above the page must be a value a theme can state, not a
#    literal baked into the code that styles every control.**
#
# The defect this stops
# ---------------------
# `theme::manager::role_base_style` used to build one shadow literal —
#
#     Some(Shadow { x: 0, y: 2, blur: 6, color: Color::rgba(0, 0, 0, 60) })
#
# — and hand it to EVERY control, from the flattest list row to the highest
# floating toast. Two consequences, both visible on screen and neither
# detectable by any declaration-based gate:
#
#   * a raised layer and a flush one are indistinguishable, so "elevation" is
#     not something the library can express at all;
#   * the same shadow on a 16-px chip and a full-window dialog reads as two
#     different materials, neither of them intended.
#
# The rule is therefore not "no shadows" — it is "no SINGLE shadow for
# everything". The value must come from `Theme::elevation(n)`, which is a
# per-level lookup a theme can restate, so a control asks for a *level* and the
# theme decides what that level looks like.
#
# What it accepts and what it rejects
# -----------------------------------
# An `X { … }` struct literal for a shadow is rejected **only inside a function
# whose job is to style every control** (`role_base_style` and its siblings). A
# literal inside a per-level table (`default_shadow` in `render::surface`, a
# theme's own `elevation` override) is the *correct* place for one, so the gate
# scopes itself to the resolution path rather than banning the syntax.
#
# Exit 0 = the resolution path reads elevation from the theme.
# Exit 1 = a finding (the resolution path still bakes one shadow for everything).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MANAGER="src/theme/manager.rs"

echo "[1/3] the resolution path exists and is addressable"
if [ ! -f "$MANAGER" ]; then
    echo "  FAIL  $MANAGER is missing; the gate cannot locate the resolution path"
    exit 1
fi

if ! grep -q 'fn role_base_style' "$MANAGER"; then
    echo "  FAIL  role_base_style is gone from $MANAGER; point this gate at its replacement"
    exit 1
fi

echo "[2/3] the resolution path does not bake one shadow for every control"
# Slice out just `role_base_style`'s body: from its signature to the closing
# brace of the function, found by the first line that is exactly `    }` at the
# function's own indent. `awk`'s range form is the portable way to do this.
#
# The signature is matched at the START OF A LINE, not anywhere in it. A bare
# `/fn role_base_style/` also matches the *doc comments* of neighbouring
# functions which mention the name, and starting the slice there swallows the
# text between them — including the comment below that quotes the old literal
# to explain why it changed. That made this gate report the presence of its own
# explanation as the defect it was written to prevent (measured: the extractor
# began 30 lines above the real `fn` and ran past its closing brace, so [2/3]
# failed with "6: // This used to build one `Shadow { x: 0, y: 2, blur: 6, .. }`").
# Anchoring on `^    fn ` fixes the slice, which is what the gate actually means.
BODY="$(awk '/^    fn role_base_style/{inside=1} inside{print} inside && /^    }$/ && NR>1 {if (seen) exit; seen=1}' "$MANAGER")"

# `awk` exits at the first `    }` it prints *after* entering the slice, so the
# body ends at the function's own closing brace. Guard against the failure mode
# above returning: if the slice did not start at the attribute line, the first
# line is not a `fn` signature and the extraction is untrustworthy.
if [ -z "$BODY" ] || ! printf '%s\n' "$BODY" | head -n 1 | grep -qE '^    fn role_base_style'; then
    echo "  FAIL  could not extract role_base_style's body (slice did not start at its signature)"
    exit 1
fi

# The exact shape of the defect: a `Shadow { … }` literal *in live code* on the
# resolution path. Comments are stripped first, because the function's own
# documentation quotes the offending literal to explain what changed — a gate
# that reads its own explanation as a finding can never be satisfied.
CODE="$(printf '%s\n' "$BODY" | sed -e 's://.*::' -e '/^[[:space:]]*\/\//d')"
if printf '%s\n' "$CODE" | grep -qE '\bShadow\s*\{'; then
    echo "  FAIL  role_base_style builds a Shadow literal instead of reading a level:"
    printf '%s\n' "$CODE" | grep -nE '\bShadow\s*\{' | sed 's/^/          /'
    echo "        Elevation must come from Theme::elevation(n) so a theme can restate"
    echo "        it and a control can ask for a level (BLUE24 §10A.6 criterion 4)."
    exit 1
fi

echo "  PASS  role_base_style reads elevation rather than baking a literal"

echo "[3/3] the elevation channel actually exists"
# A gate that passes because the literal was deleted and nothing replaced it would
# be reporting the absence of a feature as its presence. Both halves must exist:
# the theme-side lookup, and the value type it returns.
FOUND_THEME_LOOKUP=0
if grep -rqE 'fn elevation\s*\(' src/theme/ ; then
    FOUND_THEME_LOOKUP=1
fi
FOUND_TYPE=0
if grep -qE 'pub fn (elevation|shadow)\b' src/render/surface.rs; then
    FOUND_TYPE=1
fi

if [ "$FOUND_THEME_LOOKUP" -eq 0 ] || [ "$FOUND_TYPE" -eq 0 ]; then
    echo "  FAIL  the elevation channel is incomplete:"
    [ "$FOUND_THEME_LOOKUP" -eq 0 ] && echo "          Theme::elevation(n) is not defined in src/theme/"
    [ "$FOUND_TYPE" -eq 0 ] && echo "          render::SurfaceStyle::shadow is not defined"
    echo "        Removing the literal without adding the lookup is not a fix."
    exit 1
fi

echo
echo "check_elevation_is_not_one_value: OK"
