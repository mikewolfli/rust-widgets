#!/usr/bin/env bash
# The Android probe must run on a real device or emulator, not merely compile.
#
# # Why this gate exists
#
# `check_android_cross.sh` compiles the four Android ABIs, and `check_profiles.sh` compiles the
# JNI feature set. Both are **type** checks: they prove the code builds, and say nothing about
# whether anyone can run it. Measured consequence — with every compile gate green, the actual
# runner (`tools/run_android_testapp.sh`) could not complete a single step on a macOS host:
#
#   1. `build_android_testapp.sh` hardcoded the NDK toolchain directory as
#      `prebuilt/linux-x86_64`, so on macOS the linker was reported missing
#      (`.../linux-x86_64/bin/aarch64-linux-android24-clang not found`) even though
#      `prebuilt/darwin-x86_64/bin/` held exactly that binary.
#   2. The JDK was resolved as `command -v javac`, which picked a JDK 11 while `avdmanager`
#      requires 17+. Android Studio's bundled JBR — installed and new enough — was never
#      consulted because the older `javac` came first on `PATH`.
#   3. The debug-signing step read `$JAVA_HOME/bin/keytool` while `JAVA_HOME` was unset, so
#      `set -u` aborted the build with `JAVA_HOME: unbound variable`.
#   4. The device-selection step used `mapfile`, a bash 4+ builtin. macOS ships bash 3.2, so
#      the runner died with `mapfile: command not found` on a stock install.
#   5. The system-image package was hardcoded to `android-34`, which this machine did not
#      have, while `android-36.1` sat right beside it — so AVD creation failed with
#      "package path is not valid" and the failure read as "no emulator available".
#
# Every one of those is invisible to a compile gate and fatal to the first real run. That is the
# same asymmetry `check_apple_native.sh` was written for on the Apple side, and this is its
# Android counterpart.
#
# # What it asserts
#
#   [1] `build_android_testapp.sh` produces a signed APK
#   [2] an emulator (or an attached device) accepts it and the activity reports `RESULT: PASS`
#
# # Host requirement
#
# Needs `ANDROID_SDK_ROOT` with platform-tools, an emulator or system image, and a JDK 17+. A host
# without them prints `unsupported host` so the runner classifies it as skipped rather than failed
# — a limitation of the machine, not a defect in the code.
#
#   ANDROID_SDK_ROOT   SDK root (default: whatever the scripts resolve)
#   ANDROID_AVD        AVD to boot (default: the first installed one, else created)
#   ANDROID_SERIAL     attached device; skips emulator boot entirely
#
# Usage: tools/check_android_runtime.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# Resolve the SDK the same way the scripts under test do, so the gate's own "can this host run
# it?" answer cannot disagree with theirs.
SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
if [[ ! -d "$SDK_ROOT" && -d "$HOME/Library/Android/sdk" ]]; then
  SDK_ROOT="$HOME/Library/Android/sdk"
fi

if [[ ! -x "$SDK_ROOT/platform-tools/adb" ]]; then
  echo "check_android_runtime: unsupported host (no adb under '$SDK_ROOT')" >&2
  echo "  set ANDROID_SDK_ROOT to an Android SDK with platform-tools installed" >&2
  exit 2
fi
if [[ ! -x "$SDK_ROOT/emulator/emulator" && -z "${ANDROID_SERIAL:-}" ]]; then
  echo "check_android_runtime: unsupported host (no emulator binary and no ANDROID_SERIAL)" >&2
  exit 2
fi

# A boot plus an install plus a launch is the long pole; bound it so a wedged emulator reports a
# timeout instead of hanging the whole run.
ANDROID_PROBE_TIMEOUT=2400

# If no device is attached, pick an installed AVD rather than letting the runner invent a name and
# then fail to find a system image for it. `ANDROID_AVD` still wins when set.
if [[ -z "${ANDROID_SERIAL:-}" && -z "${ANDROID_AVD:-}" ]]; then
  EXISTING="$("$SDK_ROOT/emulator/emulator" -list-avds 2>/dev/null | head -1 || true)"
  if [[ -z "$EXISTING" ]]; then
    echo "check_android_runtime: unsupported host (no AVD installed and no ANDROID_SERIAL)" >&2
    echo "  create one with: sdkmanager 'system-images;android-34;google_apis;\$(uname -m)'" >&2
    exit 2
  fi
  export ANDROID_AVD="$EXISTING"
fi

echo "=== [1/2] Build + run the Android JNI integration probe ==="
echo "      SDK=$SDK_ROOT${ANDROID_AVD:+ AVD=$ANDROID_AVD}${ANDROID_SERIAL:+ serial=$ANDROID_SERIAL}"
rw_run_bounded "$ANDROID_PROBE_TIMEOUT" bash "$ROOT_DIR/tools/run_android_testapp.sh"

echo ""
echo "=== [2/2] The runtime probe reported PASS ==="
echo "      (see the RESULT line above; the runner exits non-zero if it is not PASS)"
echo ""
echo "check_android_runtime: ALL Android runtime checks PASSED"
