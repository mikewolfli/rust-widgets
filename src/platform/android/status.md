# Android Backend Integration Status

## Current Status

The Android backend has two cooperating layers:

1. **State backend** (`src/platform/android/platform_impl.rs`) — the default
   `Platform` implementation, backed by `BackendState<AndroidHandleKind>`.
2. **JNI bridge** (`src/platform/android_jni.rs`) — feature-gated behind
   `android-jni`, which constructs real Android `View` objects and keeps a
   `GlobalRef` registry keyed by `ObjectId`.

As of the current revision the two layers are connected and **verified on a real
Android emulator** (API 34, x86_64, KVM) **and on a physical arm64 device**
(Xiaomi M2102J2SC, Android 13): every `create_*` call in
`platform_impl.rs` attempts a native view through
`AndroidPlatform::attach_native_view` and stores the resulting JNI registry id
in `AndroidPlatform::native_views`, which the setter/lifecycle methods
(`show_widget`, `hide_widget`, `set_widget_geometry`, `set_widget_text`,
`set_widget_enabled`, `set_widget_visible`) forward to.

When the bridge is not initialized — no `JavaVM`, no stored Activity
`Context`, or the feature is compiled out — creation degrades to the logical
state handle only, and `native_view_of(id)` returns `None`. This degradation is
explicit and tested, not a silent no-op.

The single readiness predicate is `android_jni::native_view_creation_ready()`
(`JavaVM` **and** Activity `Context`), which `AndroidPlatform::jni_available()`
delegates to so the two can never disagree.

## Runtime selection

On `target_os = "android"`, `platform::get_platform()` returns
`AndroidPlatform`. The earlier revision had no Android arm in
`create_native_platform`, so Android silently fell through to the generic
`unknown-runtime-stub` and ignored the JNI backend entirely. `mobile_attach_to_native_view`
and `mobile_backend_name` now route through the active platform's
`MobilePlatformExtension`, so they configure the same instance widget creation
uses.

## FFI visibility

The `bindings` module (C ABI + Java JNI) was previously gated on
`feature = "desktop"`, which made it impossible to build the JNI surface for the
`mobile` profile — the primary JNI consumer. It is now available whenever
`desktop`, `jni`, or `mobile-api` is enabled (still excluding `mini`).

`nativeInit` installs a logcat `log` backend (`__android_log_write`, no extra
dependency) so the bridge's diagnostics are visible on-device. Without it the
`log::info!` calls went nowhere.

## Method inventory

| Component | Status | Notes |
|-----------|--------|-------|
| JNI bridge initialization | ✅ Verified | `nativeInit` stores `JavaVM`, installs logcat |
| Activity Context storage | ✅ Verified | `nativeAttachContext` / `set_activity_context` store a `GlobalRef` |
| Rust → Java view factory | ✅ Verified | `create_native_view` constructs any mapped `AndroidViewClass` |
| Button | ✅ Verified | `android.widget.Button` |
| TextView | ✅ Verified | `android.widget.TextView` (Label, StatusBar) |
| EditText | ✅ Verified | `android.widget.EditText` |
| CheckBox | ✅ Verified | `android.widget.CheckBox` |
| RadioButton | ✅ Verified | `android.widget.RadioButton` |
| SeekBar | ✅ Verified | `android.widget.SeekBar` (Slider) |
| ProgressBar | ✅ Verified | `android.widget.ProgressBar` |
| Spinner | ✅ Native | `android.widget.Spinner` (ComboBox) + adapter append |
| ListView | ✅ Native | `android.widget.ListView` (ListBox/ListView) + adapter seed |
| ScrollView | ✅ Native | `android.widget.ScrollView` |
| NumberPicker | ✅ Native | `android.widget.NumberPicker` (SpinBox) |
| FrameLayout | ✅ Verified | `android.widget.FrameLayout` (Panel/Window) |
| View text/bounds/visibility/enabled | ✅ Verified | forwarded to the native setters |
| MenuBar / Menu / MenuItem | 🔶 Logical only | No standalone Android View; consumed by the Activity's own menu |
| ToolBar | 🔶 Logical only (verified constraint) | Not a framework class: `androidx.appcompat.widget.Toolbar` is app-bundled. Verified on-device (Xiaomi, Android 13) that the class loads but cannot be instantiated without AAR resource merging — `NoClassDefFoundError: androidx/appcompat/R$attr`. A host app that ships AndroidX (via Gradle) can adopt it; the library does not bundle it. See "Toolbar investigation" below |
| MessageBox | ✅ Verified | `android.app.AlertDialog` (create / `show` / `dismiss` / `setMessage`) |
| FileDialog | ✅ Verified | Launches real `ACTION_OPEN_DOCUMENT` (`com.android.documentsui` picker) via the stored Activity; see "File dialog" below |
| ColorDialog / FontDialog | 🔶 Logical only | No platform picker on Android; emits an explicit diagnostic when the JNI bridge is ready |
| Menu kind validation | ✅ Enforced | `MenuBar`←Window, `Menu`←MenuBar/Menu, `MenuItem`←Menu, trigger←MenuItem |
| JNI signature validation | ✅ Automated | `tools/check_jni_signatures.sh` (Java ↔ Rust ↔ exported symbols, plus `target/qa/jni_*_map*.json` mapping reports) |
| Cross-compilation | ✅ Verified | NDK 30 clang link for `aarch64`/`x86_64`-linux-android |
| End-to-end testing | ✅ Verified | signed APK built, installed, run on **emulator (x86_64)** and **physical device (arm64-v8a)** — `RESULT: PASS` |

## Building and running the integration test

```bash
# 1. Build + sign an APK for an ABI (needs ANDROID_SDK_ROOT, NDK, JDK)
ANDROID_ABI=x86_64    bash tools/build_android_testapp.sh
ANDROID_ABI=arm64-v8a bash tools/build_android_testapp.sh

# 2. Build, install and run, asserting RESULT: PASS
#    - no device attached  → boots an emulator for the ABI
#    - exactly one attached → uses it (emulator or phone)
ANDROID_ABI=x86_64    bash tools/run_android_testapp.sh

#    - several attached    → pick one explicitly (e.g. a physical phone)
ANDROID_ABI=arm64-v8a ANDROID_SERIAL=65441025 bash tools/run_android_testapp.sh

# 3. Validate Java ↔ Rust JNI signatures (+ exported symbols when built)
bash tools/check_jni_signatures.sh
```

`ANDROID_SERIAL` is the standard adb selector; setting it targets a physical
device without booting an emulator. The script also compares the APK ABI with
`ro.product.cpu.abi` and warns on a mismatch, so a failure cannot be masked by
the wrong native library.

The graded marker is `RESULT: PASS` in logcat under the `RustWidgetsTest` tag.
Both directions are covered: Java→Rust (`nativeCreate*` calls) and Rust→Java
(`nativeAttachContext` + `nativeSelfTestKinds`, which drives `AndroidPlatform`'s
own create path).

## File dialog (2026-09-11)

Android has no file-picker *View*; selection is an Activity operation. The
backend now performs it:

- `android_jni::launch_file_dialog(mime)` checks the stored object with
  `instanceof Activity`, builds `ACTION_OPEN_DOCUMENT` +
  `CATEGORY_OPENABLE` + `setType(...)`, and invokes
  `Activity.startActivityForResult(Intent, int)` reflectively
  (`FILE_DIALOG_REQUEST_CODE`).
- `AndroidPlatform::create_file_dialog` calls it, so creating the widget
  requests the picker. The picker result is delivered to the host Activity's
  own `onActivityResult`; the bridge does not intercept it.
- A non-Activity `Context` cannot service a result launcher, so that case logs
  an explicit diagnostic and returns `false` rather than silently doing nothing.

Verified on a physical device (Xiaomi M2102J2SC, Android 13):

```
[android-jni] launch_file_dialog: ACTION_OPEN_DOCUMENT launched (mime=*/*, requestCode=21079)
ActivityTaskManager: START u0 {act=android.intent.action.OPEN_DOCUMENT
    cat=[android.intent.category.OPENABLE] typ=*/*
    cmp=com.google.android.documentsui/com.android.documentsui.picker.PickActivity}
    from uid 10384 ... callingPackage rust_widgets.testapp
ActivityTaskManager: Displayed com.google.android.documentsui/
    com.android.documentsui.picker.PickActivity: +258ms
```

`ColorDialog`/`FontDialog` remain logical-only because Android genuinely ships
no platform color/font picker — that is a platform fact, not a missing wiring.

## Toolbar investigation (2026-09-11)

Status was previously "not guaranteed; host-provided". This was verified on a
physical device (Xiaomi M2102J2SC, Android 13, arm64-v8a) and can now be stated
precisely:

1. **Not a framework library.** `appcompat` does not appear in
   `/system/framework/` and is absent from `pm list libraries`; it is shipped
   inside each app that uses it.
2. **The class is loadable but not instantiable without its resources.**
   An APK with `androidx.appcompat`'s `classes.dex` but no merged resources
   loads the class successfully:
   ```
   class loaded: androidx.appcompat.widget.Toolbar
   ```
   but construction fails:
   ```
   Caused by: java.lang.NoClassDefFoundError:
       Failed resolution of: Landroidx/appcompat/R$attr;
       at androidx.appcompat.widget.Toolbar.<init>(Toolbar.java:262)
   ```
3. **Why:** the AAR ships 545 resource files plus `R.txt`/`public.txt`. The
   `R$attr` symbol class is generated by resource linking. A Gradle-free
   `aapt2` chain would have to reproduce AAR resource merging in dependency
   order (appcompat itself fails to link standalone because it references
   attributes owned by `androidx.core`), i.e. reimplement AGP's resource merger.

Conclusion: the honest behaviour is unchanged — the toolbar stays a logical
region the host Activity populates. A host that uses AndroidX can attach a real
`Toolbar` itself; bundling the whole AndroidX resource graph into this library
would be an architectural decision, not a backend fix.

## Verified on this revision

- **Physical device: Xiaomi M2102J2SC, Android 13 (API 33), arm64-v8a.** Signed
  `arm64-v8a` APK installs and runs, `RESULT: PASS` for both JNI directions,
  `primaryCpuAbi=arm64-v8a`, zero `AndroidRuntime` errors, native
  `[android-jni]` diagnostics visible in logcat.
- `cargo check` / `cargo clippy` for `x86_64-linux-android` with and without
  `android-jni`: 0 warnings.
- `cargo build` links the NDK-built `.so`; the APK ships
  `lib/<abi>/librust_widgets.so` with all 72 JNI symbols (16 Android view +
  56 generic Java).
- Emulator (API 34, x86_64): signed APK installs, `RESULT: PASS`, zero
  `AndroidRuntime` errors, native `[android-jni]` diagnostics visible in logcat.
- `cargo test --lib` with the full platform feature set: 2506 passed,
  0 failed, 0 ignored.
- Both signed APKs (`x86_64`, `arm64-v8a`) are produced by
  `tools/build_android_testapp.sh` with zipalign verified.

## Not yet verified

- An AndroidX `Toolbar` inside the library — the on-device constraint is now
  characterised rather than merely assumed (see "Toolbar investigation").
- Device coverage beyond one arm64 device.

## Device install note (MIUI/HyperOS)

On MIUI/HyperOS the plain streamed `adb install` path is rejected with
`INSTALL_FAILED_USER_RESTRICTED` even with USB debugging authorised. The guard is
intermittent: `pm install-create` / `install-write` / `install-commit` succeeded
while a plain `adb install` failed, and even the session path needed retries.
`tools/run_android_testapp.sh` therefore falls back to the session API
automatically.

## Next Steps

1. Run the APK on additional physical devices/ABIs (only one arm64 device has
   been exercised so far).
2. Wire `ColorDialog`/`FontDialog` to a host-provided picker if the host
   supplies one (Android ships none).
