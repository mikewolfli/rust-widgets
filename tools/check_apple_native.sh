#!/usr/bin/env bash
# ============================================================================
# check_apple_native.sh — Apple native verification gate (macOS only)
# ============================================================================
# BLUE15 status: the AppKit control probes this gate used to run are DELETED.
#
# What was retired, and why
# -------------------------
# The gate ran two `examples/apple_appkit_probe*.rs` probes that asserted the
# *native control* path: that `Platform::create_button` produced a live
# `NSButton` whose `.frame` moved with `set_widget_geometry`, that
# `set_widget_text` reached the AppKit object, and so on.
#
# Under the self-drawn architecture the host creates **no** controls (BLUE15
# #55/#56): it supplies a window and a drawing surface. Every assertion those
# probes made therefore describes machinery that no longer exists, which is why
# the probes and this gate are retired rather than repaired. A gate whose subject
# is gone must be removed, not kept green by weakening its assertions
# (BLUE15 §2.8).
#
# What covers the same ground now
# -------------------------------
#   * static FFI thread-safety — `tools/check_apple_thread_safety.sh`, still run
#     below because it inspects source rather than runtime objects.
#   * the macOS window/event/IME/clipboard surface — `src/platform/macos/tests.rs`
#     and `src/platform/contract_tests.rs`, which run in the normal test suite
#     (`cargo test --no-default-features --features desktop`).
#   * surface mounting and frame production — `widget::runtime` rendering tests.
#
# On a non-macOS host this exits 2 (explicitly unsupported) rather than silently
# passing, so a Linux CI run cannot masquerade as Apple coverage.
#
# Environment overrides:
#   SKIP_IOS_PROBE   set to 1 to skip the iOS Simulator integration probe
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
echo "=== [1/2] Static FFI thread-safety gate ==="
bash "$ROOT_DIR/tools/check_apple_thread_safety.sh"

if [[ "${SKIP_IOS_PROBE:-0}" == "1" ]]; then
  echo ""
  echo "=== [2/2] iOS Simulator probe skipped (SKIP_IOS_PROBE=1) ==="
  echo "check_apple_native: Apple native checks PASSED"
  exit 0
fi

echo ""
echo "=== [2/2] iOS Simulator integration probe ==="
bash "$ROOT_DIR/tools/run_ios_testapp.sh"

echo ""
echo "check_apple_native: ALL Apple native checks PASSED"
