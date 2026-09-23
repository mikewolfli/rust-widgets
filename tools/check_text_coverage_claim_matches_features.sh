#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_text_coverage_claim_matches_features.sh — BLUE23 §0A.2 / §0A.4 (附录 G, 判据 15)
# ============================================================================
# The rule this guards (BLUE23 §0A.2, and §0A.4's judgement 15):
#
#   **What the crate says it draws must match what it draws.** No overstatement, and no
#   understatement either.
#
# # Why this is a gate
#
# §0A.2 is the section that exists because of a *promise larger than the capability*: the crate
# was asked for multilingual text, shipped a Latin-only face, and said nothing. A declaration is
# the fix — and a declaration is exactly the kind of thing that rots, in both directions:
#
#   * add font data and "default build draws Latin/ASCII only" becomes an **understatement**: the
#     reader is told they cannot have something they now can, and is never told the switch's
#     name;
#   * write "supports Unicode" without data and it is an **overstatement**, which is the original
#     defect wearing a different sentence.
#
# Neither shows up in a build. So the claim is checked against the feature graph, which is the
# only place the truth actually lives.
#
# # What this gate proves
#
#   * `src/lib.rs` and `README.md` each name the default coverage (Latin, ASCII) and what a wider
#     script would need (font data).
#   * If any `fonts-*` feature is declared, both files name a `fonts-` feature — the pointer to
#     the opt-in path. This half is the one that would otherwise rot silently, because adding a
#     face changes nothing about the default build.
#
# # What this gate does NOT prove
#
#   * It matches vocabulary, not meaning. A file could mention "Latin" in an unrelated sentence.
#     The prose is reviewed by a person; this keeps the *fact* from disappearing.
#   * It does not check the claim is exhaustive — a crate that adds Cyrillic data and keeps saying
#     "Latin/ASCII" passes. The opt-in pointer is what makes that discoverable.
#
# # Reverse injection
#
# A copy of `src/lib.rs` with the coverage vocabulary stripped must be reported, so a plain
# `grep -q Latin` cannot pass by matching something else in the file.
#
# Usage: tools/check_text_coverage_claim_matches_features.sh
# Exit 0 = the declaration matches the feature graph.
# Exit 1 = a finding is printed above.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_python.sh"

OUT="$("$PYTHON" tools/text_coverage_claim_scan.py)"
echo "$OUT"
case "$OUT" in
    *"failed=0"*) ;;
    *)
        echo ""
        echo "  The declared text coverage does not match the feature graph. Update the"
        echo "  \"Text coverage\" section in src/lib.rs and README.md (BLUE23 §0A.2)."
        exit 1
        ;;
esac

# ── Reverse injection ───────────────────────────────────────────────────────────────────────────
INJECT_DIR="$(mktemp -d)"
trap 'rm -rf "$INJECT_DIR"' EXIT
# Strip the three tokens the claim is made of, leaving the rest of the file — the state a
# silently-deleted paragraph produces.
sed -e 's/Latin//g' -e 's/ASCII//g' -e 's/font data//g' src/lib.rs > "$INJECT_DIR/claim_stripped.rs"
if "$PYTHON" tools/text_coverage_claim_scan.py --file="$INJECT_DIR/claim_stripped.rs" \
        | grep -q "failed=0"; then
    echo "FAIL: stripping the coverage vocabulary did not fail the scan"
    exit 1
fi

echo "text-coverage-claim checks passed."
