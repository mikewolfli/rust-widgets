#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# ============================================================================
# check_changelog_sync.sh — the two changelog copies must agree
# ============================================================================
# # Why this gate exists
#
# `CHANGELOG.md` says its canonical copy lives at `docs/reports/CHANGELOG.md`.
# That was not true: the canonical path did not exist, so the root file's pointer
# was a claim nothing backed — and no gate had ever looked, because no gate
# validated either path (`grep -rn CHANGELOG tools/` returned nothing).
#
# A release note is the one document a downstream user reads without being able to
# check it against the code. Two copies that can drift silently is the failure mode
# that matters here, so the fix is not "remember to update both" but this check.
#
# # What it asserts
#
#   [1] both files exist
#   [2] they are byte-identical
#   [3] the root file's pointer still names the canonical path
#
# # What it does not assert
#
# It does not check the *content* of the notes (that a version is described
# accurately). That is not mechanically decidable; the release checklist covers it.
# This gate only removes the silent-divergence failure, which is the one a
# copy-paste release process actually produces.
#
# Exit 0 = the two copies agree. Exit 1 = they have drifted or one is missing.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ROOT_CHANGELOG="CHANGELOG.md"
CANONICAL_CHANGELOG="docs/reports/CHANGELOG.md"
ERRORS=0

echo "=== [1] Both changelog copies must exist ==="
for file in "$ROOT_CHANGELOG" "$CANONICAL_CHANGELOG"; do
  if [[ -f "$file" ]]; then
    echo "  ✅ $file"
  else
    echo "  ❌ $file is missing"
    ERRORS=$((ERRORS + 1))
  fi
done

if [[ "$ERRORS" -gt 0 ]]; then
  echo ""
  echo "changelog sync: FAILED ($ERRORS finding(s))" >&2
  exit 1
fi

echo ""
echo "=== [2] The two copies must be byte-identical ==="
if diff -q "$ROOT_CHANGELOG" "$CANONICAL_CHANGELOG" >/dev/null 2>&1; then
  echo "  ✅ identical ($(wc -l < "$ROOT_CHANGELOG" | tr -d ' ') lines)"
else
  echo "  ❌ the two changelog copies have drifted. First differences:"
  diff "$ROOT_CHANGELOG" "$CANONICAL_CHANGELOG" | head -20 | sed 's/^/       /'
  echo "       Fix with: cp $ROOT_CHANGELOG $CANONICAL_CHANGELOG"
  ERRORS=$((ERRORS + 1))
fi

echo ""
echo "=== [3] The root copy must still point at the canonical one ==="
if grep -q "docs/reports/CHANGELOG.md" "$ROOT_CHANGELOG"; then
  echo "  ✅ the root file names $CANONICAL_CHANGELOG"
else
  echo "  ❌ the root file no longer names $CANONICAL_CHANGELOG, so a reader is not"
  echo "     told where the maintained copy lives"
  ERRORS=$((ERRORS + 1))
fi

echo ""
if [[ "$ERRORS" -gt 0 ]]; then
  echo "changelog sync: FAILED ($ERRORS finding(s))" >&2
  exit 1
fi
echo "✅ changelog sync: the two copies agree."
