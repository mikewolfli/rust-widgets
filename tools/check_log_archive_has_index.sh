#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_log_archive_has_index.sh — AG6-13 / A3-01 root cause
# ============================================================================
# The rule this guards:
#
#   **Every log is named in `docs/log/README.md`.**
#
# `docs/log/` is where a round's evidence lives, and `docs/log/archive/` is where a
# finished round's log moves. The plans gate already insists that every archived
# *plan* is indexed (BLUE24 §9.1); logs had no such rule, which is how A3-01 found
# 21 of 27 references in the instruction surface pointing at files that had moved
# or never existed. A log nobody lists is a log a reader has to grep for, and a
# reference to it is a claim with nothing behind it.
#
# The rule, stated structurally
# -----------------------------
#   (1) `docs/log/` and `docs/log/archive/` exist and hold at least one `.md` each;
#   (2) every live log is named in `docs/log/README.md`;
#   (3) every archived log is named in it, with its `archive/` prefix;
#   (4) no line in the README names a `log-*.md` that is not on disk — a dangling
#       index entry points at a file that moved again.
#
# Usage: tools/check_log_archive_has_index.sh
# Exit 0 = the logs and their index agree in both directions.
# Exit 1 = a finding (each unindexed / dangling file is named).
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

LOG_DIR="docs/log"
ARCHIVE="$LOG_DIR/archive"
README="$LOG_DIR/README.md"

echo "[1/4] the logs and their index exist"
for path in "$LOG_DIR" "$ARCHIVE"; do
    if [ ! -d "$path" ]; then
        echo "  FAIL  $path is missing"
        exit 1
    fi
done
if [ ! -f "$README" ]; then
    echo "  FAIL  $README is missing"
    echo "        Without it there is nothing for this gate to compare against."
    exit 1
fi
live="$(find "$LOG_DIR" -maxdepth 1 -name 'log-*.md' -exec basename {} \; | sort)"
archived="$(find "$ARCHIVE" -maxdepth 1 -name 'log-*.md' -exec basename {} \; | sort)"
live_count="$(printf '%s\n' "$live" | grep -c . || true)"
arch_count="$(printf '%s\n' "$archived" | grep -c . || true)"
if [ "$live_count" -eq 0 ] || [ "$arch_count" -eq 0 ]; then
    echo "  FAIL  the log directories are not both populated (live=$live_count archive=$arch_count)"
    echo "        An empty directory makes this gate measure nothing."
    exit 1
fi
echo "  PASS  $live_count live log(s), $arch_count archived log(s)"

echo "[2/4] every live log is named in the index"
missing=""
while IFS= read -r f; do
    [ -n "$f" ] || continue
    if ! grep -qF "]($f)" "$README"; then
        missing="${missing}${f}
"
    fi
done <<< "$live"
if [ -n "$missing" ]; then
    echo "  FAIL  these live logs are not linked from $README:"
    printf '%s' "$missing" | sed 's/^/          /'
    echo "        Add a row to the 'Live logs' table (see the README's own format)."
    exit 1
fi
echo "  PASS  every live log is linked"

echo "[3/4] every archived log is named in the index"
missing=""
while IFS= read -r f; do
    [ -n "$f" ] || continue
    if ! grep -qF "](archive/$f)" "$README"; then
        missing="${missing}${f}
"
    fi
done <<< "$archived"
if [ -n "$missing" ]; then
    echo "  FAIL  these archived logs are not linked from $README:"
    printf '%s' "$missing" | sed 's/^/          /'
    echo "        A log that moves into the archive without an index row reads like"
    echo "        a log that was deleted (A3-01's root cause)."
    exit 1
fi
echo "  PASS  every archived log is linked"

echo "[4/4] the index names no log that is not on disk"
dangling=""
for named in $(grep -oE '\]\((archive/)?log-[0-9]{8}-[0-9]+\.md\)' "$README" | sed 's/^](//;s/)$//'); do
    if [ ! -f "$LOG_DIR/$named" ]; then
        dangling="${dangling}${named}
"
    fi
done
if [ -n "$dangling" ]; then
    echo "  FAIL  $README links logs that do not exist:"
    printf '%s' "$dangling" | sed 's/^/          /'
    echo "        A dangling index row points a reader at a file that moved again."
    exit 1
fi
echo "  PASS  no dangling index row"

echo
echo "check_log_archive_has_index: OK ($live_count live + $arch_count archived, all linked)"
