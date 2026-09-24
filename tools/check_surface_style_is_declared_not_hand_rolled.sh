#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_surface_style_is_declared_not_hand_rolled.sh — BLUE24 §10A.6 criterion 3
# ============================================================================
# The rule this guards:
#
#   **A face's two tones and its direction are one relationship, declared in one
#    place — not re-derived, slightly differently, by every control that draws a
#    edge.**
#
# Why this needs a gate and not a convention
# ------------------------------------------
# Before this round, 43 files under `src/widget/` wrote the same pair of
# expressions themselves:
#
#     border.blend(&Color::rgb(255, 255, 255), 0.5)   // "light"
#     border.blend(&Color::BLACK, 0.5)                // "dark"
#
# Those two lines are *correct*. What is wrong is that they are one relationship
# stated twice, and that the DIRECTION (does the light fall on the top edges, or
# is this a cut-in well?) is carried by which block got which colour — i.e. by
# the order of two nearly identical code blocks. Reversing an inset was therefore
# a copy-paste edit that nothing could check, and `render::bevel`'s module docs
# record that it silently happened.
#
# The count, and why it is a one-way ratchet
# ------------------------------------------
# `frame.rs` has already been migrated to the `Bevel` primitive, so the measured
# baseline is 42, not the 43 the plan recorded at writing time. This gate does not
# demand the number reach zero in one step — that would be a rewrite, not a fix,
# and it would have to be done blind. It demands the number **never rise**, and
# that every file still on the list is named in an allowlist WITH A REASON. A new
# hand-rolled bevel therefore fails the gate at the moment it is written, which is
# the only moment it is cheap to fix.
#
# Why the allowlist carries a reason, not just a path
# --------------------------------------------------
# Rule #108: an exemption with no reason is indistinguishable from a defect
# someone silenced. `tools/surface_style_allowlist.txt` requires a reason per line,
# and this gate rejects a line that has only a path.
#
# Exit 0 = the count did not rise and every remaining file is justified.
# Exit 1 = a finding.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# The measured ceiling. Not "0" because migrating 42 controls is a separate,
# later job; the number going UP is the defect this gate is for.
CEILING=42

ALLOWLIST="tools/surface_style_allowlist.txt"

# The pattern the whole module is arranged to eliminate. Kept identical to the one
# in `render::bevel`'s docs and in the plan, so the gate and the prose cannot drift.
#
# The parentheses are escaped because this is an ERE: an unescaped `(` opens a group,
# and `blend(&Color::WHITE` is then an unbalanced pattern that grep rejects — which
# made the first version of this gate report `PASS 0 files` while 42 matched. A gate
# that passes on a broken pattern is worse than no gate, so the fix is verified by the
# reverse injection recorded in `tools/gates_reverse_injection.md`.
PATTERN='blend\(&Color::WHITE|blend\(&Color::rgb\(255, 255, 255\)|blend\(&Color::BLACK'

echo "[1/3] measuring hand-derived bevels under src/widget/"
if ! [ -d src/widget ]; then
    echo "  FAIL  src/widget does not exist; the gate is measuring the wrong tree"
    exit 1
fi

HITS="$(grep -rlE "$PATTERN" src/widget/ --include='*.rs' | sort || true)"
COUNT="$(printf '%s\n' "$HITS" | grep -c . || true)"

# A pattern that matches nothing at all would make every assertion below vacuously
# pass — the "measurement tool is broken" failure this repository has hit before
# (round 46: an audit tool reported 66 files reading no theme, the truth was 14).
# `bevel.rs`'s own tests and the four Pattern-A files guarantee a non-zero floor.
if [ "$COUNT" -eq 0 ]; then
    echo "  FAIL  the pattern matched nothing; the gate is measuring with a broken pattern"
    echo "        PATTERN=$PATTERN"
    exit 1
fi

if [ "$COUNT" -gt "$CEILING" ]; then
    echo "  FAIL  $COUNT files hand-derive a bevel; the ceiling is $CEILING."
    echo "        A new one was written. Use render::bevel's Bevel /"
    echo "        render::surface's BevelSpec instead of blending two tones by hand."
    echo "        Offending files not already on the list:"
    if [ -f "$ALLOWLIST" ]; then
        printf '%s\n' "$HITS" | while read -r f; do
            [ -z "$f" ] && continue
            grep -qF "$f" "$ALLOWLIST" || echo "          $f"
        done
    else
        printf '%s\n' "$HITS" | sed 's/^/          /'
    fi
    exit 1
fi
echo "  PASS  $COUNT hand-derived bevel files (ceiling $CEILING)"

echo "[2/3] every remaining file is on the allowlist"
if [ ! -f "$ALLOWLIST" ]; then
    echo "  FAIL  $ALLOWLIST is missing; it must list each file with a reason"
    exit 1
fi

# Each non-comment line must be "path — reason" (two whitespace-separated fields
# at least); a path alone is a silenced defect, not an exemption.
BAD="$(awk '!/^#/ && NF>0 && NF < 2 {print NR": "$0}' "$ALLOWLIST" || true)"
if [ -n "$BAD" ]; then
    echo "  FAIL  these allowlist entries lack a reason (need: path  reason):"
    printf '%s\n' "$BAD"
    exit 1
fi

UNLISTED="$(printf '%s\n' "$HITS" | while read -r f; do
    [ -z "$f" ] && continue
    grep -qF "$f" "$ALLOWLIST" || echo "$f"
done)"
if [ -n "$UNLISTED" ]; then
    echo "  FAIL  these files hand-derive a bevel but are not allowlisted:"
    printf '%s\n' "$UNLISTED" | sed 's/^/          /'
    exit 1
fi
echo "  PASS  every remaining file is allowlisted with a reason"

echo "[3/3] no stale allowlist entries (a migrated file must leave the list)"
STALE="$(awk '!/^#/ && NF>0 {print $1}' "$ALLOWLIST" | while read -r f; do
    [ -z "$f" ] && continue
    # A file that no longer exists, or no longer matches, is a stale exemption —
    # it excuses nothing and hides the next real occurrence behind green.
    if [ ! -f "$f" ]; then
        echo "$f (no such file)"
    elif ! grep -qE "$PATTERN" "$f"; then
        echo "$f (migrated; remove from the list)"
    fi
done)"
if [ -n "$STALE" ]; then
    echo "  FAIL  these allowlist entries are stale:"
    printf '%s\n' "$STALE"
    exit 1
fi
echo "  PASS  no stale allowlist entries"

echo
echo "check_surface_style_is_declared_not_hand_rolled: OK ($COUNT allowlisted files)"
