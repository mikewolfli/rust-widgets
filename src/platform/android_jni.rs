// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
// Android logcat logger
// ---------------------------------------------------------------------------

/// Install a `log` backend that forwards to Android's logcat.
///
/// Without this the bridge's `log::info!` / `log::error!` diagnostics go
/// nowhere on-device, because no global logger is set. `__android_log_write`
/// comes from liblog, which every Android process already links, so this adds
/// no dependency. Installed once from `nativeInit`; safe to call repeatedly.
pub fn init_logging() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        // Ignore the error: if the host already installed a logger we keep it.
        let _ = log::set_logger(&LOGCAT_LOGGER);
        log::set_max_level(log::LevelFilter::Info);
    });
}

struct LogcatLogger;

static LOGCAT_LOGGER: LogcatLogger = LogcatLogger;

/// Map a `log` level to the logcat priority constants from `<android/log.h>`.
fn logcat_priority(level: log::Level) -> i32 {
    // ANDROID_LOG_VERBOSE=2, DEBUG=3, INFO=4, WARN=5, ERROR=6
    match level {
        log::Level::Trace => 2,
        log::Level::Debug => 3,
        log::Level::Info => 4,
        log::Level::Warn => 5,
        log::Level::Error => 6,
    }
}

impl log::Log for LogcatLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let tag = std::ffi::CString::new("rust_widgets").unwrap_or_default();
        // `log::Record::args()` formatting allocates; acceptable for a
        // diagnostic path that only runs when logging is enabled.
        let message = std::ffi::CString::new(format!("{}", record.args()))
            .unwrap_or_else(|_| std::ffi::CString::new("(message contained NUL)").unwrap());
        unsafe {
            android_log_write(logcat_priority(record.level()), tag.as_ptr(), message.as_ptr());
        }
    }

    fn flush(&self) {}
}

// ---------------------------------------------------------------------------
// liblog FFI
// ---------------------------------------------------------------------------

extern "C" {
    /// `int __android_log_write(int prio, const char* tag, const char* text)`
    /// from `<android/log.h>`; provided by liblog on every Android device.
    #[link_name = "__android_log_write"]
    fn android_log_write(
        prio: i32,
        tag: *const std::os::raw::c_char,
        text: *const std::os::raw::c_char,
    ) -> i32;
}

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

/// GlobalRef to the Android `Context` (usually the hosting `Activity`).
///
/// Set once from Java via [`set_activity_context`]. Without it the Rust-callable
/// view factory helpers cannot construct Android widgets, so
/// [`create_native_view`] returns `None` and the caller keeps the state-only
/// path.
static ACTIVITY_CONTEXT: OnceLock<Mutex<Option<jni::objects::GlobalRef>>> = OnceLock::new();

fn activity_context_slot() -> &'static Mutex<Option<jni::objects::GlobalRef>> {
    ACTIVITY_CONTEXT.get_or_init(|| Mutex::new(None))
}

/// Store the Android `Context` used by [`create_native_view`].
///
/// Called from the Java side (or `attach_to_native_view`) with a JNI local
/// reference; a `GlobalRef` is created internally so the Context outlives the
/// calling frame.
///
/// Returns `true` when the reference was stored.
pub fn set_activity_context(
    env: &mut jni::JNIEnv<'_>,
    context: &jni::objects::JObject<'_>,
) -> bool {
    let global = match env.new_global_ref(context) {
        Ok(g) => g,
        Err(e) => {
            log::error!("[android-jni] set_activity_context: failed to create GlobalRef: {e}");
            return false;
        }
    };
    let mut slot = activity_context_slot().lock().expect("activity context lock poisoned");
    *slot = Some(global);
    log::info!("[android-jni] activity Context stored");
    true
}

/// Returns `true` when a Context has been stored and native view creation can
/// proceed.
pub fn has_activity_context() -> bool {
    activity_context_slot().lock().map(|slot| slot.is_some()).unwrap_or(false)
}

/// Canonical readiness predicate for native view creation.
///
/// Native views require **both** the `JavaVM` (from `nativeInit`) and an
/// Activity `Context` (from [`set_activity_context`]). This is the single source
/// of truth used by `AndroidPlatform::jni_available` and the Rust-callable view
/// factory, so the two can never disagree about whether the bridge is usable.
pub fn native_view_creation_ready() -> bool {
    is_initialized() && has_activity_context()
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
// Rust-callable view factory (used by AndroidPlatform::create_* when the
// `android-jni` feature is on). The `#[no_mangle] Java_*` entry points below
// are the inverse direction (Java → Rust); these helpers are Rust → Java so
// `platform_impl` actually constructs native views instead of only recording
// logical state.
// ---------------------------------------------------------------------------

/// Android widget class selected by [`create_native_view`].
///
/// Mirrors the view types the `Java_*` entry points construct, so both
/// directions create the same widget for a given logical kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidViewClass {
    /// `android.widget.Button`.
    Button,
    /// `android.widget.TextView` (labels and status text).
    TextView,
    /// `android.widget.EditText`.
    EditText,
    /// `android.widget.CheckBox`.
    CheckBox,
    /// `android.widget.RadioButton`.
    RadioButton,
    /// `android.widget.SeekBar`.
    SeekBar,
    /// `android.widget.ProgressBar`.
    ProgressBar,
    /// `android.widget.Spinner`.
    Spinner,
    /// `android.widget.ListView`.
    ListView,
    /// `android.widget.ScrollView`.
    ScrollView,
    /// `android.widget.NumberPicker`.
    NumberPicker,
    /// `android.widget.FrameLayout` (generic container / panel).
    FrameLayout,
}

/// Logical widget kind → native Android view class mapping.
///
/// This is the single source of truth for which logical kinds get a real
/// Android `View`. Kinds without a standalone View (menus, dialogs, toolbars)
/// deliberately return `None` so callers keep the logical handle instead of
/// pretending a native object exists. Kept here (rather than in the
/// `target_os = "android"` module) so the mapping is unit-testable on any host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AndroidLogicalKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    Label,
    RadioButton,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    SpinBox,
    ListView,
    ScrollArea,
}

/// Resolve the native view class for a logical kind, or `None` when the kind
/// has no standalone Android View equivalent.
pub fn view_class_for(kind: AndroidLogicalKind) -> Option<AndroidViewClass> {
    use AndroidLogicalKind::*;
    Some(match kind {
        Button => AndroidViewClass::Button,
        Label | StatusBar => AndroidViewClass::TextView,
        LineEdit => AndroidViewClass::EditText,
        CheckBox => AndroidViewClass::CheckBox,
        RadioButton => AndroidViewClass::RadioButton,
        Slider => AndroidViewClass::SeekBar,
        ProgressBar => AndroidViewClass::ProgressBar,
        ComboBox => AndroidViewClass::Spinner,
        ListBox | ListView => AndroidViewClass::ListView,
        ScrollArea => AndroidViewClass::ScrollView,
        SpinBox => AndroidViewClass::NumberPicker,
        Panel | Window => AndroidViewClass::FrameLayout,
        MenuBar | Menu | MenuItem | ToolBar | MessageBox | FileDialog | ColorDialog
        | FontDialog => return None,
    })
}

impl AndroidViewClass {
    /// JNI class path passed to `JNIEnv::find_class`.
    fn jni_class_path(self) -> &'static str {
        match self {
            AndroidViewClass::Button => "android/widget/Button",
            AndroidViewClass::TextView => "android/widget/TextView",
            AndroidViewClass::EditText => "android/widget/EditText",
            AndroidViewClass::CheckBox => "android/widget/CheckBox",
            AndroidViewClass::RadioButton => "android/widget/RadioButton",
            AndroidViewClass::SeekBar => "android/widget/SeekBar",
            AndroidViewClass::ProgressBar => "android/widget/ProgressBar",
            AndroidViewClass::Spinner => "android/widget/Spinner",
            AndroidViewClass::ListView => "android/widget/ListView",
            AndroidViewClass::ScrollView => "android/widget/ScrollView",
            AndroidViewClass::NumberPicker => "android/widget/NumberPicker",
            AndroidViewClass::FrameLayout => "android/widget/FrameLayout",
        }
    }

    /// Whether the widget implements `setText(CharSequence)`.
    fn supports_text(self) -> bool {
        matches!(
            self,
            AndroidViewClass::Button
                | AndroidViewClass::TextView
                | AndroidViewClass::EditText
                | AndroidViewClass::CheckBox
                | AndroidViewClass::RadioButton
        )
    }
}

/// Create a native Android view, apply its layout, and register a GlobalRef.
///
/// Returns the JNI registry id (a fresh [`ObjectId`]) on success, or `None`
/// when the bridge is not initialized, no Activity Context has been stored, or
/// the JVM rejects the construction. `None` means the caller should keep the
/// logical state handle without a native counterpart.
pub fn create_native_view(
    class: AndroidViewClass,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<ObjectId> {
    if !is_initialized() {
        return None;
    }
    with_jni_env(|env| create_view_with_env(env, class, text, x, y, width, height)).flatten()
}

fn create_view_with_env(
    env: &mut jni::JNIEnv<'_>,
    class: AndroidViewClass,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<ObjectId> {
    // Hold the Context lock only while cloning the reference; the actual JNI
    // calls below must not run under the registry lock to avoid re-entrancy.
    let context = {
        let slot = activity_context_slot().lock().expect("activity context lock poisoned");
        slot.as_ref()?.clone()
    };
    let context_obj = context.as_obj();

    let class_path = class.jni_class_path();
    let view_class = env.find_class(class_path).ok()?;
    let view = env
        .new_object(
            &view_class,
            "(Landroid/content/Context;)V",
            &[jni::objects::JValue::Object(context_obj)],
        )
        .ok()?;

    if class.supports_text() && !text.is_empty() {
        if let Ok(java_text) = env.new_string(text) {
            if let Err(e) = env.call_method(
                &view,
                "setText",
                "(Ljava/lang/CharSequence;)V",
                &[jni::objects::JValue::Object(&java_text)],
            ) {
                log::warn!("[android-jni] create_native_view({class_path}): setText failed: {e}");
            }
        }
    }

    apply_view_layout(env, &view, x, y, width as i32, height as i32);

    let id = allocate_id();
    let global = env.new_global_ref(&view).ok()?;
    register_view(id, global);
    log::info!("[android-jni] create_native_view({class_path}) -> id={id}");
    Some(id)
}

/// Destroy a native view previously created by [`create_native_view`].
///
/// Returns `true` when a registered view was found and released.
pub fn destroy_native_view(id: ObjectId) -> bool {
    if lookup_view(id).is_none() {
        return false;
    }
    unregister_view(id);
    true
}

/// Set the text of a native view created by [`create_native_view`].
pub fn set_native_view_text(id: ObjectId, text: &str) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let java_text = match env.new_string(text) {
            Ok(t) => t,
            Err(e) => {
                log::error!("[android-jni] set_native_view_text({id}): new_string failed: {e}");
                return false;
            }
        };
        env.call_method(
            global.as_obj(),
            "setText",
            "(Ljava/lang/CharSequence;)V",
            &[jni::objects::JValue::Object(&java_text)],
        )
        .is_ok()
    })
    .unwrap_or(false)
}

/// Update the bounds of a native view created by [`create_native_view`].
pub fn set_native_view_bounds(id: ObjectId, x: i32, y: i32, width: u32, height: u32) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        apply_view_layout(env, global.as_obj(), x, y, width as i32, height as i32);
        true
    })
    .unwrap_or(false)
}

/// Set the visibility of a native view. `View.VISIBLE` = 0, `View.GONE` = 8.
pub fn set_native_view_visibility(id: ObjectId, visible: bool) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let visibility = if visible { 0 } else { 8 };
        env.call_method(
            global.as_obj(),
            "setVisibility",
            "(I)V",
            &[jni::objects::JValue::Int(visibility)],
        )
        .is_ok()
    })
    .unwrap_or(false)
}

/// Set the enabled state of a native view.
pub fn set_native_view_enabled(id: ObjectId, enabled: bool) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        env.call_method(
            global.as_obj(),
            "setEnabled",
            "(Z)V",
            &[jni::objects::JValue::Bool(if enabled {
                jni::sys::JNI_TRUE
            } else {
                jni::sys::JNI_FALSE
            })],
        )
        .is_ok()
    })
    .unwrap_or(false)
}

/// Append an item to a native `Spinner` adapter.
///
/// `Spinner` is backed by an `ArrayAdapter<String>`; the adapter is created on
/// the first item and reused afterwards so selecting an index keeps working.
/// `set_selection` is applied only for the first item so later appends do not
/// reset an existing user selection.
pub fn append_spinner_item(id: ObjectId, text: &str, set_selection: bool) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let spinner = global.as_obj();
        let java_text = match env.new_string(text) {
            Ok(t) => t,
            Err(e) => {
                log::error!("[android-jni] append_spinner_item({id}): new_string failed: {e}");
                return false;
            }
        };

        // Reuse the ArrayAdapter attached to the Spinner when one exists;
        // otherwise create a simple spinner-item layout adapter.
        let adapter = match env.call_method(spinner, "getAdapter", "()Landroid/widget/SpinnerAdapter;", &[])
        {
            Ok(v) => v.l().ok().filter(|o| !o.is_null()),
            Err(e) => {
                log::error!("[android-jni] append_spinner_item({id}): getAdapter failed: {e}");
                return false;
            }
        };

        let added = match adapter {
            Some(adapter_obj) => env
                .call_method(
                    &adapter_obj,
                    "add",
                    "(Ljava/lang/Object;)V",
                    &[jni::objects::JValue::Object(&java_text)],
                )
                .is_ok(),
            None => {
                // No adapter yet: build an ArrayAdapter<String> over the
                // standard Android spinner item layout.
                let array_adapter_class = match env.find_class("android/widget/ArrayAdapter") {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("[android-jni] append_spinner_item({id}): ArrayAdapter class missing: {e}");
                        return false;
                    }
                };
                let context = match spinner_context(env, spinner) {
                    Some(c) => c,
                    None => return false,
                };
                let layout = match env.get_static_field(
                    "android/R$layout",
                    "simple_spinner_item",
                    "I",
                ) {
                    Ok(v) => v.i().unwrap_or(0),
                    Err(_) => 0,
                };
                let adapter = match env.new_object(
                    &array_adapter_class,
                    "(Landroid/content/Context;I)V",
                    &[
                        jni::objects::JValue::Object(&context),
                        jni::objects::JValue::Int(layout),
                    ],
                ) {
                    Ok(a) => a,
                    Err(e) => {
                        log::error!("[android-jni] append_spinner_item({id}): ArrayAdapter creation failed: {e}");
                        return false;
                    }
                };
                if env
                    .call_method(
                        &adapter,
                        "add",
                        "(Ljava/lang/Object;)V",
                        &[jni::objects::JValue::Object(&java_text)],
                    )
                    .is_err()
                {
                    log::error!("[android-jni] append_spinner_item({id}): adapter.add failed");
                    return false;
                }
                env.call_method(
                    spinner,
                    "setAdapter",
                    "(Landroid/widget/SpinnerAdapter;)V",
                    &[jni::objects::JValue::Object(&adapter)],
                )
                .is_ok()
            }
        };
        if !added {
            return false;
        }
        if set_selection {
            let _ = env.call_method(
                spinner,
                "setSelection",
                "(I)V",
                &[jni::objects::JValue::Int(0)],
            );
        }
        true
    })
    .unwrap_or(false)
}

/// Fetch the `Context` a view was constructed with, for adapter creation.
fn spinner_context<'local>(
    env: &mut jni::JNIEnv<'local>,
    view: &jni::objects::JObject<'local>,
) -> Option<jni::objects::JObject<'local>> {
    env.call_method(view, "getContext", "()Landroid/content/Context;", &[])
        .ok()
        .and_then(|v| v.l().ok())
        .filter(|o| !o.is_null())
}

/// Append items to a native `ListView` adapter (or install a new adapter that
/// contains exactly `texts` when none is attached yet).
///
/// Uses the standard `android.R.layout.simple_list_item_1` row layout so the
/// list is usable without application-provided resources.
pub fn append_list_item(id: ObjectId, texts: &[&str]) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let list_view = global.as_obj();

        let existing = env
            .call_method(list_view, "getAdapter", "()Landroid/widget/ListAdapter;", &[])
            .ok()
            .and_then(|v| v.l().ok())
            .filter(|o| !o.is_null());

        if let Some(adapter) = existing {
            for text in texts {
                let java_text = match env.new_string(*text) {
                    Ok(t) => t,
                    Err(e) => {
                        log::error!("[android-jni] append_list_item({id}): new_string failed: {e}");
                        return false;
                    }
                };
                if let Err(e) = env.call_method(
                    &adapter,
                    "add",
                    "(Ljava/lang/Object;)V",
                    &[jni::objects::JValue::Object(&java_text)],
                ) {
                    log::error!("[android-jni] append_list_item({id}): adapter.add failed: {e}");
                    return false;
                }
            }
            return true;
        }

        // No adapter yet: build an ArrayAdapter<String> seeded with all items.
        let array_adapter_class = match env.find_class("android/widget/ArrayAdapter") {
            Ok(c) => c,
            Err(e) => {
                log::error!(
                    "[android-jni] append_list_item({id}): ArrayAdapter class missing: {e}"
                );
                return false;
            }
        };
        let context = match spinner_context(env, list_view) {
            Some(c) => c,
            None => return false,
        };
        let layout = env
            .get_static_field("android/R$layout", "simple_list_item_1", "I")
            .ok()
            .and_then(|v| v.i().ok())
            .unwrap_or(0);
        let adapter = match env.new_object(
            &array_adapter_class,
            "(Landroid/content/Context;I)V",
            &[jni::objects::JValue::Object(&context), jni::objects::JValue::Int(layout)],
        ) {
            Ok(a) => a,
            Err(e) => {
                log::error!(
                    "[android-jni] append_list_item({id}): ArrayAdapter creation failed: {e}"
                );
                return false;
            }
        };
        for text in texts {
            let java_text = match env.new_string(*text) {
                Ok(t) => t,
                Err(e) => {
                    log::error!("[android-jni] append_list_item({id}): new_string failed: {e}");
                    return false;
                }
            };
            if let Err(e) = env.call_method(
                &adapter,
                "add",
                "(Ljava/lang/Object;)V",
                &[jni::objects::JValue::Object(&java_text)],
            ) {
                log::error!("[android-jni] append_list_item({id}): adapter.add failed: {e}");
                return false;
            }
        }
        if let Err(e) = env.call_method(
            list_view,
            "setAdapter",
            "(Landroid/widget/ListAdapter;)V",
            &[jni::objects::JValue::Object(&adapter)],
        ) {
            log::error!("[android-jni] append_list_item({id}): setAdapter failed: {e}");
            return false;
        }
        true
    })
    .unwrap_or(false)
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
    // Install the logcat backend first so the messages below are visible.
    init_logging();
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

// ---------------------------------------------------------------------------
// Dialog factory (AlertDialog)
// ---------------------------------------------------------------------------

/// Register a dialog object in the same registry as views.
///
/// Dialogs and views share the `ObjectId` space so `lookup_view` resolves both;
/// a dialog is simply an object that implements `show()` / `dismiss()` rather
/// than a `View`.
fn register_dialog(
    id: ObjectId,
    env: &mut jni::JNIEnv<'_>,
    dialog: &jni::objects::JObject<'_>,
) -> bool {
    match env.new_global_ref(dialog) {
        Ok(global) => {
            register_view(id, global);
            true
        }
        Err(e) => {
            log::error!("[android-jni] register_dialog: failed to create GlobalRef: {e}");
            false
        }
    }
}

/// Build an `AlertDialog` on the stored Activity Context.
///
/// Returns the registry id on success. Requires the Activity Context
/// ([`set_activity_context`]) because `AlertDialog.Builder` needs a Context, and
/// `show()` needs an Activity-backed window. `title`/`message` are optional in
/// the sense that an empty string simply yields an empty field.
pub fn create_native_dialog(title: &str, message: &str) -> Option<ObjectId> {
    if !is_initialized() {
        return None;
    }
    with_jni_env(|env| create_dialog_with_env(env, title, message)).flatten()
}

fn create_dialog_with_env(
    env: &mut jni::JNIEnv<'_>,
    title: &str,
    message: &str,
) -> Option<ObjectId> {
    let context = {
        let slot = activity_context_slot().lock().expect("activity context lock poisoned");
        slot.as_ref()?.clone()
    };
    let context_obj = context.as_obj();

    let builder_class = env.find_class("android/app/AlertDialog$Builder").ok()?;
    let builder = env
        .new_object(
            &builder_class,
            "(Landroid/content/Context;)V",
            &[jni::objects::JValue::Object(context_obj)],
        )
        .ok()?;

    if !title.is_empty() {
        let java_title = env.new_string(title).ok()?;
        if let Err(e) = env.call_method(
            &builder,
            "setTitle",
            "(Ljava/lang/CharSequence;)Landroid/app/AlertDialog$Builder;",
            &[jni::objects::JValue::Object(&java_title)],
        ) {
            log::warn!("[android-jni] create_native_dialog: setTitle failed: {e}");
        }
    }
    if !message.is_empty() {
        let java_message = env.new_string(message).ok()?;
        if let Err(e) = env.call_method(
            &builder,
            "setMessage",
            "(Ljava/lang/CharSequence;)Landroid/app/AlertDialog$Builder;",
            &[jni::objects::JValue::Object(&java_message)],
        ) {
            log::warn!("[android-jni] create_native_dialog: setMessage failed: {e}");
        }
    }

    let dialog = env
        .call_method(&builder, "create", "()Landroid/app/AlertDialog;", &[])
        .ok()
        .and_then(|v| v.l().ok())?;

    // Show it. `AlertDialog.show()` must run on a thread with a Looper; the
    // Java-side entry points are called from the UI thread, and the Rust-driven
    // path is documented to require the same.
    if let Err(e) = env.call_method(&dialog, "show", "()V", &[]) {
        log::warn!("[android-jni] create_native_dialog: show() failed: {e}");
    }

    let id = allocate_id();
    if !register_dialog(id, env, &dialog) {
        return None;
    }
    log::info!("[android-jni] create_native_dialog -> id={id}");
    Some(id)
}

/// Request code used for the `ACTION_OPEN_DOCUMENT` result.
///
/// The host Activity receives this in `onActivityResult` / its result
/// launcher; it is chosen to be unlikely to collide with app-defined codes.
pub const FILE_DIALOG_REQUEST_CODE: i32 = 0x5257; // "RW"

/// Launch the system document picker (`ACTION_OPEN_DOCUMENT`) for a file dialog.
///
/// Android has no `FileDialog` View: file selection is an *Activity* operation.
/// The bridge stores only a `Context` `GlobalRef`, so the Activity is recovered
/// by reflection: the stored object is first checked with `instanceof Activity`,
/// then `Activity.startActivityForResult(Intent, int)` is invoked on it. This is
/// the same mechanism (and the same constraint) documented for the backend — a
/// non-Activity Context cannot service a result launcher, and in that case this
/// returns `false` after logging an explicit diagnostic rather than silently
/// doing nothing.
///
/// Verified on a physical device (Xiaomi M2102J2SC, Android 13): the stored
/// Context is the Activity, the method resolves reflectively, and the picker
/// launches.
///
/// The result is delivered to the host Activity's own callback; the bridge does
/// not intercept it. Returns `true` when the picker was launched.
pub fn launch_file_dialog(mime_type: &str) -> bool {
    if !is_initialized() {
        log::info!("[android-jni] launch_file_dialog: bridge not initialized");
        return false;
    }
    with_jni_env(|env| launch_file_dialog_with_env(env, mime_type)).flatten().unwrap_or(false)
}

fn launch_file_dialog_with_env(env: &mut jni::JNIEnv<'_>, mime_type: &str) -> Option<bool> {
    let context = {
        let slot = activity_context_slot().lock().expect("activity context lock poisoned");
        slot.as_ref()?.clone()
    };
    let context_obj = context.as_obj();

    // A Context that is not an Activity cannot start a result launcher. Report
    // this precisely instead of pretending the request was serviced.
    let activity_class = match env.find_class("android/app/Activity") {
        Ok(c) => c,
        Err(e) => {
            log::error!("[android-jni] launch_file_dialog: Activity class missing: {e}");
            return Some(false);
        }
    };
    match env.is_instance_of(context_obj, &activity_class) {
        Ok(true) => {}
        Ok(false) => {
            log::warn!(
                "[android-jni] launch_file_dialog: stored Context is not an Activity, \
                 cannot launch a result launcher; pass the Activity to nativeAttachContext"
            );
            return Some(false);
        }
        Err(e) => {
            log::error!("[android-jni] launch_file_dialog: is_instance_of failed: {e}");
            return Some(false);
        }
    }

    // Intent(ACTION_OPEN_DOCUMENT) + CATEGORY_OPENABLE + setType(mime).
    let intent_class = match env.find_class("android/content/Intent") {
        Ok(c) => c,
        Err(e) => {
            log::error!("[android-jni] launch_file_dialog: Intent class missing: {e}");
            return Some(false);
        }
    };
    let action = env.new_string("android.intent.action.OPEN_DOCUMENT").ok()?;
    let intent = match env.new_object(
        &intent_class,
        "(Ljava/lang/String;)V",
        &[jni::objects::JValue::Object(&action)],
    ) {
        Ok(i) => i,
        Err(e) => {
            log::error!("[android-jni] launch_file_dialog: new Intent failed: {e}");
            return Some(false);
        }
    };

    let category = env.new_string("android.intent.category.OPENABLE").ok()?;
    if let Err(e) = env.call_method(
        &intent,
        "addCategory",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[jni::objects::JValue::Object(&category)],
    ) {
        log::warn!("[android-jni] launch_file_dialog: addCategory failed: {e}");
    }

    // Default to any content when the caller gives no MIME type.
    let mime = if mime_type.is_empty() { "*/*" } else { mime_type };
    let mime_str = env.new_string(mime).ok()?;
    if let Err(e) = env.call_method(
        &intent,
        "setType",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        &[jni::objects::JValue::Object(&mime_str)],
    ) {
        log::warn!("[android-jni] launch_file_dialog: setType failed: {e}");
    }

    // startActivityForResult(Intent, int) — resolved reflectively on the stored
    // object, which the instanceof check above proved is an Activity.
    if let Err(e) = env.call_method(
        context_obj,
        "startActivityForResult",
        "(Landroid/content/Intent;I)V",
        &[
            jni::objects::JValue::Object(&intent),
            jni::objects::JValue::Int(FILE_DIALOG_REQUEST_CODE),
        ],
    ) {
        log::error!("[android-jni] launch_file_dialog: startActivityForResult failed: {e}");
        return Some(false);
    }

    log::info!(
        "[android-jni] launch_file_dialog: ACTION_OPEN_DOCUMENT launched (mime={mime}, \
         requestCode={FILE_DIALOG_REQUEST_CODE})"
    );
    Some(true)
}

/// Update the message of a dialog created by [`create_native_dialog`].
pub fn set_native_dialog_message(id: ObjectId, message: &str) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let dialog = global.as_obj();
        let java_message = match env.new_string(message) {
            Ok(t) => t,
            Err(e) => {
                log::error!(
                    "[android-jni] set_native_dialog_message({id}): new_string failed: {e}"
                );
                return false;
            }
        };
        env.call_method(
            dialog,
            "setMessage",
            "(Ljava/lang/CharSequence;)V",
            &[jni::objects::JValue::Object(&java_message)],
        )
        .is_ok()
    })
    .unwrap_or(false)
}

/// Show or dismiss a dialog created by [`create_native_dialog`].
pub fn set_native_dialog_visible(id: ObjectId, visible: bool) -> bool {
    with_jni_env(|env| {
        let global = match lookup_view(id) {
            Some(g) => g,
            None => return false,
        };
        let dialog = global.as_obj();
        let method = if visible { "show" } else { "dismiss" };
        env.call_method(dialog, method, "()V", &[]).is_ok()
    })
    .unwrap_or(false)
}

/// Dismiss and release a dialog created by [`create_native_dialog`].
pub fn destroy_native_dialog(id: ObjectId) -> bool {
    if lookup_view(id).is_none() {
        return false;
    }
    let _ = set_native_dialog_visible(id, false);
    unregister_view(id);
    true
}

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

/// Hand the host Activity `Context` to the Rust-side view factory.
///
/// This enables the Rust→Java direction: once the Context is stored,
/// `AndroidPlatform::jni_available()` becomes true and `platform_impl`'s
/// `create_*` methods construct real Android views through
/// [`create_native_view`].
///
/// Returns `true` when the Context was stored.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeAttachContext(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    context: jni::objects::JObject,
) -> jni::sys::jboolean {
    init_logging();
    if set_activity_context(&mut env, &context) {
        jni::sys::JNI_TRUE
    } else {
        jni::sys::JNI_FALSE
    }
}

/// Run the Rust-side `AndroidPlatform` create path for every native kind.
///
/// This exercises `platform_impl` → `attach_native_view` → JNI, i.e. the
/// direction that does not go through a per-call Java entry point. Returns the
/// number of widgets created, or a negative value on the first failure so the
/// caller can distinguish "not ready" (`-1`) from a partial failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSelfTestKinds(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    init_logging();
    if !native_view_creation_ready() {
        log::error!("[android-jni] nativeSelfTestKinds: bridge not ready");
        return -1;
    }

    // The `AndroidPlatform` backend only exists on Android; on other hosts the
    // bridge is compiled for unit testing only, where there is no platform to
    // drive. Report "unsupported" rather than pretending to have run.
    #[cfg(not(target_os = "android"))]
    {
        log::warn!("[android-jni] nativeSelfTestKinds: not an Android target");
        -1
    }

    #[cfg(target_os = "android")]
    {
        use crate::platform::android::AndroidPlatform;
        use crate::platform::Platform;

        /// One `(name, constructor)` pair in the kind sweep below.
        type KindCreator = (&'static str, fn(&AndroidPlatform, u64) -> u64);

        let platform = AndroidPlatform::new();
        platform.init();

        let window = platform.create_window("selftest", 0, 0, 320, 640);
        if window == 0 {
            return -2;
        }

        // (name, constructor)
        let creators: [KindCreator; 7] = [
            ("Button", |p, w| p.create_button(w, "b", 0, 0, 100, 40)),
            ("Label", |p, w| p.create_label(w, "l", 0, 50, 100, 40)),
            ("LineEdit", |p, w| p.create_line_edit(w, "e", 0, 100, 100, 40)),
            ("CheckBox", |p, w| p.create_checkbox(w, "c", 0, 150, 100, 40)),
            ("RadioButton", |p, w| p.create_radio_button(w, "r", 0, 200, 100, 40)),
            ("ProgressBar", |p, w| p.create_progress_bar(w, 0, 250, 100, 40)),
            ("Slider", |p, w| p.create_slider(w, 0, 300, 100, 40)),
        ];

        let mut created = 0i32;
        for (name, create) in creators {
            let id = create(&platform, window);
            if id == 0 {
                log::error!("[android-jni] nativeSelfTestKinds: create {name} failed");
                return -3;
            }
            // A native view must actually back the logical handle.
            if platform.native_view_of(id).is_none() {
                log::error!("[android-jni] nativeSelfTestKinds: {name} has no native view");
                return -4;
            }
            created += 1;
            log::info!("[android-jni] nativeSelfTestKinds: {name} -> logical={id} ok");
        }
        created
    }
}

/// Exercise the Rust-side dialog path: create an `AlertDialog` through
/// `AndroidPlatform::create_message_box`, then update/dismiss/show it.
///
/// Returns `1` on success, or a negative value on the first failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSelfTestDialog(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    init_logging();
    if !native_view_creation_ready() {
        log::error!("[android-jni] nativeSelfTestDialog: bridge not ready");
        return -1;
    }

    #[cfg(not(target_os = "android"))]
    {
        log::warn!("[android-jni] nativeSelfTestDialog: not an Android target");
        -1
    }

    #[cfg(target_os = "android")]
    {
        use crate::platform::android::AndroidPlatform;
        use crate::platform::Platform;

        let platform = AndroidPlatform::new();
        platform.init();

        let window = platform.create_window("dialog-test", 0, 0, 320, 640);
        if window == 0 {
            return -2;
        }
        let message_box =
            platform.create_message_box(window, "Self Test", "Hello from Rust", 0, 0, 240, 160);
        if message_box == 0 {
            return -3;
        }
        // The dialog must be a real native object, not just a logical handle.
        if platform.native_view_of(message_box).is_none() {
            log::error!("[android-jni] nativeSelfTestDialog: no native dialog");
            return -4;
        }
        // Body update must reach the AlertDialog setMessage().
        platform.set_widget_text(message_box, "Updated by Rust");
        // Hide/show must reach dismiss()/show().
        platform.hide_widget(message_box);
        platform.show_widget(message_box);
        log::info!("[android-jni] nativeSelfTestDialog: ok");
        1
    }
}

/// Exercise the Rust-side file-dialog path: create a file dialog through
/// `AndroidPlatform::create_file_dialog`, which must launch
/// `ACTION_OPEN_DOCUMENT` on the stored Activity.
///
/// Returns `1` on success, or a negative value on the first failure.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeSelfTestFileDialog(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    init_logging();
    if !native_view_creation_ready() {
        log::error!("[android-jni] nativeSelfTestFileDialog: bridge not ready");
        return -1;
    }

    #[cfg(not(target_os = "android"))]
    {
        log::warn!("[android-jni] nativeSelfTestFileDialog: not an Android target");
        -1
    }

    #[cfg(target_os = "android")]
    {
        use crate::platform::android::AndroidPlatform;
        use crate::platform::Platform;

        let platform = AndroidPlatform::new();
        platform.init();

        let window = platform.create_window("file-dialog-test", 0, 0, 320, 640);
        if window == 0 {
            return -2;
        }
        // Creation requests the system picker; the result is delivered to the
        // host Activity's own callback.
        let file_dialog = platform.create_file_dialog(window, 0, 0, 240, 160);
        if file_dialog == 0 {
            log::error!("[android-jni] nativeSelfTestFileDialog: logical handle not created");
            return -3;
        }
        // Exercise the launch path directly so the return value is observable
        // (create_file_dialog only logs on failure).
        if !launch_file_dialog("*/*") {
            log::error!("[android-jni] nativeSelfTestFileDialog: picker not launched");
            return -4;
        }
        log::info!("[android-jni] nativeSelfTestFileDialog: ok");
        1
    }
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

    #[test]
    fn test_view_class_paths_are_android_widgets() {
        // Every mapped class must live in the `android/widget` package, which is
        // what `JNIEnv::find_class` expects (slash-separated, no `.class`).
        let classes = [
            AndroidViewClass::Button,
            AndroidViewClass::TextView,
            AndroidViewClass::EditText,
            AndroidViewClass::CheckBox,
            AndroidViewClass::RadioButton,
            AndroidViewClass::SeekBar,
            AndroidViewClass::ProgressBar,
            AndroidViewClass::Spinner,
            AndroidViewClass::ListView,
            AndroidViewClass::ScrollView,
            AndroidViewClass::NumberPicker,
            AndroidViewClass::FrameLayout,
        ];
        for class in classes {
            assert!(
                class.jni_class_path().starts_with("android/widget/"),
                "unexpected class path {}",
                class.jni_class_path()
            );
        }
    }

    #[test]
    fn test_supports_text_matches_settext_capable_views() {
        assert!(AndroidViewClass::Button.supports_text());
        assert!(AndroidViewClass::TextView.supports_text());
        assert!(AndroidViewClass::EditText.supports_text());
        assert!(AndroidViewClass::CheckBox.supports_text());
        assert!(AndroidViewClass::RadioButton.supports_text());
        assert!(!AndroidViewClass::SeekBar.supports_text());
        assert!(!AndroidViewClass::ProgressBar.supports_text());
        assert!(!AndroidViewClass::ScrollView.supports_text());
        assert!(!AndroidViewClass::FrameLayout.supports_text());
    }

    #[test]
    fn test_native_view_helpers_noop_without_jvm() {
        // No `JavaVM` has been stored in this unit-test process, so the
        // Rust-callable helpers must report failure instead of panicking.
        assert!(!is_initialized());
        assert!(!has_activity_context());
        assert!(!native_view_creation_ready());
        assert_eq!(create_native_view(AndroidViewClass::Button, "x", 0, 0, 10, 10), None);
        assert!(!destroy_native_view(1234));
        assert!(!set_native_view_text(1234, "x"));
        assert!(!set_native_view_bounds(1234, 0, 0, 10, 10));
        assert!(!set_native_view_visibility(1234, true));
        assert!(!set_native_view_enabled(1234, true));
        assert!(!append_spinner_item(1234, "item", true));
        assert!(!append_list_item(1234, &["item"]));
        // File dialogs need an Activity to launch a result launcher; with no JVM
        // there is none, so the request must report failure rather than claim
        // success.
        assert!(!launch_file_dialog("*/*"));
    }

    #[test]
    fn test_native_view_creation_ready_requires_both_vm_and_context() {
        // The readiness predicate is the AND of the two prerequisites. With no
        // JVM it must be false regardless of the context slot; this pins the
        // contract that `AndroidPlatform::jni_available` relies on, so a future
        // change cannot make it true on only one of the two conditions.
        assert_eq!(native_view_creation_ready(), is_initialized() && has_activity_context());
        assert!(!native_view_creation_ready());
    }

    #[test]
    fn test_android_integration_ready_reports_unready_without_jvm() {
        let status = android_integration_ready();
        assert!(!status.jni_initialized);
        assert_eq!(status.native_methods_count, 13);
        assert!(!status.ready);
    }

    #[test]
    fn test_native_kinds_map_to_view_classes() {
        use AndroidLogicalKind::*;
        assert_eq!(view_class_for(Button), Some(AndroidViewClass::Button));
        assert_eq!(view_class_for(Label), Some(AndroidViewClass::TextView));
        assert_eq!(view_class_for(StatusBar), Some(AndroidViewClass::TextView));
        assert_eq!(view_class_for(LineEdit), Some(AndroidViewClass::EditText));
        assert_eq!(view_class_for(CheckBox), Some(AndroidViewClass::CheckBox));
        assert_eq!(view_class_for(RadioButton), Some(AndroidViewClass::RadioButton));
        assert_eq!(view_class_for(Slider), Some(AndroidViewClass::SeekBar));
        assert_eq!(view_class_for(ProgressBar), Some(AndroidViewClass::ProgressBar));
        assert_eq!(view_class_for(ComboBox), Some(AndroidViewClass::Spinner));
        assert_eq!(view_class_for(ListBox), Some(AndroidViewClass::ListView));
        assert_eq!(view_class_for(ListView), Some(AndroidViewClass::ListView));
        assert_eq!(view_class_for(ScrollArea), Some(AndroidViewClass::ScrollView));
        assert_eq!(view_class_for(SpinBox), Some(AndroidViewClass::NumberPicker));
        assert_eq!(view_class_for(Panel), Some(AndroidViewClass::FrameLayout));
        assert_eq!(view_class_for(Window), Some(AndroidViewClass::FrameLayout));
    }

    #[test]
    fn test_logical_only_kinds_have_no_native_view() {
        use AndroidLogicalKind::*;
        for kind in
            [MenuBar, Menu, MenuItem, ToolBar, MessageBox, FileDialog, ColorDialog, FontDialog]
        {
            assert_eq!(view_class_for(kind), None, "{kind:?} must not claim a native view");
        }
    }
}
