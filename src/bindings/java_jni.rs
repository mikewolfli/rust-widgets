// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Java JNI bridge for desktop/mobile — delegates to the C ABI layer.
//!
//! Each `#[no_mangle] pub extern "system"` function follows the JNI
//! naming convention `Java_io_github_rustwidgets_RustWidgets_<method>`.
//!
//! This module compiles when the optional `jni` crate is enabled
//! (`cargo build --features jni`). No Android-specific features are
//! required — the same `.so`/`.dylib`/`.dll` can be loaded from desktop
//! Java.

#![cfg(feature = "jni")]

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jint, jlong, jlongArray, jstring};
use jni::JNIEnv;

/// Width/height argument type for the C ABI layer. The `jni` crate exposes
/// only signed JNI types; the C ABI uses `c_uint`, which is `u32` here.
#[allow(non_camel_case_types)]
type juint = u32;

// ---------------------------------------------------------------------------
// Helper: convert Java string → owned Rust String
// ---------------------------------------------------------------------------
fn jstring_to_string(env: &mut JNIEnv<'_>, input: &JString) -> String {
    if input.is_null() {
        return String::new();
    }
    env.get_string(input).map(|s| s.into()).unwrap_or_default()
}

/// Convert a Java string to a `CString`, throwing a Java exception on a NUL.
///
/// A Java `String` may legitimately contain U+0000, but a C string cannot: the C
/// ABI reads these arguments with `CStr::from_ptr`, which stops at the first
/// NUL. The previous `CString::new(..).unwrap_or_default()` turned such a value
/// into an **empty** string — a silently different value, with no error the
/// caller could observe (D09-JNI-03). This throws
/// `java.lang.IllegalArgumentException` from the JNI frame instead, so the
/// wrapper boundary reports the input it cannot represent; the caller supplies
/// the value used when a Java exception is already pending.
fn jstring_to_cstring(
    env: &mut JNIEnv<'_>,
    input: &JString<'_>,
    fallback: &'static str,
) -> std::ffi::CString {
    let value = jstring_to_string(env, input);
    match std::ffi::CString::new(value) {
        Ok(c) => c,
        Err(_) => {
            // `with_nul` is `InteriorNul`; the payload is the value up to the NUL.
            let _ = env.throw_new(
                "java/lang/IllegalArgumentException",
                "string contains a NUL (U+0000) character, which the C string ABI cannot \
                 represent",
            );
            // A CString cannot hold the rejected value, but the JNI function must
            // still return *something*; the pending Java exception aborts the call
            // before this value is observed. An empty C string is the documented
            // "no value" shape the previous code also produced.
            std::ffi::CString::new(fallback).expect("the fallback literal has no interior NUL")
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: create a Java String from a *const c_char C string pointer
// ---------------------------------------------------------------------------
fn c_string_to_jstring(env: &mut JNIEnv<'_>, ptr: *const std::ffi::c_char) -> jstring {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: The caller guarantees `ptr` is a valid NUL-terminated C string
    // obtained from CString::into_raw() or a compatible FFI source.
    let rust_str = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    // Free the C string that was allocated by the C ABI layer
    if !ptr.is_null() {
        // SAFETY: `ptr` was obtained from CString::into_raw() in the C ABI layer,
        // so rw_free_string will reclaim and deallocate it correctly.
        unsafe {
            crate::bindings::rw_free_string(ptr as *mut std::ffi::c_char);
        }
    }
    env.new_string(&rust_str).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

// ---------------------------------------------------------------------------
// Helper: validate a signed JNI size against the unsigned C ABI
// ---------------------------------------------------------------------------

/// Validate a signed `jint` size for the unsigned C ABI and throw on a negative.
///
/// The Java API takes `int width` / `int height`, but the C ABI takes
/// `u32`. `width as juint` reinterprets `-1` as `4294967295`, so a negative
/// size asked the library for a control as large as an unsigned 32-bit value
/// can be, instead of being reported as invalid (D09-JNI-02). This throws
/// `java.lang.IllegalArgumentException` from the JNI frame, matching the
/// class-level contract that invalid arguments raise.
fn checked_size(env: &mut JNIEnv<'_>, value: jint, name: &str) -> juint {
    if value < 0 {
        let message = format!("{name} must be non-negative, got {value}");
        let _ = env.throw_new("java/lang/IllegalArgumentException", message);
        return 0;
    }
    value as juint
}

// ===========================================================================
// Lifecycle
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeInit`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeInit(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    crate::bindings::rw_init();
}

#[no_mangle]
/// JNI entry point for Java `nativeRun`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeRun(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    crate::bindings::rw_run();
}

#[no_mangle]
/// JNI entry point for Java `nativeQuit`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeQuit(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    crate::bindings::rw_quit();
}

// ===========================================================================
// Widget Creation
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeCreateWindow`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeCreateWindow(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    title: JString<'_>,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) -> jlong {
    let c_title = jstring_to_cstring(&mut env, &title, "");
    let width = checked_size(&mut env, width, "width");
    let height = checked_size(&mut env, height, "height");
    crate::bindings::rw_create_window(c_title.as_ptr(), x, y, width, height) as jlong
}

/// Macro to generate a widget creation JNI function for widgets that take
/// a `text` parameter (button, checkbox, label, radio_button, line_edit,
/// status_bar, menu, message_box, file_dialog, color_dialog, font_dialog).
macro_rules! jni_create_widget_with_text {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
        #[doc = concat!("JNI entry point for Java `", stringify!($name), "`.")]
        pub extern "system" fn $name(
            mut env: JNIEnv<'_>,
            _class: JClass<'_>,
            parent: jlong,
            text: JString<'_>,
            x: jint,
            y: jint,
            width: jint,
            height: jint,
        ) -> jlong {
            let c_text = jstring_to_cstring(&mut env, &text, "");
            let width = checked_size(&mut env, width, "width");
            let height = checked_size(&mut env, height, "height");
            crate::bindings::$c_func(parent as u64, c_text.as_ptr(), x, y, width, height) as jlong
        }
    };
}

/// Macro to generate a widget creation JNI function for widgets without
/// a `text` parameter (slider, progress_bar, combo_box, list_box, panel,
/// spin_box, list_view, scroll_area, tool_bar, menu_bar).
macro_rules! jni_create_widget_no_text {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
        #[doc = concat!("JNI entry point for Java `", stringify!($name), "`.")]
        pub extern "system" fn $name(
            mut env: JNIEnv<'_>,
            _class: JClass<'_>,
            parent: jlong,
            x: jint,
            y: jint,
            width: jint,
            height: jint,
        ) -> jlong {
            let width = checked_size(&mut env, width, "width");
            let height = checked_size(&mut env, height, "height");
            crate::bindings::$c_func(parent as u64, x, y, width, height) as jlong
        }
    };
}

jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateButton,
    rw_create_button
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateCheckbox,
    rw_create_checkbox
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateLineEdit,
    rw_create_line_edit
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateLabel,
    rw_create_label
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateRadioButton,
    rw_create_radio_button
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateStatusBar,
    rw_create_status_bar
);
jni_create_widget_with_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateMenu,
    rw_create_menu
);

jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateSlider,
    rw_create_slider
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateProgressBar,
    rw_create_progress_bar
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateComboBox,
    rw_create_combo_box
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateListBox,
    rw_create_list_box
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreatePanel,
    rw_create_panel
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateSpinBox,
    rw_create_spin_box
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateListView,
    rw_create_list_view
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateScrollArea,
    rw_create_scroll_area
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateToolBar,
    rw_create_tool_bar
);
jni_create_widget_no_text!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateMenuBar,
    rw_create_menu_bar
);

// Dialog variants — take an extra `title` parameter (message box)
#[no_mangle]
/// JNI entry point for Java `nativeCreateMessageBox`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeCreateMessageBox(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    parent: jlong,
    title: JString<'_>,
    text: JString<'_>,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) -> jlong {
    let c_title = jstring_to_cstring(&mut env, &title, "");
    let c_text = jstring_to_cstring(&mut env, &text, "");
    let width = checked_size(&mut env, width, "width");
    let height = checked_size(&mut env, height, "height");
    crate::bindings::rw_create_message_box(
        parent as u64,
        c_title.as_ptr(),
        c_text.as_ptr(),
        x,
        y,
        width,
        height,
    ) as jlong
}

/// Helper macro for dialog creation (file, color, font) — parent + title + geometry.
macro_rules! jni_create_dialog {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
        #[doc = concat!("JNI entry point for Java `", stringify!($name), "`.")]
        pub extern "system" fn $name(
            mut env: JNIEnv<'_>,
            _class: JClass<'_>,
            parent: jlong,
            title: JString<'_>,
            x: jint,
            y: jint,
            width: jint,
            height: jint,
        ) -> jlong {
            let c_title = jstring_to_cstring(&mut env, &title, "");
            let width = checked_size(&mut env, width, "width");
            let height = checked_size(&mut env, height, "height");
            crate::bindings::$c_func(parent as u64, c_title.as_ptr(), x, y, width, height) as jlong
        }
    };
}

jni_create_dialog!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateFileDialog,
    rw_create_file_dialog
);
jni_create_dialog!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateColorDialog,
    rw_create_color_dialog
);
jni_create_dialog!(
    Java_io_github_rustwidgets_RustWidgets_nativeCreateFontDialog,
    rw_create_font_dialog
);

// ===========================================================================
// Widget Manipulation
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeShowWidget`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeShowWidget(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) {
    crate::bindings::rw_show_widget(widget_id as u64);
}

#[no_mangle]
/// JNI entry point for Java `nativeHideWidget`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeHideWidget(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) {
    crate::bindings::rw_hide_widget(widget_id as u64);
}

#[no_mangle]
/// JNI entry point for Java `nativeDestroyWidget`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeDestroyWidget(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) {
    crate::bindings::rw_destroy_widget(widget_id as u64);
}

#[no_mangle]
/// JNI entry point for Java `nativeSetWidgetText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    text: JString<'_>,
) {
    let c_text = jstring_to_cstring(&mut env, &text, "");
    crate::bindings::rw_set_widget_text(widget_id as u64, c_text.as_ptr());
}

#[no_mangle]
/// JNI entry point for Java `nativeGetWidgetText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeGetWidgetText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) -> jstring {
    let ptr = crate::bindings::rw_get_widget_text(widget_id as u64);
    c_string_to_jstring(&mut env, ptr)
}

#[no_mangle]
/// JNI entry point for Java `nativeSetWidgetEnabled`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetEnabled(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    enabled: jboolean,
) {
    crate::bindings::rw_set_widget_enabled(widget_id as u64, enabled != 0);
}

#[no_mangle]
/// JNI entry point for Java `nativeIsWidgetEnabled`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeIsWidgetEnabled(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) -> jboolean {
    if crate::bindings::rw_is_widget_enabled(widget_id as u64) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeSetWidgetGeometry`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetGeometry(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) {
    let width = checked_size(&mut env, width, "width");
    let height = checked_size(&mut env, height, "height");
    crate::bindings::rw_set_widget_geometry(widget_id as u64, x, y, width, height);
}

// ===========================================================================
// Combo Box
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxAddItem`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
    text: JString<'_>,
) -> jboolean {
    let c_text = jstring_to_cstring(&mut env, &text, "");
    if crate::bindings::rw_combo_box_add_item(combo_box as u64, c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxClearItems`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxClearItems(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
) -> jboolean {
    if crate::bindings::rw_combo_box_clear_items(combo_box as u64) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxSetCurrentIndex`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxSetCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
    index: jint,
) -> jboolean {
    if crate::bindings::rw_combo_box_set_current_index(combo_box as u64, index as juint) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxCurrentIndex`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
) -> jint {
    crate::bindings::rw_combo_box_current_index(combo_box as u64) as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxItemCount`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxItemCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
) -> jint {
    crate::bindings::rw_combo_box_item_count(combo_box as u64) as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeComboBoxItemText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxItemText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
    index: jint,
) -> jstring {
    let ptr = crate::bindings::rw_combo_box_item_text(combo_box as u64, index as juint);
    c_string_to_jstring(&mut env, ptr)
}

// ===========================================================================
// List Box
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeListBoxAddItem`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
    text: JString<'_>,
) -> jboolean {
    let c_text = jstring_to_cstring(&mut env, &text, "");
    if crate::bindings::rw_list_box_add_item(list_box as u64, c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxRemoveItem`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxRemoveItem(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
    index: jint,
) -> jboolean {
    if crate::bindings::rw_list_box_remove_item(list_box as u64, index as juint) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxClearItems`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxClearItems(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
) -> jboolean {
    if crate::bindings::rw_list_box_clear_items(list_box as u64) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxSetCurrentIndex`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxSetCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
    index: jint,
) -> jboolean {
    if crate::bindings::rw_list_box_set_current_index(list_box as u64, index as juint) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxCurrentIndex`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
) -> jint {
    crate::bindings::rw_list_box_current_index(list_box as u64) as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxItemCount`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxItemCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
) -> jint {
    crate::bindings::rw_list_box_item_count(list_box as u64) as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeListBoxItemText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxItemText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
    index: jint,
) -> jstring {
    let ptr = crate::bindings::rw_list_box_item_text(list_box as u64, index as juint);
    c_string_to_jstring(&mut env, ptr)
}

// ===========================================================================
// Menus
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeAttachMenuBarToWindow`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeAttachMenuBarToWindow(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    window: jlong,
    menu_bar: jlong,
) -> jboolean {
    if crate::bindings::rw_attach_menu_bar_to_window(window as u64, menu_bar as u64) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeMenuAddItem`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeMenuAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    parent_menu: jlong,
    text: JString<'_>,
    shortcut: JString<'_>,
) -> jlong {
    let c_text = jstring_to_cstring(&mut env, &text, "");
    let c_shortcut = jstring_to_cstring(&mut env, &shortcut, "");
    crate::bindings::rw_menu_add_item(parent_menu as u64, c_text.as_ptr(), c_shortcut.as_ptr())
        as jlong
}

#[no_mangle]
/// JNI entry point for Java `nativePollMenuTriggered`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePollMenuTriggered(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    crate::bindings::rw_poll_menu_triggered() as jlong
}

// ===========================================================================
// Events
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativePollWidgetTriggered`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePollWidgetTriggered(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    crate::bindings::rw_poll_widget_triggered() as jlong
}

/// Polls one widget trigger event.
///
/// Returns a `long[2]` — `{widget_id, kind_code}` — or `null` when no event is
/// available.
///
/// # Why an array and not a packed `jlong`
///
/// `ObjectId` is a full `u64` and `kind_code` needs 32 bits, so the two cannot
/// both fit in one signed 64-bit value. An earlier revision packed them as
/// `kind_code << 32 | (widget_id & 0xFFFF_FFFF)`, which **silently truncated the
/// id to 32 bits**: any id above `u32::MAX` was reported as a different,
/// unrelated widget, and the intended one could not be recovered. An earlier
/// doc comment even described a "long array of 2 elements" while the code
/// returned a packed scalar — the comment described the fix.
///
/// Returning the pair losslessly is the honest shape: no bit of either value is
/// discarded, and `RustWidgets.pollWidgetTriggerEvent` decodes two elements.
#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePollWidgetTriggerEvent(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlongArray {
    let mut widget_id_out: u64 = 0;
    let kind_code =
        unsafe { crate::bindings::rw_poll_widget_trigger_event(&mut widget_id_out as *mut u64) };
    if kind_code == 0 {
        return std::ptr::null_mut();
    }
    let values = [widget_id_out as jlong, kind_code as jlong];
    let Ok(array) = env.new_long_array(2) else {
        return std::ptr::null_mut();
    };
    if env.set_long_array_region(&array, 0, &values).is_err() {
        return std::ptr::null_mut();
    }
    array.into_raw()
}

// ===========================================================================
// Clipboard
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeSetClipboardText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetClipboardText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    text: JString<'_>,
) -> jboolean {
    let c_text = jstring_to_cstring(&mut env, &text, "");
    if crate::bindings::rw_set_clipboard_text(c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
/// JNI entry point for Java `nativeGetClipboardText`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeGetClipboardText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let ptr = crate::bindings::rw_get_clipboard_text();
    c_string_to_jstring(&mut env, ptr)
}

// ===========================================================================
// Platform Information
// ===========================================================================

#[no_mangle]
/// JNI entry point for Java `nativeBackendName`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeBackendName(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let ptr = crate::bindings::rw_backend_name();
    c_string_to_jstring(&mut env, ptr)
}

#[no_mangle]
/// JNI entry point for Java `nativePlatformCapabilities`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePlatformCapabilities(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    crate::bindings::rw_platform_capabilities() as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeBindingsApiVersion`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeBindingsApiVersion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    crate::bindings::rw_bindings_api_version() as jint
}

#[no_mangle]
/// JNI entry point for Java `nativeFreeString`.
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeFreeString(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe {
            crate::bindings::rw_free_string(ptr as *mut std::ffi::c_char);
        }
    }
}
