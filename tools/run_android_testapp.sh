#!/usr/bin/env bash
# ============================================================================
# run_android_testapp.sh — build, install and run the JNI integration test
# ============================================================================
# End-to-end Android verification:
#
#   1. Build + sign the test APK for the requested ABI (see build script)
#   2. Boot (or reuse) an emulator AVD matching that ABI
#   3. Install the APK and launch the activity
#   4. Assert the logcat marker `RESULT: PASS`
#
# Environment overrides:
#   ANDROID_ABI    arm64-v8a (default) | x86_64 | armeabi-v7a
#   ANDROID_AVD    AVD name; defaults to rw_x64 / rw_arm64 by ABI
#   ANDROID_SERIAL adb device serial. Set this to target a specific device —
#                  in particular a physical phone, which skips emulator boot
#                  entirely. When unset and exactly one device is attached it is
#                  selected automatically; otherwise an emulator is booted.
#   ANDROID_SDK_ROOT, ANDROID_JAR_LEVEL, JAVA_HOME  (see build script)
#
# The emulator is started headless with KVM and reused if already running.
# Requires an installed system image for the ABI, e.g.
#   sdkmanager "system-images;android-34;google_apis;x86_64"
# Physical devices need no system image.
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
ANDROID_ABI="${ANDROID_ABI:-arm64-v8a}"
API_LEVEL="${ANDROID_JAR_LEVEL:-34}"

case "$ANDROID_ABI" in
  arm64-v8a) DEFAULT_AVD="rw_arm64"; SYS_IMG="system-images;android-$API_LEVEL;google_apis;arm64-v8a" ;;
  x86_64)    DEFAULT_AVD="rw_x64";   SYS_IMG="system-images;android-$API_LEVEL;google_apis;x86_64" ;;
  *) echo "error: unsupported ANDROID_ABI '$ANDROID_ABI'" >&2; exit 2 ;;
esac
AVD="${ANDROID_AVD:-$DEFAULT_AVD}"

ADB="$SDK_ROOT/platform-tools/adb"
EMULATOR="$SDK_ROOT/emulator/emulator"
export PATH="$SDK_ROOT/platform-tools:$SDK_ROOT/emulator:$PATH"

if [[ ! -x "$ADB" ]]; then
  echo "error: adb not found at $ADB" >&2
  exit 2
fi

echo "[1/4] Building + signing test APK ($ANDROID_ABI)"
ANDROID_ABI="$ANDROID_ABI" bash "$ROOT_DIR/tools/build_android_testapp.sh" >/dev/null
APK="$ROOT_DIR/target/android-testapp-$ANDROID_ABI/app-signed.apk"
if [[ ! -f "$APK" ]]; then
  echo "error: expected APK not produced: $APK" >&2
  exit 1
fi
echo "      $APK"

# Select the target device. An explicit ANDROID_SERIAL always wins; this is how
# a physical phone is used without booting an emulator. Otherwise, if exactly
# one device is attached, use it. Only when nothing is attached do we boot an
# emulator (which needs KVM and a system image).
ADB_ARGS=()
if [[ -n "${ANDROID_SERIAL:-}" ]]; then
  ADB_ARGS=(-s "$ANDROID_SERIAL")
  echo "[2/4] Targeting device ANDROID_SERIAL=$ANDROID_SERIAL"
else
  mapfile -t ATTACHED < <("$ADB" devices | awk 'NR>1 && $2=="device" {print $1}')
  if [[ "${#ATTACHED[@]}" -eq 1 ]]; then
    ADB_ARGS=(-s "${ATTACHED[0]}")
    echo "[2/4] Using the only attached device: ${ATTACHED[0]}"
  elif [[ "${#ATTACHED[@]}" -gt 1 ]]; then
    echo "error: multiple devices attached; set ANDROID_SERIAL to choose one:" >&2
    printf '  %s\n' "${ATTACHED[@]}" >&2
    exit 2
  fi
fi

# Resolve the physical-vs-emulator distinction once, for the boot step and the
# final report. With ADB_ARGS empty we manage an emulator ourselves.
TARGET_SERIAL=""
if [[ "${#ADB_ARGS[@]}" -gt 0 ]]; then
  TARGET_SERIAL="${ADB_ARGS[1]}"
fi
IS_EMULATOR="no"
if [[ -n "$TARGET_SERIAL" ]]; then
  case "$TARGET_SERIAL" in
    emulator-*) IS_EMULATOR="yes" ;;
  esac
fi

if [[ -z "$TARGET_SERIAL" ]]; then
  echo "      no device attached; booting emulator $AVD"
  if ! "$SDK_ROOT/cmdline-tools/latest/bin/avdmanager" list avd 2>/dev/null | grep -q "Name: $AVD"; then
    echo "      creating AVD $AVD from $SYS_IMG"
    echo "no" | "$SDK_ROOT/cmdline-tools/latest/bin/avdmanager" create avd \
      -n "$AVD" -k "$SYS_IMG" -d pixel_5 --force >/dev/null
  fi
  nohup "$EMULATOR" -avd "$AVD" -no-window -no-audio -no-boot-anim \
    -gpu swiftshader_indirect -no-snapshot > /tmp/emu_${AVD}.log 2>&1 &
  "$ADB" wait-for-device
  TARGET_SERIAL="$("$ADB" get-serialno)"
  ADB_ARGS=(-s "$TARGET_SERIAL")
  IS_EMULATOR="yes"
fi

if [[ "$IS_EMULATOR" == "yes" ]]; then
  echo -n "      waiting for boot"
  for _ in $(seq 1 90); do
    if [[ -n "$("$ADB" "${ADB_ARGS[@]}" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" ]]; then
      echo " done"
      break
    fi
    echo -n "."
    sleep 2
  done
else
  echo "      physical device; skipping boot wait"
fi

# The APK ABI must match the device's preferred ABI, otherwise the native
# library fails to load at runtime and the test would fail for the wrong reason.
DEVICE_ABI="$("$ADB" "${ADB_ARGS[@]}" shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r')"
if [[ -n "$DEVICE_ABI" ]]; then
  echo "      target: serial=$TARGET_SERIAL abi=$DEVICE_ABI emulator=$IS_EMULATOR"
  if [[ "$DEVICE_ABI" != "$ANDROID_ABI" ]]; then
    echo "warning: APK ABI ($ANDROID_ABI) != device ABI ($DEVICE_ABI);" >&2
    echo "         rebuild with ANDROID_ABI=$DEVICE_ABI" >&2
  fi
fi

echo "[3/4] Installing and launching"
"$ADB" "${ADB_ARGS[@]}" uninstall rust_widgets.testapp >/dev/null 2>&1 || true

# Install the APK. MIUI/HyperOS rejects the plain streamed `adb install` path
# with INSTALL_FAILED_USER_RESTRICTED even when USB debugging is allowed, so
# fall back to the session API (`pm install-create/-write/-commit`), which goes
# through a different code path and succeeds on those devices.
install_apk() {
  if "$ADB" "${ADB_ARGS[@]}" install -r "$APK" 2>&1 | tee /tmp/rw_install.log | grep -q "Success"; then
    return 0
  fi
  echo "      plain install failed; retrying via pm install session" >&2

  local size
  size=$(stat -c %s "$APK")
  local remote="/data/local/tmp/rw-testapp.apk"
  "$ADB" "${ADB_ARGS[@]}" push "$APK" "$remote" >/dev/null || return 1

  local out sid
  out=$("$ADB" "${ADB_ARGS[@]}" shell pm install-create -r -t -S "$size" 2>&1)
  sid=$(echo "$out" | sed -n 's/.*\[\([0-9]*\)\].*/\1/p')
  if [[ -z "$sid" ]]; then
    echo "      could not create install session: $out" >&2
    return 1
  fi
  "$ADB" "${ADB_ARGS[@]}" shell pm install-write -S "$size" "$sid" base "$remote" >/dev/null || return 1
  "$ADB" "${ADB_ARGS[@]}" shell rm -f "$remote" >/dev/null 2>&1 || true
  "$ADB" "${ADB_ARGS[@]}" shell pm install-commit "$sid" 2>&1 | tee /tmp/rw_install.log | grep -q "Success"
}

if ! install_apk; then
  echo "❌ APK install failed:" >&2
  cat /tmp/rw_install.log >&2 || true
  echo "   On MIUI/HyperOS this usually means the device still needs" >&2
  echo "   'Install via USB' enabled in Developer options." >&2
  exit 1
fi

"$ADB" "${ADB_ARGS[@]}" logcat -c
"$ADB" "${ADB_ARGS[@]}" shell am start -n rust_widgets.testapp/.MainActivity >/dev/null
sleep 8

echo "[4/4] Checking result in logcat"
RESULT="$("$ADB" "${ADB_ARGS[@]}" logcat -d -s RustWidgetsTest:* 2>/dev/null | tail -40)"
echo "$RESULT"

if echo "$RESULT" | grep -q "RESULT: PASS"; then
  echo "✅ Android JNI integration test PASSED ($ANDROID_ABI, serial=$TARGET_SERIAL, emulator=$IS_EMULATOR)"
  exit 0
fi

echo "❌ Android JNI integration test did not report PASS" >&2
echo "--- native log ---" >&2
"$ADB" "${ADB_ARGS[@]}" logcat -d 2>/dev/null | grep -i "android-jni" | tail -40 >&2 || true
exit 1
