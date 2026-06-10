//! Android JNI bridge — native method implementations for creating
//! real Android native views (Button, TextView, EditText, etc.)
//! corresponding to rust_widgets logical widgets.
//!
//! # Architecture
//!
//! This module exports `#[no_mangle]` JNI native methods that are called
//! from Java/Kotlin code on the Android side. The Java class
//! `rust.widgets.RustWidgets` loads the native library and calls these
//! methods to create and manage Android native `View` objects.
//!
//! # Java-side usage
//!
//! ```java
//! package rust.widgets;
//!
//! public class RustWidgets {
//!     static { System.loadLibrary("rust_widgets"); }
//!
//!     public static native void nativeInit();
//!     public static native long nativeCreateButton(
//!         android.content.Context context, String text,
//!         int x, int y, int w, int h);
//!     // … more native methods …
//! }
//! ```
//!
//! # Thread safety
//!
//! The `JAVA_VM` static is set once during initialization and is then
//! immutable. The view registry is protected by a `Mutex`. All public
//! JNI entry points are safe to call from any thread.

use crate::core::ObjectId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Global JavaVM reference, set once by the JNI_OnLoad / nativeInit entry.
static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();

/// Thread-safe registry mapping rust_widgets ObjectId → JNI GlobalRef.
///
/// GlobalRefs are created via `JNIEnv::new_global_ref()` and stored here
/// so the JNI object is not garbage-collected as long as the widget exists.
static VIEW_REGISTRY: OnceLock<Mutex<HashMap<ObjectId, jni::objects::GlobalRef>>> = OnceLock::new();

fn view_registry() -> &'static Mutex<HashMap<ObjectId, jni::objects::GlobalRef>> {
    VIEW_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Internal helper to generate fresh ObjectId values.
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn allocate_id() -> ObjectId {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Public helpers (used by AndroidMobilePlatform when JNI is available)
// ---------------------------------------------------------------------------

/// Returns `true` when the JNI bridge has been initialized (JavaVM stored).
pub fn is_initialized() -> bool {
    JAVA_VM.get().is_some()
}

/// Integration status report for the Android JNI backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationStatus {
    /// Whether `JAVA_VM` has been initialized via `nativeInit`.
    pub jni_initialized: bool,
    /// Number of `#[no_mangle]` JNI native method implementations exported.
    pub native_methods_count: u32,
    /// Overall readiness: JNI initialized + methods available.
    pub ready: bool,
}

/// Returns the current integration readiness of the Android JNI backend.
pub fn android_integration_ready() -> IntegrationStatus {
    let jni_initialized = JAVA_VM.get().is_some();
    let native_methods_count = 13;
    IntegrationStatus {
        jni_initialized,
        native_methods_count,
        ready: jni_initialized && native_methods_count > 0,
    }
}

/// Attach the current thread to the Java VM and call `f` with a `JNIEnv`.
///
/// Returns the result of `f`, or `None` if the bridge was never initialized
/// or thread attachment fails.
pub fn with_jni_env<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut jni::JNIEnv<'_>) -> R,
{
    let vm = JAVA_VM.get()?;
    let mut guard = vm.attach_current_thread().ok()?;
    Some(f(&mut guard))
}

/// Store a JNI GlobalRef for a given widget ObjectId.
pub fn register_view(id: ObjectId, global_ref: jni::objects::GlobalRef) {
    view_registry().lock().expect("view registry lock poisoned").insert(id, global_ref);
}

/// Look up a stored GlobalRef by widget ObjectId.
pub fn lookup_view(id: ObjectId) -> Option<jni::objects::GlobalRef> {
    view_registry().lock().expect("view registry lock poisoned").get(&id).cloned()
}

/// Remove and drop a GlobalRef for a given widget ObjectId.
pub fn unregister_view(id: ObjectId) {
    view_registry().lock().expect("view registry lock poisoned").remove(&id);
}

// ---------------------------------------------------------------------------
// JNI entry points — called from Java side
// ---------------------------------------------------------------------------

/// Initialize the JNI bridge. Called from Java when native lib is loaded.
///
/// Java equivalent:
/// ```java
/// public static native void nativeInit();
/// ```
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeInit(
    env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    match env.get_java_vm() {
        Ok(vm) => {
            if JAVA_VM.set(vm).is_ok() {
                log::info!("[android-jni] JavaVM stored, native library initialized");
            } else {
                log::info!("[android-jni] JavaVM already initialized (duplicate call)");
            }
        }
        Err(e) => {
            log::error!("[android-jni] failed to get JavaVM: {e}");
        }
    }
}

/// Create an Android `android.widget.Button`.
///
/// Returns a `jlong` representing the rust_widgets ObjectId, or `0` on failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateButton<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    text: jni::objects::JString<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeCreateButton: failed to get text string: {e}");
            return 0;
        }
    };

    log::info!("[android-jni] nativeCreateButton: text={text_str}, pos=({x},{y}), size=({w},{h})");

    // Find the android.widget.Button class
    let button_class = match env.find_class("android/widget/Button") {
        Ok(c) => c,
        Err(e) => {
            log::error!("[android-jni] nativeCreateButton: cannot find android/widget/Button: {e}");
            return 0;
        }
    };

    // Create a new Button(context)
    let button = match env.new_object(
        &button_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(b) => b,
        Err(e) => {
            log::error!("[android-jni] nativeCreateButton: failed to create Button: {e}");
            return 0;
        }
    };

    // Set text
    if let Err(e) = env.call_method(
        &button,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeCreateButton: setText failed: {e}");
    }

    // Create LayoutParams for positioning
    apply_view_layout(&mut env, &button, x, y, w, h);

    // Store as global ref
    let id = allocate_id();
    match env.new_global_ref(&button) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateButton: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.TextView` (used for Label).
///
/// Returns a `jlong` representing the rust_widgets ObjectId, or `0` on failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateTextView<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    text: jni::objects::JString<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeCreateTextView: failed to get text string: {e}");
            return 0;
        }
    };

    log::info!(
        "[android-jni] nativeCreateTextView: text={text_str}, pos=({x},{y}), size=({w},{h})"
    );

    let text_view_class = match env.find_class("android/widget/TextView") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateTextView: cannot find android/widget/TextView: {e}"
            );
            return 0;
        }
    };

    let text_view = match env.new_object(
        &text_view_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(tv) => tv,
        Err(e) => {
            log::error!("[android-jni] nativeCreateTextView: failed to create TextView: {e}");
            return 0;
        }
    };

    // Set text
    if let Err(e) = env.call_method(
        &text_view,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeCreateTextView: setText failed: {e}");
    }

    apply_view_layout(&mut env, &text_view, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&text_view) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateTextView: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.EditText` (used for LineEdit).
///
/// Returns a `jlong` representing the rust_widgets ObjectId, or `0` on failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateEditText<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    text: jni::objects::JString<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeCreateEditText: failed to get text string: {e}");
            return 0;
        }
    };

    log::info!(
        "[android-jni] nativeCreateEditText: text={text_str}, pos=({x},{y}), size=({w},{h})"
    );

    let edit_text_class = match env.find_class("android/widget/EditText") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateEditText: cannot find android/widget/EditText: {e}"
            );
            return 0;
        }
    };

    let edit_text = match env.new_object(
        &edit_text_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(et) => et,
        Err(e) => {
            log::error!("[android-jni] nativeCreateEditText: failed to create EditText: {e}");
            return 0;
        }
    };

    // Set text
    if let Err(e) = env.call_method(
        &edit_text,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeCreateEditText: setText failed: {e}");
    }

    apply_view_layout(&mut env, &edit_text, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&edit_text) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateEditText: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.CheckBox`.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateCheckBox<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    text: jni::objects::JString<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeCreateCheckBox: failed to get text string: {e}");
            return 0;
        }
    };

    log::info!(
        "[android-jni] nativeCreateCheckBox: text={text_str}, pos=({x},{y}), size=({w},{h})"
    );

    let check_box_class = match env.find_class("android/widget/CheckBox") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateCheckBox: cannot find android/widget/CheckBox: {e}"
            );
            return 0;
        }
    };

    let check_box = match env.new_object(
        &check_box_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(cb) => cb,
        Err(e) => {
            log::error!("[android-jni] nativeCreateCheckBox: failed to create CheckBox: {e}");
            return 0;
        }
    };

    if let Err(e) = env.call_method(
        &check_box,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeCreateCheckBox: setText failed: {e}");
    }

    apply_view_layout(&mut env, &check_box, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&check_box) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateCheckBox: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.RadioButton`.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateRadioButton<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    text: jni::objects::JString<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeCreateRadioButton: failed to get text string: {e}");
            return 0;
        }
    };

    log::info!(
        "[android-jni] nativeCreateRadioButton: text={text_str}, pos=({x},{y}), size=({w},{h})"
    );

    let radio_button_class = match env.find_class("android/widget/RadioButton") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateRadioButton: cannot find android/widget/RadioButton: {e}"
            );
            return 0;
        }
    };

    let radio_button = match env.new_object(
        &radio_button_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(rb) => rb,
        Err(e) => {
            log::error!("[android-jni] nativeCreateRadioButton: failed to create RadioButton: {e}");
            return 0;
        }
    };

    if let Err(e) = env.call_method(
        &radio_button,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeCreateRadioButton: setText failed: {e}");
    }

    apply_view_layout(&mut env, &radio_button, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&radio_button) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateRadioButton: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.ProgressBar`.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateProgressBar<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    log::info!("[android-jni] nativeCreateProgressBar: pos=({x},{y}), size=({w},{h})");

    let progress_bar_class = match env.find_class("android/widget/ProgressBar") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateProgressBar: cannot find android/widget/ProgressBar: {e}"
            );
            return 0;
        }
    };

    let progress_bar = match env.new_object(
        &progress_bar_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(pb) => pb,
        Err(e) => {
            log::error!("[android-jni] nativeCreateProgressBar: failed to create ProgressBar: {e}");
            return 0;
        }
    };

    apply_view_layout(&mut env, &progress_bar, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&progress_bar) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateProgressBar: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

/// Create an Android `android.widget.SeekBar` (used for Slider).
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeCreateSeekBar<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) -> jni::sys::jlong {
    log::info!("[android-jni] nativeCreateSeekBar: pos=({x},{y}), size=({w},{h})");

    let seek_bar_class = match env.find_class("android/widget/SeekBar") {
        Ok(c) => c,
        Err(e) => {
            log::error!(
                "[android-jni] nativeCreateSeekBar: cannot find android/widget/SeekBar: {e}"
            );
            return 0;
        }
    };

    let seek_bar = match env.new_object(
        &seek_bar_class,
        "(Landroid/content/Context;)V",
        &[jni::objects::JValue::Object(&context)],
    ) {
        Ok(sb) => sb,
        Err(e) => {
            log::error!("[android-jni] nativeCreateSeekBar: failed to create SeekBar: {e}");
            return 0;
        }
    };

    apply_view_layout(&mut env, &seek_bar, x, y, w, h);

    let id = allocate_id();
    match env.new_global_ref(&seek_bar) {
        Ok(global_ref) => {
            register_view(id, global_ref);
        }
        Err(e) => {
            log::error!("[android-jni] nativeCreateSeekBar: failed to create global ref: {e}");
            return 0;
        }
    }

    id as jni::sys::jlong
}

// ---------------------------------------------------------------------------
// View manipulation helpers & JNI methods
// ---------------------------------------------------------------------------

/// Apply layout params (position + size) to an Android View.
///
/// This creates a `ViewGroup.MarginLayoutParams` (or `LayoutParams` with
/// absolute coordinates) so the view appears at (x, y) with size (w, h).
fn apply_view_layout(
    env: &mut jni::JNIEnv<'_>,
    view: &jni::objects::JObject<'_>,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) {
    // Get the ViewGroup.LayoutParams class
    let lp_class = match env.find_class("android/view/ViewGroup$LayoutParams") {
        Ok(c) => c,
        Err(e) => {
            log::error!("[android-jni] apply_view_layout: cannot find LayoutParams class: {e}");
            return;
        }
    };

    // Create new LayoutParams(width, height)
    let lp = match env.new_object(
        &lp_class,
        "(II)V",
        &[jni::objects::JValue::Int(w), jni::objects::JValue::Int(h)],
    ) {
        Ok(lp) => lp,
        Err(e) => {
            log::error!("[android-jni] apply_view_layout: failed to create LayoutParams: {e}");
            return;
        }
    };

    // Store the original x/y in the LayoutParams's leftMargin/topMargin.
    // We directly set margins; if the parent doesn't support MarginLayoutParams,
    // the margin values are simply ignored by the layout system.
    if let Err(e) = env.call_method(view, "setLeft", "(I)V", &[jni::objects::JValue::Int(x)]) {
        log::error!("[android-jni] apply_view_layout: setLeft failed: {e}");
    }
    if let Err(e) = env.call_method(view, "setTop", "(I)V", &[jni::objects::JValue::Int(y)]) {
        log::error!("[android-jni] apply_view_layout: setTop failed: {e}");
    }

    // Set the layout params on the view
    if let Err(e) = env.call_method(
        view,
        "setLayoutParams",
        "(Landroid/view/ViewGroup$LayoutParams;)V",
        &[jni::objects::JValue::Object(&lp)],
    ) {
        log::error!("[android-jni] apply_view_layout: setLayoutParams failed: {e}");
    }
}

/// Set the text content of a View that has `setText(CharSequence)`.
///
/// Java equivalent:
/// ```java
/// public static native void nativeSetViewText(long nativePtr, String text);
/// ```
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSetViewText<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    native_ptr: jni::sys::jlong,
    text: jni::objects::JString<'local>,
) {
    let id = native_ptr as ObjectId;
    let text_str: String = match env.get_string(&text) {
        Ok(s) => s.into(),
        Err(e) => {
            log::error!("[android-jni] nativeSetViewText({id}): failed to get text string: {e}");
            return;
        }
    };

    let global_ref = match lookup_view(id) {
        Some(r) => r,
        None => {
            log::warn!("[android-jni] nativeSetViewText({id}): view not found in registry");
            return;
        }
    };

    let view_obj = global_ref.as_obj();
    if let Err(e) = env.call_method(
        view_obj,
        "setText",
        "(Ljava/lang/CharSequence;)V",
        &[jni::objects::JValue::Object(&text)],
    ) {
        log::error!("[android-jni] nativeSetViewText({id}): setText failed: {e}");
    }

    log::info!("[android-jni] nativeSetViewText({id}): text={text_str}");
}

/// Update the bounds (position + size) of a View by applying new LayoutParams.
///
/// Java equivalent:
/// ```java
/// public static native void nativeSetViewBounds(long nativePtr, int x, int y, int w, int h);
/// ```
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSetViewBounds<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    native_ptr: jni::sys::jlong,
    x: jni::sys::jint,
    y: jni::sys::jint,
    w: jni::sys::jint,
    h: jni::sys::jint,
) {
    let id = native_ptr as ObjectId;
    log::info!("[android-jni] nativeSetViewBounds({id}): pos=({x},{y}), size=({w},{h})");

    let global_ref = match lookup_view(id) {
        Some(r) => r,
        None => {
            log::warn!("[android-jni] nativeSetViewBounds({id}): view not found in registry");
            return;
        }
    };

    let view_obj = global_ref.as_obj();
    apply_view_layout(&mut env, view_obj, x, y, w, h);
}

/// Set the visibility of a View.
///
/// `visible`: `JNI_TRUE` (1) for `View.VISIBLE`, `JNI_FALSE` (0) for `View.GONE`.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSetViewVisibility(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    native_ptr: jni::sys::jlong,
    visible: jni::sys::jboolean,
) {
    let id = native_ptr as ObjectId;
    let visibility = if visible != 0 { 0 } else { 8 }; // View.VISIBLE = 0, View.GONE = 8

    let global_ref = match lookup_view(id) {
        Some(r) => r,
        None => {
            log::warn!("[android-jni] nativeSetViewVisibility({id}): view not found in registry");
            return;
        }
    };

    let view_obj = global_ref.as_obj();
    if let Err(e) =
        env.call_method(view_obj, "setVisibility", "(I)V", &[jni::objects::JValue::Int(visibility)])
    {
        log::error!("[android-jni] nativeSetViewVisibility({id}): setVisibility failed: {e}");
    }
}

/// Set the enabled state of a View.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSetViewEnabled(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    native_ptr: jni::sys::jlong,
    enabled: jni::sys::jboolean,
) {
    let id = native_ptr as ObjectId;

    let global_ref = match lookup_view(id) {
        Some(r) => r,
        None => {
            log::warn!("[android-jni] nativeSetViewEnabled({id}): view not found in registry");
            return;
        }
    };

    let view_obj = global_ref.as_obj();
    if let Err(e) = env.call_method(
        view_obj,
        "setEnabled",
        "(Z)V",
        &[jni::objects::JValue::Bool(if enabled != 0 {
            jni::sys::JNI_TRUE
        } else {
            jni::sys::JNI_FALSE
        })],
    ) {
        log::error!("[android-jni] nativeSetViewEnabled({id}): setEnabled failed: {e}");
    }
}

/// Destroy (remove global ref for) a previously created View.
///
/// Called from Java when the widget is no longer needed.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeDestroyView(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    native_ptr: jni::sys::jlong,
) {
    let id = native_ptr as ObjectId;
    log::info!("[android-jni] nativeDestroyView({id})");
    unregister_view(id);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_id_is_monotonic() {
        let id1 = allocate_id();
        let id2 = allocate_id();
        assert!(id2 > id1);
    }

    #[test]
    fn test_register_and_lookup_view() {
        // Create a mock ObjectId and a mock registry entry is not
        // possible without a real JVM. Verify the registry API doesn't panic.
        let id = 42;
        assert!(lookup_view(id).is_none());
        unregister_view(id); // should not panic
        assert!(lookup_view(id).is_none());
    }

    #[test]
    fn test_unregister_nonexistent_view() {
        unregister_view(999); // should not panic
    }
}
