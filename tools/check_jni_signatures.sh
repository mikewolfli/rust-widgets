#!/usr/bin/env bash
# ============================================================================
# check_jni_signatures.sh — Java ↔ Rust JNI signature + symbol gate
# ============================================================================
# Validates that the Java `native` declarations match the Rust JNI exports in
# name, arity and parameter types, for both JNI surfaces:
#
#   1. Generic Java / C-ABI binding  — src/bindings/java_jni.rs
#   2. Android native-view binding   — src/platform/android_jni.rs
#
# When the Android shared library has already been built (aarch64-linux-android)
# the corresponding exported symbols are also checked against the declarations,
# so a name-mangling drift is caught against the real binary rather than only
# against the source.
#
# The Java-method -> JNI-symbol -> Rust-export mapping for both surfaces is
# always written as JSON under target/qa/ for auditing.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"

QA_DIR="target/qa"
mkdir -p "$QA_DIR"

echo "[1/3] Generic Java binding (io.github.rustwidgets.RustWidgets)"
"$PYTHON" tools/check_jni_signatures.py \
  --java bindings/java/RustWidgets.java \
  --java-class io.github.rustwidgets.RustWidgets \
  --rust src/bindings/java_jni.rs \
  --report "$QA_DIR/jni_binding_map.json"

echo "[2/3] Android native-view binding (rust_widgets.RustWidgets)"
"$PYTHON" tools/check_jni_signatures.py \
  --java bindings/java/RustWidgetsAndroid.java \
  --java-class rust_widgets.RustWidgets \
  --rust src/platform/android_jni.rs \
  --report "$QA_DIR/jni_android_view_map.json"

echo "[3/3] Exported-symbol parity (optional, requires a built Android .so)"
# Check every built Android target so a stale artifact for one ABI cannot mask
# (or falsely flag) drift. Skip silently when none has been built yet.
SO_FOUND=0
for SO in target/*-linux-android/debug/librust_widgets.so; do
  [[ -f "$SO" ]] || continue
  SO_FOUND=1
  echo "  -- $SO"
  "$PYTHON" tools/check_jni_signatures.py \
    --java bindings/java/RustWidgetsAndroid.java \
    --java-class rust_widgets.RustWidgets \
    --rust src/platform/android_jni.rs \
    --symbols "$SO" \
    --report "$QA_DIR/jni_android_view_map.$(basename "$(dirname "$(dirname "$SO")")").json"
  "$PYTHON" tools/check_jni_signatures.py \
    --java bindings/java/RustWidgets.java \
    --java-class io.github.rustwidgets.RustWidgets \
    --rust src/bindings/java_jni.rs \
    --symbols "$SO" \
    --report "$QA_DIR/jni_binding_map.$(basename "$(dirname "$(dirname "$SO")")").json"
done
if [[ "$SO_FOUND" -eq 0 ]]; then
  echo "  (skipped — no Android .so built; run: cargo build --lib --target aarch64-linux-android --features \"android-jni jni mobile-api\")"
fi

echo "✅ JNI signature checks passed (mapping reports in $QA_DIR/)"
