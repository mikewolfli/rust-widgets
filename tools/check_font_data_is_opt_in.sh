#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_font_data_is_opt_in.sh — BLUE23 §0A.4 (附录 G, 约束 1)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, constraint 1 — "三条不可让的约束" first):
#
#   **The default build carries no font data.** Coverage beyond Latin/ASCII is an opt-in
#   feature, never something a `mini`/`embedded` profile pays for by accident. "Perfect is
#   optional" is the whole contract.
#
# # Why this is a gate
#
# Font data is invisible where it enters and expensive where it lands. Adding
# `"fonts-cjk-bitmap"` to the `desktop` list — or to `default` — is a one-word edit that
# compiles, passes every test, and silently grows every binary by ~85 KB and every `--all-features`
# snapshot's font coverage. Nothing else in the tree would notice: the tests that assert the
# Latin boundary are written to pass with the data on *and* off, precisely so they cannot be used
# as the alarm.
#
# So the contract is checked where it is stated: in the feature graph, which is the thing a
# consumer reads to decide what they are building.
#
# # What this gate proves
#
#   * No `fonts-*` feature is a member of `default`, `desktop`, `tablet`, `mobile`, `embedded`
#     or `mini`, directly or through one level of feature indirection.
#   * Every `fonts-*` feature is declared in `[features]` (so a gate can name it), and at least
#     one exists (so the scan is not vacuous).
#   * Every `include_bytes!` of a font payload lives under `src/render/text/font_assets/`, the
#     one directory whose submodules are each feature-gated.
#
# # What this gate does NOT prove
#
#   * It does not weigh the payloads; a 4 MB font in a gated module passes. Size is a review
#     question, reported in the round's log with measured numbers.
#   * It does not check the *reverse* direction — that enabling a `fonts-*` feature actually
#     changes what is drawn. `render::text::glyph_source`'s feature-gated tests do that.
#   * `full` and `--all-features` are excluded on purpose: they mean "every capability", so they
#     are expected to carry data.
#
# # Reverse injection
#
# A copy of `Cargo.toml` with `"fonts-cjk-bitmap"` added to `default` must make the scan report a
# finding, so a parser that silently found nothing cannot pass.
#
# Usage: tools/check_font_data_is_opt_in.sh
# Exit 0 = no profile carries font data.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/font_data_opt_in_scan.py)"
echo "$OUT"
case "$OUT" in
    *"findings=0"*) ;;
    *)
        echo ""
        echo "  A profile enables font data, or a payload sits outside the gated directory."
        echo "  Font data is opt-in (BLUE23 §0A.4 constraint 1): remove it from the profile and"
        echo "  let the caller ask for it by name."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
sed 's/^default = \[/default = ["fonts-cjk-bitmap", /' Cargo.toml > "$INJECT_DIR/Cargo.toml"
if grep -q '^default = \["fonts-cjk-bitmap"' "$INJECT_DIR/Cargo.toml"; then
    if "$PYTHON" tools/font_data_opt_in_scan.py --manifest="$INJECT_DIR/Cargo.toml" \
            | grep -q "findings=0"; then
        echo "FAIL: a default feature list that enables font data did not fail the scan"
        exit 1
    fi
else
    echo "FAIL: could not construct the injection; the `default = [` line moved"
    exit 1
fi

echo "font-data opt-in checks passed."
