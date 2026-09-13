<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# Android Activity Integration

## Two integration directions

The Android bridge supports both directions between Rust and Java:

### 1. Java → Rust (existing `#[no_mangle]` entry points)

Java code declares `native` methods and calls into Rust. These remain the
canonical entry points for Java-driven creation:

```kotlin
object RustWidgets {
    init { System.loadLibrary("rust_widgets") }
    external fun nativeInit()
    external fun nativeCreateButton(
        context: android.content.Context, text: String,
        x: Int, y: Int, w: Int, h: Int
    ): Long
    // … nativeCreateTextView / nativeCreateEditText / nativeCreateCheckBox /
    //    nativeCreateRadioButton / nativeCreateProgressBar / nativeCreateSeekBar
    external fun nativeSetViewText(nativePtr: Long, text: String)
    external fun nativeSetViewBounds(nativePtr: Long, x: Int, y: Int, w: Int, h: Int)
    external fun nativeSetViewVisibility(nativePtr: Long, visible: Boolean)
    external fun nativeSetViewEnabled(nativePtr: Long, enabled: Boolean)
    external fun nativeDestroyView(nativePtr: Long)
}
```

### 2. Rust → Java (feature `android-jni`)

`platform_impl.rs` drives native view creation directly through
`android_jni::create_native_view`, so the `Platform` trait API constructs real
Android Views without requiring a Java call per widget.

For this direction the bridge needs the host `Context`, supplied in one of two
ways:

- Call `set_activity_context(env, context)` from Java after `nativeInit()`.
- Call the C ABI `rw_mobile_attach_native_view(contextPtr)` (or the Rust
  `mobile_attach_to_native_view(handle)`), passing the Activity's Java object
  pointer. The handle is converted to a `GlobalRef` so it outlives the call.

Until a Context is stored, `AndroidPlatform::jni_available()` is `false` and
creation stays state-backed.

## Method parity table

Each Rust-created view mirrors the class used by the Java entry points:

| Logical kind | Android class | Java entry point | Rust factory |
|---|---|---|---|
| Window / Panel | `FrameLayout` | — | `create_native_view` |
| Button | `Button` | `nativeCreateButton` | `create_native_view` |
| Label / StatusBar | `TextView` | `nativeCreateTextView` | `create_native_view` |
| LineEdit | `EditText` | `nativeCreateEditText` | `create_native_view` |
| CheckBox | `CheckBox` | `nativeCreateCheckBox` | `create_native_view` |
| RadioButton | `RadioButton` | `nativeCreateRadioButton` | `create_native_view` |
| Slider | `SeekBar` | `nativeCreateSeekBar` | `create_native_view` |
| ProgressBar | `ProgressBar` | `nativeCreateProgressBar` | `create_native_view` |
| ComboBox | `Spinner` | — | `create_native_view` + `append_spinner_item` |
| ListBox / ListView | `ListView` | — | `create_native_view` + `append_list_item` |
| ScrollArea | `ScrollView` | — | `create_native_view` |
| SpinBox | `NumberPicker` | — | `create_native_view` |
| MenuBar / Menu / MenuItem | (none) | — | logical handle only |
| ToolBar | (none) | — | logical handle only |
| MessageBox | `AlertDialog` | `nativeSelfTestDialog` | `create_native_dialog` |
| FileDialog | `ACTION_OPEN_DOCUMENT` (Activity operation) | `nativeSelfTestFileDialog` | `launch_file_dialog` |
| ColorDialog / FontDialog | (none) | — | logical handle only (Android ships no picker) |

## File dialog wiring

`create_file_dialog` requests the system picker. Because the bridge stores only a
`Context`, the Activity is recovered by reflection:

1. `instanceof Activity` on the stored object (a non-Activity Context cannot
   start a result launcher, and logs an explicit diagnostic).
2. `startActivityForResult(Intent(ACTION_OPEN_DOCUMENT) + CATEGORY_OPENABLE +
   setType(mime), FILE_DIALOG_REQUEST_CODE)`.

The result arrives at the host Activity's own `onActivityResult`; the bridge does
not intercept it. `nativeAttachContext` should therefore be given the Activity
(the test app passes `this`).

## Required Steps for End-to-End Validation

1. Create an Android library project that links `rust_widgets`.
2. Declare the JNI `native` methods (see above) in a Kotlin `RustWidgets` object.
3. Call `RustWidgets.nativeInit()` and store the Activity Context via
   `set_activity_context` (or `rw_mobile_attach_native_view`). Pass the
   **Activity** (not a bare application Context) so the file dialog can launch a
   result launcher.
4. Verify JNI method names/signatures match `android_jni.rs`
   (`tools/check_jni_signatures.sh`).
5. Test on a device or emulator (API 24+); `tools/run_android_testapp.sh`
   targets either, via `ANDROID_SERIAL` for a physical phone.

## Current verification level

- **Executed on a physical device**: Xiaomi M2102J2SC, Android 13 (API 33),
  arm64-v8a. Signed APK installs, both JNI directions pass, `RESULT: PASS`,
  and `ACTION_OPEN_DOCUMENT` is launched by the Rust-driven file dialog.
- **Executed on an emulator**: API 34, x86_64, KVM. Signed APK installs, both
  JNI directions pass, logcat reports `RESULT: PASS`.
- Compile-verified for `x86_64-linux-android` and `aarch64-linux-android` with
  and without `android-jni`; `cargo clippy` reports 0 warnings for both.
- The class-mapping, readiness, and no-JVM degradation contracts have
  host-runnable unit tests in `android_jni.rs`.
- JNI signatures are validated automatically against both the Rust source and
  the exported symbols of the built `.so`
  (`tools/check_jni_signatures.sh`).

## Running it

```bash
# build + sign (no Gradle needed)
ANDROID_ABI=arm64-v8a bash tools/build_android_testapp.sh

# build, install, run, assert RESULT: PASS
# no device → boots an emulator; one device → uses it
ANDROID_ABI=x86_64 bash tools/run_android_testapp.sh
# a physical phone, explicitly
ANDROID_ABI=arm64-v8a ANDROID_SERIAL=<serial> bash tools/run_android_testapp.sh

# signature contract gate
bash tools/check_jni_signatures.sh
```

## Not yet verified

- `ColorDialog`/`FontDialog` nativisation — Android ships no platform picker for
  either, so these stay logical-only by platform fact, not by missing wiring.
- AndroidX `Toolbar` nativisation (see `status.md`, "Toolbar investigation").
