<!-- SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com) -->
<!-- SPDX-License-Identifier: MIT -->

# Android Activity Integration

## Two integration directions

The Android bridge supports both directions between Rust and Java:

### 1. Java → Rust (existing `#[no_mangle]` entry points)

Java code declares `native` methods and calls into Rust. These are the canonical
entry points for Java-driven creation.

> The list below is the **actual exported set** — it is verified against the Rust exports by
> `tools/check_jni_signatures.sh`. An earlier revision named `nativeCreateTextView`,
> `nativeCreateEditText`, `nativeCreateCheckBox`, `nativeCreateSeekBar` and `nativeSetViewText`,
> none of which exist: the real names are `nativeCreateLabel`, `nativeCreateLineEdit`,
> `nativeCreateCheckbox`, `nativeCreateSlider`, and there is no `nativeSetView*` family at all.
> `tools/check_status_docs_name_real_types.sh` now covers the class-name half of the same class.
>
> **This page is about the Android host**, so the sample must show the **Android** wrapper —
> `bindings/android/java/rust_widgets/RustWidgets.java` in package `rust_widgets`. It had been
> quoting the *desktop* wrapper's method list (package `io.github.rustwidgets`, 44 creators) as if
> it were this one. They are different classes with different contents, and a reader who copied
> the sample would have bound against names this class does not declare — `UnsatisfiedLinkError`
> at the first call, which is exactly the failure the Android test's own header records having
> been fixed once already.

The Android wrapper is eight methods, all of which exist (`bindings/android/java/rust_widgets/RustWidgets.java`):

```kotlin
package rust_widgets

object RustWidgets {
    init { System.loadLibrary("rust_widgets") }
    // Lifecycle: capture the JavaVM, then the Activity Context the bridge resolves
    // platform facilities through.
    external fun nativeInit()
    external fun nativeAttachContext(context: android.content.Context): Boolean
    external fun nativeDetachContext()
    // Diagnostics, so a stale .so is caught at startup instead of at the first call.
    external fun nativeIntegrationStatus(): Int
    external fun nativeMethodCount(): Int
    external fun nativeInstallLogging()
    // The one fact only the host observes: the surface resized. `windowId` is the
    // library's window handle, which the host got from `rw_create_window`.
    external fun nativeNotifyResize(windowId: Long, width: Int, height: Int): Int
    external fun nativeOpenDocument(uri: String): Boolean
}

// Typed per-kind creators (`nativeCreateButton`, `nativeCreateLabel`, …) live in the
// *desktop* wrapper, `io.github.rustwidgets.RustWidgets`. They are not part of this class:
// on Android the library paints every WidgetKind itself, so the host supplies a window and
// a drawing surface rather than one native View per kind (BLUE15 #55/#56).
```

### 2. Rust → Java (feature `android-jni`)

> **What this direction is *not*.** It does not construct Android Views. The library paints every
> `WidgetKind` itself (BLUE15 #55/#56), so a `nativeCreateButton` call creates a **library**
> widget that the library draws, exactly as `rw_create_button` does in C — it does not build an
> `android.widget.Button`. The `create_native_view` factory and its `AndroidViewClass` table that
> earlier revisions of this paragraph described were deleted under BLUE15 #59, and this sentence
> was left claiming they still ran. See the parity-table note below, which had been corrected
> while this section had not — the two halves of one page disagreeing is the documentation defect
> rule #18 forbids.
>
> What *is* real in this direction: the bridge captures the `JavaVM` at `nativeInit` and a host
> `Context` when one is supplied, and those two facts are what `AndroidPlatform::jni_available()`
> reports. Nothing consumes that answer in production because there is no per-kind native creation
> to gate — it exists so a host can ask `nativeIntegrationStatus` rather than probing blindly, and
> the integration test asserts it.

For this direction the bridge needs the host `Context`, supplied in one of two
ways:

- Call `nativeAttachContext(context)` from Java after `nativeInit()`.
- Call the C ABI `rw_mobile_attach_native_view(contextPtr)` (or the Rust
  `mobile_attach_to_native_view(handle)`), passing the Activity's Java object
  pointer. The handle is converted to a `GlobalRef` so it outlives the call.

Until a Context is stored, `AndroidPlatform::jni_available()` is `false` and
every platform request is answered from the state backend.

## Method parity table

> **Corrected 2026-09-30.** Earlier revisions of this page framed each logical kind as a mapping onto
> an Android widget class (`Button` → `android.widget.Button`, …) with a `Rust factory` column reading
> `create_native_view`. **That model no longer exists.** The library paints every `WidgetKind` itself
> (BLUE15 #55/#56): `grep -c 'fn create_button' src/platform/android/platform_impl.rs` is `0`, and
> the same holds for **every** backend. There is no per-kind Android view to name — the only native
> object an Android host supplies is a **window plus a drawing surface**, and the library draws into
> it. The `create_native_view` factory and its `AndroidViewClass` table were deleted under BLUE15 #59
> (`src/platform/android_jni.rs`'s module doc records it).
>
> What follows is therefore **not** a parity table between logical kinds and OS widgets — there is no
> such correspondence to state. It is the inventory of entry points a Java host may call, derived
> from the **actual exported set** in `src/bindings/java_jni.rs` (verified by
> `tools/check_jni_signatures.sh`). `nativeCreateButton` does **not** build an Android `Button`; it
> creates a library widget, exactly as `rw_create_button` does in C, which is why the two are one
> line of forwarding in `java_jni.rs`.

| Logical kind | Exported JNI entry point | What the library does |
|---|---|---|
| Window / Panel | `nativeCreatePanel` | creates a library widget; the host supplies the surface it is painted into |
| Button | `nativeCreateButton` | paints `WidgetKind::Button` |
| Label / StatusBar | `nativeCreateLabel` / `nativeCreateStatusBar` | painted |
| LineEdit | `nativeCreateLineEdit` | painted |
| CheckBox | `nativeCreateCheckbox` | painted |
| RadioButton | `nativeCreateRadioButton` | painted |
| Slider | `nativeCreateSlider` | painted |
| ProgressBar | `nativeCreateProgressBar` | painted |
| ComboBox | `nativeCreateComboBox` | painted |
| ListBox / ListView | `nativeCreateListBox` / `nativeCreateListView` | painted |
| ScrollArea | `nativeCreateScrollArea` | painted |
| SpinBox | `nativeCreateSpinBox` | painted |
| MenuBar / Menu / MenuItem | `nativeCreateMenuBar` / `nativeCreateMenu` | logical handle only — the library keeps the menu tree in-process and the host renders it |
| ToolBar | `nativeCreateToolBar` | logical handle only |
| MessageBox | — | no `create_native_dialog` is exported; a message box is a painted widget in this library |
| FileDialog | `nativeSelfTestFileDialog` | the one genuine **Activity operation**: launches `ACTION_OPEN_DOCUMENT` and hands the picked URI back (see below). Not a control |
| ColorDialog / FontDialog | — | logical handle only (Android ships no system picker) |

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
