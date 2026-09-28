#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_font_data_is_opt_in.sh — BLUE23 §0A.4 (附录 G, 约束 1)
# ============================================================================
# The rule this guards (BLUE23 §0A.4, constraint 1 — "三条不可让的约束" first):
#
#   **Font data ships in the capable device profiles (`desktop`/`tablet`/`mobile`) and
#   never in `default` or the sized profiles (`embedded`/`mini`).**
#
# "Perfect is optional" is the whole contract, and it is the *sized* profiles that express it: a
# `mini`/`embedded` build must never pay for a face it did not ask for. A `desktop`/`tablet`/`mobile`
# host, by contrast, has the bytes and a reason to want antialiased text, so the vector faces are
# part of the profile rather than a separate opt-in.
#
# # Why this is a gate
#
# Font data is invisible where it enters and expensive where it lands. Adding
# `"fonts-cjk-bitmap"` to `default` — or to a sized profile — is a one-word edit that
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
#   * Every capable profile (`desktop`, `tablet`, `mobile`) names a `fonts-*` feature, directly or
#     through one level of feature indirection.
#   * No `fonts-*` feature is a member of `default`, `embedded` or `mini`.
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
# Exit 0 = the capable profiles carry font data and the sized ones do not.
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
        echo '  A sized profile enables font data, a capable profile names none, or a payload'
        echo '  sits outside the gated directory. Font data belongs in desktop/tablet/mobile'
        echo '  (BLUE23 §0A.4 constraint 1, as revised): remove it from the sized profile, or'
        echo '  add it to the capable one.'
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
# Two injections, one per half of the contract: adding font data to `default` must fail, and
# removing the only font feature from `desktop` must fail. A parser that silently found nothing
# fails both.
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
# The other half: strip the vector faces out of `desktop` and the scan must notice that a capable
# profile now carries no font data. `desktop` lists the faces one per line, so deleting those lines
# is a faithful "someone removed the opt-in" edit.
sed '/^    "fonts-vector-latin",$/d; /^    "fonts-complex",$/d; /^    "fonts-cjk",$/d; /^    "fonts-emoji-color",$/d' \
    Cargo.toml > "$INJECT_DIR/Cargo.no-desktop-fonts.toml"
if diff -q Cargo.toml "$INJECT_DIR/Cargo.no-desktop-fonts.toml" >/dev/null; then
    echo "FAIL: could not construct the injection; no `fonts-*` line was removed from `desktop`"
    exit 1
fi
if "$PYTHON" tools/font_data_opt_in_scan.py --manifest="$INJECT_DIR/Cargo.no-desktop-fonts.toml" \
        | grep -q "findings=0"; then
    echo "FAIL: a capable profile with no font data did not fail the scan"
    exit 1
fi

echo "font-data opt-in checks passed."
