#!/usr/bin/env bash
# ============================================================================
# build_ios_testapp.sh — build the iOS Simulator integration-test app bundle
# ============================================================================
# Apple analogue of `tools/build_android_testapp.sh`. Builds a real, installable
# `.app` for the iOS Simulator without Xcode project files, using the toolchain
# that ships with Xcode directly:
#
#   1. cargo build the Rust library as a staticlib for the simulator target
#   2. clang the Objective-C host (bindings/ios/main.m) against the iOS SDK
#   3. link the host + librust_widgets.a + required system frameworks
#   4. assemble the .app bundle (binary + Info.plist)
#
# Requires: Xcode (xcrun/clang) and the Rust target
# `aarch64-apple-ios-sim` (Apple Silicon) or `x86_64-apple-ios` (Intel).
#
# Environment overrides:
#   IOS_SIM_ARCH   arm64 (default) | x86_64
#   IOS_DEPLOY_MIN minimum deployment target (default: 15.0)
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: build_ios_testapp.sh requires macOS (found $(uname -s))" >&2
  exit 2
fi

IOS_SIM_ARCH="${IOS_SIM_ARCH:-$(uname -m)}"
case "$IOS_SIM_ARCH" in
  arm64)  RUST_TARGET="aarch64-apple-ios-sim" ;;
  x86_64) RUST_TARGET="x86_64-apple-ios" ;;
  *)
    echo "error: unsupported IOS_SIM_ARCH '$IOS_SIM_ARCH' (use arm64 or x86_64)" >&2
    exit 2
    ;;
esac
IOS_DEPLOY_MIN="${IOS_DEPLOY_MIN:-15.0}"

APP_DIR="$ROOT_DIR/bindings/ios"
OUT_DIR="$ROOT_DIR/target/ios-testapp-$IOS_SIM_ARCH"
APP_BUNDLE="$OUT_DIR/rwiosprobe.app"
SDK_PATH="$(xcrun --sdk iphonesimulator --show-sdk-path)"
CLANG="$(xcrun --sdk iphonesimulator --find clang)"

if ! rustup target list --installed | grep -qx "$RUST_TARGET"; then
  echo "error: Rust target '$RUST_TARGET' is not installed." >&2
  echo "       run: rustup target add $RUST_TARGET" >&2
  exit 2
fi

echo "[1/4] Building librust_widgets.a for $RUST_TARGET"
# Build the crate as a `staticlib` so the Objective-C host can link a single
# self-contained archive (an rlib cannot be linked by clang directly because it
# still references Rust std and the transitive dependency rlibs).
#
# # Why the profile is `mobile` and not `desktop`
#
# This asked for `desktop` because it had to name *some* device profile — `build.rs` derives
# `full_widgets` from "a profile is on AND the widget set is not stripped", and `mobile-api`
# is an empty marker that satisfies neither half. Without a profile, `mount_widget_of_kind`
# returns `0` for every kind and `rw_create_window` produces nothing. The Android build made
# exactly that mistake and its on-device widget path was dead for it (see
# `build_android_testapp.sh`); this one happened to dodge the symptom by naming the **wrong**
# profile, so it worked while compiling the desktop surface — window chrome, print, PDF, CJK
# faces — into an iOS phone binary.
#
# `mobile` is the profile that describes the target, so the probe now links the surface an
# iOS app actually ships rather than a desktop build that happens to boot.
#
# `ios-uikit-ffi` selects the Apple-mobile backend; `controls-custom` is the one creation
# mechanism (every `WidgetKind` is library-painted).
cargo rustc --lib --target "$RUST_TARGET" --no-default-features \
  --crate-type staticlib \
  --features "mobile,ios-uikit-ffi,controls-custom,controls-native"

STATIC_LIB="$ROOT_DIR/target/$RUST_TARGET/debug/librust_widgets.a"
if [[ ! -f "$STATIC_LIB" ]]; then
  echo "error: $STATIC_LIB not produced by cargo rustc" >&2
  exit 2
fi

echo "[2/4] Compiling Objective-C host"
rm -rf "$OUT_DIR"
mkdir -p "$APP_BUNDLE"

"$CLANG" -arch "$IOS_SIM_ARCH" -isysroot "$SDK_PATH" \
  -mios-simulator-version-min="$IOS_DEPLOY_MIN" \
  -fobjc-arc -fmodules -O0 -g \
  -I"$ROOT_DIR/examples" \
  -c "$APP_DIR/main.m" -o "$OUT_DIR/main.o"

echo "[3/4] Linking app"
"$CLANG" -arch "$IOS_SIM_ARCH" -isysroot "$SDK_PATH" \
  -mios-simulator-version-min="$IOS_DEPLOY_MIN" \
  -fobjc-arc \
  "$OUT_DIR/main.o" "$STATIC_LIB" \
  -framework UIKit -framework Foundation -framework CoreGraphics \
  -framework Security -framework CoreVideo -framework CoreMedia \
  -framework Metal -framework QuartzCore -framework CoreText \
  -framework ImageIO -framework AudioToolbox -framework AVFoundation \
  -framework Accelerate \
  -lSystem -lc++ -lz -lbz2 -liconv -lresolv -framework CoreFoundation \
  -o "$APP_BUNDLE/rwiosprobe"

echo "[4/4] Assembling .app bundle"
cp "$APP_DIR/Info.plist" "$APP_BUNDLE/Info.plist"

if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$APP_BUNDLE" >/dev/null 2>&1 || true
fi

echo "App bundle: $APP_BUNDLE"
ls -la "$APP_BUNDLE"
echo "Run with: bash tools/run_ios_testapp.sh"
