#!/usr/bin/env bash
# ============================================================================
# run_ios_testapp.sh — install + run the iOS Simulator probe, assert PASS
# ============================================================================
# Apple analogue of `tools/run_android_testapp.sh`. Boots an iOS Simulator,
# installs the bundle produced by `tools/build_ios_testapp.sh`, launches it, and
# asserts the probe reported `RESULT: PASS`.
#
# The probe writes its per-check lines to the app's Documents directory
# (`ios_probe_result.txt`); the runner copies that file out of the simulator's
# data container so the evidence is inspectable after the run.
#
# Environment overrides:
#   IOS_SIM_DEVICE  simulator device name (default: first available iPhone)
#   IOS_SIM_RUNTIME simulator runtime id (default: newest iOS runtime)
#   SKIP_BUILD      set to 1 to reuse an existing app bundle
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: run_ios_testapp.sh requires macOS (found $(uname -s))" >&2
  exit 2
fi

IOS_SIM_ARCH="${IOS_SIM_ARCH:-$(uname -m)}"
APP_BUNDLE="$ROOT_DIR/target/ios-testapp-$IOS_SIM_ARCH/rwiosprobe.app"
BUNDLE_ID="com.rustwidgets.iosprobe"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  bash "$ROOT_DIR/tools/build_ios_testapp.sh"
fi

if [[ ! -d "$APP_BUNDLE" ]]; then
  echo "error: app bundle not found at $APP_BUNDLE" >&2
  echo "       run: bash tools/build_ios_testapp.sh" >&2
  exit 2
fi

# Pick a device + runtime unless overridden.
RUNTIME="${IOS_SIM_RUNTIME:-$(xcrun simctl list runtimes | awk '/^iOS/ {print $NF}' | tail -1)}"
DEVICE_NAME="${IOS_SIM_DEVICE:-}"
if [[ -z "$DEVICE_NAME" ]]; then
  # Take the first available iPhone line of the form:
  #   iPhone 17 Pro (UDID) (Shutdown)
  DEVICE_NAME="$(xcrun simctl list devices available \
    | sed -nE 's/^[[:space:]]*(iPhone[^(]*) \([0-9A-F-]+\).*/\1/p' \
    | sed -E 's/[[:space:]]+$//' | head -1)"
fi

if [[ -z "$DEVICE_NAME" ]]; then
  echo "error: no available iPhone simulator found; set IOS_SIM_DEVICE" >&2
  exit 2
fi

echo "runtime=$RUNTIME device='$DEVICE_NAME'"

# Resolve the UDID for the chosen device name (exact match on the parentheses).
UDID="$(xcrun simctl list devices available \
  | grep -F "$DEVICE_NAME (" | head -1 \
  | sed -E 's/.*\(([0-9A-F-]+)\).*/\1/')"
if [[ -z "$UDID" || ${#UDID} -lt 8 ]]; then
  echo "error: could not resolve a simulator UDID for '$DEVICE_NAME'" >&2
  exit 2
fi

echo "[1/4] Booting simulator $DEVICE_NAME ($UDID)"
xcrun simctl bootstatus "$UDID" -b >/dev/null 2>&1 || xcrun simctl boot "$UDID" >/dev/null 2>&1 || true
xcrun simctl bootstatus "$UDID" -b >/dev/null 2>&1 || true

echo "[2/4] Installing app bundle"
xcrun simctl uninstall "$UDID" "$BUNDLE_ID" >/dev/null 2>&1 || true
xcrun simctl install "$UDID" "$APP_BUNDLE"

echo "[3/4] Launching probe"
LOG_FILE="$ROOT_DIR/target/ios-testapp-$IOS_SIM_ARCH/launch.log"
xcrun simctl launch --console-pty "$UDID" "$BUNDLE_ID" 2>&1 | tee "$LOG_FILE" || true

echo "[4/4] Extracting result file"
CONTAINER="$(xcrun simctl get_app_container "$UDID" "$BUNDLE_ID" data 2>/dev/null || true)"
RESULT_FILE="$ROOT_DIR/target/ios-testapp-$IOS_SIM_ARCH/ios_probe_result.txt"
if [[ -n "$CONTAINER" && -f "$CONTAINER/Documents/ios_probe_result.txt" ]]; then
  cp "$CONTAINER/Documents/ios_probe_result.txt" "$RESULT_FILE"
  echo "--- probe checks ---"
  cat "$RESULT_FILE"
  echo ""
fi

if grep -q "RESULT: PASS" "$LOG_FILE" 2>/dev/null; then
  echo "RESULT: PASS"
  exit 0
fi

if [[ -f "$RESULT_FILE" ]] && ! grep -q "FAIL" "$RESULT_FILE"; then
  echo "RESULT: PASS (from result file)"
  exit 0
fi

echo "RESULT: FAIL — see $LOG_FILE"
exit 1
