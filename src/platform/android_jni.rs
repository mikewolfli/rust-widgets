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
//! package rust_widgets;
//!
//! public class RustWidgets {
//!     static { System.loadLibrary("rust_widgets"); }
//!
//!     public static native void nativeInit();
//!     // `int`, not `void`/`boolean`: the Rust exports return `jint` (1/0). See
//!     // `bindings/android/java/rust_widgets/RustWidgets.java` for why the return type is
//!     // ABI rather than decoration, and `tools/check_jni_signatures.sh` for the gate that
//!     // now compares it.
//!     public static native int nativeAttachContext(android.content.Context context);
//!     public static native int nativeOpenDocument(String mimeType);
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
        // SAFETY: both pointers come from live `CString`s in this scope and are NUL-terminated.
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
    fn __android_log_write(
        prio: i32,
        tag: *const core::ffi::c_char,
        text: *const core::ffi::c_char,
    ) -> i32;
}

/// Thin FFI forwarder to `__android_log_write`.
///
/// # Safety
/// `tag` and `text` must be valid NUL-terminated C strings that outlive the call.
unsafe fn android_log_write(
    prio: i32,
    tag: *const core::ffi::c_char,
    text: *const core::ffi::c_char,
) -> i32 {
    // SAFETY: forwarded from `android_log_write`'s contract: both pointers are valid NUL-terminated strings.
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
pub const NATIVE_METHOD_COUNT: u32 = 17;

/// The `startActivityForResult` request code the document picker uses.
///
/// The host's `onActivityResult` must compare against this constant; it is published here so the
/// Rust launcher and the Java host cannot drift. `21079` is arbitrary but stable.
pub const PICK_REQUEST_CODE: i32 = 21079;

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

/// Prints `job_file` through Android's `PrintManager` (BLUE28 B28.27).
///
/// Android printing needs a `PrintDocumentAdapter`, which cannot be constructed from Rust without
/// defining a Java class. The adapter lives in `rust_widgets.RustWidgets.printFile(Context,
/// String)`: it streams the job file's bytes (a PDF or an image produced by the library) to the
/// destination the print UI chose and reports completion through the adapter callbacks. The
/// print UI itself is the system `PrintManager.print`, so this hands the file to the real queue
/// rather than claiming a job that was never submitted.
///
/// Returns the failure reason when there is no attached `Context`, the file is not readable, or
/// the Java helper refused the job.
#[cfg(feature = "android-jni")]
pub fn print_file(job_file: &std::path::Path) -> Result<(), String> {
    if !job_file.is_file() {
        return Err(format!("print job '{}' is not a readable file", job_file.display()));
    }
    let context = activity_context_slot()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
        .ok_or_else(|| "printing needs an attached Activity context".to_string())?;
    let path = job_file.to_string_lossy().into_owned();
    with_jni_env(|env| {
        let class = env
            .find_class("rust_widgets/RustWidgets")
            .map_err(|e| format!("cannot resolve rust_widgets.RustWidgets: {e}"))?;
        let path = env
            .new_string(path)
            .map_err(|e| format!("cannot create the print path string: {e}"))?;
        let result = env
            .call_static_method(
                class,
                "printFile",
                "(Landroid/content/Context;Ljava/lang/String;)I",
                &[
                    jni::objects::JValue::Object(context.as_obj()),
                    jni::objects::JValue::Object(&path),
                ],
            )
            .map_err(|e| format!("printFile call failed: {e}"))?
            .i()
            .map_err(|e| format!("printFile returned a non-int: {e}"))?;
        if result == 1 {
            Ok(())
        } else {
            Err(format!("the system print manager refused '{}'", job_file.display()))
        }
    })
    .unwrap_or_else(|| Err("no JavaVM is stored; nativeInit has not run".to_string()))
}

/// Whether the Android host can print: a JVM and an Activity context are attached.
#[cfg(feature = "android-jni")]
pub fn print_available() -> bool {
    is_initialized() && has_activity_context()
}

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
            &[jni::objects::JValue::Object(&intent), jni::objects::JValue::Int(PICK_REQUEST_CODE)],
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
// System clipboard
// ---------------------------------------------------------------------------

/// The Java service name behind `Context.CLIPBOARD_SERVICE`.
///
/// Spelled out rather than reaching for the constant: the constant lives in `android.content.Context`,
/// which would mean another class lookup to read a field whose value is a fixed string in the
/// platform's public API.
const CLIPBOARD_SERVICE: &str = "clipboard";

/// Whether the bridge can reach the system clipboard.
///
/// Both halves are required and neither is sufficient: the `JavaVM` to attach a thread and call
/// into Java at all, and the `Activity` `Context` because `Context.getSystemService` is what hands
/// out the `ClipboardManager`.
pub fn clipboard_ready() -> bool {
    is_initialized() && has_activity_context()
}

/// Puts `text` on the system clipboard.
///
/// * `None` — the bridge is not ready (no `JavaVM`, or no attached `Activity` `Context`), so no
///   request was made. A caller must not read this as "the write failed": it means this process
///   has no host clipboard *yet*.
/// * `Some(true)` / `Some(false)` — the host was asked, and this is its answer.
///
/// The answer is a **read-back**, not the absence of an exception: `setPrimaryClip` returns
/// `void`, and on Android 10+ the system may refuse a clipboard write from a process that is not
/// in the foreground. Reporting success there would be exactly the "reported success for
/// something that did not happen" failure the trait forbids.
pub fn set_clipboard_text(text: &str) -> Option<bool> {
    with_jni_env(|env| write_primary_clip(env, text)).flatten()
}

/// The system clipboard's text, or `None` when it holds none.
///
/// `None` also covers "the bridge is not ready", which is the same fact the synchronous
/// accessor's empty string is honest about: there is nothing this process can read.
pub fn clipboard_text() -> Option<String> {
    with_jni_env(read_primary_clip).flatten()
}

/// The host `Activity`'s `Context`, cloned out of the slot.
fn attached_context() -> Option<jni::objects::GlobalRef> {
    let slot = activity_context_slot().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    slot.clone()
}

/// `Context.getSystemService(Context.CLIPBOARD_SERVICE)`, or `None` when there is no context.
fn clipboard_manager<'local>(
    env: &mut jni::JNIEnv<'local>,
) -> Option<jni::objects::JObject<'local>> {
    let context = attached_context()?;
    let service = env.new_string(CLIPBOARD_SERVICE).ok()?;
    let manager = env
        .call_method(
            context.as_obj(),
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[jni::objects::JValue::Object(&service)],
        )
        .ok()?
        .l()
        .ok()?;
    if manager.is_null() {
        log::warn!("[android-jni] getSystemService(\"clipboard\") returned null");
        return None;
    }
    Some(manager)
}

/// The first item of the primary clip, or `None` when the clipboard holds no clip.
fn primary_clip_item<'local>(
    env: &mut jni::JNIEnv<'local>,
    manager: &jni::objects::JObject<'local>,
) -> Option<jni::objects::JObject<'local>> {
    let has_clip = env.call_method(manager, "hasPrimaryClip", "()Z", &[]).ok()?.z().ok()?;
    if !has_clip {
        return None;
    }
    let clip = env
        .call_method(manager, "getPrimaryClip", "()Landroid/content/ClipData;", &[])
        .ok()?
        .l()
        .ok()?;
    if clip.is_null() {
        return None;
    }
    let item = env
        .call_method(
            &clip,
            "getItemAt",
            "(I)Landroid/content/ClipData$Item;",
            &[jni::objects::JValue::Int(0)],
        )
        .ok()?
        .l()
        .ok()?;
    if item.is_null() {
        log::warn!("[android-jni] the primary clip holds no item");
        return None;
    }
    Some(item)
}

/// Calls a no-argument `CharSequence` accessor on a clip item and reads it as Rust text.
fn item_text(
    env: &mut jni::JNIEnv<'_>,
    item: &jni::objects::JObject<'_>,
    method: &str,
) -> Option<String> {
    let chars = env.call_method(item, method, "()Ljava/lang/CharSequence;", &[]).ok()?.l().ok()?;
    if chars.is_null() {
        return None;
    }
    let text = env.call_method(&chars, "toString", "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
    if text.is_null() {
        return None;
    }
    env.get_string(&jni::objects::JString::from(text)).ok().map(|value| value.into())
}

/// Reads the primary clip's first item as text.
fn read_primary_clip(env: &mut jni::JNIEnv<'_>) -> Option<String> {
    let context = attached_context()?;
    let manager = clipboard_manager(env)?;
    let item = primary_clip_item(env, &manager)?;

    // `coerceToText` is what makes a URI or an `Intent` item answer with its textual form; a
    // plain `CharSequence` item comes back unchanged. Reading the `text` field directly would
    // report `None` for every non-text clip, which is the distinction this exists for.
    let chars = env
        .call_method(
            &item,
            "coerceToText",
            "(Landroid/content/Context;)Ljava/lang/CharSequence;",
            &[jni::objects::JValue::Object(context.as_obj())],
        )
        .ok()?
        .l()
        .ok()?;
    if chars.is_null() {
        return None;
    }
    let text = env.call_method(&chars, "toString", "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
    if text.is_null() {
        return None;
    }
    env.get_string(&jni::objects::JString::from(text)).ok().map(|value| value.into())
}

/// The primary clip's HTML, or `None` when the clip carries none.
fn read_primary_html(env: &mut jni::JNIEnv<'_>) -> Option<String> {
    let manager = clipboard_manager(env)?;
    let item = primary_clip_item(env, &manager)?;
    item_text(env, &item, "getHtmlText")
}

/// The primary `ClipData`, or `None` when the clipboard holds no clip.
fn primary_clip<'local>(
    env: &mut jni::JNIEnv<'local>,
    manager: &jni::objects::JObject<'local>,
) -> Option<jni::objects::JObject<'local>> {
    let has_clip = env.call_method(manager, "hasPrimaryClip", "()Z", &[]).ok()?.z().ok()?;
    if !has_clip {
        return None;
    }
    let clip = env
        .call_method(manager, "getPrimaryClip", "()Landroid/content/ClipData;", &[])
        .ok()?
        .l()
        .ok()?;
    (!clip.is_null()).then_some(clip)
}

/// The MIME types the primary clip's description declares.
///
/// A custom type is negotiated through the **description**, not through the item: the item only
/// holds text, a URI or an `Intent`. Reading the description is what lets a consumer ask "does
/// the clipboard carry `application/vnd.example.payload+json`?" before touching the payload.
fn primary_clip_types(env: &mut jni::JNIEnv<'_>) -> Option<Vec<String>> {
    let manager = clipboard_manager(env)?;
    let clip = primary_clip(env, &manager)?;
    let description = env
        .call_method(&clip, "getDescription", "()Landroid/content/ClipDescription;", &[])
        .ok()?
        .l()
        .ok()?;
    if description.is_null() {
        return None;
    }
    let count = env.call_method(&description, "getMimeTypeCount", "()I", &[]).ok()?.i().ok()?;
    let mut types = Vec::new();
    for index in 0..count {
        let mime = env
            .call_method(
                &description,
                "getMimeType",
                "(I)Ljava/lang/String;",
                &[jni::objects::JValue::Int(index)],
            )
            .ok()?
            .l()
            .ok()?;
        if mime.is_null() {
            continue;
        }
        let text =
            env.call_method(&mime, "toString", "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
        if let Ok(value) = env.get_string(&jni::objects::JString::from(text)) {
            types.push(value.into());
        }
    }
    Some(types)
}

/// Whether the primary clip declares `content_type`.
fn primary_clip_has_type(env: &mut jni::JNIEnv<'_>, content_type: &str) -> bool {
    primary_clip_types(env)
        .is_some_and(|types| types.iter().any(|declared| declared == content_type))
}

/// The first custom MIME type on the primary clip, skipping the two fixed kinds this backend
/// implements separately.
fn primary_clip_custom_type(env: &mut jni::JNIEnv<'_>) -> Option<String> {
    primary_clip_types(env)?
        .into_iter()
        .find(|declared| declared != "text/plain" && declared != "text/html")
}

/// Encodes bytes as a Java `String` without loss.
///
/// `ClipData.Item` has no byte payload: it holds a `CharSequence`, a `Uri` or an `Intent`. A
/// `Uri` would need a `ContentProvider` the library does not own, and an `Intent` cannot carry
/// arbitrary bytes. The text channel can, through a one-to-one byte-to-character mapping
/// (ISO-8859-1): every byte becomes the character with the same code point, and decoding maps
/// each character back. UTF-8 would not round-trip arbitrary bytes, because it is not defined
/// for every byte sequence.
fn bytes_to_latin1(data: &[u8]) -> String {
    data.iter().map(|byte| char::from(*byte)).collect()
}

/// Decodes text written by [`bytes_to_latin1`].
///
/// `None` when a character is outside `0..=0xFF`: that text was not written by this encoding
/// (ordinary user text, for example), and returning its low byte would corrupt it.
fn latin1_to_bytes(text: &str) -> Option<Vec<u8>> {
    text.chars().map(|character| u8::try_from(u32::from(character)).ok()).collect()
}

/// `new ClipData(new ClipDescription(label, [type]), new ClipData.Item(payload))`.
///
/// The constructor, rather than `ClipData.newPlainText`, is what puts a **custom MIME type** on
/// the clipboard: the `new*` factories hardcode `text/plain`, `text/html` or `text/uri-list`.
fn build_custom_clip<'local>(
    env: &mut jni::JNIEnv<'local>,
    content_type: &str,
    data: &[u8],
) -> Option<jni::objects::JObject<'local>> {
    let label = env.new_string("rust-widgets").ok()?;
    let mime = env.new_string(content_type).ok()?;
    let string_class = env.find_class("java/lang/String").ok()?;
    let types = env.new_object_array(1, string_class, &mime).ok()?;

    let description_class = env.find_class("android/content/ClipDescription").ok()?;
    let description = env
        .new_object(
            description_class,
            "(Ljava/lang/CharSequence;[Ljava/lang/String;)V",
            &[jni::objects::JValue::Object(&label), jni::objects::JValue::Object(&types)],
        )
        .ok()?;

    let payload = env.new_string(bytes_to_latin1(data)).ok()?;
    let item_class = env.find_class("android/content/ClipData$Item").ok()?;
    let item = env
        .new_object(
            item_class,
            "(Ljava/lang/CharSequence;)V",
            &[jni::objects::JValue::Object(&payload)],
        )
        .ok()?;

    let clip_class = env.find_class("android/content/ClipData").ok()?;
    env.new_object(
        clip_class,
        "(Landroid/content/ClipDescription;Landroid/content/ClipData$Item;)V",
        &[jni::objects::JValue::Object(&description), jni::objects::JValue::Object(&item)],
    )
    .ok()
    .filter(|clip| !clip.is_null())
}

/// Reads the primary clip's payload for a custom MIME type.
///
/// `getText()` is read directly rather than `coerceToText`: a custom payload is our own text
/// encoding, not a human-readable rendering of a URI or `Intent`.
fn read_primary_custom(env: &mut jni::JNIEnv<'_>, content_type: &str) -> Option<Vec<u8>> {
    if !primary_clip_has_type(env, content_type) {
        log::warn!(
            "[android-jni] custom read: the primary clip does not declare {content_type}; it \
             declares {:?}",
            primary_clip_types(env)
        );
        return None;
    }
    let manager = clipboard_manager(env)?;
    let item = primary_clip_item(env, &manager)?;
    let Some(text) = item_text(env, &item, "getText") else {
        log::warn!("[android-jni] custom read: ClipData.Item.getText() returned null");
        return None;
    };
    let bytes = latin1_to_bytes(&text);
    if bytes.is_none() {
        log::warn!(
            "[android-jni] custom read: the item text is not in this crate's byte encoding \
             ({} chars, first non-Latin-1 char at {:?})",
            text.chars().count(),
            text.chars().position(|character| u32::from(character) > 0xFF)
        );
    }
    bytes
}

/// Publishes `clip` as the primary clip and confirms that `content_type` reads back as `data`.
fn publish_primary_custom_clip(
    env: &mut jni::JNIEnv<'_>,
    clip: &jni::objects::JObject<'_>,
    content_type: &str,
    data: &[u8],
) -> Option<bool> {
    let manager = clipboard_manager(env)?;
    if let Err(error) = env.call_method(
        &manager,
        "setPrimaryClip",
        "(Landroid/content/ClipData;)V",
        &[jni::objects::JValue::Object(clip)],
    ) {
        log::error!("[android-jni] setPrimaryClip (custom type) failed: {error}");
        return Some(false);
    }
    let written = read_primary_custom(env, content_type).is_some_and(|stored| stored == data);
    if !written {
        log::warn!(
            "[android-jni] setPrimaryClip returned without error but the custom type {content_type} \
             does not hold the bytes that were written (the platform refuses background clipboard \
             writes, or another app replaced the clip); the clip now declares {:?}",
            primary_clip_types(env)
        );
    }
    Some(written)
}

/// Writes `data` under an application-defined MIME type and confirms it by reading it back.
fn write_primary_custom_clip(
    env: &mut jni::JNIEnv<'_>,
    content_type: &str,
    data: &[u8],
) -> Option<bool> {
    let clip = build_custom_clip(env, content_type, data)?;
    publish_primary_custom_clip(env, &clip, content_type, data)
}

/// Stores `data` on the system clipboard under `content_type`.
///
/// `None` means the bridge is not ready (no `JavaVM` or no attached `Activity` `Context`);
/// `Some(written)` is the platform's answer, confirmed by a read-back.
pub fn set_clipboard_type(content_type: &str, data: &[u8]) -> Option<bool> {
    with_jni_env(|env| write_primary_custom_clip(env, content_type, data)).flatten()
}

/// The bytes the system clipboard holds under `content_type`, or `None` when it holds none.
pub fn clipboard_type(content_type: &str) -> Option<Vec<u8>> {
    with_jni_env(|env| read_primary_custom(env, content_type)).flatten()
}

/// Whether the system clipboard currently declares `content_type`.
pub fn clipboard_has_type(content_type: &str) -> bool {
    with_jni_env(|env| primary_clip_has_type(env, content_type)).unwrap_or(false)
}

/// The first custom MIME type the primary clip declares, if any.
pub fn clipboard_custom_type() -> Option<String> {
    with_jni_env(primary_clip_custom_type).flatten()
}

// ---------------------------------------------------------------------------
// Soft keyboard
// ---------------------------------------------------------------------------

/// `Context.getSystemService(Context.INPUT_METHOD_SERVICE)`, or `None` when there is no context.
fn input_method_manager<'local>(
    env: &mut jni::JNIEnv<'local>,
) -> Option<jni::objects::JObject<'local>> {
    let context = attached_context()?;
    let service = env.new_string("input_method").ok()?;
    let manager = env
        .call_method(
            context.as_obj(),
            "getSystemService",
            "(Ljava/lang/String;)Ljava/lang/Object;",
            &[jni::objects::JValue::Object(&service)],
        )
        .ok()?
        .l()
        .ok()?;
    if manager.is_null() {
        log::warn!("[android-jni] getSystemService(\"input_method\") returned null");
        return None;
    }
    Some(manager)
}

/// The host `Activity`'s decor view, which is the view an input method attaches to.
///
/// `Context` has no `getWindow()`, so a non-Activity context (an application context, or a
/// service) answers `None`. That is the honest answer for a process that cannot present a soft
/// keyboard.
fn activity_decor_view<'local>(
    env: &mut jni::JNIEnv<'local>,
) -> Option<jni::objects::JObject<'local>> {
    let context = attached_context()?;
    let window =
        match env.call_method(context.as_obj(), "getWindow", "()Landroid/view/Window;", &[]) {
            Ok(value) => value.l().ok()?,
            Err(error) => {
                log::debug!("[android-jni] Context.getWindow() is unavailable: {error}");
                // A pending exception from a non-Activity context would poison later calls.
                let _ = env.exception_clear();
                return None;
            }
        };
    if window.is_null() {
        return None;
    }
    let view =
        env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[]).ok()?.l().ok()?;
    (!view.is_null()).then_some(view)
}

/// Asks the `InputMethodManager` to show the soft keyboard for the host window.
///
/// This is the same platform operation winit's Android backend performs through
/// `AndroidApp::show_soft_input`; the `InputMethodManager` needs the Activity's decor view, which
/// is why this is not a context-only call. Returns `false` when no context/decor view is
/// available, or when the manager refused the request (the platform may do so if the window is
/// not focused yet).
fn show_soft_input_env(env: &mut jni::JNIEnv<'_>) -> bool {
    let Some(manager) = input_method_manager(env) else {
        return false;
    };
    let Some(view) = activity_decor_view(env) else {
        return false;
    };
    match env.call_method(
        &manager,
        "showSoftInput",
        "(Landroid/view/View;I)Z",
        &[jni::objects::JValue::Object(&view), jni::objects::JValue::Int(0)],
    ) {
        Ok(value) => value.z().unwrap_or(false),
        Err(error) => {
            log::warn!("[android-jni] showSoftInput failed: {error}");
            let _ = env.exception_clear();
            false
        }
    }
}

/// Asks the `InputMethodManager` to hide the soft keyboard for the host window.
fn hide_soft_input_env(env: &mut jni::JNIEnv<'_>) -> bool {
    let Some(manager) = input_method_manager(env) else {
        return false;
    };
    let Some(view) = activity_decor_view(env) else {
        return false;
    };
    let token = match env.call_method(&view, "getWindowToken", "()Landroid/os/IBinder;", &[]) {
        Ok(value) => value.l().ok(),
        Err(error) => {
            log::warn!("[android-jni] View.getWindowToken failed: {error}");
            let _ = env.exception_clear();
            None
        }
    };
    let Some(token) = token else {
        return false;
    };
    match env.call_method(
        &manager,
        "hideSoftInputFromWindow",
        "(Landroid/os/IBinder;I)Z",
        &[jni::objects::JValue::Object(&token), jni::objects::JValue::Int(0)],
    ) {
        Ok(value) => value.z().unwrap_or(false),
        Err(error) => {
            log::warn!("[android-jni] hideSoftInputFromWindow failed: {error}");
            let _ = env.exception_clear();
            false
        }
    }
}

/// Shows the soft keyboard for the host `Activity`. `false` when there is nothing to show it on.
pub fn show_soft_input() -> bool {
    with_jni_env(show_soft_input_env).unwrap_or(false)
}

/// Hides the soft keyboard for the host `Activity`. `false` when there is nothing to hide it on.
pub fn hide_soft_input() -> bool {
    with_jni_env(hide_soft_input_env).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Displays
// ---------------------------------------------------------------------------

/// One display as Android's `Display`/`DisplayMetrics` report it.
///
/// A small snapshot rather than the crate's [`Display`](crate::platform::types::Display): the JNI
/// side must not depend on the platform types (it is also compiled on hosts), and the caller in
/// `platform_impl` is the one that knows about `DisplayId`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AndroidDisplayInfo {
    /// `Display.getDisplayId()`.
    pub id: u64,
    /// Full display width in pixels.
    pub width: u32,
    /// Full display height in pixels.
    pub height: u32,
    /// Width of the part an application window may occupy.
    pub work_width: u32,
    /// Height of the part an application window may occupy.
    pub work_height: u32,
    /// Logical density (`densityDpi / 160`).
    pub scale: f32,
    /// Whether this is the default display.
    pub is_primary: bool,
}

/// Reads the default display's metrics.
///
/// # Why the default display
///
/// The library paints into the host `Activity`'s window, which Android always places on the
/// default display. Enumerating every physical display would need `DisplayManager`, which is not
/// bound here and has no consumer; the default display is the one fact the window-placement
/// contract needs.
///
/// # Why only `getRealMetrics`
///
/// `getRealMetrics` reports the full display including system bars, which is what the feedback's
/// `bounds` field means. `Display.getRectSize` looked like the natural source for the app-usable
/// work area, but on API 34 it throws and leaves the JNI exception pending; the host Activity's
/// window is full-screen, so the full bounds are the truthful work area for this backend.
pub fn display_info() -> Option<AndroidDisplayInfo> {
    /// Unwraps a JNI result, logging the failing step instead of returning silently.
    ///
    /// A `?` chain here used to turn any failing call into the same `None`, so the on-device
    /// probe reported "displays() returned []" without saying which of the fifteen JNI steps
    /// failed. The log line names the step and the JNI error.
    macro_rules! need {
        ($env:expr, $expr:expr, $what:literal) => {
            match $expr {
                Ok(value) => value,
                Err(error) => {
                    log::warn!("[android-jni] display_info: {} failed: {error}", $what);
                    // A throwing JNI call leaves the exception pending, which makes every
                    // later call on this thread fail with the same generic error. Clearing it
                    // here keeps one failed step from poisoning the rest of the probe.
                    let _ = $env.exception_clear();
                    return None;
                }
            }
        };
    }

    with_jni_env(|env| {
        let Some(context) = attached_context() else {
            log::warn!("[android-jni] display_info: no Activity Context is attached");
            return None;
        };
        let service = need!(env, env.new_string("window"), "new_string(\"window\")");
        let manager = need!(
            env,
            need!(
                env,
                env.call_method(
                    context.as_obj(),
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[jni::objects::JValue::Object(&service)],
                ),
                "getSystemService"
            )
            .l(),
            "getSystemService -> object"
        );
        if manager.is_null() {
            log::warn!("[android-jni] display_info: getSystemService(window) returned null");
            return None;
        }
        let display = need!(
            env,
            need!(
                env,
                env.call_method(&manager, "getDefaultDisplay", "()Landroid/view/Display;", &[]),
                "getDefaultDisplay"
            )
            .l(),
            "getDefaultDisplay -> object"
        );
        if display.is_null() {
            log::warn!("[android-jni] display_info: getDefaultDisplay returned null");
            return None;
        }
        let id = need!(
            env,
            need!(env, env.call_method(&display, "getDisplayId", "()I", &[]), "getDisplayId").i(),
            "getDisplayId -> int"
        ) as u64;

        let metrics_class =
            need!(env, env.find_class("android/util/DisplayMetrics"), "find_class(DisplayMetrics)");
        let metrics = need!(env, env.new_object(metrics_class, "()V", &[]), "new DisplayMetrics()");
        need!(
            env,
            env.call_method(
                &display,
                "getRealMetrics",
                "(Landroid/util/DisplayMetrics;)V",
                &[jni::objects::JValue::Object(&metrics)],
            ),
            "getRealMetrics"
        );
        let width = need!(
            env,
            need!(env, env.get_field(&metrics, "widthPixels", "I"), "get widthPixels").i(),
            "widthPixels -> int"
        );
        let height = need!(
            env,
            need!(env, env.get_field(&metrics, "heightPixels", "I"), "get heightPixels").i(),
            "heightPixels -> int"
        );
        let density_dpi = need!(
            env,
            need!(env, env.get_field(&metrics, "densityDpi", "I"), "get densityDpi").i(),
            "densityDpi -> int"
        );

        // The work area is the full display on this backend. `Display.getRectSize` was the
        // obvious API for the app-usable part, but on API 34 it throws and leaves the exception
        // pending, which then failed every later JNI call in the probe. The host's Activity
        // window is full-screen, so bounds is the truthful work area here; a host that runs a
        // split-screen session can still use the per-window surface metrics it owns.
        //
        // Android reports these in physical pixels. The trait contract
        // (`platform::DisplayInfo::bounds`) is the desktop's **logical** pixel space, and this
        // file's other two conversion paths divide by `densityDpi/160` for exactly that reason
        // (`window_insets`, the pointer dispatch). Reporting physical bounds *beside* the scale
        // factor made a 3x device answer a bounds three times too large and then invited a second
        // multiplication downstream — a 9x area (ENG-C3).
        let scale = if density_dpi > 0 { density_dpi as f32 / 160.0 } else { 1.0 };
        let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
        let logical = |pixels: i32| (pixels.max(0) as f32 / scale).round() as u32;
        let width = logical(width);
        let height = logical(height);
        let work_width = width;
        let work_height = height;

        Some(AndroidDisplayInfo {
            id,
            width,
            height,
            work_width,
            work_height,
            // Android's baseline density is 160 dpi; a non-positive reading is not a scale.
            scale,
            is_primary: true,
        })
    })
    .flatten()
}

/// The user-interface facts Android keeps in the current `Configuration`.
struct AndroidUiConfiguration {
    /// `Some(true)` when night mode is on, `Some(false)` when it is off, `None` for
    /// `UI_MODE_NIGHT_UNDEFINED`.
    night_mode: Option<bool>,
    /// `Configuration.fontScale`: the user's text-size multiplier.
    font_scale: f32,
}

/// Reads the `Configuration` the activity's `Resources` currently holds.
///
/// This is the per-process configuration Android updates on `Activity.onConfigurationChanged`, so
/// it follows a live light/dark switch and a text-size change without caching either. `None` when
/// no `Context` is attached or any JNI step fails; the pending exception is cleared so one failed
/// call does not poison the thread.
fn ui_configuration() -> Option<AndroidUiConfiguration> {
    /// `Configuration.UI_MODE_NIGHT_MASK` and its two defined values.
    const UI_MODE_NIGHT_MASK: i32 = 0x30;
    const UI_MODE_NIGHT_NO: i32 = 0x10;
    const UI_MODE_NIGHT_YES: i32 = 0x20;

    with_jni_env(|env| {
        let context = attached_context()?;
        let read = (|| -> Option<AndroidUiConfiguration> {
            let resources = env
                .call_method(
                    context.as_obj(),
                    "getResources",
                    "()Landroid/content/res/Resources;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;
            if resources.is_null() {
                return None;
            }
            let configuration = env
                .call_method(
                    &resources,
                    "getConfiguration",
                    "()Landroid/content/res/Configuration;",
                    &[],
                )
                .ok()?
                .l()
                .ok()?;
            if configuration.is_null() {
                return None;
            }
            let ui_mode = env.get_field(&configuration, "uiMode", "I").ok()?.i().ok()?;
            let font_scale = env.get_field(&configuration, "fontScale", "F").ok()?.f().ok()?;
            Some(AndroidUiConfiguration {
                night_mode: match ui_mode & UI_MODE_NIGHT_MASK {
                    UI_MODE_NIGHT_YES => Some(true),
                    UI_MODE_NIGHT_NO => Some(false),
                    _ => None,
                },
                font_scale,
            })
        })();
        if read.is_none() {
            let _ = env.exception_clear();
        }
        read
    })
    .flatten()
}

/// Whether the system is in night (dark) mode.
///
/// `Configuration.uiMode & UI_MODE_NIGHT_MASK` is Android's own answer to "should apps use the
/// dark theme", which is the fact winit's Android backend reads from its `MainActivity`
/// configuration. `None` when no `Context` is attached or the mode is undefined — never a
/// fabricated light preference.
pub fn system_theme() -> Option<bool> {
    ui_configuration()?.night_mode
}

/// The user's text-size multiplier (`Configuration.fontScale`).
///
/// `1.0` is the platform default and values above it are the accessibility settings. A
/// non-finite or non-positive reading is refused rather than handed to a layout that would divide
/// by it.
pub fn font_scale() -> Option<f32> {
    let scale = ui_configuration()?.font_scale;
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

/// Whether Android's high-text-contrast accessibility setting is on.
///
/// `AccessibilityManager.isHighTextContrastEnabled()` is the API behind the accessibility
/// "High contrast text" switch. `None` without an attached `Context` or when any JNI step fails.
pub fn high_contrast() -> Option<bool> {
    with_jni_env(|env| {
        let context = attached_context()?;
        let service = env.new_string("accessibility").ok()?;
        let manager = env
            .call_method(
                context.as_obj(),
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[jni::objects::JValue::Object(&service)],
            )
            .ok()?
            .l()
            .ok()?;
        if manager.is_null() {
            log::warn!("[android-jni] getSystemService(\"accessibility\") returned null");
            return None;
        }
        let answer =
            env.call_method(&manager, "isHighTextContrastEnabled", "()Z", &[]).ok()?.z().ok()?;
        Some(answer)
    })
    .flatten()
}

/// Whether Android's animator duration scale is zero, which is reduced motion.
///
/// `Settings.Global.ANIMATOR_DURATION_SCALE` is the developer setting every animation honours;
/// `0.0` means "remove animations". The default is `1.0`, matching Android's own API default.
/// `None` without a `Context` or when the setting cannot be read.
pub fn reduce_motion() -> Option<bool> {
    with_jni_env(|env| {
        let context = attached_context()?;
        let resolver = env
            .call_method(
                context.as_obj(),
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;
        if resolver.is_null() {
            return None;
        }
        let settings = env.find_class("android/provider/Settings$Global").ok()?;
        let key = env.new_string("animator_duration_scale").ok()?;
        let scale = env
            .call_static_method(
                settings,
                "getFloat",
                "(Landroid/content/ContentResolver;Ljava/lang/String;F)F",
                &[
                    jni::objects::JValue::Object(&resolver),
                    jni::objects::JValue::Object(&key),
                    jni::objects::JValue::Float(1.0),
                ],
            )
            .ok()?
            .f()
            .ok()?;
        Some(scale == 0.0)
    })
    .flatten()
}

/// The user's locale, from `Locale.getDefault().toLanguageTag()`.
///
/// `None` when the JVM cannot be reached. The tag is returned as an owned `String`; the platform
/// backend interns it for the style layer.
pub fn system_locale() -> Option<String> {
    with_jni_env(|env| {
        let locale_class = env.find_class("java/util/Locale").ok()?;
        let locale = env
            .call_static_method(locale_class, "getDefault", "()Ljava/util/Locale;", &[])
            .ok()?
            .l()
            .ok()?;
        if locale.is_null() {
            return None;
        }
        let tag = env
            .call_method(&locale, "toLanguageTag", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?;
        if tag.is_null() {
            return None;
        }
        let tag: jni::objects::JString = tag.into();
        env.get_string(&tag).ok().map(|value| value.into())
    })
    .flatten()
}

/// The activity window's safe-area insets, in **logical** pixels (top, bottom, left, right).
///
/// # Why `WindowMetrics` and not a `View`
///
/// `View.getRootWindowInsets()` needs a live `View`, and this bridge holds only the Activity
/// `Context`. On API 30+ `WindowManager.getCurrentWindowMetrics().getWindowInsets()` answers the
/// same question for the Activity's own window without one, which is the window every library
/// widget is painted into. The mask combines the system bars (status bar, navigation bar, gesture
/// bar) with the display cutout (a notch or camera housing).
///
/// # Honest absence
///
/// `None` when no `Context` is attached, on an API below 30 (the call does not exist), or when any
/// JNI step fails. The pending exception is cleared so a refusal does not poison later calls. The
/// pixels Android reports are divided by `DisplayMetrics.densityDpi / 160` to make them logical,
/// because the trait's contract is logical pixels.
pub fn window_insets() -> Option<(u32, u32, u32, u32)> {
    let scale = display_info().map(|info| info.scale).unwrap_or(1.0);
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };

    let pixels = with_jni_env(|env| {
        // Logs the failing step, clears the pending exception, and returns `None` from the
        // enclosing closure. A bare `?` left "which of the nine calls failed" unanswerable, which
        // is the same diagnosis gap `display_info` was given this macro for.
        macro_rules! need {
            ($expr:expr, $what:literal) => {
                match $expr {
                    Ok(value) => value,
                    Err(error) => {
                        log::warn!("[android-jni] window_insets: {} failed: {error}", $what);
                        let _ = env.exception_clear();
                        return None;
                    }
                }
            };
        }

        let context = attached_context()?;
        let read = (|| -> Option<(i32, i32, i32, i32)> {
            let service = env.new_string("window").ok()?;
            let manager = need!(
                env.call_method(
                    context.as_obj(),
                    "getSystemService",
                    "(Ljava/lang/String;)Ljava/lang/Object;",
                    &[jni::objects::JValue::Object(&service)],
                ),
                "getSystemService(window)"
            )
            .l()
            .ok()?;
            if manager.is_null() {
                log::warn!("[android-jni] window_insets: getSystemService(window) returned null");
                return None;
            }
            let metrics = need!(
                env.call_method(
                    &manager,
                    "getCurrentWindowMetrics",
                    "()Landroid/view/WindowMetrics;",
                    &[],
                ),
                "getCurrentWindowMetrics"
            )
            .l()
            .ok()?;
            if metrics.is_null() {
                log::warn!("[android-jni] window_insets: getCurrentWindowMetrics returned null");
                return None;
            }
            let insets = need!(
                env.call_method(&metrics, "getWindowInsets", "()Landroid/view/WindowInsets;", &[],),
                "getWindowInsets"
            )
            .l()
            .ok()?;
            if insets.is_null() {
                log::warn!("[android-jni] window_insets: getWindowInsets returned null");
                return None;
            }
            let type_class = need!(
                env.find_class("android/view/WindowInsets$Type"),
                "find_class(WindowInsets$Type)"
            );
            let system_bars = need!(
                env.call_static_method(&type_class, "systemBars", "()I", &[]),
                "WindowInsets.Type.systemBars"
            )
            .i()
            .ok()?;
            let cutout = need!(
                env.call_static_method(&type_class, "displayCutout", "()I", &[]),
                "WindowInsets.Type.displayCutout"
            )
            .i()
            .ok()?;
            let edges = need!(
                env.call_method(
                    &insets,
                    "getInsets",
                    "(I)Landroid/graphics/Insets;",
                    &[jni::objects::JValue::Int(system_bars | cutout)],
                ),
                "getInsets(systemBars|displayCutout)"
            )
            .l()
            .ok()?;
            if edges.is_null() {
                log::warn!("[android-jni] window_insets: getInsets returned null");
                return None;
            }
            let top = need!(env.get_field(&edges, "top", "I"), "Insets.top").i().ok()?;
            let bottom = need!(env.get_field(&edges, "bottom", "I"), "Insets.bottom").i().ok()?;
            let left = need!(env.get_field(&edges, "left", "I"), "Insets.left").i().ok()?;
            let right = need!(env.get_field(&edges, "right", "I"), "Insets.right").i().ok()?;
            Some((top, bottom, left, right))
        })();
        if read.is_none() {
            let _ = env.exception_clear();
        }
        read
    })
    .flatten()?;

    let logical = |pixels: i32| (pixels.max(0) as f32 / scale).round() as u32;
    Some((logical(pixels.0), logical(pixels.1), logical(pixels.2), logical(pixels.3)))
}

/// Publishes `clip` as the primary clip and reports whether the clipboard then holds `expect`.
///
/// The read-back is the evidence: `setPrimaryClip` returns `void` and cannot fail loudly, and on
/// API 30+ the platform silently refuses a background process's clipboard write. Reporting the
/// request as success there is the "reported success for something that did not happen" failure
/// the trait forbids.
fn publish_primary_clip(
    env: &mut jni::JNIEnv<'_>,
    clip: &jni::objects::JObject<'_>,
    expect: &str,
    reader: fn(&mut jni::JNIEnv<'_>) -> Option<String>,
) -> Option<bool> {
    let manager = clipboard_manager(env)?;
    if let Err(error) = env.call_method(
        &manager,
        "setPrimaryClip",
        "(Landroid/content/ClipData;)V",
        &[jni::objects::JValue::Object(clip)],
    ) {
        log::error!("[android-jni] setPrimaryClip failed: {error}");
        return Some(false);
    }
    let written = reader(env).is_some_and(|stored| stored == expect);
    if !written {
        log::warn!(
            "[android-jni] setPrimaryClip returned without error but the clipboard does not hold \
             the value that was written (the platform refuses background clipboard writes)"
        );
    }
    Some(written)
}

/// Writes `text` as the primary clip and confirms it by reading the clipboard back.
fn write_primary_clip(env: &mut jni::JNIEnv<'_>, text: &str) -> Option<bool> {
    let clip = build_text_clip(env, text)?;
    publish_primary_clip(env, &clip, text, read_primary_clip)
}

/// Writes `html` (with `plain` as its fallback) as the primary clip.
fn write_primary_html_clip(env: &mut jni::JNIEnv<'_>, html: &str, plain: &str) -> Option<bool> {
    let clip = build_html_clip(env, html, plain)?;
    publish_primary_clip(env, &clip, html, read_primary_html)
}

/// `ClipData.newPlainText(label, text)`.
fn build_text_clip<'local>(
    env: &mut jni::JNIEnv<'local>,
    text: &str,
) -> Option<jni::objects::JObject<'local>> {
    let label = env.new_string("rust-widgets").ok()?;
    let value = env.new_string(text).ok()?;
    let clip_data = env.find_class("android/content/ClipData").ok()?;
    new_clip(
        env,
        &clip_data,
        "newPlainText",
        "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
        &[jni::objects::JValue::Object(&label), jni::objects::JValue::Object(&value)],
    )
}

/// `ClipData.newHtmlText(label, text, htmlText)`.
fn build_html_clip<'local>(
    env: &mut jni::JNIEnv<'local>,
    html: &str,
    plain: &str,
) -> Option<jni::objects::JObject<'local>> {
    let label = env.new_string("rust-widgets").ok()?;
    let text = env.new_string(plain).ok()?;
    let html_text = env.new_string(html).ok()?;
    let clip_data = env.find_class("android/content/ClipData").ok()?;
    new_clip(
        env,
        &clip_data,
        "newHtmlText",
        "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;",
        &[
            jni::objects::JValue::Object(&label),
            jni::objects::JValue::Object(&text),
            jni::objects::JValue::Object(&html_text),
        ],
    )
}

/// Calls one of `ClipData`'s static factories and returns the clip it built.
fn new_clip<'local>(
    env: &mut jni::JNIEnv<'local>,
    clip_data: &jni::objects::JClass<'local>,
    factory: &str,
    signature: &str,
    args: &[jni::objects::JValue<'_, '_>],
) -> Option<jni::objects::JObject<'local>> {
    match env.call_static_method(clip_data, factory, signature, args) {
        Ok(clip) => clip.l().ok().filter(|clip| !clip.is_null()),
        Err(error) => {
            log::error!("[android-jni] ClipData.{factory} failed: {error}");
            None
        }
    }
}

/// The Android host clipboard.
///
/// # What it carries, and what it does not
///
/// Text and HTML, through `ClipData.newPlainText` / `ClipData.newHtmlText` — the same pair the
/// macOS and iOS backends implement — **plus application-defined MIME types** through
/// `ClipDescription` + `ClipData.Item`. An arbitrary byte payload travels over the item's text
/// channel with a one-to-one byte-to-character mapping (see [`bytes_to_latin1`]); the MIME type is
/// what the description negotiates, so a consumer asks for it by name rather than sniffing the
/// body. Images, RTF and file lists are still refused (`false` / `None`) rather than silently
/// dropped: a file list would need a `ContentResolver`-backed `Uri`, which the bridge has no handle
/// on, and returning `true` for a conversion that did not happen is the fake-success shape this
/// crate's principles forbid.
pub struct AndroidClipboard;

impl AndroidClipboard {
    /// Creates the backend. It holds no state: the host clipboard is the state.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AndroidClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::platform::clipboard::RichClipboardBackend for AndroidClipboard {
    fn set_contents(&self, content: crate::platform::clipboard::ClipboardContent) -> bool {
        use crate::platform::clipboard::ClipboardContent;
        let written = match content {
            ClipboardContent::Text(text) => {
                with_jni_env(|env| write_primary_clip(env, &text)).flatten()
            }
            ClipboardContent::Html { html, plain } => {
                with_jni_env(|env| write_primary_html_clip(env, &html, &plain)).flatten()
            }
            // An application-defined type goes through the `ClipDescription` + `ClipData.Item`
            // constructor pair, which is the only Android channel that names a MIME type other
            // than the three the `ClipData.new*` factories hardcode.
            ClipboardContent::Custom { content_type, data } => {
                with_jni_env(|env| write_primary_custom_clip(env, &content_type, &data)).flatten()
            }
            other => {
                log::warn!(
                    "[android-jni] {} is not carried by this backend; the write is reported as \
                     refused rather than pretended",
                    other.content_type()
                );
                return false;
            }
        };
        written.unwrap_or(false)
    }

    fn get_contents(&self) -> Option<crate::platform::clipboard::ClipboardContent> {
        use crate::platform::clipboard::ClipboardContent;
        // A custom clip is checked **before** the text channels: its payload is carried as a
        // `CharSequence`, so a text-first read would return the payload as ordinary text and the
        // type would be lost at the reader.
        if let Some(content_type) = clipboard_custom_type() {
            if let Some(data) = clipboard_type(&content_type) {
                return Some(ClipboardContent::Custom { content_type, data });
            }
        }
        if let Some(html) = clipboard_text_of(read_primary_html) {
            let plain = clipboard_text_of(read_primary_clip).unwrap_or_default();
            return Some(ClipboardContent::Html { html, plain });
        }
        clipboard_text_of(read_primary_clip).map(ClipboardContent::Text)
    }

    fn has_format(&self, content_type: &str) -> bool {
        match content_type {
            "text/html" => clipboard_text_of(read_primary_html).is_some(),
            "text/plain" => clipboard_text_of(read_primary_clip).is_some(),
            other => clipboard_has_type(other),
        }
    }

    /// Stores bytes under an application-defined MIME type.
    ///
    /// Overridden rather than inherited because the default would route through
    /// [`Self::set_contents`] — which is the same path — but only this entry point documents and
    /// checks the byte-level contract directly.
    fn set_contents_with_type(&self, content_type: &str, data: &[u8]) -> bool {
        set_clipboard_type(content_type, data).unwrap_or(false)
    }

    /// Reads the bytes the clipboard holds under `content_type`.
    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        match content_type {
            "text/plain" => clipboard_text_of(read_primary_clip).map(String::into_bytes),
            "text/html" => clipboard_text_of(read_primary_html).map(String::into_bytes),
            other => clipboard_type(other),
        }
    }
}

/// Runs one of the clipboard readers through the JNI bridge.
fn clipboard_text_of(reader: fn(&mut jni::JNIEnv<'_>) -> Option<String>) -> Option<String> {
    with_jni_env(reader).flatten()
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

/// Runs the on-device widget self-test and returns a pass/fail bitmask.
///
/// # Why the bridge needs this
///
/// Everything else in this file proves the **plumbing**: the VM is stored, a Context is
/// attached, a resize is reported. None of it proves that the library can create, mutate
/// and read back a control on Android — and that is the part a host app actually depends
/// on. The iOS probe has exercised the same path through the C ABI since it was written
/// (`rw_create_window` → `rw_create_button` → `rw_set_widget_text` → `rw_get_widget_text`),
/// so Android was the platform whose widget path had no runtime evidence at all.
///
/// # Why this is a JNI entry point and not a `nativeCreate*` JS declaration
///
/// The Android Java wrapper deliberately exposes no per-kind creators: the library paints
/// every `WidgetKind` itself, so a host supplies a window and a drawing surface rather than
/// one native `View` per kind (BLUE15 #55/#56). Adding `nativeCreateButton` back just to
/// test would re-introduce exactly the API the project removed. A single self-test entry
/// point that drives the real widget API keeps the Java surface honest and still produces
/// on-device evidence.
///
/// Returns a bitmask so a host can log *which* step failed rather than only that one did:
///
/// | bit | step |
/// |---|---|
/// | 0 | `create_window` returned a live id |
/// | 1 | `create_button` returned a live id under it |
/// | 2 | the button's text round-trips through the property contract |
/// | 3 | geometry is recorded and read back |
/// | 4 | visibility toggles and reads back |
/// | 5 | `destroy` actually unmounted both controls |
/// | 6 | the backend does not claim a native menu it has none of |
/// | 7 | the backend identifies itself |
///
/// A return of `0b11111111` (255) is a full pass.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeWidgetSelfTest(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    widget_self_test()
}

/// The widget self-test body, separate from the JNI shim so its steps can be unit-tested.
///
/// See [`Java_rust_1widgets_RustWidgets_nativeWidgetSelfTest`] for the bit meanings.
#[cfg(not(alloc_frugal))]
fn widget_self_test() -> jni::sys::jint {
    let mut passed = 0;

    // 1. A window. This is the `create_*` entry a host actually calls, so the test
    //    exercises the same path an application does rather than reaching past it.
    let window = crate::create_window("Self Test", 0, 0, 1080, 1920);
    if window == 0 {
        log::error!("[android-jni] widget self-test: create_window returned 0");
        return passed;
    }
    passed |= 1;

    // 2. A child control under it.
    let button = crate::create_button(window, "Self Test", 16, 16, 240, 64);
    if button == 0 {
        log::error!("[android-jni] widget self-test: create_button returned 0");
        destroy(window);
        return passed;
    }
    passed |= 1 << 1;

    // 3. Text round-trip through the real property contract.
    crate::set_widget_text(button, "Round Tripped");
    let read_back = crate::get_widget_text(button);
    if read_back == "Round Tripped" {
        passed |= 1 << 2;
    } else {
        log::error!("[android-jni] widget self-test: text read back as {read_back:?}");
    }

    // 4. Geometry is recorded and read back by the painter.
    let (x, y, w, h) = (32, 48, 320, 80);
    crate::set_widget_geometry(button, x, y, w, h);
    match crate::widget_geometry(button) {
        Some(rect) if rect == (x, y, w, h) => {
            passed |= 1 << 3;
        }
        other => log::error!("[android-jni] widget self-test: geometry read back as {other:?}"),
    }

    // 5. Visibility toggles and reads back through the same predicate a painter consults.
    crate::hide_widget(button);
    let hidden = !crate::is_widget_visible(button);
    crate::show_widget(button);
    let shown = crate::is_widget_visible(button);
    if hidden && shown {
        passed |= 1 << 4;
    } else {
        log::error!("[android-jni] widget self-test: hidden={hidden} shown={shown}");
    }

    // 6. Nothing was left mounted behind the test: the ids are gone, so a host that runs
    //    this cannot inherit controls it never created.
    destroy(button);
    destroy(window);
    if !crate::widget::runtime::is_mounted(button) && !crate::widget::runtime::is_mounted(window) {
        passed |= 1 << 5;
    } else {
        log::error!("[android-jni] widget self-test: a control survived destroy");
    }

    // 7. The platform facts the host reads to decide which affordances to offer.
    //
    // `capabilities()` is a **claim**: a set bit promises the method behind it answers. The
    // Android backend inherits the trait default (all false except `typed_widget_trigger`),
    // so the only assertion worth making here is that it does not claim a native menu — an
    // over-claim on this platform is the defect `src/platform/ios/status.md` records for its
    // own flags, and the one `capabilities()` was written to stop.
    let caps = crate::platform::capabilities();
    if !caps.native_menu {
        passed |= 1 << 6;
    } else {
        log::error!("[android-jni] widget self-test: Android claims a native menu it has none of");
    }

    // 8. The backend identifies itself, so a host can report which one it linked.
    let backend = crate::platform::backend_name();
    if !backend.is_empty() {
        passed |= 1 << 7;
    } else {
        log::error!("[android-jni] widget self-test: the backend reported no name");
    }

    passed
}

/// The result queue for asynchronous path requests (BLUE28 B28.08).
///
/// The Activity result arrives on the JVM thread; `take_path_choice` is polled by the library's
/// frame loop. A queue rather than a single slot so two requests cannot overwrite each other's
/// answer before the host reads it.
static PATH_CHOICES: std::sync::Mutex<std::collections::VecDeque<crate::platform::PathChoice>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());

/// Pushes one completed picker answer for `take_path_choice` to read.
pub fn push_path_choice(choice: crate::platform::PathChoice) {
    let mut queue = PATH_CHOICES.lock().unwrap_or_else(|error| error.into_inner());
    queue.push_back(choice);
}

/// Pops the next completed picker answer, or `None` when none has arrived.
pub fn take_path_choice() -> Option<crate::platform::PathChoice> {
    PATH_CHOICES.lock().unwrap_or_else(|error| error.into_inner()).pop_front()
}

/// Whether `launch_file_dialog` has the bridge state it needs (a VM and an Activity Context).
pub fn picker_ready() -> bool {
    JAVA_VM.get().is_some() && attached_context().is_some()
}

/// Host-side delivery of a document-picker result.
///
/// The Java host calls this from `onActivityResult`: `path` is the selected document URI's
/// filesystem path (or empty) and `cancelled` is non-zero when the user dismissed the picker.
/// Returns `1` when the answer was queued.
#[no_mangle]
#[cfg(not(alloc_frugal))]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeInjectPathChoice(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    path: jni::objects::JString,
    cancelled: jni::sys::jint,
) -> jni::sys::jint {
    if cancelled != 0 {
        push_path_choice(crate::platform::PathChoice::Cancelled);
        return 1;
    }
    let value: String = match env.get_string(&path) {
        Ok(value) => value.into(),
        Err(error) => {
            log::warn!("[android-jni] nativeInjectPathChoice: get_string failed: {error}");
            push_path_choice(crate::platform::PathChoice::Failed);
            return 0;
        }
    };
    if value.is_empty() {
        push_path_choice(crate::platform::PathChoice::Failed);
    } else {
        push_path_choice(crate::platform::PathChoice::Chosen(std::path::PathBuf::from(value)));
    }
    1
}

/// Reports a configuration change: `Display.getRotation()` and `Configuration.orientation`.
///
/// `rotation` is Android's `Surface.ROTATION_*` (`0..=3`) or `-1` when the host cannot read it;
/// `axis` is `Configuration.ORIENTATION_*` (`1` portrait, `2` landscape) or `0`. When both are
/// absent the previous answer is left untouched rather than replaced with a guess. Returns `1`
/// when the report was accepted.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeOnConfigurationChanged(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    rotation: jni::sys::jint,
    axis: jni::sys::jint,
) -> jni::sys::jint {
    let accepted = if (-1..=3).contains(&rotation) && rotation >= 0 {
        crate::platform::lifecycle::set_rotation(rotation)
    } else {
        crate::platform::lifecycle::set_axis_orientation(axis)
    };
    i32::from(accepted)
}

/// Reports an Activity lifecycle transition (BLUE28 B28.24).
///
/// `state` is `0` active (`onResume`), `1` inactive (`onPause` without losing visibility), or `2`
/// background (`onStop`). The library stops the animation clock for the latter two and starts it
/// again on `0`. Returns `1` when the code was understood.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeOnLifecycleChanged(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    state: jni::sys::jint,
) -> jni::sys::jint {
    let phase = match state {
        0 => crate::platform::MobileLifecycleState::Active,
        1 => crate::platform::MobileLifecycleState::Inactive,
        2 => crate::platform::MobileLifecycleState::Background,
        _ => return 0,
    };
    i32::from(crate::platform::lifecycle::set_state(phase))
}

/// Runs the on-device platform-services probe and returns the mask of passed facts.
///
/// # Why this delegates to the shared probe
///
/// The bit layout belongs to [`crate::platform::diagnostics`], not to Android. Keeping a second,
/// Android-only order here is exactly the drift principle #54 forbids: a host reading masks from
/// two platforms could not compare them, and adding a fact would have to be done twice. The
/// Android Java host still calls this entry point; it now returns the unified layout.
///
/// | bit | fact |
/// |---:|---|
/// | 0 | `clipboard_backend` carries an application-defined MIME type |
/// | 1 | `displays()` returns at least one usable display |
/// | 2 | `display_of_window()` places a real window |
/// | 3 | `display_generation()` is non-zero and stable |
/// | 4 | `dpi_scale_factor()` is measured |
/// | 5 | `system_theme()` answers `Some(..)` |
/// | 6 | `text_scale()` answers a usable multiplier |
/// | 7 | `safe_area_insets()` answers `Some(..)` |
/// | 8 | `ime_bridge()` is present |
/// | 9 | `accessibility_bridge()` is present |
/// | 10 | the host picker is declared |
/// | 11 | external opening is declared |
///
/// A return of `0b1111_1111_1111` (4095) is a full pass. A clear bit with a `skip` entry in the
/// JSON log means the fact does not apply here; only a `fail` entry is a defect.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativePlatformProbe(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    platform_services_probe()
}

/// The body behind [`Java_rust_1widgets_RustWidgets_nativePlatformProbe`].
///
/// Kept out of the JNI shim so the log line and the mask come from one probe run: a host reading
/// the logcat sees the same twelve facts the mask counts.
#[cfg(not(alloc_frugal))]
fn platform_services_probe() -> jni::sys::jint {
    let diagnostics = crate::platform::diagnostics::run();
    log::info!("[android-jni] platform diagnostics: {}", diagnostics.to_json());
    for entry in &diagnostics.entries {
        match entry.status {
            crate::platform::diagnostics::DiagnosticStatus::Pass => {}
            crate::platform::diagnostics::DiagnosticStatus::Skip => {
                log::info!(
                    "[android-jni] platform probe: bit {} ({}) skipped: {}",
                    entry.bit,
                    entry.name,
                    entry.detail
                );
            }
            crate::platform::diagnostics::DiagnosticStatus::Fail => {
                log::error!(
                    "[android-jni] platform probe: bit {} ({}) failed: {}",
                    entry.bit,
                    entry.name,
                    entry.detail
                );
            }
        }
    }
    diagnostics.mask() as jni::sys::jint
}

/// `alloc_frugal` has no platform singleton to probe.
#[cfg(alloc_frugal)]
fn platform_services_probe() -> jni::sys::jint {
    0
}

/// The **failed** bits of the same unified probe, as a 12-bit mask.
///
/// `nativePlatformProbe` answers the passed bits, which cannot express the probe's third state:
/// a fact the backend honestly does not have is `skip`, not `fail` (see
/// [`crate::platform::diagnostics`]). A host that read an unset bit as a failure therefore
/// marked Android's `accessibility` and `open_external` skips as failures and printed
/// `RESULT: FAIL` for a run with zero failures. This entry point carries the distinction, so a
/// host can label each bit `PASS` / `SKIP` / `FAIL` and decide on failures alone.
#[no_mangle]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativePlatformProbeFailures(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
) -> jni::sys::jint {
    platform_services_failures()
}

/// The body behind [`Java_rust_1widgets_RustWidgets_nativePlatformProbeFailures`].
#[cfg(not(alloc_frugal))]
fn platform_services_failures() -> jni::sys::jint {
    let diagnostics = crate::platform::diagnostics::run();
    diagnostics
        .entries
        .iter()
        .filter(|entry| entry.status == crate::platform::diagnostics::DiagnosticStatus::Fail)
        .fold(0i32, |mask, entry| mask | (1i32 << entry.bit)) as jni::sys::jint
}

/// `alloc_frugal` has no platform singleton to probe, so nothing can have failed.
#[cfg(alloc_frugal)]
fn platform_services_failures() -> jni::sys::jint {
    0
}

/// Tears a control down through the same backend call the C ABI's `rw_destroy_widget` uses.
///
/// Kept for the widget self-test above, which still owns its own window lifecycle.
#[cfg(not(alloc_frugal))]
fn destroy(widget_id: crate::core::ObjectId) {
    let _ = crate::control_backend::get_control_backend().destroy_widget(widget_id);
}

/// The first mounted top-level window, which is the root a host pointer event addresses.
///
/// The Android host model is one Activity window, so the first is the only one. A host event
/// that arrives before any widget window exists answers `None` and is ignored rather than
/// dispatched to an id that addresses nothing.
#[cfg(not(alloc_frugal))]
fn first_window() -> Option<crate::core::ObjectId> {
    let found = core::cell::Cell::new(None);
    crate::widget::runtime::for_each_mounted_widget(|id, widget| {
        if found.get().is_none() && widget.kind() == crate::widget::WidgetKind::Window {
            found.set(Some(id));
        }
    });
    found.get()
}

/// Host-side pointer forwarding: `onTouchEvent` / `onGenericMotionEvent` call this.
///
/// `code` is the same stable vocabulary the C ABI uses: `1` press, `2` release, `3` move, `4`
/// enter, `5` leave. Coordinates are **view pixels**; they are divided by the display density so
/// the widget runtime sees the logical space its geometry was created in. Returns `1` when a
/// window accepted the event, `0` otherwise (including an unknown code).
#[no_mangle]
#[cfg(not(alloc_frugal))]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeDispatchPointerEvent(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    code: jni::sys::jint,
    x: jni::sys::jint,
    y: jni::sys::jint,
    button: jni::sys::jint,
) -> jni::sys::jint {
    let Some(window) = first_window() else {
        log::warn!("[android-jni] pointer event before a widget window exists; ignored");
        return 0;
    };
    let scale = crate::platform::get_platform().dpi_scale_factor();
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let logical_x = (x as f32 / scale).round() as i32;
    let logical_y = (y as f32 / scale).round() as i32;
    let point = crate::core::Point::new(logical_x, logical_y);
    let event = match code {
        1 => crate::event::Event::mouse_press(logical_x, logical_y, button as u32),
        2 => crate::event::Event::mouse_release(logical_x, logical_y, button as u32),
        3 => crate::event::Event::mouse_move(logical_x, logical_y),
        4 => crate::event::Event::MouseEnter { pos: point },
        5 => crate::event::Event::MouseLeave { pos: point },
        _ => {
            log::warn!("[android-jni] unknown pointer event code {code}");
            return 0;
        }
    };
    if crate::widget::runtime::dispatch_pointer_event(window, &event, point) {
        1
    } else {
        0
    }
}

/// Host-side text commit: an IME's `commitText` reaches this entry point.
///
/// The text goes to the focused widget, which is the same target the library's IME bridge
/// uses. Returns `1` when a focused widget accepted it.
#[no_mangle]
#[cfg(not(alloc_frugal))]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeDispatchText(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    text: jni::objects::JString,
) -> jni::sys::jint {
    let value: String = match env.get_string(&text) {
        Ok(value) => value.into(),
        Err(error) => {
            log::warn!("[android-jni] nativeDispatchText: get_string failed: {error}");
            return 0;
        }
    };
    let Some(focused) = crate::widget::runtime::focused_widget() else {
        return 0;
    };
    if crate::widget::runtime::dispatch_event(focused, &crate::event::Event::text_input(value)) {
        1
    } else {
        0
    }
}

/// Host-side key forwarding: `onKeyDown`/`onKeyUp` call this.
///
/// `key_code` is Android's `KeyEvent.getKeyCode()`; `pressed` is `1` for down and `0` for up.
/// The focused widget receives the same `KeyPress`/`KeyRelease` the desktop backends dispatch.
#[no_mangle]
#[cfg(not(alloc_frugal))]
pub extern "system" fn Java_rust_1widgets_RustWidgets_nativeDispatchKeyEvent(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    key_code: jni::sys::jint,
    modifiers: jni::sys::jint,
    pressed: jni::sys::jint,
) -> jni::sys::jint {
    let Some(focused) = crate::widget::runtime::focused_widget() else {
        return 0;
    };
    let event = if pressed != 0 {
        crate::event::Event::key_press(key_code as u32, modifiers as u32)
    } else {
        crate::event::Event::key_release(key_code as u32, modifiers as u32)
    };
    if crate::widget::runtime::dispatch_event(focused, &event) {
        1
    } else {
        0
    }
}

/// `alloc_frugal` compiles `widget::runtime` out, so there is no registry to self-test.
///
/// Answers `0` — "nothing was verified" — rather than a fabricated pass, which is the same
/// honest-absence rule the rest of this module follows.
#[cfg(alloc_frugal)]
fn widget_self_test() -> jni::sys::jint {
    0
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
        // The test reads its own source file: `include_str!` is resolved relative to the
        // file that contains the macro, which is this one.
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

    /// Every byte value survives the text-channel encoding used for a custom MIME payload.
    ///
    /// `ClipData.Item` has no byte field, so the payload travels as a `CharSequence`. The
    /// byte-to-character mapping must be total and reversible for **all** 256 values; a UTF-8
    /// shortcut would fail for byte sequences that are not valid UTF-8.
    #[test]
    fn a_custom_payload_round_trips_through_the_text_channel() {
        let data: Vec<u8> = (0..=255).collect();
        let text = bytes_to_latin1(&data);
        assert_eq!(text.chars().count(), data.len(), "one character per byte");
        assert_eq!(latin1_to_bytes(&text), Some(data));
    }

    /// Text outside the byte range is not mistaken for a payload.
    ///
    /// Ordinary user text can be on the clipboard under a custom MIME only if another
    /// application put it there; decoding it by taking each character's low byte would corrupt
    /// it. `None` is the honest "this is not our encoding" answer.
    #[test]
    fn text_outside_the_byte_range_is_not_decoded_as_a_payload() {
        assert_eq!(latin1_to_bytes("你好"), None);
        assert_eq!(latin1_to_bytes("abc"), Some(b"abc".to_vec()));
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
