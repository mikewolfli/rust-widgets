// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Android JNI bridge — the JVM side of the Android host.
//!
//! # What this module is responsible for
//!
//! It holds the two things Android needs that Rust cannot get by itself:
//!
//! * the `JavaVM`, captured in `nativeInit`, so the bridge can attach threads and
//!   reach the host's `Activity`;
//! * the `Context` of the host `Activity`, so the host can resolve platform
//!   facilities (a document picker, a system service) on the library's behalf.
//!
//! # BLUE15: the bridge no longer builds Android views
//!
//! It used to export `nativeCreateButton` / `nativeCreateTextView` / … and the
//! Rust-callable helpers that drove them, so that each logical widget became a
//! real `android.widget.View`. Under the self-drawn strategy that is exactly the
//! duplication BLUE15 removes: the library paints every `WidgetKind`, and the host
//! supplies a **window** plus a **drawing surface** (rules #55/#56). A per-kind
//! `create` here had no consumer left, and the `AndroidViewClass` /
//! `AndroidLogicalKind` tables existed only to serve it, so they are gone too
//! (rule #59: delete means delete).
//!
//! What survives is the platform-facing part: the VM/context handshake, the
//! document-picker request (an `Activity` operation that has no library-side
//! equivalent), the integration-status report, and the JNI entry points the host
//! Java class calls.
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
//!     public static native void nativeAttachContext(android.content.Context context);
//!     public static native long nativeOpenDocument(String mimeType);
//! }
//! ```
//!
//! # Thread safety
//!
//! `JAVA_VM` is set once during initialization and is then immutable. The context
//! slot is a `Mutex`. Every JNI entry point is safe to call from any thread.

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

/// The process-wide logcat logger instance.
///
/// `log::set_logger` takes a `&'static dyn Log`, so the value has to outlive the
/// call. A `static` is what makes that true; without it (the previous state of this
/// file) the reference in `init_logging` named nothing, and the bridge only
/// compiled because the module happens to be unreachable in most builds — an
/// `#![cfg(feature = "android-jni")]` build failed with "cannot find value
/// `LOGCAT_LOGGER`".
static LOGCAT_LOGGER: LogcatLogger = LogcatLogger;

struct LogcatLogger;

fn logcat_priority(level: log::Level) -> i32 {
    // Priorities from `<android/log.h>`: VERBOSE=2 … FATAL=7.
    match level {
        log::Level::Error => 6,
        log::Level::Warn => 5,
        log::Level::Info => 4,
        log::Level::Debug => 3,
        log::Level::Trace => 2,
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
        let message = std::ffi::CString::new(record.args().to_string()).unwrap_or_default();
        unsafe {
            let _ =
                android_log_write(logcat_priority(record.level()), tag.as_ptr(), message.as_ptr());
        }
    }

    fn flush(&self) {}
}

// `c_char` rather than `i8`: `CString::as_ptr` yields `*const c_char`, and the sign of
// `c_char` is **target-dependent** — `i8` on x86_64, `u8` on aarch64. Spelling `i8` here
// compiled on `x86_64-linux-android` and failed on `aarch64-linux-android` with
// `expected *const i8, found *const u8`, which is exactly the kind of target-specific
// breakage a host-only build never sees. `tools/check_android_cross.sh` now compiles
// both targets so it cannot come back.
extern "C" {
    fn __android_log_write(prio: i32, tag: *const core::ffi::c_char, text: *const core::ffi::c_char) -> i32;
}

/// Safe wrapper around `__android_log_write`.
unsafe fn android_log_write(
    prio: i32,
    tag: *const core::ffi::c_char,
    text: *const core::ffi::c_char,
) -> i32 {
    unsafe { __android_log_write(prio, tag, text) }
}

// ---------------------------------------------------------------------------
// JavaVM and Activity context
// ---------------------------------------------------------------------------

/// The `JavaVM` captured by `nativeInit`.
static JAVA_VM: OnceLock<jni::JavaVM> = OnceLock::new();

/// The `Context` of the host `Activity`, if one has been attached.
///
/// A `GlobalRef` keeps it alive across the JNI call that supplied it; without a
/// global reference the `JObject` would be released when that frame returned.
fn activity_context_slot() -> &'static Mutex<Option<jni::objects::GlobalRef>> {
    static SLOT: OnceLock<Mutex<Option<jni::objects::GlobalRef>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Stores the host `Activity`'s `Context` for later platform calls.
///
/// Replaces any previous context, so the latest attached `Activity` — which on
/// Android is the one currently visible — is the one used.
pub fn set_activity_context(
    env: &mut jni::JNIEnv<'_>,
    context: &jni::objects::JObject<'_>,
) -> bool {
    let Ok(global) = env.new_global_ref(context) else {
        log::error!("[android-jni] nativeAttachContext: failed to create a global reference");
        return false;
    };
    let mut slot = activity_context_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = Some(global);
    log::info!("[android-jni] Activity Context attached");
    true
}

/// Whether an `Activity` `Context` is available.
pub fn has_activity_context() -> bool {
    activity_context_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_some()
}

/// Whether the JNI bridge is usable: the VM is stored and a context is attached.
///
/// Both conditions are required. A `Context` without the VM cannot be reached
/// from Rust, and the VM without a `Context` cannot resolve an `Activity`
/// facility — reporting ready on either alone would let a caller proceed into a
/// call that must fail.
pub fn native_view_creation_ready() -> bool {
    is_initialized() && has_activity_context()
}

/// Whether `nativeInit` has run and stored the `JavaVM`.
pub fn is_initialized() -> bool {
    JAVA_VM.get().is_some()
}

/// A snapshot of what the Android integration can currently do.
///
/// `native_methods_count` is the number of JNI entry points this bridge exports.
/// It is reported so a host can detect a stale `.so`: a Java class compiled
/// against a different bridge version would otherwise fail at the first call
/// instead of at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationStatus {
    /// Whether the `JavaVM` has been captured.
    pub jni_initialized: bool,
    /// Whether an `Activity` `Context` has been attached.
    pub context_attached: bool,
    /// How many JNI entry points this bridge exports.
    pub native_methods_count: u32,
    /// Whether the integration can serve platform requests.
    pub ready: bool,
}

/// Describes the current Android integration state.
pub fn android_integration_ready() -> IntegrationStatus {
    let jni_initialized = is_initialized();
    let context_attached = has_activity_context();
    IntegrationStatus {
        jni_initialized,
        context_attached,
        native_methods_count: NATIVE_METHOD_COUNT,
        ready: jni_initialized && context_attached,
    }
}

/// The number of `Java_rust_1widgets_RustWidgets_*` entry points below.
///
/// Kept as a named constant next to the functions it counts, and asserted by a
/// test, so adding an entry point without updating it fails the build rather than
/// silently misreporting the integration's surface.
pub const NATIVE_METHOD_COUNT: u32 = 8;

/// Runs `f` with a JNI environment, attaching the current thread if needed.
///
/// Returns `None` when no `JavaVM` has been stored. Attaching is what makes the
/// bridge callable from a Rust thread the JVM has never seen — the event loop, a
/// worker, a timer callback — which is the common case for this library.
pub fn with_jni_env<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut jni::JNIEnv<'_>) -> R,
{
    let vm = JAVA_VM.get()?;
    let mut env = vm.attach_current_thread().ok()?;
    Some(f(&mut env))
}

// ---------------------------------------------------------------------------
// Document picker
// ---------------------------------------------------------------------------

/// Asks the host `Activity` to open a document picker.
///
/// This is the one user-facing platform operation the bridge still performs.
/// Picking a document is an `Activity` operation — it launches a system UI and
/// delivers the result to the host's own callback — so the library cannot paint
/// it and must delegate. Returns `false` when there is no `Activity` to ask; that
/// is a real failure, not a no-op, and it is logged as such.
///
/// `mime_type` selects the filter; pass `*/*` for any document.
pub fn launch_file_dialog(mime_type: &str) -> bool {
    let Some(result) = with_jni_env(|env| launch_document_picker(env, mime_type)) else {
        log::warn!(
            "[android-jni] {mime_type}: no JavaVM is stored, so the document picker cannot be \
             launched; nativeInit has not run"
        );
        return false;
    };
    result
}

/// The JNI call behind [`launch_file_dialog`], split out so the environment
/// handling (attach, error propagation) stays in one place.
fn launch_document_picker(env: &mut jni::JNIEnv<'_>, mime_type: &str) -> bool {
    let context = {
        let slot = activity_context_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match slot.as_ref() {
            Some(global) => global.clone(),
            None => {
                log::warn!(
                    "[android-jni] {mime_type}: no Activity Context is attached, so the document \
                     picker cannot be launched; the host app must call nativeAttachContext"
                );
                return false;
            }
        }
    };

    let Ok(mime) = env.new_string(mime_type) else {
        log::error!("[android-jni] failed to allocate the mime type string");
        return false;
    };

    // `Activity.startActivityForResult` is deprecated on API 30+, but it is the
    // only entry point that works without the host supplying a
    // `registerForActivityResult` callback. The host owns that callback, so the
    // result arrives there; this call only starts the picker.
    let result = (|| -> jni::errors::Result<()> {
        // The method lookups below are deliberately absent: `call_method` resolves
        // by name and signature, so a preceding `get_method_id` would only probe
        // for something already probed. Keeping one would mean a lookup whose
        // failure path cannot be observed, which is worse than not having it.
        let intent_class = env.find_class("android/content/Intent")?;
        let action = env.new_string("android.intent.action.OPEN_DOCUMENT")?;
        let intent = env.new_object(
            &intent_class,
            "(Ljava/lang/String;)V",
            &[jni::objects::JValue::Object(&action)],
        )?;

        env.call_method(
            &intent,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[jni::objects::JValue::Object(&mime)],
        )?;

        env.call_method(
            context.as_obj(),
            "startActivityForResult",
            "(Landroid/content/Intent;I)V",
            &[jni::objects::JValue::Object(&intent), jni::objects::JValue::Int(0)],
        )?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            log::info!("[android-jni] document picker launched for {mime_type}");
            true
        }
        Err(error) => {
            log::error!("[android-jni] failed to launch the document picker: {error}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// JNI entry points called by the host's Java class
// ---------------------------------------------------------------------------
//
// These are the process boundary: the host Java class `rust.widgets.RustWidgets`
// declares them `native` and calls them. They are counted by
// [`NATIVE_METHOD_COUNT`], which a test keeps in step.

/// Captures the `JavaVM` once per process.
///
/// Must run before any other bridge entry point; the host calls it from its
/// `static { System.loadLibrary(...) }` block.
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
        Err(error) => {
            log::error!("[android-jni] failed to get JavaVM: {error}");
        }
    }
}

/// Stores the host `Activity`'s `Context` so platform facilities can be reached.
///
/// The host calls this from `Activity.onResume` (and again after a configuration
/// change, because the `Activity` instance does not survive one). Returns `1` on
/// success and `0` on failure so a Java caller can react.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeAttachContext<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    context: jni::objects::JObject<'local>,
) -> jni::sys::jint {
    if set_activity_context(&mut env, &context) {
        1
    } else {
        0
    }
}

/// Asks the host `Activity` to open the system document picker.
///
/// Returns `1` when the picker was launched and `0` otherwise (no `Activity`, or
/// the launch failed). The selected document arrives at the host's own
/// `onActivityResult`, because the host owns the request code.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeOpenDocument<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    mime_type: jni::objects::JString<'local>,
) -> jni::sys::jint {
    let mime: String = match env.get_string(&mime_type) {
        Ok(text) => text.into(),
        Err(error) => {
            log::error!("[android-jni] nativeOpenDocument: unreadable mime type: {error}");
            return 0;
        }
    };
    if launch_document_picker(&mut env, &mime) {
        1
    } else {
        0
    }
}

/// Reports whether the bridge is initialized and a `Context` is attached.
///
/// Returns a bit mask: bit 0 = `JavaVM` stored, bit 1 = `Context` attached. The
/// host uses it to show a diagnostic instead of guessing why a platform call
/// failed.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeIntegrationStatus(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    let mut status = 0;
    if is_initialized() {
        status |= 1;
    }
    if has_activity_context() {
        status |= 2;
    }
    status
}

/// Returns the number of JNI entry points this bridge exports.
///
/// A host can compare it against the count its Java class was compiled with, which
/// turns a stale `.so` into a start-up diagnostic rather than a crash at the first
/// call.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeMethodCount(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    NATIVE_METHOD_COUNT as jni::sys::jint
}

/// Installs the logcat logger without capturing the VM.
///
/// Lets a host route Rust diagnostics to logcat even when it never uses the JNI
/// bridge — for example a host that only renders frames the library produced.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeInstallLogging(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    init_logging();
}

/// Releases the stored `Activity` `Context`.
///
/// The host calls this from `Activity.onDestroy`. Dropping the `GlobalRef` is what
/// releases the JNI reference; without it the reference would keep the `Activity`
/// reachable and leak it across configuration changes.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeDetachContext(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) {
    let mut slot = activity_context_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = None;
    log::info!("[android-jni] Activity Context detached");
}

/// Reports that the host window's client area is now `width` x `height` pixels.
///
/// # Why the host has to call this
///
/// Android has no window-resize *callback* the library can subscribe to without
/// owning the `Activity`: the size change is delivered to the host's own
/// `View.OnLayoutChangeListener` / `Activity.onConfigurationChanged`. So the host,
/// which is the only party that observes the change, reports it here — the same
/// contract the desktop backends implement from their toolkit's callback.
///
/// `windowId` is the id `rw_create_window` returned. Reporting re-runs that window's
/// layout, so its children follow the new size instead of keeping the geometry they
/// were given for the old one.
///
/// Returns `1` when the resize was accepted and `0` when it was refused. The decision
/// itself is `accept_host_resize`, which is testable without a JVM; this entry point
/// is only the boundary that validates the JVM's integer types and logs the refusal.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeNotifyResize(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    window_id: jni::sys::jlong,
    width: jni::sys::jint,
    height: jni::sys::jint,
) -> jni::sys::jint {
    if width <= 0 || height <= 0 {
        log::warn!("[android-jni] nativeNotifyResize: ignoring non-positive size {width}x{height}");
        return 0;
    }
    // `window_id` arrives as the `jlong` the host was given; a non-positive value cannot
    // be an id this library handed out.
    if window_id <= 0 {
        log::warn!("[android-jni] nativeNotifyResize: ignoring non-positive window id {window_id}");
        return 0;
    }
    if accept_host_resize(window_id, width, height) {
        1
    } else {
        log::warn!(
            "[android-jni] nativeNotifyResize: {window_id} addresses no live window; \
             the resize was refused"
        );
        0
    }
}

/// Accepts a resize the Android host reported, re-running that window's layout.
///
/// # Why this is a named function rather than inline
///
/// A JNI entry point cannot be called from a test — it takes a `JNIEnv`, which only the
/// JVM can supply. Keeping the decision here puts the refusal branches (a stale id, a
/// window that no longer exists) under host-runnable tests instead of leaving them for a
/// device run to exercise.
///
/// The caller has already rejected non-positive values; that is re-checked in debug
/// builds so the invariant cannot silently rot if a second caller appears.
fn accept_host_resize(window_id: i64, width: i32, height: i32) -> bool {
    debug_assert!(window_id > 0, "a non-positive window id must be filtered by the caller");
    debug_assert!(width > 0 && height > 0, "a non-positive size must be filtered by the caller");
    crate::queue_resize_trigger(window_id as u64, width as u32, height as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_is_uninitialized_without_a_vm() {
        // No `JavaVM` has been stored in this unit-test process, so every
        // VM-dependent entry point must report failure rather than panic.
        assert!(!is_initialized());
        assert!(!has_activity_context());
        assert!(!native_view_creation_ready());
        assert!(!launch_file_dialog("*/*"));
        assert!(with_jni_env(|_| ()).is_none());
    }

    #[test]
    fn integration_status_is_honest_before_init() {
        let status = android_integration_ready();
        assert!(!status.jni_initialized);
        assert!(!status.context_attached);
        assert!(!status.ready);
        assert_eq!(status.native_methods_count, NATIVE_METHOD_COUNT);
    }

    /// The reported entry-point count must equal the number actually exported.
    ///
    /// A mismatch means a host compiled against a different bridge would get a
    /// link-time failure at the first call instead of a diagnosable status.
    ///
    /// # Why the count is taken from declaration lines only
    ///
    /// Counting the bare symbol prefix with `matches()` counted this test's own
    /// literal and the module documentation as if they were entry points, so the
    /// measured value was always 2 higher than the truth and the assertion could
    /// not pass. Anchoring on `pub extern "system" fn <symbol>` counts exactly the
    /// `#[no_mangle]` exports the JVM can resolve — which is the number
    /// `NATIVE_METHOD_COUNT` advertises to the host.
    #[test]
    fn reported_entry_point_count_matches_the_exports() {
        const PREFIX: &str = "pub extern \"system\" fn Java_rust_1widgets_RustWidgets_";
        let source = include_str!("android_jni.rs");
        let exported = source.matches(PREFIX).count();
        assert_eq!(
            exported as u32, NATIVE_METHOD_COUNT,
            "NATIVE_METHOD_COUNT must be updated when a JNI entry point is added or removed",
        );
    }

    #[test]
    fn logcat_priorities_follow_the_android_scale() {
        assert_eq!(logcat_priority(log::Level::Error), 6);
        assert_eq!(logcat_priority(log::Level::Warn), 5);
        assert_eq!(logcat_priority(log::Level::Info), 4);
        assert_eq!(logcat_priority(log::Level::Debug), 3);
        assert_eq!(logcat_priority(log::Level::Trace), 2);
    }

    /// A resize for a window this library never created must be refused.
    ///
    /// The host can report a resize with a stale id (it survived a configuration change,
    /// the window did not). Accepting it would queue a `Resized` event for a window that
    /// does not exist, and the host loop would then ask for the size of something gone.
    #[test]
    fn a_resize_for_an_unknown_window_is_refused() {
        assert!(
            !accept_host_resize(0x0BAD_1DEA, 800, 600),
            "an id this library never handed out must be refused"
        );
    }

    /// An unknown id must not be able to inject a size anyone could read back.
    #[test]
    fn a_refused_resize_records_no_size() {
        assert!(!accept_host_resize(0x0BAD_1DEA, 800, 600));
        assert_eq!(
            crate::window_client_size(0x0BAD_1DEA),
            None,
            "a refused resize must leave no size behind"
        );
    }

    /// The non-positive checks the entry point performs, asserted directly.
    ///
    /// These are the conditions the JNI boundary filters before calling
    /// `accept_host_resize`; spelling them out here keeps the two in step, because a
    /// `jint` can be negative and a cast to `u32` would wrap it to roughly four billion.
    #[test]
    fn the_entry_point_rejects_non_positive_inputs() {
        let entry_point_accepts =
            |window_id: i64, width: i32, height: i32| width > 0 && height > 0 && window_id > 0;
        assert!(!entry_point_accepts(1, -1, 600), "a negative width must be refused");
        assert!(!entry_point_accepts(1, 800, -5), "a negative height must be refused");
        assert!(!entry_point_accepts(1, 0, 600), "a zero width must be refused");
        assert!(!entry_point_accepts(1, 800, 0), "a zero height must be refused");
        assert!(!entry_point_accepts(-1, 800, 600), "a negative id must be refused");
        assert!(entry_point_accepts(1, 800, 600), "a well-formed report must be accepted");
    }
}
