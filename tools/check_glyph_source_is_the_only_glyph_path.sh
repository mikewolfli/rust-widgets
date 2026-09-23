#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_glyph_source_is_the_only_glyph_path.sh — BLUE23 §0A.4 (附录 G, G-2b)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, G-2b "`GlyphSource` 抽象"):
#
#   **A glyph is reached through the text layer's stack, never by handing a character to a face
#   table.** Exactly one file may name a face.
#
# # Why this is a gate
#
# The crate used to answer "what does this character look like?" in one place — an accessor that
# returned `[u8; 8]` straight from the `font8x8` table. That return type is the defect, not the
# table: it hard-codes the cell size, so a CJK glyph (16x16) cannot be expressed and a second
# face cannot be added without editing both renderers. Naming the source as an abstraction moves
# the decision into data.
#
# What makes the abstraction decay is the same thing that made the old accessor easy: a
# renderer can always reach around it in one line. `BASIC_FONTS.get(ch)` compiles, needs no
# import of the text layer, and silently bypasses the stack — including any face a host added.
#
# # What this gate proves
#
# A lexical, whole-tree, no-build scan of `src/**`:
#
#   * no file outside `src/render/text/glyph_source.rs` mentions `BASIC_FONTS` or `font8x8::`
#     in code (comment lines are excluded — many doc comments describe the old output);
#   * the retired `fn glyph_bitmap` accessor does not exist anywhere;
#   * both renderers still read the shared `glyph_rects` geometry, so "one derivation" is a fact
#     about the tree rather than a claim in a comment.
#
# # What this gate does NOT prove
#
#   * It is lexical. A renderer could call the stack and then ignore the result. The
#     `render::text::glyph_source` tests and the snapshot gate cover the behavioural half.
#   * It does not check the *geometries* agree, only that both read the same function.
#
# # Reverse injection
#
# A seeded file containing `font8x8::BASIC_FONTS.get(c)` is scanned with `--inject` and must be
# reported, so a scan that matched nothing cannot pass.
#
# Usage: tools/check_glyph_source_is_the_only_glyph_path.sh
# Exit 0 = the stack is the only route to a glyph.
# Exit 1 = an offender is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/glyph_path_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A glyph is fetched without going through the font stack. Resolve it with"
        echo "  \`crate::render::text::resolve\` (or \`glyph_rects\`) instead, so a face added"
        echo "  by a feature is reachable from every renderer (BLUE23 §0A.4, G-2b)."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
cat > "$INJECT_DIR/seeded.rs" <<'RS'
fn seeded(ch: char) -> [u8; 8] {
    if let Some(rows) = font8x8::BASIC_FONTS.get(ch) {
        return rows;
    }
    [0; 8]
}
RS
if "$PYTHON" tools/glyph_path_scan.py --inject="$INJECT_DIR/seeded.rs" | grep -q "failed=0"; then
    echo "FAIL: injecting a direct face access did not fail the scan"
    exit 1
fi

echo "glyph-path checks passed."
