#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_icons_not_symbol_glyphs.sh — the "an icon is an icon" gate
# ============================================================================
# The rule this guards:
#
#   **A control that wants a tick, a chevron, an arrow or a dot draws an
#   `IconName` outline — never a Unicode symbol character handed to `draw_text`.**
#
# # Why this is a gate, and why twice is enough
#
# The crate has shipped this defect **twice**, both times the same way. A control used a symbol
# character (`★ ▼ ▲ ◀ ▶ ✓ ← ‹ › − •`) where it meant an icon:
#
#   * **No bundled outline face covers those blocks** (Geometric Shapes, Arrows, Dingbats,
#     Miscellaneous Symbols). The glyph therefore falls through the outline path to the 8x8
#     **bitmap** face, and the control paints a coarse block where an outline belongs. The SVG
#     snapshot then showed a bitmap shape the pixels did not have — a fidelity gap in the one
#     artifact that exists to be a faithful picture.
#   * **A character carries no geometry contract.** Nothing asserts on its shape, size or
#     centring, so the defect is invisible to every test that does not know the codepoint; it
#     was found by rendering and looking.
#
# The first sweep fixed 16 sites across 15 controls. The second found five more
# (`code_editor`, `find_replace_dialog`, `grid_table`, `kanban_board`, `app_bar`,
# `cupertino/nav_bar`, `action`, `menu_button`, `otp_input`) — because the first sweep scanned for
# the characters it already knew about instead of for the *class*. A gate closes the class.
#
# What this checks
# ----------------
# A lexical, whole-tree, no-build scan of `src/**` (`tools/symbol_glyph_scan.py`):
#
#   * every `draw_text` / `draw_text_fitted` call is inspected for a string literal holding a
#     character in a symbol/pictograph block;
#   * the typographic marks a text run may legitimately carry (an ellipsis the fitter appends, a
#     dash in a message) are listed in the scan with a reason, so the exemption is deliberate and
#     reviewable rather than a hole.
#
# Test modules and `*/tests.rs` are skipped: their literals describe a *document*, not a control's
# chrome, and the rule is about what ships.
#
# What this gate does NOT prove
# -----------------------------
#   * It is lexical. A site that builds a symbol string at runtime (a `char` from a variable)
#     is not seen. The snapshot gate and review are the second line.
#   * It says nothing about whether an `IconName` is the *right* icon, only that an icon is used.
#
# Reverse injection
# -----------------
# A seeded file drawing `\u{2713}` through `draw_text` is scanned with `--inject` and must be
# reported, so a scan that matched nothing cannot pass.
#
# Usage: tools/check_icons_not_symbol_glyphs.sh
# Exit 0 = no control draws a symbol character as an icon.
# Exit 1 = an offender is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/symbol_glyph_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A control draws a Unicode symbol character as an icon. Replace it with an"
        echo "  \`IconName\` outline via \`crate::widget::draw_icon_at\` / \`draw_icon_centered\`, so"
        echo "  the shape is geometry both backends agree on. If the character is a genuine"
        echo "  typographic mark in a text run (not an icon), add it to \`ALLOWED_TEXT_MARKS\` in"
        echo "  tools/symbol_glyph_scan.py with a reason."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
cat > "$INJECT_DIR/seeded.rs" <<'RS'
fn seeded(context: &mut RenderContext, baseline: i32) {
    context.draw_text(
        Point::new(0, baseline),
        "\u{2713}",
        &Font::default(),
        Color::BLACK,
        HorizontalAlignment::Left,
    );
}
RS
if "$PYTHON" tools/symbol_glyph_scan.py --inject="$INJECT_DIR/seeded.rs" | grep -q "failed=0"; then
    echo "FAIL: injecting a symbol glyph drawn as text did not fail the scan"
    exit 1
fi

echo "icon-not-symbol-glyph checks passed."
