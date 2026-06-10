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
use jni::sys::{jboolean, jint, jlong, jstring, juint};
use jni::JNIEnv;

// ---------------------------------------------------------------------------
// Helper: convert Java string → owned Rust String
// ---------------------------------------------------------------------------
fn jstring_to_string(env: &mut JNIEnv<'_>, input: &JString) -> String {
    if input.is_null() {
        return String::new();
    }
    env.get_string(input).map(|s| s.into()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Helper: create a Java String from a *const c_char C string pointer
// ---------------------------------------------------------------------------
fn c_string_to_jstring(env: &mut JNIEnv<'_>, ptr: *const std::ffi::c_char) -> jstring {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let rust_str = unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
    // Free the C string that was allocated by the C ABI layer
    if !ptr.is_null() {
        unsafe {
            crate::bindings::rw_free_string(ptr as *mut std::ffi::c_char);
        }
    }
    env.new_string(&rust_str).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

// ===========================================================================
// Lifecycle
// ===========================================================================

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeInit(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    crate::bindings::rw_init();
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeRun(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    crate::bindings::rw_run();
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeCreateWindow(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    title: JString<'_>,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) -> jlong {
    let title_str = jstring_to_string(&mut env, &title);
    let c_title = std::ffi::CString::new(title_str).unwrap_or_default();
    crate::bindings::rw_create_window(c_title.as_ptr(), x, y, width as juint, height as juint)
        as jlong
}

/// Macro to generate a widget creation JNI function for widgets that take
/// a `text` parameter (button, checkbox, label, radio_button, line_edit,
/// status_bar, menu, message_box, file_dialog, color_dialog, font_dialog).
macro_rules! jni_create_widget_with_text {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
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
            let text_str = jstring_to_string(&mut env, &text);
            let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
            crate::bindings::$c_func(
                parent as u64,
                c_text.as_ptr(),
                x,
                y,
                width as juint,
                height as juint,
            ) as jlong
        }
    };
}

/// Macro to generate a widget creation JNI function for widgets without
/// a `text` parameter (slider, progress_bar, combo_box, list_box, panel,
/// spin_box, list_view, scroll_area, tool_bar, menu_bar).
macro_rules! jni_create_widget_no_text {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
        pub extern "system" fn $name(
            _env: JNIEnv<'_>,
            _class: JClass<'_>,
            parent: jlong,
            x: jint,
            y: jint,
            width: jint,
            height: jint,
        ) -> jlong {
            crate::bindings::$c_func(parent as u64, x, y, width as juint, height as juint) as jlong
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
    let title_str = jstring_to_string(&mut env, &title);
    let text_str = jstring_to_string(&mut env, &text);
    let c_title = std::ffi::CString::new(title_str).unwrap_or_default();
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    crate::bindings::rw_create_message_box(
        parent as u64,
        c_title.as_ptr(),
        c_text.as_ptr(),
        x,
        y,
        width as juint,
        height as juint,
    ) as jlong
}

/// Helper macro for dialog creation (file, color, font) — parent + title + geometry.
macro_rules! jni_create_dialog {
    ($name:ident, $c_func:ident) => {
        #[no_mangle]
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
            let title_str = jstring_to_string(&mut env, &title);
            let c_title = std::ffi::CString::new(title_str).unwrap_or_default();
            crate::bindings::$c_func(
                parent as u64,
                c_title.as_ptr(),
                x,
                y,
                width as juint,
                height as juint,
            ) as jlong
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeShowWidget(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) {
    crate::bindings::rw_show_widget(widget_id as u64);
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeHideWidget(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) {
    crate::bindings::rw_hide_widget(widget_id as u64);
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    text: JString<'_>,
) {
    let text_str = jstring_to_string(&mut env, &text);
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    crate::bindings::rw_set_widget_text(widget_id as u64, c_text.as_ptr());
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeGetWidgetText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
) -> jstring {
    let ptr = crate::bindings::rw_get_widget_text(widget_id as u64);
    c_string_to_jstring(&mut env, ptr)
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetEnabled(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    enabled: jboolean,
) {
    crate::bindings::rw_set_widget_enabled(widget_id as u64, enabled != 0);
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetWidgetGeometry(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    widget_id: jlong,
    x: jint,
    y: jint,
    width: jint,
    height: jint,
) {
    crate::bindings::rw_set_widget_geometry(
        widget_id as u64,
        x,
        y,
        width as juint,
        height as juint,
    );
}

// ===========================================================================
// Combo Box
// ===========================================================================

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
    text: JString<'_>,
) -> jboolean {
    let text_str = jstring_to_string(&mut env, &text);
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    if crate::bindings::rw_combo_box_add_item(combo_box as u64, c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
) -> jint {
    crate::bindings::rw_combo_box_current_index(combo_box as u64) as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeComboBoxItemCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    combo_box: jlong,
) -> jint {
    crate::bindings::rw_combo_box_item_count(combo_box as u64) as jint
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
    text: JString<'_>,
) -> jboolean {
    let text_str = jstring_to_string(&mut env, &text);
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    if crate::bindings::rw_list_box_add_item(list_box as u64, c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxCurrentIndex(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
) -> jint {
    crate::bindings::rw_list_box_current_index(list_box as u64) as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeListBoxItemCount(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    list_box: jlong,
) -> jint {
    crate::bindings::rw_list_box_item_count(list_box as u64) as jint
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeMenuAddItem(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    parent_menu: jlong,
    text: JString<'_>,
    shortcut: JString<'_>,
) -> jlong {
    let text_str = jstring_to_string(&mut env, &text);
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    let shortcut_str = jstring_to_string(&mut env, &shortcut);
    let c_shortcut = std::ffi::CString::new(shortcut_str).unwrap_or_default();
    crate::bindings::rw_menu_add_item(parent_menu as u64, c_text.as_ptr(), c_shortcut.as_ptr())
        as jlong
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePollWidgetTriggered(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    crate::bindings::rw_poll_widget_triggered() as jlong
}

/// Returns a long array of 2 elements: [widget_id, trigger_kind_code].
/// Returns null if no event is available.
#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePollWidgetTriggerEvent(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    // Use C ABI poll_widget_trigger_event with a stack variable
    let mut widget_id_out: u64 = 0;
    let kind_code =
        unsafe { crate::bindings::rw_poll_widget_trigger_event(&mut widget_id_out as *mut u64) };
    if kind_code == 0 {
        return 0;
    }
    // Pack widget_id and kind_code into a single long:
    // upper 32 bits = kind_code, lower 32 bits = widget_id
    ((kind_code as jlong) << 32) | (widget_id_out as jlong & 0xFFFF_FFFF)
}

// ===========================================================================
// Clipboard
// ===========================================================================

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeSetClipboardText(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    text: JString<'_>,
) -> jboolean {
    let text_str = jstring_to_string(&mut env, &text);
    let c_text = std::ffi::CString::new(text_str).unwrap_or_default();
    if crate::bindings::rw_set_clipboard_text(c_text.as_ptr()) {
        1
    } else {
        0
    }
}

#[no_mangle]
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
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeBackendName(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    let ptr = crate::bindings::rw_backend_name();
    c_string_to_jstring(&mut env, ptr)
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativePlatformCapabilities(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    crate::bindings::rw_platform_capabilities() as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_rustwidgets_RustWidgets_nativeBindingsApiVersion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    crate::bindings::rw_bindings_api_version() as jint
}

#[no_mangle]
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
