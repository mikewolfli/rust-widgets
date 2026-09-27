#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_single_open_plan.sh — BLUE24 §9.1 criterion 2
# ============================================================================
# The rule this guards:
#
#   **One subject, one open plan.** A theme may have at most one un-archived plan
#   that still contains `[ ]` outstanding items. Several *different* plans may be
#   open at once (BLUE23 and BLUE24 are running in parallel and that is allowed).
#
# The defect this stops
# ---------------------
# `docs/plans/` once held 47 files, 40 of them `blue*`. With no rule, the same
# requirement could be written half in one plan and half in another, and BLUE22
# appendix G and BLUE23 §0A.4 did exactly that until round 73 merged them. A half
# requirement in each of two places is worse than a whole one in either: each reads
# as covered.
#
# Note the *deliberate* asymmetry
# -------------------------------
# This is **not** "the whole repository may have one open plan". That would fail
# immediately while BLUE23 is still running, so nobody would wire it into CI — which
# is the failure mode BLUE22 §F.3 lesson 4 records ("a gate's shape decides whether
# it ever runs"). The rule is per **subject**: the subject is read from the plan's
# own title, so two plans on two subjects coexist and two plans on one subject do
# not.
#
# The rule, stated structurally
# -----------------------------
#   (1) every `docs/plans/blue*.md` has a machine-readable subject: its `# ` title's
#       first token after `BLUE<number> —` (or the whole title when it does not use
#       that shape);
#   (2) no two un-archived plans share a subject while **both** still contain `[ ]`;
#       a plan with no `[ ]` items is finished for this purpose and does not compete.
#
# Usage: tools/check_single_open_plan.sh
# Exit 0 = every subject has at most one plan with outstanding items.
# Exit 1 = a finding (each subject with a collision is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PLANS_DIR="docs/plans"
README="$PLANS_DIR/README.md"

echo "[1/3] the index exists"
if [ ! -f "$README" ]; then
    echo "  FAIL  $README is missing"
    echo "        The index is what makes 'one entry point' a fact rather than a habit."
    exit 1
fi
echo "  PASS  $README is present"

echo "[2/3] every live plan declares a readable subject"
# The subject is the title's text after the em-dash. Plans that do not use the
# `BLUE<n> — <subject>` shape take their whole title, which is still a subject and
# still de-duplicates.
declare -a SUBJECTS=() FILES=()
for plan in "$PLANS_DIR"/*.md; do
    [ -f "$plan" ] || continue
    case "$(basename "$plan")" in
        README.md|principle.md|codemap.md|TODO.md|FUTURE.md) continue ;;
    esac
    title="$(grep -m1 '^# ' "$plan" || true)"
    if [ -z "$title" ]; then
        echo "  FAIL  $plan has no \`# \` title, so its subject cannot be read"
        exit 1
    fi
    subject="${title#\# }"
    # Prefer the text after the em-dash; fall back to the whole title.
    case "$subject" in
        *—*) subject="${subject##*—}" ;;
    esac
    subject="$(printf '%s' "$subject" | sed -e 's/^ *//' -e 's/ *$//')"
    SUBJECTS+=("$subject")
    FILES+=("$(basename "$plan")")
done
if [ "${#SUBJECTS[@]}" -eq 0 ]; then
    echo "  FAIL  no plans found under $PLANS_DIR"
    exit 1
fi
echo "  PASS  ${#SUBJECTS[@]} live plan(s) each declare a subject"

echo "[3/3] no subject is open in two plans at once"
# A plan is "open" for this purpose when it still carries an unchecked item. A plan
# with none is finished, so it does not compete with a successor on the same subject.
collisions=""
for i in "${!SUBJECTS[@]}"; do
    for j in "${!SUBJECTS[@]}"; do
        [ "$i" -lt "$j" ] || continue
        [ "${SUBJECTS[$i]}" = "${SUBJECTS[$j]}" ] || continue
        a="$PLANS_DIR/${FILES[$i]}"
        b="$PLANS_DIR/${FILES[$j]}"
        a_open="$(grep -cE '^[[:space:]]*-[[:space:]]*\[ \]' "$a" || true)"
        b_open="$(grep -cE '^[[:space:]]*-[[:space:]]*\[ \]' "$b" || true)"
        if [ "$a_open" -gt 0 ] && [ "$b_open" -gt 0 ]; then
            collisions="${collisions}${FILES[$i]} and ${FILES[$j]} both open on the same subject: ${SUBJECTS[$i]}
"
        fi
    done
done

if [ -n "$collisions" ]; then
    echo "  FAIL  the same subject is open in two plans:"
    printf '%s' "$collisions" | sed 's/^/          /'
    echo "        Merge them, or archive the finished one: a requirement written half"
    echo "        in each place reads as covered in both (BLUE24 §9.1)."
    exit 1
fi
echo "  PASS  every subject has at most one open plan"

echo
echo "check_single_open_plan: OK"
