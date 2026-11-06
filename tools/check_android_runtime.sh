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
# The system image must also match the **host** architecture: `emulator` refuses anything else
# (`Avd's CPU Architecture 'arm64' is not supported by the QEMU2 emulator on x86_64 host`). An
# installed AVD of the wrong architecture is therefore not a usable target, and this gate now
# rules it out before building anything — see the AVD selection below.
#
#   ANDROID_SDK_ROOT   SDK root (default: whatever the scripts resolve)
#   ANDROID_AVD        AVD to boot (default: the first installed AVD this host can boot)
#   ANDROID_ABI        APK ABI (default: the selected AVD's architecture)
#   ANDROID_SERIAL     attached device; skips emulator boot entirely
#
# rw-gate-budget: 2400
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
# timeout instead of hanging the whole run. The runner honours this number because it is declared in
# the `rw-gate-budget` line above `set`: while the runner's own floor (900 s) was the smaller of the
# two, its bound fired first and this budget never applied — the run then reported `TIMEOUT` with no
# probe output instead of this gate's verdict.
ANDROID_PROBE_TIMEOUT=2400

# If no device is attached, pick an installed AVD rather than letting the runner invent a name and
# then fail to find a system image for it. `ANDROID_AVD` still wins when set.
#
# # Why the AVD is chosen by architecture rather than by name
#
# This used to take `emulator -list-avds | head -1`. On this repository's own host that was an
# `arm64-v8a` AVD ahead of a bootable `x86_64` one, and the emulator rejects it on the spot:
#
#   FATAL | Avd's CPU Architecture 'arm64' is not supported by the QEMU2 emulator on x86_64
#           host. System image must match the host architecture.
#
# Nothing read that verdict. The boot step waited its full 180 s for a device that was never going
# to register, so the gate spent 238 s (APK build + dead wait) to end in `unsupported host` — on a
# host that could run it, with a perfectly good `x86_64` AVD sitting right next to the bad one.
# Reading `abi.type` from each AVD's `config.ini` answers the question in milliseconds and makes
# the AVD the gate actually boots one this host can boot.
host_abi_preferences() {
  case "$(uname -m)" in
    x86_64|amd64) printf 'x86_64\nx86\n' ;;
    aarch64|arm64) printf 'arm64-v8a\n' ;;
    *) : ;;
  esac
}

# `abi.type` is what the emulator itself reads to decide whether it can boot an AVD.
avd_abi() {
  local config="$HOME/.android/avd/$1.avd/config.ini"
  [[ -f "$config" ]] || return 0
  sed -n 's/^abi\.type=//p' "$config" | head -1
}

if [[ -z "${ANDROID_SERIAL:-}" && -z "${ANDROID_AVD:-}" ]]; then
  AVD_LIST="$("$SDK_ROOT/emulator/emulator" -list-avds 2>/dev/null || true)"
  EXISTING=""
  while IFS= read -r wanted; do
    [[ -n "$wanted" ]] || continue
    while IFS= read -r candidate; do
      [[ -n "$candidate" ]] || continue
      if [[ "$(avd_abi "$candidate")" == "$wanted" ]]; then
        EXISTING="$candidate"
        break
      fi
    done <<< "$AVD_LIST"
    [[ -n "$EXISTING" ]] && break
  done <<< "$(host_abi_preferences)"

  if [[ -z "$EXISTING" ]]; then
    echo "check_android_runtime: unsupported host (no AVD whose architecture this host can boot)" >&2
    echo "  host architecture: $(uname -m)" >&2
    if [[ -n "$AVD_LIST" ]]; then
      echo "  installed AVDs (name: abi.type):" >&2
      while IFS= read -r candidate; do
        [[ -n "$candidate" ]] || continue
        echo "    - $candidate: $(avd_abi "$candidate")" >&2
      done <<< "$AVD_LIST"
    else
      echo "  no AVD installed" >&2
    fi
    echo "  set ANDROID_AVD or ANDROID_SERIAL to a bootable target, or create one with:" >&2
    echo "    sdkmanager 'system-images;android-34;google_apis;\$(uname -m)'" >&2
    exit 2
  fi
  export ANDROID_AVD="$EXISTING"
  # Build the APK for the architecture the chosen AVD can install. Only when the caller did not
  # ask for a specific ABI — an explicit `ANDROID_ABI` stays authoritative.
  if [[ -z "${ANDROID_ABI:-}" ]]; then
    ANDROID_ABI="$(avd_abi "$EXISTING")"
    [[ -n "$ANDROID_ABI" ]] && export ANDROID_ABI
  fi
  echo "      selected AVD $EXISTING (abi ${ANDROID_ABI:-default}) for host $(uname -m)"
fi

echo "=== [1/2] Build + run the Android JNI integration probe ==="
echo "      SDK=$SDK_ROOT${ANDROID_AVD:+ AVD=$ANDROID_AVD}${ANDROID_SERIAL:+ serial=$ANDROID_SERIAL}"
rw_run_bounded "$ANDROID_PROBE_TIMEOUT" bash "$ROOT_DIR/tools/run_android_testapp.sh"

echo ""
echo "=== [2/2] The runtime probe reported PASS ==="
echo "      (see the RESULT line above; the runner exits non-zero if it is not PASS)"
echo ""
echo "check_android_runtime: ALL Android runtime checks PASSED"
