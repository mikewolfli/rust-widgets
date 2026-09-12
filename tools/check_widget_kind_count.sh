#!/usr/bin/env bash
# ============================================================================
# check_widget_kind_count.sh — documentation/code count consistency gate
# ============================================================================
# Hand-written widget counts in the docs drifted before: `codemap.md` claimed
# "166 variants" while `WidgetKind` actually had 167. That is exactly the
# "documentation deception" failure mode the project rules forbid (a doc that
# mirrors a code fact must be mechanically derived, or checked).
#
# This gate extracts the real count from `src/widget/kind.rs` — the single
# source of truth — and fails when a document states a different number.
#
# Note the enum is heavily `#[cfg]`-gated (137 of 167 variants), so the count is
# the *maximum* number of variants across feature sets. That is the number the
# docs describe; a per-feature count would need per-profile documents.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

KIND_FILE="src/widget/kind.rs"
if [[ ! -f "$KIND_FILE" ]]; then
  echo "missing $KIND_FILE" >&2
  exit 2
fi

ACTUAL="$(python3 -c '
import re, sys
src = open("src/widget/kind.rs").read()
m = re.search(r"pub enum WidgetKind \{(.*?)\n\}", src, re.S)
if not m:
    print("could not locate the WidgetKind enum", file=sys.stderr)
    sys.exit(2)
body = re.sub(r"//[^\n]*", "", m.group(1))
body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
variants = re.findall(r"^\s*([A-Z][A-Za-z0-9_]*)\s*(?:\([^)]*\))?\s*,", body, re.M)
print(len(variants))
')"

echo "WidgetKind variants (parsed from $KIND_FILE): $ACTUAL"

ERRORS=0

# Any document line of the form "<n> variants" must agree with ACTUAL.
check_doc() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  while IFS= read -r entry; do
    local lineno="${entry%%:*}"
    local text="${entry#*:}"
    local num
    num="$(printf '%s' "$text" | grep -oE '[0-9]+ variants' | grep -oE '[0-9]+' || true)"
    [[ -z "$num" ]] && continue
    if [[ "$num" != "$ACTUAL" ]]; then
      echo "❌ ${file}:${lineno} states '${num} variants' but the enum has ${ACTUAL}" >&2
      ERRORS=$((ERRORS + 1))
    fi
  done < <(grep -nE '[0-9]+ variants' "$file" || true)
}

check_doc "docs/plans/codemap.md"
check_doc "docs/plans/platform_capability_matrix.md"
check_doc "README.md"
check_doc "README.zh-CN.md"

# The READMEs also advertise the widget-kind total in prose (e.g. "167 widget
# kinds" / "167 种控件"). Those numbers must match too.
check_prose_count() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  while IFS= read -r entry; do
    local lineno="${entry%%:*}"
    local text="${entry#*:}"
    local num
    num="$(printf '%s' "$text" | grep -oE '[0-9]+ (widget kinds|种控件)' | grep -oE '[0-9]+' || true)"
    [[ -z "$num" ]] && continue
    if [[ "$num" != "$ACTUAL" ]]; then
      echo "❌ ${file}:${lineno} advertises '${num}' widget kinds but the enum has ${ACTUAL}" >&2
      ERRORS=$((ERRORS + 1))
    fi
  done < <(grep -nE '[0-9]+ (widget kinds|种控件)' "$file" || true)
}

check_prose_count "README.md"
check_prose_count "README.zh-CN.md"

if [[ "$ERRORS" -ne 0 ]]; then
  echo "" >&2
  echo "check_widget_kind_count: FAILED (${ERRORS} mismatched count(s))" >&2
  exit 1
fi

echo "✅ check_widget_kind_count: documented variant counts match the enum"
