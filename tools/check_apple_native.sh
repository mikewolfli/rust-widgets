#!/usr/bin/env bash
# ============================================================================
# check_apple_native.sh — Apple native verification gate (macOS only)
# ============================================================================
# Closes blue14 §二 A 类 #4 (iOS simulator view behaviour) and #5 (macOS AppKit
# interaction) as a *continuously reproducible* gate rather than a one-off run.
#
# Runs, in order:
#   1. The main-thread AppKit probe with the cocoa-legacy backend (`desktop`).
#   2. The main-thread AppKit probe with the objc2 backend (`macos`).
#   3. The iOS Simulator integration app (build + boot + install + run).
#
# On a non-macOS host the gate exits 2 (explicitly unsupported) rather than
# silently passing, so an accidental run on Linux CI cannot masquerade as
# coverage.
#
# Environment overrides:
#   SKIP_IOS_PROBE   set to 1 to run only the macOS AppKit probes
#   SKIP_BUILD       forwarded to the iOS runner
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "check_apple_native: unsupported host '$(uname -s)' (Apple native verification requires macOS)" >&2
  exit 2
fi

# The static guard check is host-independent; run it first so an omission is
# reported before the (slower) runtime probes.
echo "=== [0/3] Static FFI thread-safety gate ==="
bash "$ROOT_DIR/tools/check_apple_thread_safety.sh"

echo ""
echo "=== [1/3] AppKit probe — cocoa-legacy backend (--features desktop) ==="
cargo run --quiet --example apple_appkit_probe --features desktop

echo ""
echo "=== [2/3] AppKit probe — objc2 backend (--features macos) ==="
# `macos` alone does not enable the serde snapshot helpers the macOS objc2
# backend compiles against, so the probe needs them explicitly.
cargo run --quiet --example apple_appkit_probe_objc2 \
  --no-default-features --features "macos,serde,serde_json"

if [[ "${SKIP_IOS_PROBE:-0}" == "1" ]]; then
  echo ""
  echo "=== [3/3] iOS Simulator probe skipped (SKIP_IOS_PROBE=1) ==="
  echo "check_apple_native: macOS AppKit checks PASSED"
  exit 0
fi

echo ""
echo "=== [3/3] iOS Simulator integration probe ==="
bash "$ROOT_DIR/tools/run_ios_testapp.sh"

echo ""
echo "check_apple_native: ALL Apple native checks PASSED"
