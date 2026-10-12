#!/usr/bin/env bash
# ============================================================================
# check_binding_symbol_coverage.sh — binding surface gate
# ============================================================================
# Two checks, because they cover different failure modes:
#
#   1. **Static** (`check_binding_symbol_coverage.py`): every published `rw_*`
#      symbol and every `RW_VALUE_*` discriminator is nameable from each direct
#      FFI binding.
#   2. **Runtime** (`bindings/python/test_abi_contract.py`): the Python binding
#      actually loads the built library and calls through it.
#
# The runtime half exists because the static half cannot see a `NameError` in
# `_setup_argtypes` or a wrong `restype`. Both shipped: the binding's local type
# aliases omitted `c_char`/`c_uint8`, so `_setup_argtypes` raised on **every**
# call, and `rw_free_string` was declared `c_char_p` while the owned-string
# returns produced `c_void_p` addresses. The gate reported full coverage
# throughout, because it only ever read the source.
#
# The library is built here unconditionally, so the runtime half measures the
# *current* source rather than whatever the last `cargo build` happened to leave
# behind. The previous form built only when the file was **missing**, which was
# wrong in both directions and both were observed:
#
#   * a **false failure**: an artifact built before the `RW_VALUE_ICON_SPEC`
#     round trip existed made the `icon_source` check report "the property does
#     not accept that value kind" against source that had supported it for
#     hours, and reading the gate's output alone could not tell that apart from
#     a real regression;
#   * a **false pass**: a source-level regression made after the artifact was
#     built is invisible, which is the more dangerous half.
#
# Cargo decides what to rebuild, so an unconditional build costs nothing when
# the sources are unchanged. A build failure is reported as FAILED rather than
# skipped, since a binding that cannot load its library is the defect this half
# exists to catch.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_cargo_cache.sh"
# `lib_timeout.sh` provides `rw_run_bounded`, used below to bound the build. Sourced explicitly
# (also reachable transitively via `lib_python.sh`) so the dependency is visible at the call site.
. "$ROOT_DIR/tools/lib_timeout.sh"

"$PYTHON" tools/check_binding_symbol_coverage.py "$@"

# ── Runtime half ─────────────────────────────────────────────────────────
LIB_PATH="target/debug/rust_widgets.dll"
if [[ ! -f "$LIB_PATH" ]]; then
  LIB_PATH="target/debug/librust_widgets.so"
fi
if [[ ! -f "$LIB_PATH" ]]; then
  LIB_PATH="target/debug/librust_widgets.dylib"
fi

echo ""
echo "[runtime] building the shared library so the check measures the current source"
rw_run_bounded 900 cargo build --lib --no-default-features --features desktop >/dev/null 2>&1 || {
  echo "❌ runtime half: could not build the shared library" >&2
  exit 1
}

# Re-resolve the path **after** the build: on a clean tree the file did not exist when the
# block above ran, so a path chosen before it could be the empty string.
if [[ ! -f "$LIB_PATH" ]]; then
  LIB_PATH="target/debug/librust_widgets.so"
fi
if [[ ! -f "$LIB_PATH" ]]; then
  LIB_PATH="target/debug/librust_widgets.dylib"
fi
if [[ ! -f "$LIB_PATH" ]]; then
  echo "❌ runtime half: the build produced no shared library to load" >&2
  exit 1
fi

echo ""
echo "[runtime] Python binding ABI contract"
if ! PYTHONPATH="$ROOT_DIR/bindings/python" "$PYTHON" "$ROOT_DIR/bindings/python/test_abi_contract.py"; then
  echo "❌ the Python binding could not complete its ABI contract checks" >&2
  exit 1
fi

echo ""
echo "✅ binding coverage + Python ABI contract checks passed."
