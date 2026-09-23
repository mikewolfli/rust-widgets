#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_generated_font_table_integrity.sh — BLUE23 §0A.4 (附录 G, G-4b/G-4a)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, G-4b):
#
#   **A generated font table must satisfy the assumptions its reader makes.** The reader looks a
#   codepoint up with a binary search and then reads a fixed-width window; neither assumption is
#   checked by the compiler or the type system.
#
# # Why this is a gate
#
# The CJK table is 2361 codepoints read by `CODEPOINTS.binary_search(&cp)`, then
# `ROWS[i * 32 .. i * 32 + 32]`. That is two unchecked premises:
#
#   * **ascending and unique codepoints** — necessary for a binary search to find anything, and
#     a table that is merely *most* sorted passes every test that happens to use an early
#     codepoint;
#   * **exactly `32 * N` row bytes** — a short array reads blanks rather than panicking (the
#     reader is written not to panic inside a paint call), so a truncated table renders some
#     glyphs as empty boxes and *nothing* reports an error.
#
# Both are properties of generated data, which means a regeneration with a different range or a
# hand edit is exactly how they break.
#
# # What this gate proves
#
#   * `CODEPOINTS` is strictly ascending (so unique), and its declared length matches its
#     contents.
#   * `ROWS` has exactly `32 * CODEPOINTS.len()` bytes, and its declared length matches.
#   * No ASCII codepoint is in the table (the 8x8 face covers it, and including it would change
#     the default build's snapshots).
#   * The table is gated by `fonts-cjk-bitmap`, so no profile compiles 85 KB it did not ask for.
#
# # What this gate does NOT prove
#
#   * It does not verify the glyph *pixels*. Only `gen_cjk_bitmap.py --check` can, and that needs
#     the upstream download. `glyph_source`'s feature-gated test pins one glyph's exact bits as a
#     second, independent anchor.
#   * It does not re-run the generator, so it cannot tell the file was produced by it — the
#     licence gate checks that its header says so.
#
# # Reverse injection
#
# A perturbed copy with two codepoints swapped out of order must be reported, so a scan that
# silently accepted everything cannot pass.
#
# Usage: tools/check_generated_font_table_integrity.sh
# Exit 0 = the table's reader's assumptions hold.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/font_table_integrity_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  The generated CJK table no longer satisfies its reader's assumptions. Regenerate"
        echo "  it with \`python3 tools/gen_cjk_bitmap.py --license=ofl-1.1\` rather than editing"
        echo "  the arrays (BLUE23 §0A.4, G-4b)."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
"$PYTHON" - "$INJECT_DIR/unsorted.rs" <<'PY'
import re
import sys

source = open("src/render/text/cjk_bitmap_data.rs", encoding="utf-8").read()
match = re.search(r"CODEPOINTS: \[u32; (\d+)\] = \[(.*?)\];", source, re.S)
codepoints = re.findall(r"0x[0-9A-Fa-f]+", match.group(2))
# Swap two neighbouring entries so the array is no longer ascending, leaving its declared
# length and every other invariant intact — the failure a seed must produce to be a test of the
# ordering check specifically.
codepoints[10], codepoints[11] = codepoints[11], codepoints[10]
replacement = "CODEPOINTS: [u32; %s] = [%s];" % (match.group(1), ", ".join(codepoints))
open(sys.argv[1], "w", encoding="utf-8").write(
    source[: match.start()] + replacement + source[match.end() :]
)
PY
if "$PYTHON" tools/font_table_integrity_scan.py --inject="$INJECT_DIR/unsorted.rs" \
        | grep -q "failed=0"; then
    echo "FAIL: injecting an out-of-order table did not fail the scan"
    exit 1
fi

echo "generated-font-table checks passed."
