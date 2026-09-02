#!/usr/bin/env bash
# ============================================================================
# check_platform_capability_matrix.sh — R6 Platform Capability Matrix Validator
# ============================================================================
# Validates:
#   1. The capability matrix document exists at docs/plans/platform_capability_matrix.md
#   2. Every WidgetKind variant appears as a row in the matrix
#   3. The matrix covers all 7 target platforms
#   4. Each cell contains exactly one valid emoji code
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

MATRIX_FILE="docs/plans/platform_capability_matrix.md"
ERRORS=0

# ---------------------------------------------------------------------------
# Helper: log an error and increment counter
# ---------------------------------------------------------------------------
error() {
  echo "❌ $*" >&2
  ERRORS=$((ERRORS + 1))
}

# ---------------------------------------------------------------------------
# 1. Matrix file exists
# ---------------------------------------------------------------------------
echo "--- [1] Checking matrix file exists ---"
if [[ ! -f "$MATRIX_FILE" ]]; then
  error "Matrix file not found: $MATRIX_FILE"
else
  echo "  ✅ Found: $MATRIX_FILE"
fi

# ---------------------------------------------------------------------------
# 2. Extract all widget row names from the matrix table
# ---------------------------------------------------------------------------
echo "--- [2] Validating widget coverage ---"

# Parse the markdown table: lines between the header separator (|---|---|...)
# and the first blank line or horizontal rule after the table.
# Widget rows start with "| **<name>** |". We extract the bold text.
WIDGETS_IN_MATRIX=()
while IFS= read -r line; do
  if [[ "$line" =~ ^\|\ \*\*([A-Za-z]+)\*\*\ \| ]]; then
    WIDGETS_IN_MATRIX+=("${BASH_REMATCH[1]}")
  fi
done < "$MATRIX_FILE"

if [[ ${#WIDGETS_IN_MATRIX[@]} -eq 0 ]]; then
  error "No widget rows found in the matrix table"
fi

echo "  Found ${#WIDGETS_IN_MATRIX[@]} widget rows in matrix"

# ---------------------------------------------------------------------------
# 3. Extract WidgetKind variants from source code and compare
# ---------------------------------------------------------------------------
WIDGET_KIND_FILE="src/widget/kind.rs"
if [[ ! -f "$WIDGET_KIND_FILE" ]]; then
  error "WidgetKind source file not found: $WIDGET_KIND_FILE"
else
  # Extract enum variants: lines with 4-space indent, an identifier, then a
  # trailing comma (doc comments, `#[cfg]` attributes, and `cfg_attr(...)`
  # continuations do not match).
  WIDGET_KINDS=()
  while IFS= read -r line; do
    # Match lines like "    Window," or "    WebEngineView, // comment" — the
    # comma must immediately follow the identifier, which excludes attribute
    # lines such as "    all(feature = ...)" and "    derive(...)".
    if [[ "$line" =~ ^[[:space:]]{4}([A-Za-z][A-Za-z0-9]*), ]]; then
      WIDGET_KINDS+=("${BASH_REMATCH[1]}")
    fi
  done < "$WIDGET_KIND_FILE"

  echo "  Found ${#WIDGET_KINDS[@]} WidgetKind variants in source"

  # Check each variant appears in the matrix
  MISSING=()
  for kind in "${WIDGET_KINDS[@]}"; do
    found=false
    for w in "${WIDGETS_IN_MATRIX[@]}"; do
      if [[ "$kind" == "$w" ]]; then
        found=true
        break
      fi
    done
    if ! $found; then
      MISSING+=("$kind")
    fi
  done

  if [[ ${#MISSING[@]} -gt 0 ]]; then
    for m in "${MISSING[@]}"; do
      error "WidgetKind variant '$m' is missing from the matrix"
    done
  else
    echo "  ✅ All WidgetKind variants are covered"
  fi
fi

# ---------------------------------------------------------------------------
# 4. Validate each cell has a valid emoji code
# ---------------------------------------------------------------------------
echo "--- [3] Validating matrix cell values ---"

VALID_CODES=("✅" "🔶" "⬜" "➖")
VALID_COLS=("Windows" "Linux/X11" "macOS" "Wayland" "Mobile" "Harmony" "Embedded/Stub")

LINE_NUM=0
TABLE_STARTED=false
TABLE_ENDED=false

while IFS= read -r line; do
  LINE_NUM=$((LINE_NUM + 1))

  # Detect table header start
  if echo "$line" | grep -qE '^\|.*Widget.*Windows.*Linux.*macOS'; then
    TABLE_STARTED=true
    continue
  fi

  # Detect separator row and skip
  if $TABLE_STARTED && ! $TABLE_ENDED; then
    if echo "$line" | grep -qE '^\|[- ]+\|'; then
      continue
    fi
  fi

  # Detect end of table (blank line after table content)
  if $TABLE_STARTED && ! $TABLE_ENDED; then
    if [[ -z "$line" ]] || echo "$line" | grep -qE '^---$'; then
      TABLE_ENDED=true
      continue
    fi

    # Skip non-row lines (like the platform description table or section headers)
    if ! echo "$line" | grep -qE '^\|.*\|.*\|.*\|.*\|.*\|'; then
      continue
    fi

    # Extract the 7 cell values (skip widget name column)
    # Split by | and trim whitespace
    IFS='|' read -ra CELLS <<< "$line"

    # cells[0] is empty (before first |), cells[1] is widget name
    # cells[2] through cells[8] are the 7 platform columns
    for ((i=2; i<=8; i++)); do
      cell_val=$(echo "${CELLS[$i]}" | xargs)
      valid=false
      for code in "${VALID_CODES[@]}"; do
        if [[ "$cell_val" == "$code" ]]; then
          valid=true
          break
        fi
      done
      if ! $valid; then
        col_name="${VALID_COLS[$((i-2))]}"
        widget_name=$(echo "${CELLS[1]}" | xargs | sed 's/\*\*//g')
        error "Row '$widget_name', column '$col_name' has invalid value '$cell_val'"
      fi
    done
  fi
done < "$MATRIX_FILE"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "---"
if [[ $ERRORS -eq 0 ]]; then
  echo "✅ All platform capability matrix checks passed."
  exit 0
else
  echo "❌ $ERRORS error(s) found in platform capability matrix."
  exit 1
fi
