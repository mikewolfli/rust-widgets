# Android Activity Integration (BLUE11 R2.5)

## Current Status
The Android backend exists as a JNI bridge (`android_jni.rs`)
with native method declarations but has NOT been validated
against a real Android app.

## Required Steps for End-to-End Validation

1. Create an Android library project that links rust_widgets
2. Add JNI native method declarations in Kotlin:
   ```kotlin
   object RustWidgets {
       init { System.loadLibrary("rust_widgets") }
       external fun nativeInit()
       external fun nativeCreateButton(
           context: android.content.Context, text: String,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateTextView(
           context: android.content.Context, text: String,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateEditText(
           context: android.content.Context, text: String,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateCheckBox(
           context: android.content.Context, text: String,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateRadioButton(
           context: android.content.Context, text: String,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateProgressBar(
           context: android.content.Context,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeCreateSeekBar(
           context: android.content.Context,
           x: Int, y: Int, w: Int, h: Int
       ): Long
       external fun nativeSetViewText(nativePtr: Long, text: String)
       external fun nativeSetViewBounds(
           nativePtr: Long, x: Int, y: Int, w: Int, h: Int
       )
       external fun nativeSetViewVisibility(nativePtr: Long, visible: Boolean)
       external fun nativeSetViewEnabled(nativePtr: Long, enabled: Boolean)
       external fun nativeDestroyView(nativePtr: Long)
   }
   ```
3. Create an Activity that calls RustWidgets methods
4. Verify JNI method signatures match android_jni.rs
5. Test on Android emulator (API 33+)

## Current Native Methods

The following native methods are declared in `src/platform/android_jni.rs`:

| # | JNI Symbol | Rust Signature | Java Equivalent |
|---|-----------|---------------|-----------------|
| 1 | `Java_rust_1widgets_RustWidgets_nativeInit` | `fn(env, _class)` → `void` | `native void nativeInit()` |
| 2 | `Java_rust_1widgets_RustWidgets_nativeCreateButton` | `fn(env, _class, context, text, x, y, w, h)` → `jlong` | `native long nativeCreateButton(Context, String, int, int, int, int)` |
| 3 | `Java_rust_1widgets_RustWidgets_nativeCreateTextView` | `fn(env, _class, context, text, x, y, w, h)` → `jlong` | `native long nativeCreateTextView(Context, String, int, int, int, int)` |
| 4 | `Java_rust_1widgets_RustWidgets_nativeCreateEditText` | `fn(env, _class, context, text, x, y, w, h)` → `jlong` | `native long nativeCreateEditText(Context, String, int, int, int, int)` |
| 5 | `Java_rust_1widgets_RustWidgets_nativeCreateCheckBox` | `fn(env, _class, context, text, x, y, w, h)` → `jlong` | `native long nativeCreateCheckBox(Context, String, int, int, int, int)` |
| 6 | `Java_rust_1widgets_RustWidgets_nativeCreateRadioButton` | `fn(env, _class, context, text, x, y, w, h)` → `jlong` | `native long nativeCreateRadioButton(Context, String, int, int, int, int)` |
| 7 | `Java_rust_1widgets_RustWidgets_nativeCreateProgressBar` | `fn(env, _class, context, x, y, w, h)` → `jlong` | `native long nativeCreateProgressBar(Context, int, int, int, int)` |
| 8 | `Java_rust_1widgets_RustWidgets_nativeCreateSeekBar` | `fn(env, _class, context, x, y, w, h)` → `jlong` | `native long nativeCreateSeekBar(Context, int, int, int, int)` |
| 9 | `Java_rust_1widgets_RustWidgets_nativeSetViewText` | `fn(env, _class, native_ptr, text)` → `void` | `native void nativeSetViewText(long, String)` |
| 10 | `Java_rust_1widgets_RustWidgets_nativeSetViewBounds` | `fn(env, _class, native_ptr, x, y, w, h)` → `void` | `native void nativeSetViewBounds(long, int, int, int, int)` |
| 11 | `Java_rust_1widgets_RustWidgets_nativeSetViewVisibility` | `fn(env, _class, native_ptr, visible)` → `void` | `native void nativeSetViewVisibility(long, boolean)` |
| 12 | `Java_rust_1widgets_RustWidgets_nativeSetViewEnabled` | `fn(env, _class, native_ptr, enabled)` → `void` | `native void nativeSetViewEnabled(long, boolean)` |
| 13 | `Java_rust_1widgets_RustWidgets_nativeDestroyView` | `fn(env, _class, native_ptr)` → `void` | `native void nativeDestroyView(long)` |

## Blocker Details

### No Android NDK CI Build
- The `android-jni` feature requires `jni` crate and Android NDK toolchain.
- CI (GitHub Actions / GitLab CI) currently has no Android target or NDK step.
- A cross-compilation target (e.g. `aarch64-linux-android`) must be added.

### No Automated JNI Signature Validation
- The JNI method names and signatures in `android_jni.rs` must exactly match the Java `native` declarations.
- Mismatches cause `UnsatisfiedLinkError` at runtime.
- A unit test or build script that parses both sides and compares signatures would catch drift.

### No Real Device/Emulator Test Harness
- No Android instrumentation test project exists.
- No Gradle / Android Studio project that links `librust_widgets.so`.
- Manual testing on API 33+ emulator has not been performed.

## Next Steps

1. Set up Android CI with NDK (android-ndk r26+) and target `aarch64-linux-android`.
2. Write an Android test app with `RustWidgets` Kotlin object and an `Activity`.
3. Add a `build.rs` or `xtask` that cross-compiles for Android.
4. Run `adb` instrumentation tests.
