#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

PUBLISHED_HEADER="include/rw_generated.h"
EXAMPLE_HEADER="examples/rust_widgets.generated.h"

TMP_BEFORE="$(mktemp)"
TMP_PUBLISHED="$(mktemp)"
trap 'rm -f "$TMP_BEFORE" "$TMP_PUBLISHED"' EXIT

echo "[1/6] Regenerate C header snapshots"
cp "$EXAMPLE_HEADER" "$TMP_BEFORE"
cp "$PUBLISHED_HEADER" "$TMP_PUBLISHED"
"$PYTHON" tools/generate_c_header.py
"$PYTHON" tools/generate_c_header.py --output "$PUBLISHED_HEADER"

echo "[2/6] Check example header consistency"
if ! cmp -s "$TMP_BEFORE" "$EXAMPLE_HEADER"; then
  echo "ABI header drift detected in ${EXAMPLE_HEADER}: regenerate with tools/generate_c_header.py" >&2
  diff -u "$TMP_BEFORE" "$EXAMPLE_HEADER" || true
  exit 1
fi

echo "[3/6] Check published header consistency"
if ! cmp -s "$TMP_PUBLISHED" "$PUBLISHED_HEADER"; then
  echo "ABI header drift detected in ${PUBLISHED_HEADER}: regenerate with tools/generate_c_header.py --output ${PUBLISHED_HEADER}" >&2
  diff -u "$TMP_PUBLISHED" "$PUBLISHED_HEADER" || true
  exit 1
fi

echo "[4/6] Check published header matches example header"
if ! cmp -s "$PUBLISHED_HEADER" "$EXAMPLE_HEADER"; then
  echo "Published and example ABI headers disagree: ${PUBLISHED_HEADER} vs ${EXAMPLE_HEADER}" >&2
  diff -u "$PUBLISHED_HEADER" "$EXAMPLE_HEADER" || true
  exit 1
fi

echo "[5/6] Check ABI version constant alignment"
RUST_ABI_VERSION="$({
  awk '
    /rw_bindings_api_version\(\)/ { in_fn=1; next }
    in_fn {
      if ($0 ~ /[0-9]+/) {
        value = $0;
        gsub(/[^0-9]/, "", value);
        if (value != "") {
          print value;
          exit;
        }
      }
      if ($0 ~ /^}/) {
        exit;
      }
    }
  ' src/bindings/binding_impl.rs
} || true)"
if [[ -z "$RUST_ABI_VERSION" ]]; then
  echo "Unable to read ABI version from rw_bindings_api_version()." >&2
  exit 1
fi
if ! grep -q '^unsigned int rw_bindings_api_version(void);$' "$PUBLISHED_HEADER"; then
  echo "Missing rw_bindings_api_version declaration in published header." >&2
  exit 1
fi
echo "Detected ABI version: ${RUST_ABI_VERSION}"

echo "[6/6] Check required exported ABI symbols and documented function count"
# The published header is the contract consumers compile against, so both the
# required-symbol sweep and the count assertion read it (not examples/).
for symbol in \
  rw_bindings_api_version \
  rw_create_label \
  rw_create_radio_button \
  rw_create_slider \
  rw_destroy_widget \
  rw_platform_capabilities \
  rw_platform_dpi_scale_factor \
  rw_harmony_bind_node \
  rw_harmony_on_widget_event
  do
  if ! grep -q "${symbol}" "$PUBLISHED_HEADER"; then
    echo "Missing ABI symbol declaration in published header: ${symbol}" >&2
    exit 1
  fi
done

ACTUAL_COUNT="$(grep -cE '^[a-z].*\(.*\);$' "$PUBLISHED_HEADER")"
for readme in README.md README.zh-CN.md; do
  if [[ ! -f "$readme" ]]; then
    echo "Missing ${readme}; cannot verify documented ABI function count." >&2
    exit 1
  fi
  DOCUMENTED_COUNT="$(grep -oE '[0-9]+ ([`]?rw_[`]?|个 `rw_\*`)' "$readme" | head -1 | grep -oE '^[0-9]+' || true)"
  if [[ -z "$DOCUMENTED_COUNT" ]]; then
    echo "Unable to find the documented rw_* function count in ${readme}." >&2
    exit 1
  fi
  if [[ "$DOCUMENTED_COUNT" != "$ACTUAL_COUNT" ]]; then
    echo "${readme} documents ${DOCUMENTED_COUNT} rw_* functions but ${PUBLISHED_HEADER} declares ${ACTUAL_COUNT}." >&2
    exit 1
  fi
done
echo "Published ABI declares ${ACTUAL_COUNT} rw_* functions (matches README and README.zh-CN)."

echo "ABI checks passed."
