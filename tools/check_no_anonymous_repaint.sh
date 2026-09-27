#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_no_anonymous_repaint.sh — BLUE24 §8 criterion 5
# ============================================================================
# The rule this guards:
#
#   **Every repaint the library submits names a reason.** A frame's ledger has to be
#   able to say *why* each surface was redrawn, so no submission may go out without
#   a `RepaintReason`.
#
# The defect this stops
# ---------------------
# "This frame submitted seven repaints" is not actionable. The whole point of the
# frame ledger (BLUE24 §8) is to answer by **cause** — seven because an animation is
# in flight, seven because styles changed, seven because a caller repaints by hand. A
# bare `invalidate_surface` call produces a submission no mechanism can explain, so
# the ledger would be a count with no account, which is exactly what §8 exists to
# replace.
#
# The rule, stated structurally
# -----------------------------
# Under `src/widget/**`, every call to the crate's own invalidation entry points
# (`crate::invalidate_surface` / `crate::invalidate_surface_rect`) either:
#   (a) is reached through a function that named a cause (`RepaintReason`), or
#   (b) is in an explicit allowlist entry (with a reason).
#
# The two submission points in `runtime.rs` — `request_repaint_because` and
# `mark_dirty_rect` — are the ones that name a cause, and they are what the rest of
# the crate goes through. A *new* direct call is the finding.
#
# What this gate does NOT prove
# -----------------------------
#   * It is lexical. It reads call sites, not the runtime cause. The behavioural
#     half is `last_frame_stats()`'s tests, which assert the reasons arrive.
#   * It does not check the platform layer. A backend calls `crate::invalidate_surface`
#     as part of *implementing* the platform trait, which is not a submission the
#     library makes; those are the platform's own, and §7's gate is what covers
#     telling the library about them.
#
# Reverse injection
# -----------------
# Adding a bare `crate::invalidate_surface(id);` to a widget file must make this gate
# name it. See the round's report for the exact output.
#
# Usage: tools/check_no_anonymous_repaint.sh
# Exit 0 = every library-submitted repaint names a cause.
# Exit 1 = a finding (each anonymous call site is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] the reason channel exists"
if ! grep -qE '^pub enum RepaintReason' src/widget/runtime.rs; then
    echo "  FAIL  src/widget/runtime.rs no longer defines \`pub enum RepaintReason\`"
    echo "        It is the vocabulary every submission must name."
    exit 1
fi
if ! grep -qE '^pub fn request_repaint_because' src/widget/runtime.rs; then
    echo "  FAIL  \`request_repaint_because\` disappeared; submissions have no way to name a cause"
    exit 1
fi
echo "  PASS  RepaintReason and request_repaint_because are present"

echo "[2/3] every library-side invalidation names a cause"
# The submission points that legitimately call the crate's invalidation directly, each
# because it *is* the place that attaches the reason. Named individually rather than by
# file so a new bare call in the same file is still a finding.
ALLOWED='request_repaint_because|fn mark_dirty_rect|note_frame_repaint_reason'
RAW="$(grep -rn 'crate::invalidate_surface' src/widget/ --include=*.rs \
    | sed -e 's://.*::' || true)"

if [ -z "$RAW" ] || [ "$(printf '%s' "$RAW" | tr -d '[:space:]' | wc -c)" -eq 0 ]; then
    echo "  FAIL  no crate::invalidate_surface call sites found under src/widget/"
    echo "        The search is measuring nothing, which must not be reported as a pass."
    exit 1
fi

anonymous=""
while IFS= read -r hit; do
    [ -n "$hit" ] || continue
    file="${hit%%:*}"
    ln="${hit#*:}"; ln="${ln%%:*}"
    enclosing="$(awk -v upto="$ln" '
        NR <= upto && match($0, /fn[[:space:]]+[A-Za-z0-9_]+/) {
            s = substr($0, RSTART, RLENGTH); sub(/fn[[:space:]]+/, "", s); name = s
        }
        END { print name }
    ' "$file")"
    case "$enclosing" in
        *request_repaint_because|*mark_dirty_rect) continue ;;
    esac
    # A call inside a gated block that itself names a reason (the `Full` arm of
    # `request_repaint`) is covered by its caller; anything else is anonymous.
    if printf '%s' "$hit" | grep -qE "$ALLOWED"; then
        continue
    fi
    anonymous="${anonymous}${file}:${ln} in ${enclosing}()
"
done <<< "$RAW"

if [ -n "$anonymous" ]; then
    echo "  FAIL  these repaints name no cause, so the frame ledger cannot explain them:"
    printf '%s' "$anonymous" | sed 's/^/          /'
    echo "        Go through \`request_repaint_because(id, RepaintReason::…)\` (or"
    echo "        \`mark_dirty_rect\`, which names \`State\`) instead of the bare call."
    exit 1
fi
echo "  PASS  every library-side invalidation names a cause"

echo "[3/3] the raw entry points still exist for the paths that need them"
# `crate::invalidate_surface` / `invalidate_surface_rect` remain the platform trait's
# surface; removing them would mean the platform cannot reach them at all. This step
# asserts they were not deleted in pursuit of the rule above.
for f in 'fn invalidate_surface' 'fn invalidate_surface_rect'; do
    if ! grep -qE "$f" src/platform/types.rs; then
        echo "  FAIL  \`$f\` is gone from the Platform trait"
        echo "        The platform must still be able to invalidate its own surfaces; this"
        echo "        gate is about the *library's* submissions naming a cause, not about"
        echo "        removing the platform's ability to repaint."
        exit 1
    fi
done
echo "  PASS  the platform's own invalidation entry points are intact"

echo
echo "check_no_anonymous_repaint: OK"
