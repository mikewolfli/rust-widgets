#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_plan_archive_has_index.sh — BLUE24 §9.1 criterion 3
# ============================================================================
# The rule this guards:
#
#   **Every archived plan is named in the index.** `docs/plans/archive/` holds the
#   plans that are finished or superseded; `docs/plans/README.md` is the one place a
#   reader learns which is which and why.
#
# The defect this stops
# ---------------------
# An archive nobody indexes is a directory a reader has to grep, and a plan that
# moves into it without a line saying *why* looks exactly like a plan that was
# deleted. The index is what makes "this is finished" a stated fact rather than an
# inference from a file's location.
#
# The rule, stated structurally
# -----------------------------
#   (1) `docs/plans/archive/` exists and holds at least one `.md`;
#   (2) every file in it is named (by basename, with or without the `.md`) somewhere
#       in `docs/plans/README.md`;
#   (3) no line in the README names an archived file that is not there — a dangling
#       index entry points a reader at a file that moved again.
#
# Usage: tools/check_plan_archive_has_index.sh
# Exit 0 = the archive and its index agree in both directions.
# Exit 1 = a finding (each unindexed / dangling file is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ARCHIVE="docs/plans/archive"
README="docs/plans/README.md"

echo "[1/4] the archive and its index exist"
if [ ! -d "$ARCHIVE" ]; then
    echo "  FAIL  $ARCHIVE is missing"
    exit 1
fi
if [ ! -f "$README" ]; then
    echo "  FAIL  $README is missing"
    exit 1
fi
archived="$(find "$ARCHIVE" -maxdepth 1 -name '*.md' -exec basename {} \; | sort)"
count="$(printf '%s\n' "$archived" | grep -c . || true)"
if [ "$count" -eq 0 ]; then
    echo "  FAIL  $ARCHIVE holds no .md files"
    echo "        An empty archive makes this gate measure nothing."
    exit 1
fi
echo "  PASS  $count archived plan(s) found"

echo "[2/4] every archived plan is named in the index"
unindexed=""
while IFS= read -r f; do
    [ -n "$f" ] || continue
    base="${f%.md}"
    if ! grep -qF "$base" "$README"; then
        unindexed="${unindexed}${f}
"
    fi
done <<< "$archived"

if [ -n "$unindexed" ]; then
    echo "  FAIL  these archived plans are not named in $README:"
    printf '%s' "$unindexed" | sed 's/^/          /'
    echo "        A plan that moves into the archive without a line saying why reads"
    echo "        like a plan that was deleted (BLUE24 §9.1)."
    exit 1
fi
echo "  PASS  every archived plan is named in the index"

echo "[3/4] the index names no file that is not archived"
# Read the `archive/`-linked names out of the index and check each exists. This is
# the other direction: a dangling entry sends a reader to a file that moved again.
dangling=""
while IFS= read -r name; do
    [ -n "$name" ] || continue
    if [ ! -f "$ARCHIVE/$name" ]; then
        dangling="${dangling}${name}
"
    fi
done < <(grep -oE 'archive/[A-Za-z0-9_.-]+\.md' "$README" | sed -e 's#archive/##' | sort -u)

if [ -n "$dangling" ]; then
    echo "  FAIL  $README names archived files that do not exist:"
    printf '%s' "$dangling" | sed 's/^/          /'
    exit 1
fi
echo "  PASS  no dangling archive entry in the index"

echo "[4/4] the counts stated in the index match the tree (W7-31)"
# Three numbers in the README are claims about the tree, and all three had drifted.
# A hand-maintained count is a fact nobody re-measures; deriving the expected value
# from the tree makes the README's arithmetic part of the gate instead of a memory.
live_root="$(find docs/plans -maxdepth 1 -name '*.md' -type f | grep -c . || true)"
live_arch="$(find "$ARCHIVE" -maxdepth 1 -name '*.md' -type f | grep -c . || true)"
live_principle="$(grep -oE '^[0-9]+\. ' docs/plans/principle.md | grep -oE '^[0-9]+' | sort -n | tail -1)"

declared_root="$(grep -oE '目录里有 [0-9]+ 份' "$README" | grep -oE '[0-9]+' | head -1 || true)"
declared_arch_one="$(grep -oE '[0-9]+ 份历史计划已归档' "$README" | grep -oE '^[0-9]+' | head -1 || true)"
declared_arch_two="$(grep -oE '归档（[0-9]+ 份）' "$README" | grep -oE '[0-9]+' | head -1 || true)"
declared_principle="$(grep -oE '#1–#[0-9]+' "$README" | grep -oE '[0-9]+$' | head -1 || true)"

count_problems=0
check_count() {
    local label="$1" declared="$2" live="$3"
    if [ -z "$declared" ]; then
        echo "  FAIL  $label: the README states no count (nothing to check)"
        count_problems=$((count_problems + 1))
    elif [ "$declared" != "$live" ]; then
        echo "  FAIL  $label: README says $declared, the tree has $live"
        count_problems=$((count_problems + 1))
    else
        echo "  PASS  $label: $live"
    fi
}
check_count "root plan files" "$declared_root" "$live_root"
check_count "archived plan files (§ intro)" "$declared_arch_one" "$live_arch"
check_count "archived plan files (§4 heading)" "$declared_arch_two" "$live_arch"
check_count "principle range end" "$declared_principle" "$live_principle"
if [ "$count_problems" -ne 0 ]; then
    echo "        These numbers are claims about the tree; update the README prose with"
    echo "        the live value instead of editing this gate."
    exit 1
fi

echo
echo "check_plan_archive_has_index: OK"
