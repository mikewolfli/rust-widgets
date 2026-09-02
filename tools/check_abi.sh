#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TMP_BEFORE="$(mktemp)"
trap 'rm -f "$TMP_BEFORE"' EXIT

echo "[1/4] Regenerate C header snapshot"
cp examples/rust_widgets.generated.h "$TMP_BEFORE"
python3 tools/generate_c_header.py

echo "[2/4] Check generated header consistency"
if ! cmp -s "$TMP_BEFORE" examples/rust_widgets.generated.h; then
  echo "ABI header drift detected: regenerate with tools/generate_c_header.py" >&2
  diff -u "$TMP_BEFORE" examples/rust_widgets.generated.h || true
  exit 1
fi

echo "[3/4] Check ABI version constant alignment"
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
if ! grep -q '^unsigned int rw_bindings_api_version(void);$' examples/rust_widgets.generated.h; then
  echo "Missing rw_bindings_api_version declaration in generated header." >&2
  exit 1
fi
echo "Detected ABI version: ${RUST_ABI_VERSION}"

echo "[4/4] Check required exported ABI symbols in header"
for symbol in \
  rw_bindings_api_version \
  rw_create_label \
  rw_create_radio_button \
  rw_create_slider \
  rw_platform_capabilities \
  rw_platform_dpi_scale_factor \
  rw_harmony_bind_node \
  rw_harmony_on_widget_event
  do
  if ! grep -q "${symbol}" examples/rust_widgets.generated.h; then
    echo "Missing ABI symbol declaration in header: ${symbol}" >&2
    exit 1
  fi
done

echo "ABI checks passed."
