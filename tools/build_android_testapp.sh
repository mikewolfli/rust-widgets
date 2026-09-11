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

SDK_ROOT="${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}"
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

if [[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/javac" ]]; then
  JAVAC="$JAVA_HOME/bin/javac"
  JAVA="$JAVA_HOME/bin/java"
elif [[ -x "/home/mikeli/Desktop/app/android-studio/jbr/bin/javac" ]]; then
  JAVAC="/home/mikeli/Desktop/app/android-studio/jbr/bin/javac"
  JAVA="/home/mikeli/Desktop/app/android-studio/jbr/bin/java"
else
  JAVAC="$(command -v javac)"
  JAVA="$(command -v java)"
fi

for tool in "$ANDROID_JAR" "$BUILD_TOOLS/aapt2" "$BUILD_TOOLS/d8" "$BUILD_TOOLS/zipalign"; do
  if [[ ! -e "$tool" ]]; then
    echo "error: missing required tool: $tool" >&2
    exit 2
  fi
done

echo "[1/5] Building librust_widgets.so for $RUST_TARGET ($ANDROID_ABI)"
NDK_BIN="$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CARGO_TARGET_$(echo "$RUST_TARGET" | tr 'a-z-' 'A-Z_')_LINKER="$NDK_BIN/${NDK_TRIPLE}24-clang"
export "CC_${RUST_TARGET//-/_}=$NDK_BIN/${NDK_TRIPLE}24-clang"
export "AR_${RUST_TARGET//-/_}=$NDK_BIN/llvm-ar"
cargo build --lib --target "$RUST_TARGET" --no-default-features \
  --features "android-jni jni mobile-api controls-custom controls-native serde serde_json"

echo "[2/5] Compiling Java sources"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/classes" "$OUT_DIR/dex" "$OUT_DIR/lib/$ANDROID_ABI"
"$JAVAC" -source 8 -target 8 -bootclasspath "$ANDROID_JAR" -classpath "$ANDROID_JAR" \
  -d "$OUT_DIR/classes" \
  "$APP_DIR/java/rust_widgets/RustWidgets.java" \
  "$APP_DIR/java/rust_widgets/testapp/MainActivity.java" 2>&1 | grep -v "bootstrap class path" || true

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
python3 - "$OUT_DIR/app-unsigned.apk" "$OUT_DIR/dex" "$OUT_DIR/lib" <<'PY'
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
if [[ ! -f "$KEYSTORE" ]]; then
  mkdir -p "$(dirname "$KEYSTORE")"
  "$JAVA_HOME/bin/keytool" -genkeypair -v -keystore "$KEYSTORE" \
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
