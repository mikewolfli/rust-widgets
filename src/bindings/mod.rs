//! Stable C ABI for foreign language bindings.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};

type CBool = bool;

fn trigger_kind_from_code(code: c_uint) -> crate::platform::WidgetTriggerKind {
    match code {
        1 => crate::platform::WidgetTriggerKind::Clicked,
        2 => crate::platform::WidgetTriggerKind::ValueChanged,
        _ => crate::platform::WidgetTriggerKind::Unknown,
    }
}

fn c_str_or_default(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

#[no_mangle]
pub extern "C" fn rust_widgets_init() {
    crate::init();
}

#[no_mangle]
pub extern "C" fn rust_widgets_run() {
    crate::run();
}

#[no_mangle]
pub extern "C" fn rust_widgets_quit() {
    crate::quit();
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_window(&c_str_or_default(title), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_button(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_button(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_checkbox(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_checkbox(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_line_edit(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_line_edit(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_menu_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_menu_bar(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_menu(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_menu(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_attach_menu_bar_to_window(window: u64, menu_bar: u64) -> CBool {
    crate::platform::get_platform().attach_menu_bar_to_window(window, menu_bar)
}

#[no_mangle]
pub extern "C" fn rust_widgets_menu_add_item(
    parent_menu: u64,
    text: *const c_char,
    shortcut: *const c_char,
) -> u64 {
    let shortcut_text = if shortcut.is_null() {
        None
    } else {
        Some(c_str_or_default(shortcut))
    };
    crate::platform::get_platform().menu_add_item(
        parent_menu,
        &c_str_or_default(text),
        shortcut_text.as_deref(),
    )
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_menu_triggered() -> u64 {
    crate::platform::get_platform()
        .poll_menu_triggered()
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_widget_triggered() -> u64 {
    crate::platform::get_platform()
        .poll_widget_triggered()
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn rust_widgets_poll_widget_trigger_event(widget_id_out: *mut u64) -> c_uint {
    let Some(event) = crate::platform::get_platform().poll_widget_trigger_event() else {
        return 0;
    };
    if !widget_id_out.is_null() {
        unsafe {
            *widget_id_out = event.widget_id;
        }
    }
    event.kind as c_uint
}

#[no_mangle]
pub extern "C" fn rust_widgets_inject_menu_trigger(menu_item_id: u64) -> CBool {
    crate::platform::get_platform().inject_menu_trigger(menu_item_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_inject_widget_trigger_event(widget_id: u64, kind_code: c_uint) -> CBool {
    crate::platform::get_platform().inject_widget_trigger_event(
        widget_id,
        trigger_kind_from_code(kind_code),
    )
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_tool_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_tool_bar(parent, x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_create_status_bar(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    crate::platform::get_platform().create_status_bar(parent, &c_str_or_default(text), x, y, width, height)
}

#[no_mangle]
pub extern "C" fn rust_widgets_show_widget(widget_id: u64) {
    crate::platform::get_platform().show_widget(widget_id);
}

#[no_mangle]
pub extern "C" fn rust_widgets_hide_widget(widget_id: u64) {
    crate::platform::get_platform().hide_widget(widget_id);
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_text(widget_id: u64, text: *const c_char) {
    crate::platform::get_platform().set_widget_text(widget_id, &c_str_or_default(text));
}

#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_text(widget_id: u64) -> *const c_char {
    let text = crate::platform::get_platform().get_widget_text(widget_id);
    match CString::new(text) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new("").expect("static string is valid").into_raw(),
    }
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_enabled(widget_id: u64, enabled: CBool) {
    crate::platform::get_platform().set_widget_enabled(widget_id, enabled);
}

#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_enabled(widget_id: u64) -> CBool {
    crate::platform::get_platform().is_widget_enabled(widget_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_visible(widget_id: u64, visible: CBool) {
    crate::platform::get_platform().set_widget_visible(widget_id, visible);
}

#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_visible(widget_id: u64) -> CBool {
    crate::platform::get_platform().is_widget_visible(widget_id)
}

#[no_mangle]
pub extern "C" fn rust_widgets_backend_name() -> *const c_char {
    CString::new(crate::platform::get_platform().backend_name())
        .expect("backend string is valid")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn rust_widgets_bindings_api_version() -> c_uint {
    1
}

#[no_mangle]
pub extern "C" fn rust_widgets_python_reserved() -> c_uint {
    0
}

#[no_mangle]
pub extern "C" fn rust_widgets_cpp_reserved() -> c_uint {
    0
}

#[no_mangle]
pub extern "C" fn rust_widgets_java_reserved() -> c_uint {
    0
}

#[no_mangle]
pub extern "C" fn rust_widgets_free_string(ptr: *const c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr as *mut c_char);
    }
}
