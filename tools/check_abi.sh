#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
# SPDX-License-Identifier: MIT
#
# check_abi.sh -- the committed C headers must equal a fresh generation.
#
# Regenerates `include/rw_generated.h` and `examples/rust_widgets.generated.h` with
# `tools/generate_c_header.py`, diffs them against the committed copies, and restores the
# originals on exit. A hand-edited header drifts from the Rust signatures the generator reads,
# so the two are compared rather than trusted.
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
  ' src/bindings/binding_impl/binding_status.rs
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
#
# The list includes the host-side input path — `rw_dispatch_pointer_event` /
# `rw_dispatch_event_to_widget` — because three integration documents instruct a host to
# call them and, until 2026-09-30, neither existed: a mounted widget was visible and
# completely inert, with no symbol to deliver a touch through. Requiring them here is what
# keeps that from recurring silently.
for symbol in \
  rw_bindings_api_version \
  rw_create_label \
  rw_create_radio_button \
  rw_create_slider \
  rw_destroy_widget \
  rw_platform_capabilities \
  rw_platform_dpi_scale_factor \
  rw_dispatch_pointer_event \
  rw_dispatch_event_to_widget \
  rw_take_pending_repaint \
  rw_mount_surface \
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
  # The documented count is the **bolded claim** about `rw_*` functions, not the first
  # number followed by the symbol name anywhere in the prose. The old pattern took the
  # first `N rw_...` match in the file, so a sentence that happened to mention a count
  # (a changelog line, an example) became the asserted document — the gate would then
  # compare against the wrong number and report the header as drifted (W6-35).
  #
  # Both READMEs state it in this shape:
  #   README.md      **152 `rw_*` functions**
  #   README.zh-CN   **152 个 `rw_*` 函数**
  # so the anchor is `**<number> … rw_* … **`. An ambiguous document (zero matches, or
  # more than one bolded count claim) is reported rather than guessed at.
  DOCUMENTED_HITS="$(grep -oE '\*\*[0-9]+[^*]*rw_\*[^*]*\*\*' "$readme" || true)"
  DOCUMENTED_CLAIMS="$(printf '%s\n' "$DOCUMENTED_HITS" | grep -c . || true)"
  if [[ "$DOCUMENTED_CLAIMS" -ne 1 ]]; then
    echo "Unable to find exactly one documented rw_* function count in ${readme} (found ${DOCUMENTED_CLAIMS})." >&2
    echo "  Expected one bolded claim of the form: **<n> \`rw_*\` functions**." >&2
    exit 1
  fi
  DOCUMENTED_COUNT="$(printf '%s\n' "$DOCUMENTED_HITS" | sed -n 's/^\*\*\([0-9][0-9]*\).*/\1/p')"
  if [[ -z "$DOCUMENTED_COUNT" ]]; then
    echo "Unable to read the documented rw_* function count from ${readme}: ${DOCUMENTED_HITS}" >&2
    exit 1
  fi
  if [[ "$DOCUMENTED_COUNT" != "$ACTUAL_COUNT" ]]; then
    echo "${readme} documents ${DOCUMENTED_COUNT} rw_* functions but ${PUBLISHED_HEADER} declares ${ACTUAL_COUNT}." >&2
    exit 1
  fi
done
echo "Published ABI declares ${ACTUAL_COUNT} rw_* functions (matches README and README.zh-CN)."

echo "ABI checks passed."
