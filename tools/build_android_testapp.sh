#!/usr/bin/env bash
# ============================================================================
# build_android_testapp.sh — build the Android JNI integration-test APK
# ============================================================================
# Builds a real, installable APK without Gradle, using the Android SDK
# build-tools directly:
#
#   1. cargo build the Rust library for aarch64-linux-android (cdylib)
#   2. javac the app + bridge sources against android.jar
#   3. d8 the class files to DEX
#   4. aapt2 compile/link resources + manifest, injecting the DEX
#   5. zip the native .so into lib/arm64-v8a/ and zipalign
#
# Requires:
#   ANDROID_SDK_ROOT (or ~/Android/Sdk), NDK under $ANDROID_SDK_ROOT/ndk/<ver>
#   An API 34 android.jar and build-tools with aapt2/d8/zipalign.
#
# Environment overrides:
#   ANDROID_SDK_ROOT   Android SDK root (default: $HOME/Android/Sdk)
#   ANDROID_NDK_HOME   NDK root (default: newest under $ANDROID_SDK_ROOT/ndk)
#   ANDROID_JAR_LEVEL  platform API level for android.jar (default: 34)
#   ANDROID_ABI        target ABI: arm64-v8a (default), x86_64, armeabi-v7a
#   JAVA_HOME          JDK to use (default: Android Studio's bundled jbr)
# ============================================================================

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
. "$ROOT_DIR/tools/lib_python.sh"
. "$ROOT_DIR/tools/lib_jdk.sh"

SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
# macOS installs the SDK under `~/Library/Android/sdk` — a different layout from the Linux
# default above, and a different spelling (`Sdk` vs `sdk`) from the env var's own name.
# Falling back to it means the documented invocation
# (`ANDROID_SDK_ROOT=... bash tools/...`) is not needed on a stock macOS install, which is
# what made this path look Linux-only in the first place.
if [[ ! -d "$SDK_ROOT" && -d "$HOME/Library/Android/sdk" ]]; then
  SDK_ROOT="$HOME/Library/Android/sdk"
fi
NDK_ROOT="${ANDROID_NDK_HOME:-$(ls -d "$SDK_ROOT"/ndk/* 2>/dev/null | sort -V | tail -1)}"
API_LEVEL="${ANDROID_JAR_LEVEL:-34}"
ANDROID_JAR="$SDK_ROOT/platforms/android-$API_LEVEL/android.jar"
BUILD_TOOLS="$(ls -d "$SDK_ROOT"/build-tools/* 2>/dev/null | sort -V | tail -1)"
APP_DIR="$ROOT_DIR/bindings/android"

# Map the desired APK ABI to the Rust target triple, NDK clang prefix, and the
# jniLibs subdirectory Android expects inside the APK.
ANDROID_ABI="${ANDROID_ABI:-arm64-v8a}"
case "$ANDROID_ABI" in
  arm64-v8a)   RUST_TARGET="aarch64-linux-android"; NDK_TRIPLE="aarch64-linux-android" ;;
  x86_64)      RUST_TARGET="x86_64-linux-android";  NDK_TRIPLE="x86_64-linux-android"  ;;
  armeabi-v7a) RUST_TARGET="armv7-linux-androideabi"; NDK_TRIPLE="armv7a-linux-androideabi" ;;
  *)
    echo "error: unsupported ANDROID_ABI '$ANDROID_ABI'" >&2
    exit 2
    ;;
esac

OUT_DIR="$ROOT_DIR/target/android-testapp-$ANDROID_ABI"
LIB_SO="$ROOT_DIR/target/$RUST_TARGET/debug/librust_widgets.so"

# Locate a JDK the Android build tools accept. See `tools/lib_jdk.sh` for why the bundled
# JBRs are preferred over `PATH` and why a version check happens here.
rw_resolve_jdk

for tool in "$ANDROID_JAR" "$BUILD_TOOLS/aapt2" "$BUILD_TOOLS/d8" "$BUILD_TOOLS/zipalign"; do
  if [[ ! -e "$tool" ]]; then
    echo "error: missing required tool: $tool" >&2
    exit 2
  fi
done

# The NDK ships one prebuilt toolchain per **host**, and its directory name is not
# the same as the host's rust triple: `linux-x86_64`, `darwin-x86_64`, `windows-x86_64`.
#
# This was hardcoded to `linux-x86_64`, so the whole Android verification path could
# only ever run on Linux — on macOS the linker was reported as missing:
#
#   error: linker `.../prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang` not found
#
# even though `prebuilt/darwin-x86_64/bin/` held exactly that binary. A script that
# verifies a cross-platform library must not itself be single-host, and the fix is to
# read the tag from the host rather than to assume one.
case "$(uname -s)" in
  Darwin) NDK_HOST_TAG="darwin-x86_64" ;;
  Linux)  NDK_HOST_TAG="linux-x86_64"  ;;
  MINGW*|MSYS*|CYGWIN*) NDK_HOST_TAG="windows-x86_64" ;;
  *)
    echo "error: unsupported host '$(uname -s)' for the Android NDK toolchain" >&2
    exit 2
    ;;
esac

# Apple silicon runs the x86_64 prebuilt under Rosetta, which is what the NDK
# publishes; there is no `darwin-arm64` toolchain directory in current releases. If a
# future NDK ships one, prefer it rather than relying on Rosetta silently.
if [[ "$NDK_HOST_TAG" == "darwin-x86_64" ]] && [[ -x "$NDK_ROOT/toolchains/llvm/prebuilt/darwin-arm64/bin/clang" ]]; then
  NDK_HOST_TAG="darwin-arm64"
fi

NDK_PREBUILT="$NDK_ROOT/toolchains/llvm/prebuilt/$NDK_HOST_TAG"
if [[ ! -d "$NDK_PREBUILT/bin" ]]; then
  echo "error: the NDK at $NDK_ROOT has no '$NDK_HOST_TAG' toolchain." >&2
  echo "       available: $(ls "$NDK_ROOT/toolchains/llvm/prebuilt" 2>/dev/null | tr '\n' ' ')" >&2
  exit 2
fi

echo "[1/5] Building librust_widgets.so for $RUST_TARGET ($ANDROID_ABI) with $NDK_HOST_TAG"
# # Why `mobile`, and not `mobile-api`
#
# This built with `mobile-api` alone for a long time, and the difference is not cosmetic.
# `mobile-api` is an empty marker feature (`mobile-api = []`); `mobile` is a **device profile**,
# and `build.rs` derives `full_widgets` from "a device profile is on AND the widget set is not
# stripped". `mobile-api` satisfies neither half, so `full_widgets` was **false** in this build
# and `control_backend::custom::mount_widget_of_kind` takes its `#[cfg(not(full_widgets))]`
# arm — which returns `0` for *every* kind, with a warning that the constructor registry is
# not compiled in.
#
# The measured consequence, once the on-device probe was deepened to actually create a widget:
#
#   I RustWidgetsTest: nativeWidgetSelfTest -> 0 (0b0)
#   E rust_widgets: [android-jni] widget self-test: create_window returned 0
#
# Every `create_*` on Android returned `0`. Nothing in the tree noticed, because the probe
# exercised only the JNI plumbing and every compile gate is satisfied by an empty registry:
# the code builds, warns, and creates nothing. See `check_android_runtime.sh`.
NDK_BIN="$NDK_PREBUILT/bin"
export CARGO_TARGET_$(echo "$RUST_TARGET" | tr 'a-z-' 'A-Z_')_LINKER="$NDK_BIN/${NDK_TRIPLE}24-clang"
export "CC_${RUST_TARGET//-/_}=$NDK_BIN/${NDK_TRIPLE}24-clang"
export "AR_${RUST_TARGET//-/_}=$NDK_BIN/llvm-ar"
cargo build --lib --target "$RUST_TARGET" --no-default-features \
  --features "android-jni jni mobile controls-custom controls-native serde serde_json"

echo "[2/5] Compiling Java sources"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/classes" "$OUT_DIR/dex" "$OUT_DIR/lib/$ANDROID_ABI"

# javac's status has to be the pipeline's status.
#
# # Why the obvious spelling is wrong
#
# This used to be:
#
#   "$JAVAC" ... 2>&1 | grep -v "bootstrap class path" || true
#
# which reads as "compile, quieten one noisy warning". It is not. A pipeline's status is the
# **last** command's, so `grep`'s (or `true`'s) status replaced javac's, and under `set -e`
# a **failed compile continued to the next step**. The consequence was measured: a Javadoc
# block closed early by a stray `*` in a comment made javac emit `错误: 非法的类型开始`,
# the script printed no error, built an APK with no classes in it, and the failure surfaced
# three steps later as `INSTALL_FAILED_INVALID_APK: … code is missing` — a message that
# describes the symptom and points nowhere near the cause.
#
# So the output goes to a file, the status is captured from javac itself (`PIPESTATUS` is
# bash-specific and this script runs under `sh` in places), the filtered view is printed, and
# a non-zero status aborts with the unfiltered log so the real line is visible.
JAVAC_LOG="$OUT_DIR/javac.log"
if ! "$JAVAC" -source 8 -target 8 -bootclasspath "$ANDROID_JAR" -classpath "$ANDROID_JAR" \
  -d "$OUT_DIR/classes" \
  "$APP_DIR/java/rust_widgets/RustWidgets.java" \
  "$APP_DIR/java/rust_widgets/testapp/MainActivity.java" > "$JAVAC_LOG" 2>&1; then
  echo "error: javac failed; full output follows" >&2
  cat "$JAVAC_LOG" >&2
  exit 1
fi
# The `-source 8` deprecation notes and the bootclasspath notice are noise on every run; the
# errors are not, and they are already reported by the branch above.
grep -v -e "bootstrap class path" -e "已过时" -e "deprecat" "$JAVAC_LOG" || true

echo "[3/5] Dexing"
# d8's --output must be an existing directory; it writes classes.dex inside it.
"$BUILD_TOOLS/d8" --min-api 24 --output "$OUT_DIR/dex" \
  $(find "$OUT_DIR/classes" -name '*.class')

echo "[4/5] Linking resources + manifest (aapt2)"
mkdir -p "$OUT_DIR/res"
"$BUILD_TOOLS/aapt2" link \
  -o "$OUT_DIR/app-unsigned.apk" \
  -I "$ANDROID_JAR" \
  --manifest "$APP_DIR/AndroidManifest.xml" \
  --min-sdk-version 24 \
  --target-sdk-version "$API_LEVEL" >/dev/null

echo "[5/5] Adding classes.dex + native lib, then aligning"
cp "$LIB_SO" "$OUT_DIR/lib/$ANDROID_ABI/librust_widgets.so"
# aapt2 cannot take DEX as input, so insert classes.dex and lib/ into the
# archive after linking (this is what the AGP packaging step does).
"$PYTHON" - "$OUT_DIR/app-unsigned.apk" "$OUT_DIR/dex" "$OUT_DIR/lib" <<'PY'
import os, sys, zipfile
apk, dexdir, libdir = sys.argv[1], sys.argv[2], sys.argv[3]
with zipfile.ZipFile(apk, "a", zipfile.ZIP_DEFLATED) as z:
    for name in sorted(os.listdir(dexdir)):
        if name.endswith(".dex"):
            z.write(os.path.join(dexdir, name), name)
    for base, _dirs, files in os.walk(libdir):
        for name in files:
            full = os.path.join(base, name)
            rel = os.path.relpath(full, os.path.dirname(libdir))
            z.write(full, rel)
PY
"$BUILD_TOOLS/zipalign" -f 4 "$OUT_DIR/app-unsigned.apk" "$OUT_DIR/app-aligned.apk"

echo "APK written: $OUT_DIR/app-aligned.apk"
ls -la "$OUT_DIR/app-aligned.apk"

# Sign with a throwaway debug keystore so the APK is installable via adb.
# A stable keystore is reused across runs to keep the signature consistent
# (Android rejects updates signed with a different key).
KEYSTORE="${ANDROID_DEBUG_KEYSTORE:-$HOME/.android/rw_debug.keystore}"
# The same resolution order as `javac`/`java` above, for the same reason: `JAVA_HOME`
# may be unset entirely, and the JDK that supplied `javac` is exactly the one that has
# `keytool` beside it. Reading `$JAVA_HOME/bin/keytool` directly aborted the whole build
# with `JAVA_HOME: unbound variable` under `set -u` on any host that did not export it.
KEYTOOL="$(dirname "$JAVAC")/keytool"
if [[ ! -x "$KEYTOOL" ]]; then
  KEYTOOL="$(command -v keytool || true)"
fi
if [[ ! -x "$KEYTOOL" ]]; then
  echo "error: keytool not found (looked beside javac at $JAVAC, then on PATH);" >&2
  echo "       it ships with every JDK, so set JAVA_HOME to one." >&2
  exit 2
fi
if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  "$KEYTOOL" -genkeypair -v -keystore "$KEYSTORE" \
    -storepass android -keypass android -alias androiddebugkey \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1 || true
fi
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" \
  --ks-pass pass:android --key-pass pass:android \
  --ks-key-alias androiddebugkey \
  --out "$OUT_DIR/app-signed.apk" "$OUT_DIR/app-aligned.apk"

# Re-align after signing: apksigner preserves alignment, but verify explicitly.
"$BUILD_TOOLS/zipalign" -c -v 4 "$OUT_DIR/app-signed.apk" >/dev/null 2>&1 \
  && ALIGN_OK="yes" || ALIGN_OK="no"

echo "APK signed:  $OUT_DIR/app-signed.apk (zipalign verified: $ALIGN_OK)"
ls -la "$OUT_DIR/app-signed.apk"
echo "Install with: adb install -r $OUT_DIR/app-signed.apk"
