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

echo "[1/3] the archive and its index exist"
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

echo "[2/3] every archived plan is named in the index"
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

echo "[3/3] the index names no file that is not archived"
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

echo
echo "check_plan_archive_has_index: OK"
