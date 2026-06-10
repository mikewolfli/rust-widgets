//! Stable C ABI for foreign language bindings.
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::control_backend::get_control_backend;
use crate::{c_try, c_try_void};
use alloc::boxed::Box;
use alloc::ffi::CString;
use core::ffi::{c_char, c_float, c_int, c_uint, CStr};
#[cfg(not(feature = "mini"))]
use std::sync::OnceLock;
type CBool = bool;
/// Global node-handle registry used by Harmony native bridge callbacks.
fn harmony_node_registry() -> &'static Mutex<HashMap<u64, u64>> {
    static REGISTRY: OnceLock<Mutex<HashMap<u64, u64>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}
fn harmony_lookup_widget(node_handle: u64) -> Option<u64> {
    if node_handle == 0 {
        return None;
    }
    harmony_node_registry().lock().unwrap_or_else(|e| e.into_inner()).get(&node_handle).copied()
}
/// Convert stable C ABI trigger code to internal typed trigger enum.
fn trigger_kind_from_code(code: c_uint) -> crate::platform::WidgetTriggerKind {
    match code {
        1 => crate::platform::WidgetTriggerKind::Clicked,
        2 => crate::platform::WidgetTriggerKind::ValueChanged,
        3 => crate::platform::WidgetTriggerKind::SelectionChanged,
        4 => crate::platform::WidgetTriggerKind::Closed,
        _ => crate::platform::WidgetTriggerKind::Unknown,
    }
}
fn capability_contract_mask(contract: crate::platform::CapabilityContract) -> c_uint {
    match contract {
        crate::platform::CapabilityContract::Native(native) => {
            let mut mask: c_uint = 0;
            mask |= 1 << 0;
            if native.dpi_scaling {
                mask |= 1 << 1;
            }
            if native.ime {
                mask |= 1 << 2;
            }
            if native.accessibility {
                mask |= 1 << 3;
            }
            if native.native_menu {
                mask |= 1 << 4;
            }
            if native.typed_widget_trigger {
                mask |= 1 << 5;
            }
            mask
        }
        crate::platform::CapabilityContract::Embedded(embedded) => {
            let mut mask: c_uint = 0;
            if embedded.fixed_dpi {
                mask |= 1 << 1;
            }
            if embedded.low_memory_mode {
                mask |= 1 << 2;
            }
            if embedded.typed_widget_trigger {
                mask |= 1 << 3;
            }
            mask
        }
    }
}
/// Convert nullable C string pointer to owned Rust `String`.
/// Logs a warning when a null pointer is received.
fn c_str_or_default(ptr: *const c_char) -> String {
    if ptr.is_null() {
        log::warn!("[bindings] c_str_or_default: received null C string pointer");
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

/// Convert a Rust string to a C string pointer, returning an empty C string on failure.
/// Never panics — all interior-NUL errors are caught.
fn to_c_string_or_empty(s: impl Into<String>) -> *const c_char {
    let owned: String = s.into();
    match CString::new(owned) {
        Ok(cs) => cs.into_raw(),
        Err(nul_err) => {
            let pos = nul_err.nul_position();
            log::warn!(
                "[bindings] CString::new failed (interior NUL at position {}), truncating",
                pos
            );
            // Truncate — return an empty C string.
            CString::new("").unwrap().into_raw()
        }
    }
}
#[no_mangle]
pub extern "C" fn rust_widgets_init() {
    c_try_void!({
        crate::init();
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_run() {
    c_try_void!({
        crate::run();
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_quit() {
    c_try_void!({
        crate::quit();
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_window(&c_str_or_default(title), x, y, width, height) })
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
    c_try!({
        get_control_backend().create_button(parent, &c_str_or_default(text), x, y, width, height)
    })
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
    c_try!({
        get_control_backend().create_checkbox(parent, &c_str_or_default(text), x, y, width, height)
    })
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
    c_try!({
        get_control_backend().create_line_edit(parent, &c_str_or_default(text), x, y, width, height)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_label(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_label(parent, &c_str_or_default(text), x, y, width, height)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_radio_button(
    parent: u64,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_radio_button(
            parent,
            &c_str_or_default(text),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_slider(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_slider(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_progress_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_progress_bar(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_combo_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_combo_box(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_list_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_list_box(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_panel(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_panel(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_message_box(
    parent: u64,
    title: *const c_char,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_message_box(
            parent,
            &c_str_or_default(title),
            &c_str_or_default(text),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_file_dialog(
    parent: u64,
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_file_dialog(
            parent,
            &c_str_or_default(title),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_color_dialog(
    parent: u64,
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_color_dialog(
            parent,
            &c_str_or_default(title),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_font_dialog(
    parent: u64,
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_font_dialog(
            parent,
            &c_str_or_default(title),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_spin_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_spin_box(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_list_view(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_list_view(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_scroll_area(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_scroll_area(parent, x, y, width, height) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_geometry(
    widget_id: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) {
    c_try_void!({
        get_control_backend().set_widget_geometry(widget_id, x, y, width, height);
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_geometry(
    widget_id: u64,
    x_out: *mut c_int,
    y_out: *mut c_int,
    width_out: *mut c_uint,
    height_out: *mut c_uint,
) -> CBool {
    c_try!({
        let geo = get_control_backend().get_widget_geometry(widget_id);
        if let Some((x, y, w, h)) = geo {
            unsafe {
                if !x_out.is_null() {
                    *x_out = x;
                }
                if !y_out.is_null() {
                    *y_out = y;
                }
                if !width_out.is_null() {
                    *width_out = w;
                }
                if !height_out.is_null() {
                    *height_out = h;
                }
            }
            true
        } else {
            false
        }
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_add_item(combo_box: u64, text: *const c_char) -> CBool {
    c_try!({ get_control_backend().combo_box_add_item(combo_box, &c_str_or_default(text)) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_clear_items(combo_box: u64) -> CBool {
    c_try!({ get_control_backend().combo_box_clear_items(combo_box) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_set_current_index(combo_box: u64, index: c_uint) -> CBool {
    c_try!({
        crate::platform::get_platform().combo_box_set_current_index(combo_box, index as usize)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_current_index(combo_box: u64) -> c_int {
    c_try!({
        match crate::platform::get_platform().combo_box_current_index(combo_box) {
            Some(idx) => idx as c_int,
            None => -1,
        }
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_item_count(combo_box: u64) -> c_uint {
    c_try!({ crate::platform::get_platform().combo_box_item_count(combo_box) as c_uint })
}
#[no_mangle]
pub extern "C" fn rust_widgets_combo_box_item_text(combo_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::platform::get_platform().combo_box_item_text(combo_box, index as usize);
        to_c_string_or_empty(text.unwrap_or_default())
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_add_item(list_box: u64, text: *const c_char) -> CBool {
    c_try!({ get_control_backend().list_box_add_item(list_box, &c_str_or_default(text)) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_remove_item(list_box: u64, index: c_uint) -> CBool {
    c_try!({ get_control_backend().list_box_remove_item(list_box, index as usize) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_clear_items(list_box: u64) -> CBool {
    c_try!({ get_control_backend().list_box_clear_items(list_box) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_set_current_index(list_box: u64, index: c_uint) -> CBool {
    c_try!({ crate::platform::get_platform().list_box_set_current_index(list_box, index as usize) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_current_index(list_box: u64) -> c_int {
    c_try!({
        match crate::platform::get_platform().list_box_current_index(list_box) {
            Some(idx) => idx as c_int,
            None => -1,
        }
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_item_count(list_box: u64) -> c_uint {
    c_try!({ crate::platform::get_platform().list_box_item_count(list_box) as c_uint })
}
#[no_mangle]
pub extern "C" fn rust_widgets_list_box_item_text(list_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::platform::get_platform().list_box_item_text(list_box, index as usize);
        to_c_string_or_empty(text.unwrap_or_default())
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_clipboard_text(text: *const c_char) -> CBool {
    c_try!({ get_control_backend().set_clipboard_text(&c_str_or_default(text)) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_clipboard_text() -> *const c_char {
    c_try!({
        let text = get_control_backend().get_clipboard_text();
        to_c_string_or_empty(text)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_begin_drag(
    source: u64,
    mime_type: *const c_char,
    payload: *const u8,
    payload_len: c_uint,
) -> CBool {
    c_try!({
        let slice = if payload.is_null() || payload_len == 0 {
            &[]
        } else {
            unsafe { core::slice::from_raw_parts(payload, payload_len as usize) }
        };
        get_control_backend().begin_drag(source, &c_str_or_default(mime_type), slice)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_poll_drop_event(
    source_out: *mut u64,
    target_out: *mut u64,
    mime_out: *mut *mut c_char,
    payload_out: *mut *mut u8,
    payload_len_out: *mut c_uint,
) -> CBool {
    c_try!({
        let Some(event) = get_control_backend().poll_drop_event() else {
            return false;
        };
        unsafe {
            if !source_out.is_null() {
                *source_out = event.source_widget_id;
            }
            if !target_out.is_null() {
                *target_out = event.target_widget_id;
            }
            if !mime_out.is_null() {
                let cs = CString::new(event.mime).unwrap_or_else(|_| CString::new("").unwrap());
                *mime_out = cs.into_raw();
            }
            if !payload_out.is_null() && !payload_len_out.is_null() && !event.payload.is_empty() {
                let len = event.payload.len();
                let slice = event.payload.into_boxed_slice();
                *payload_out = Box::into_raw(slice) as *mut u8;
                *payload_len_out = len as c_uint;
            } else {
                if !payload_out.is_null() {
                    *payload_out = std::ptr::null_mut();
                }
                if !payload_len_out.is_null() {
                    *payload_len_out = 0;
                }
            }
        }
        true
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_menu_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_menu_bar(parent, x, y, width, height) })
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
    c_try!({
        get_control_backend().create_menu(parent, &c_str_or_default(text), x, y, width, height)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_attach_menu_bar_to_window(window: u64, menu_bar: u64) -> CBool {
    c_try!({ get_control_backend().attach_menu_bar_to_window(window, menu_bar) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_menu_add_item(
    parent_menu: u64,
    text: *const c_char,
    shortcut: *const c_char,
) -> u64 {
    c_try!({
        let shortcut_text =
            if shortcut.is_null() { None } else { Some(c_str_or_default(shortcut)) };
        get_control_backend().menu_add_item(
            parent_menu,
            &c_str_or_default(text),
            shortcut_text.as_deref(),
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_poll_menu_triggered() -> u64 {
    c_try!({ get_control_backend().poll_menu_triggered().unwrap_or(0) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_poll_widget_triggered() -> u64 {
    c_try!({ get_control_backend().poll_widget_triggered().unwrap_or(0) })
}
/// Polls the next widget trigger event and optionally writes the widget ID to the provided pointer.
///
/// # Safety
/// The `widget_id_out` pointer must be either null or valid for writing a `u64`.
#[no_mangle]
pub unsafe extern "C" fn rust_widgets_poll_widget_trigger_event(widget_id_out: *mut u64) -> c_uint {
    c_try!({
        let Some(event) = get_control_backend().poll_widget_trigger_event() else {
            return 0;
        };
        if !widget_id_out.is_null() {
            *widget_id_out = event.widget_id;
        }
        event.kind as c_uint
    })
}
/// Generic menu trigger injection entrypoint for native hosts.
#[no_mangle]
pub extern "C" fn rust_widgets_inject_menu_trigger(menu_item_id: u64) -> CBool {
    c_try!({ get_control_backend().inject_menu_trigger(menu_item_id) })
}
/// Generic typed widget trigger injection entrypoint for native hosts.
#[no_mangle]
pub extern "C" fn rust_widgets_inject_widget_trigger_event(
    widget_id: u64,
    kind_code: c_uint,
) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
/// Harmony callback alias: direct menu item trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_menu_item(menu_item_id: u64) -> CBool {
    c_try!({ get_control_backend().inject_menu_trigger(menu_item_id) })
}
/// Harmony callback alias: direct click trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_click(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
    })
}
/// Harmony callback alias: direct value-changed trigger by widget id.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_value_changed(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend().inject_widget_trigger_event(
            widget_id,
            crate::platform::WidgetTriggerKind::ValueChanged,
        )
    })
}
/// Harmony callback alias: direct typed trigger by widget id and kind code.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_widget_event(widget_id: u64, kind_code: c_uint) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
/// Register a Harmony node handle to logical widget id mapping.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_bind_node(node_handle: u64, widget_id: u64) -> CBool {
    c_try!({
        if node_handle == 0 || widget_id == 0 {
            return false;
        }
        harmony_node_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(node_handle, widget_id);
        true
    })
}
/// Remove a single Harmony node-handle mapping.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_unbind_node(node_handle: u64) -> CBool {
    c_try!({
        if node_handle == 0 {
            return false;
        }
        harmony_node_registry()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&node_handle)
            .is_some()
    })
}
/// Resolve mapped widget id from Harmony node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_lookup_widget_id(node_handle: u64) -> u64 {
    c_try!({ harmony_lookup_widget(node_handle).unwrap_or(0) })
}
/// Clear all Harmony node-handle mappings.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_clear_node_bindings() {
    c_try_void!({
        harmony_node_registry().lock().unwrap_or_else(|e| e.into_inner()).clear();
    })
}
/// Harmony callback alias: menu trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_menu_item(node_handle: u64) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend().inject_menu_trigger(widget_id)
    })
}
/// Harmony callback alias: click trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_click(node_handle: u64) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend()
            .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
    })
}
/// Harmony callback alias: value-changed trigger by node handle.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_value_changed(node_handle: u64) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend().inject_widget_trigger_event(
            widget_id,
            crate::platform::WidgetTriggerKind::ValueChanged,
        )
    })
}
/// Harmony callback alias: typed trigger by node handle and kind code.
#[no_mangle]
pub extern "C" fn rust_widgets_harmony_on_node_widget_event(
    node_handle: u64,
    kind_code: c_uint,
) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_create_tool_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_tool_bar(parent, x, y, width, height) })
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
    c_try!({
        get_control_backend().create_status_bar(
            parent,
            &c_str_or_default(text),
            x,
            y,
            width,
            height,
        )
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_show_widget(widget_id: u64) {
    c_try_void!({
        get_control_backend().show_widget(widget_id);
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_hide_widget(widget_id: u64) {
    c_try_void!({
        get_control_backend().hide_widget(widget_id);
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_text(widget_id: u64, text: *const c_char) {
    c_try_void!({
        get_control_backend().set_widget_text(widget_id, &c_str_or_default(text));
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_text(widget_id: u64) -> *const c_char {
    c_try!({
        let text = get_control_backend().get_widget_text(widget_id);
        to_c_string_or_empty(text)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_enabled(widget_id: u64, enabled: CBool) {
    c_try_void!({
        get_control_backend().set_widget_enabled(widget_id, enabled);
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_enabled(widget_id: u64) -> CBool {
    c_try!({ get_control_backend().is_widget_enabled(widget_id) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_visible(widget_id: u64, visible: CBool) {
    c_try_void!({
        get_control_backend().set_widget_visible(widget_id, visible);
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_visible(widget_id: u64) -> CBool {
    c_try!({ get_control_backend().is_widget_visible(widget_id) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_ime_enabled(widget_id: u64, enabled: CBool) -> CBool {
    c_try!({ crate::platform::get_platform().set_widget_ime_enabled(widget_id, enabled) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_is_widget_ime_enabled(widget_id: u64) -> CBool {
    c_try!({ crate::platform::get_platform().is_widget_ime_enabled(widget_id) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_widget_accessibility_name(
    widget_id: u64,
    name: *const c_char,
) -> CBool {
    c_try!({
        crate::platform::get_platform()
            .set_widget_accessibility_name(widget_id, &c_str_or_default(name))
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_widget_accessibility_name(widget_id: u64) -> *const c_char {
    c_try!({
        let name = crate::platform::get_platform().get_widget_accessibility_name(widget_id);
        to_c_string_or_empty(name)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_backend_name() -> *const c_char {
    c_try!({ to_c_string_or_empty(get_control_backend().backend_name()) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_platform_capabilities() -> c_uint {
    c_try!({
        let caps = crate::platform::capabilities();
        let mut mask: c_uint = 0;
        if caps.dpi_scaling {
            mask |= 1 << 0;
        }
        if caps.ime {
            mask |= 1 << 1;
        }
        if caps.accessibility {
            mask |= 1 << 2;
        }
        if caps.native_menu {
            mask |= 1 << 3;
        }
        if caps.typed_widget_trigger {
            mask |= 1 << 4;
        }
        mask
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_platform_dpi_scale_factor() -> c_float {
    c_try!({ crate::platform::dpi_scale_factor() })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_render_aa_samples_per_axis(samples: c_uint) -> c_uint {
    c_try!({
        let config =
            crate::render::SoftwareRenderConfig { aa_samples_per_axis: samples as u8 }.normalized();
        crate::render::set_default_software_render_config(config);
        crate::render::default_software_render_config().aa_samples_per_axis as c_uint
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_render_aa_samples_per_axis() -> c_uint {
    c_try!({ crate::render::default_software_render_config().aa_samples_per_axis as c_uint })
}
#[no_mangle]
pub extern "C" fn rust_widgets_set_embedded_target_fps(fps: c_uint) -> c_uint {
    c_try!({ crate::render_engine::set_embedded_target_fps(fps) as c_uint })
}
#[no_mangle]
pub extern "C" fn rust_widgets_get_embedded_target_fps() -> c_uint {
    c_try!({ crate::render_engine::embedded_target_fps() as c_uint })
}
#[no_mangle]
pub extern "C" fn rust_widgets_submit_embedded_noop_task(label: *const c_char) -> u64 {
    c_try!({ crate::render_engine::submit_embedded_task(c_str_or_default(label), |_| {}) })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_is_initialized() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().initialized })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_is_running() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().running })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_frame_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().frame_count })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_pending_task_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().pending_task_count as u64 })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_window_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().window_count as u64 })
}
#[no_mangle]
pub extern "C" fn rust_widgets_embedded_engine_button_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().button_count as u64 })
}
#[no_mangle]
pub extern "C" fn rust_widgets_platform_capability_contract(profile_code: c_uint) -> c_uint {
    c_try!({
        let profile = if profile_code == 1 {
            crate::core::RuntimeProfile::Embedded
        } else {
            crate::core::RuntimeProfile::Full
        };
        let contract = crate::platform::negotiate_capability_contract(profile);
        capability_contract_mask(contract)
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_mobile_backend_name() -> *const c_char {
    c_try!({
        #[cfg(feature = "mobile-api")]
        {
            to_c_string_or_empty(crate::platform::mobile_backend_name())
        }
        #[cfg(not(feature = "mobile-api"))]
        {
            // Empty string never contains interior NUL bytes.
            CString::new("").unwrap().into_raw()
        }
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_mobile_attach_native_view(native_handle: u64) -> CBool {
    c_try!({
        #[cfg(feature = "mobile-api")]
        {
            crate::platform::mobile_attach_to_native_view(native_handle as usize)
        }
        #[cfg(not(feature = "mobile-api"))]
        {
            let _ = native_handle;
            false
        }
    })
}
#[no_mangle]
pub extern "C" fn rust_widgets_bindings_api_version() -> c_uint {
    c_try!({ 7 })
}
/// Return Python binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Python adapter/example available
/// - bit2: profile-aware capability query available
#[no_mangle]
pub extern "C" fn rust_widgets_python_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}
/// Return C++ wrapper status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: C++ wrapper skeleton/example available
#[no_mangle]
pub extern "C" fn rust_widgets_cpp_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) })
}
/// Return Java/JNI binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Java native-method skeleton available
/// - bit2: JNI bridge skeleton available
#[no_mangle]
pub extern "C" fn rust_widgets_java_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}
/// Return Java/JNI skeleton ABI version.
#[no_mangle]
pub extern "C" fn rust_widgets_java_jni_skeleton_version() -> c_uint {
    c_try!({ 1 })
}
#[no_mangle]
pub extern "C" fn rust_widgets_python_reserved() -> c_uint {
    c_try!({ rust_widgets_python_binding_status() })
}
#[no_mangle]
pub extern "C" fn rust_widgets_cpp_reserved() -> c_uint {
    c_try!({ rust_widgets_cpp_binding_status() })
}
#[no_mangle]
pub extern "C" fn rust_widgets_java_reserved() -> c_uint {
    c_try!({ rust_widgets_java_binding_status() })
}
#[no_mangle]
/// # Safety
///
/// `s` must be either null or a pointer returned by this crate through
/// `CString::into_raw` and not already freed. Passing any other pointer or
/// double-freeing is undefined behavior.
pub unsafe extern "C" fn rust_widgets_free_string(s: *mut c_char) {
    c_try_void!({
        if s.is_null() {
            return;
        }
        unsafe {
            let _ = CString::from_raw(s);
        }
    })
}

/// Alias for [`rust_widgets_free_string`] — explicitly named for callers
/// who hold a `*mut c_char` from Rust-allocated strings and want clarity
/// in their own code.
#[no_mangle]
/// # Safety
///
/// Same requirements as [`rust_widgets_free_string`]: `s` must be null or a
/// valid pointer previously allocated by this crate for C ownership transfer.
pub unsafe extern "C" fn rust_widgets_free_rust_string(s: *mut c_char) {
    rust_widgets_free_string(s);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_aa_sample_abi_roundtrip_clamps_values() {
        let _guard = crate::render::software_render_config_test_lock()
            .lock()
            .expect("software render config test lock poisoned");
        let original = rust_widgets_get_render_aa_samples_per_axis();
        let low = rust_widgets_set_render_aa_samples_per_axis(0);
        assert_eq!(low, 1);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), 1);
        let high = rust_widgets_set_render_aa_samples_per_axis(100);
        assert_eq!(high, 8);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), 8);
        rust_widgets_set_render_aa_samples_per_axis(original);
        assert_eq!(rust_widgets_get_render_aa_samples_per_axis(), original.clamp(1, 8));
    }
    #[test]
    fn embedded_target_fps_abi_roundtrip_clamps_values() {
        let original = rust_widgets_get_embedded_target_fps();
        let low = rust_widgets_set_embedded_target_fps(0);
        assert_eq!(low, 1);
        assert_eq!(rust_widgets_get_embedded_target_fps(), 1);
        let high = rust_widgets_set_embedded_target_fps(1000);
        assert_eq!(high, 240);
        assert_eq!(rust_widgets_get_embedded_target_fps(), 240);
        rust_widgets_set_embedded_target_fps(original);
        assert_eq!(rust_widgets_get_embedded_target_fps(), original.clamp(1, 240));
    }
}
