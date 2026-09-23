#!/usr/bin/env bash
# Every Android ABI the build script ships must compile (BLUE19 T-11).
#
# # Why this gate exists — and what it found on its first run
#
# `tools/build_android_testapp.sh` builds the APK for `aarch64-linux-android` only, and no gate
# compiled the Android target at all. The result was a real defect that had been sitting in the tree:
#
#   src/platform/android_jni.rs: `android_log_write(prio, tag: *const i8, text: *const i8)`
#
# `CString::as_ptr()` yields `*const c_char`, and the **sign of `c_char` is target-dependent** —
# `i8` on x86_64, `u8` on aarch64. So this compiled if anyone ever checked `x86_64-linux-android`
# and failed on the ABI the APK actually ships:
#
#   error[E0308]: mismatched types
#   expected `*const i8`, found `*const u8`
#
# A host-only `cargo check` cannot see this: the `android-jni` feature set is what pulls the module
# in, and the sign difference only appears on the 64-bit ARM target. That is the same class as the
# `mini`×backend matrix `check_profiles.sh` step [9b] was added for — a code path no job compiled.
#
# # What it asserts
#
# The Android feature set `build_android_testapp.sh` uses, compiled **for each of the four ABIs the
# NDK supports**. All four, not just the shipped one: a target-specific break in `c_char`, `size_of`
# or a pointer width shows up on the ABIs that are *not* the primary one, and checking only arm64
# would have left the `i8`/`u8` split undiscovered on the other three.
#
# # Host requirement
#
# The targets come from `rustup target add`, so a missing one is reported rather than silently
# skipped — a check that quietly does nothing is worse than no check.
#
# Usage: tools/check_android_cross.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

. "$ROOT_DIR/tools/lib_timeout.sh"

# One `cargo check` per ABI. Bounded per rule #58.
GATE_TIMEOUT="${GATE_TIMEOUT:-900}"

TARGETS=(
  aarch64-linux-android
  armv7-linux-androideabi
  i686-linux-android
  x86_64-linux-android
)

# The feature set `tools/build_android_testapp.sh` passes. It has **no device profile**, so
# `crate::theme` and `crate::json` are compiled out while `src/bindings/` is compiled in — the
# combination `check_profiles.sh` step [6c/9] calls out as historically broken. Reusing the same set
# here means this gate covers the build script's actual configuration rather than a convenient one.
FEATURES="android-jni jni mobile-api controls-custom controls-native serde serde_json"

# `ANDROID_HOME` is exported when the SDK is in a standard location, because `cargo check` for an
# Android target resolves the NDK's linker search path from it. The check does not link, but a host
# that has the SDK should not have to set the variable by hand just to run this gate.
#
# Both conventional locations are probed: macOS puts the SDK under `Library/Android/sdk`, while
# Linux uses `Android/Sdk`. Checking only the Linux path left a macOS host with a perfectly good
# SDK reporting "no SDK" — the directory was there, the gate looked in the wrong place.
if [[ -z "${ANDROID_HOME:-}" ]]; then
  for candidate in "$HOME/Library/Android/sdk" "$HOME/Android/Sdk"; do
    if [[ -d "$candidate" ]]; then
      export ANDROID_HOME="$candidate"
      break
    fi
  done
fi

MISSING=()
for target in "${TARGETS[@]}"; do
  if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
    MISSING+=("$target")
  fi
done
if [[ "${#MISSING[@]}" -gt 0 ]]; then
  # `unsupported host` is the marker `tools/run_all_gates.sh` classifies on. Without it this
  # gate reports FAIL on a host that simply has no Android toolchain — a *host* limitation
  # counted as a defect in the code under test. The runner's own doc says the two must not be
  # conflated (principle #59.4), and the marker is how the distinction is expressed.
  echo "unsupported host: these Android targets are not installed, so this gate cannot check them:" >&2
  for target in "${MISSING[@]}"; do
    echo "  - $target" >&2
  done
  echo "" >&2
  echo "Install them with:" >&2
  echo "  rustup target add ${MISSING[*]}" >&2
  exit 1
fi

echo "=== Android ABIs: ${#TARGETS[@]} target(s) ==="
for target in "${TARGETS[@]}"; do
  echo "--- $target"
  if ! rw_run_bounded "$GATE_TIMEOUT" cargo check \
      --target "$target" --no-default-features --features "$FEATURES"; then
    echo "" >&2
    echo "FAIL: \`$target\` does not compile with the APK's feature set." >&2
    echo "      A target-specific break (a \`c_char\` sign, a pointer width, an alignment) shows up" >&2
    echo "      here and nowhere else: the host build has a different answer for all three." >&2
    exit 1
  fi
done

echo ""
echo "android cross-target checks passed."
