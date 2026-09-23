#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_text_model_is_single_sourced.sh — BLUE23 §0A.4 (附录 G, G-3/G-2b)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, and principle #51):
#
#   **Every decision about text has exactly one definition.** How a string clusters, what one
#   cluster advances, which scalars occupy a full em, and where glyph geometry comes from.
#
# # Why this is a gate
#
# Each of the four decisions is a two-to-six line expression, which is what makes it easy to
# copy — and the copies are what make the crate inconsistent in ways no test sees. The round
# this gate landed found and removed three live copies of two of them:
#
#   * the widget-side advance model spelled the width factors a second time, and its
#     wide-scalar table had *different ranges* from the renderer's — so a control reserved a
#     different width than the renderer drew;
#   * the SVG backend and the software rasteriser each had their own clustering loop, which is
#     why adding bidirectional reordering to one of them would have silently made the two
#     backends disagree;
#   * the SVG backend's copy did not merge a zero-width-joiner continuation the rasteriser's
#     did, so a family emoji measured and painted as a different number of clusters in the two.
#
# A gate is the right instrument because the failure mode is *silence*: a duplicate that drifts
# by one range produces a label a few pixels wide of its box, which no assertion catches and no
# snapshot review notices.
#
# # What this gate proves
#
# A lexical, whole-tree scan of `src/**`:
#
#   * `fn is_wide_scalar`, `fn estimate_cluster_advance` and `fn for_each_cluster` are each
#     defined exactly once — neither twice nor not at all;
#   * no file outside `src/render/text/` constructs a `TextCluster`, so no renderer re-implements
#     the clustering loop;
#   * both renderers call the text layer's `shape_line`, so the *use* of the single definition is
#     asserted and not just its existence (one definition nobody calls is a different defect).
#
# # What this gate does NOT prove
#
#   * It cannot prove the definitions are *equivalent* to what they replaced — only that there is
#     one of each. The round's tests (`render::text::line`, `widget::metrics`) pin the values.
#   * It does not see a copy that was renamed, or one inlined into its caller.
#
# # Reverse injection
#
# A seeded file defining `fn is_wide_scalar` must be reported, so a counter that found nothing
# cannot pass.
#
# Usage: tools/check_text_model_is_single_sourced.sh
# Exit 0 = one definition per decision.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/text_model_single_source_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  A text decision has more than one definition, or a renderer re-implements one."
        echo "  Call the text layer instead (BLUE23 §0A.4, principle #51)."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
cat > "$INJECT_DIR/seeded.rs" <<'RS'
fn is_wide_scalar(scalar: char) -> bool {
    matches!(scalar as u32, 0x4E00..=0x9FFF)
}
RS
if "$PYTHON" tools/text_model_single_source_scan.py --inject="$INJECT_DIR/seeded.rs" \
        | grep -q "failed=0"; then
    echo "FAIL: injecting a second wide-scalar table did not fail the scan"
    exit 1
fi

echo "text-model single-source checks passed."
