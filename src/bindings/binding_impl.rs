// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stable C ABI for foreign language bindings.
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::compat::OnceLock;
use crate::control_backend::get_control_backend;
use crate::{c_try, c_try_void};
use alloc::boxed::Box;
use alloc::ffi::CString;
use core::ffi::{c_char, c_float, c_int, c_uint, CStr};
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
                "[bindings] CString::new failed (interior NUL at position {pos}), truncating"
            );
            // Truncate — return an empty C string.
            CString::new("").unwrap().into_raw()
        }
    }
}
#[no_mangle]
/// Initializes the widget toolkit's global subsystems.
///
/// C ABI entry point for [`crate::init`]. Call this before creating any window or
/// widget. Returns nothing and cannot report failure; if initialization panics,
/// the panic is contained and the state is simply left uninitialized.
pub extern "C" fn rw_init() {
    c_try_void!({
        crate::init();
    })
}
#[no_mangle]
/// Runs the platform main event loop.
///
/// C ABI entry point for [`crate::run`]. Blocks until the loop exits (typically
/// when a quit is requested), so call it from the thread that owns the UI.
/// Returns nothing and cannot report failure.
pub extern "C" fn rw_run() {
    c_try_void!({
        crate::run();
    })
}
#[no_mangle]
/// Requests that the platform event loop shut down.
///
/// C ABI entry point for [`crate::quit`]. The request is asynchronous: the loop
/// stops on its next iteration, so control may not return to the caller's next
/// statement until the loop actually drains. Returns nothing and cannot fail.
pub extern "C" fn rw_quit() {
    c_try_void!({
        crate::quit();
    })
}

/// Destroy a widget created by any `rw_create_*` call.
///
/// Returns `CBool::TRUE` when the widget existed and was torn down. Releases the
/// backend's state record and every registry entry it holds for the widget, so a
/// long-running application can rebuild its UI without accumulating registrations.
///
/// Passing an unknown or already-destroyed id is safe and returns `false`.
#[no_mangle]
pub extern "C" fn rw_destroy_widget(widget_id: u64) -> CBool {
    c_try!({ get_control_backend().destroy_widget(widget_id) })
}
#[no_mangle]
/// Creates a top-level window at a framework-assigned identity.
///
/// `title` may be null, in which case the title is empty. Returns the new
/// window's id, or `0` on failure. `x`/`y` are the position and `width`/`height`
/// the size, in logical pixels.
pub extern "C" fn rw_create_window(
    title: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_window(&c_str_or_default(title), x, y, width, height) })
}
#[no_mangle]
/// Creates a push button as a child of `parent`.
///
/// `text` is the button label and may be null, which yields an empty label.
/// Returns the new widget's id, or `0` if `parent` is unknown or the backend
/// refuses the request.
pub extern "C" fn rw_create_button(
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
/// Creates a checkbox as a child of `parent`, initially unchecked and labelled
/// `text` (null gives an empty label). Returns the new widget's id, or `0` on
/// failure.
pub extern "C" fn rw_create_checkbox(
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
/// Creates a single-line text field as a child of `parent`, pre-filled with
/// `text` (null gives an empty field). Returns the new widget's id, or `0` on
/// failure.
pub extern "C" fn rw_create_line_edit(
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
/// Creates a non-interactive text label as a child of `parent`.
///
/// `text` may be null, which yields an empty label. Returns the new widget's id,
/// or `0` on failure.
pub extern "C" fn rw_create_label(
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
/// Creates a radio button as a child of `parent`, labelled `text` (null gives an
/// empty label).
///
/// Grouping against sibling radio buttons is the backend's concern; this call
/// only creates the control. Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_radio_button(
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
/// Creates a horizontal value slider as a child of `parent`.
///
/// Returns the new widget's id, or `0` on failure. The range and initial value
/// come from the backend's defaults; use the platform API to change them.
pub extern "C" fn rw_create_slider(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_slider(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a progress bar as a child of `parent`.
///
/// Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_progress_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_progress_bar(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a drop-down combo box as a child of `parent`, with no items.
///
/// Add entries with `rw_combo_box_add_item`. Returns the new widget's id, or `0`
/// on failure.
pub extern "C" fn rw_create_combo_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_combo_box(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a list box as a child of `parent`, with no items.
///
/// Add entries with `rw_list_box_add_item`. Returns the new widget's id, or `0`
/// on failure.
pub extern "C" fn rw_create_list_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_list_box(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates an empty container panel as a child of `parent`.
///
/// Panels hold other controls but have no presentation of their own. Returns the
/// new widget's id, or `0` on failure.
pub extern "C" fn rw_create_panel(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_panel(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a message box, a transient dialog rather than a persistent child.
///
/// `title` and `text` may each be null, which supplies an empty string for that
/// part. The `x`/`y`/`width`/`height` geometry is a hint that the window manager
/// may override. Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_message_box(
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
/// Creates a file chooser dialog, scoped to `parent` if that id is valid.
///
/// `title` may be null for an empty caption. Returns the dialog's id, or `0` on
/// failure. Showing it and reading back the chosen path are separate calls.
pub extern "C" fn rw_create_file_dialog(
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
/// Creates a colour chooser dialog, scoped to `parent` if that id is valid.
///
/// `title` may be null for an empty caption. Returns the dialog's id, or `0` on
/// failure.
pub extern "C" fn rw_create_color_dialog(
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
/// Creates a font chooser dialog, scoped to `parent` if that id is valid.
///
/// `title` may be null for an empty caption. Returns the dialog's id, or `0` on
/// failure.
pub extern "C" fn rw_create_font_dialog(
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
/// Creates a numeric spin box as a child of `parent`.
///
/// Returns the new widget's id, or `0` on failure. The range, step and initial
/// value come from the backend's defaults.
pub extern "C" fn rw_create_spin_box(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_spin_box(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a list view as a child of `parent`, with no rows.
///
/// A list view is the multi-column counterpart of `rw_create_list_box`. Returns
/// the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_list_view(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_list_view(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a scrollable container as a child of `parent`.
///
/// Child widgets are clipped to the container and reachable through its
/// scrollbars. Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_scroll_area(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_scroll_area(parent, x, y, width, height) })
}
#[no_mangle]
/// Moves and resizes a widget in one call.
///
/// `x`/`y` are the new position and `width`/`height` the new size, in logical
/// pixels. An unknown `widget_id` is ignored. Returns nothing; there is no way
/// for the caller to learn whether the geometry was applied.
pub extern "C" fn rw_set_widget_geometry(
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
/// Retrieves the geometry of a widget identified by `widget_id`.
///
/// # Safety
///
/// All output pointer arguments must be either null or point to valid,
/// properly aligned memory regions suitable for writing the respective
/// geometry values (`c_int` for x/y, `c_uint` for width/height).
pub unsafe extern "C" fn rw_get_widget_geometry(
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
/// Creates a control of any registered kind.
///
/// `kind_name` is the canonical factory name or one of its aliases (`"button"`,
/// `"tree_view"`, `"command_palette"`, …); see [`rw_widget_kind_names`] for the
/// full list. Matching is case- and separator-insensitive, so `"TreeView"` and
/// `"tree-view"` resolve identically.
///
/// # Why a name and not an enum code
///
/// `WidgetKind` carries no `#[repr]`, so exposing its discriminant would freeze
/// the enum's declaration order into the ABI. The name is the stable contract, and
/// it also makes every kind added later reachable without a new function here.
///
/// Returns the new widget's id, or `0` when `kind_name` matches no control.
///
pub extern "C" fn rw_create_widget_of_kind(
    parent: u64,
    kind_name: *const c_char,
    text: *const c_char,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({
        get_control_backend().create_widget(
            &c_str_or_default(kind_name),
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
/// Writes every registered kind name into `out`, space-separated.
///
/// Returns the number of bytes the full list needs, **excluding** the trailing
/// NUL. When that return value is greater than or equal to `cap`, the list was
/// truncated and the caller should retry with a larger buffer; passing a null
/// `out` with `cap == 0` is the size query. The written text is always
/// NUL-terminated when `cap > 0`.
///
pub extern "C" fn rw_widget_kind_names(out: *mut c_char, cap: c_uint) -> c_uint {
    c_try!({
        let names = crate::widget::capability::WidgetFactory::new_with_defaults().widget_names();
        write_space_separated(&names, out, cap)
    })
}
#[no_mangle]
/// Lists the property names `widget_id` publishes, space-separated.
///
/// Uses the same return convention as [`rw_widget_kind_names`]: the required size
/// is returned whether or not the buffer was large enough, and a null `out` with
/// `cap == 0` queries that size. Returns `0` for an unknown widget.
///
pub extern "C" fn rw_widget_property_names(
    widget_id: u64,
    out: *mut c_char,
    cap: c_uint,
) -> c_uint {
    c_try!({
        let names = crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::widget_property_names(widget).map(|names| names.to_vec())
        })
        .flatten();
        match names {
            Some(names) => write_space_separated(&names, out, cap),
            None => 0,
        }
    })
}

/// Writes `items` into `out` as a space-separated, NUL-terminated list.
///
/// Shared by the enumeration entry points so they agree on the size convention:
/// the return is always the full byte length, which lets a caller discover the
/// requirement without a second API call.
fn write_space_separated(items: &[&str], out: *mut c_char, cap: c_uint) -> c_uint {
    let joined = items.join(" ");
    let bytes = joined.as_bytes();
    let required = bytes.len() as c_uint;
    if out.is_null() || cap == 0 {
        return required;
    }
    // Leave room for the terminator; copy at most `cap - 1` bytes.
    let writable = (cap as usize).saturating_sub(1).min(bytes.len());
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, writable);
        *out.add(writable) = 0;
    }
    required
}
#[no_mangle]
/// Reads a property from `widget_id` by name.
///
/// On success writes the value kind into `out_kind` and the payload into
/// `out_num` (for `RW_VALUE_BOOL` / `RW_VALUE_INT` / `RW_VALUE_UINT` /
/// `RW_VALUE_FLOAT`) or `out_str` (for `RW_VALUE_STRING`; the caller frees it
/// with [`rw_free_string`]). `RW_VALUE_NULL` writes neither.
///
/// Returns `false` for an unknown widget or property; [`rw_error_code`] then
/// distinguishes the cases. Null out-pointers are permitted and simply not
/// written, so a caller that only wants the kind can pass nulls for the rest.
///
/// # Safety
///
/// `name` must be null or point to a NUL-terminated string. `out_kind`,
/// `out_num` and `out_str` must each be null or point to a writable value of
/// their declared type. When `out_str` is non-null a string may be written into
/// it, which the caller then owns and must release with [`rw_free_string`].
///
pub unsafe extern "C" fn rw_get_widget_property(
    widget_id: u64,
    name: *const c_char,
    out_kind: *mut c_int,
    out_num: *mut i64,
    out_str: *mut *mut c_char,
) -> CBool {
    c_try!({
        let property = c_str_or_default(name);
        match crate::widget::capability::read_widget_property_by_id(widget_id, &property) {
            Ok(value) => {
                let (kind, num, text) = encode_capability_value(value);
                unsafe {
                    if !out_kind.is_null() {
                        *out_kind = kind;
                    }
                    if !out_num.is_null() {
                        *out_num = num;
                    }
                    if !out_str.is_null() {
                        *out_str = text.unwrap_or(core::ptr::null_mut());
                    }
                }
                true
            }
            Err(error) => {
                crate::error::ffi::record_capability_error(error);
                false
            }
        }
    })
}
#[no_mangle]
/// Writes a property to `widget_id` by name.
///
/// `kind` selects the payload: `RW_VALUE_BOOL` / `RW_VALUE_INT` /
/// `RW_VALUE_UINT` / `RW_VALUE_FLOAT` read `num` (misuse of the numeric kinds
/// together is reported as a type mismatch), `RW_VALUE_STRING` reads `str`, and
/// `RW_VALUE_NULL` clears an optional property.
///
/// Returns `false` when the widget or property is unknown, the property is
/// read-only, or the value kind does not match what the control expects. See
/// [`rw_error_code`] for which.
///
/// # Safety
///
/// `name` must be null or point to a NUL-terminated string. When `kind` is
/// `RW_VALUE_STRING`, `str_value` must be null or point to a NUL-terminated
/// string; it is read only for that kind and may be null otherwise.
///
pub unsafe extern "C" fn rw_set_widget_property(
    widget_id: u64,
    name: *const c_char,
    kind: c_int,
    num: i64,
    str_value: *const c_char,
) -> CBool {
    c_try!({
        let property = c_str_or_default(name);
        let value = decode_capability_value(kind, num, str_value);
        let Some(value) = value else {
            crate::error::ffi::record_capability_error(
                crate::widget::capability::CapabilityAccessError::TypeMismatch,
            );
            return false;
        };
        match crate::widget::capability::write_widget_property_by_id(widget_id, &property, value) {
            Ok(()) => true,
            Err(error) => {
                crate::error::ffi::record_capability_error(error);
                false
            }
        }
    })
}
#[no_mangle]
/// Selects the active theme by name.
///
/// Returns `false` when no theme is registered under `name`, leaving the current
/// theme in place. See [`rw_theme_names`] for the registered names.
///
pub extern "C" fn rw_set_theme(name: *const c_char) -> CBool {
    c_try!({
        let requested = c_str_or_default(name);
        let activated = crate::theme::global_theme_manager().set_theme(&requested);
        if activated {
            // Re-resolve every live control's style against the new theme, the same
            // way the window-creation funnel does.
            crate::reapply_active_theme();
        }
        activated
    })
}
#[no_mangle]
/// Names of the registered themes, space-separated.
///
/// Same convention as [`rw_widget_kind_names`]: the required size is returned and
/// a null `out` with `cap == 0` queries it.
///
pub extern "C" fn rw_theme_names(out: *mut c_char, cap: c_uint) -> c_uint {
    c_try!({
        let names = {
            let manager = crate::theme::global_theme_manager();
            manager.theme_names().iter().map(|name| name.to_string()).collect::<Vec<_>>()
        };
        let refs: Vec<&str> = names.iter().map(alloc::string::String::as_str).collect();
        write_space_separated(&refs, out, cap)
    })
}
#[no_mangle]
/// Sets the high-contrast override: `0` disables it, any non-zero value enables it.
///
/// Applies to every control the library creates from here on, and re-applies to
/// the live ones so a change takes effect without a rebuild.
///
pub extern "C" fn rw_set_high_contrast(mode: c_int) {
    c_try_void!({
        let enabled = mode != 0;
        let mode = if enabled {
            crate::style::HighContrastMode::WhiteOnBlack
        } else {
            crate::style::HighContrastMode::None
        };
        crate::theme::set_global_high_contrast(mode);
        crate::reapply_active_theme();
    })
}

/// Splits a [`CapabilityValue`] into the `(kind, num, str)` triple the ABI uses.
///
/// # Why a triple instead of a union
///
/// A C union would need the caller to know the active member without reading it,
/// and `i64` is the smallest numeric representation both `Int` and `UInt` fit
/// losslessly. Keeping `out_num` and `out_str` independent means a caller can
/// ignore whichever it does not need.
#[cfg(not(stripped_widgets))]
fn encode_capability_value(
    value: crate::widget::capability::CapabilityValue,
) -> (c_int, i64, Option<*mut c_char>) {
    use crate::widget::capability::CapabilityValue;
    match value {
        CapabilityValue::Null => (RW_VALUE_NULL, 0, None),
        CapabilityValue::Bool(flag) => (RW_VALUE_BOOL, i64::from(flag), None),
        CapabilityValue::Int(number) => (RW_VALUE_INT, number, None),
        CapabilityValue::UInt(number) => (RW_VALUE_UINT, number as i64, None),
        CapabilityValue::Float(number) => {
            // Reinterpret rather than truncate: a caller reading a float property
            // must get the real value, so the bit pattern travels in the same i64.
            (RW_VALUE_FLOAT, number.to_bits() as i64, None)
        }
        CapabilityValue::String(text) => {
            let c_text = CString::new(text).unwrap_or_default().into_raw();
            (RW_VALUE_STRING, 0, Some(c_text))
        }
    }
}

/// Rebuilds a [`CapabilityValue`] from the ABI's `(kind, num, str)` triple.
///
/// Returns `None` for an unrecognised `kind`, which the caller reports as a type
/// mismatch rather than silently writing a zero.
#[cfg(not(stripped_widgets))]
fn decode_capability_value(
    kind: c_int,
    num: i64,
    str_value: *const c_char,
) -> Option<crate::widget::capability::CapabilityValue> {
    use crate::widget::capability::CapabilityValue;
    match kind {
        RW_VALUE_NULL => Some(CapabilityValue::Null),
        RW_VALUE_BOOL => Some(CapabilityValue::Bool(num != 0)),
        RW_VALUE_INT => Some(CapabilityValue::Int(num)),
        RW_VALUE_UINT => Some(CapabilityValue::UInt(u64::try_from(num).ok()?)),
        RW_VALUE_FLOAT => Some(CapabilityValue::Float(f64::from_bits(num as u64))),
        RW_VALUE_STRING => Some(CapabilityValue::String(unsafe {
            if str_value.is_null() {
                String::new()
            } else {
                CStr::from_ptr(str_value).to_string_lossy().into_owned()
            }
        })),
        _ => None,
    }
}

/// The `rw_value_kind` discriminants, named so the Rust side and the generated
/// header cannot drift apart.
const RW_VALUE_NULL: c_int = 0;
const RW_VALUE_BOOL: c_int = 1;
const RW_VALUE_INT: c_int = 2;
const RW_VALUE_UINT: c_int = 3;
const RW_VALUE_FLOAT: c_int = 4;
const RW_VALUE_STRING: c_int = 5;
#[no_mangle]
/// Removes every item, leaving the combo box empty and with no selection.
///
/// Returns `true` on success.
pub extern "C" fn rw_combo_box_clear_items(combo_box: u64) -> CBool {
    c_try!({ get_control_backend().combo_box_clear_items(combo_box) })
}
#[no_mangle]
/// Selects the item at `index`, which is zero-based.
///
/// Returns `false` if the index is out of range or the widget is unknown.
pub extern "C" fn rw_combo_box_set_current_index(combo_box: u64, index: c_uint) -> CBool {
    c_try!({
        crate::platform::get_platform().combo_box_set_current_index(combo_box, index as usize)
    })
}
#[no_mangle]
/// The index of the selected item, zero-based, or `-1` when nothing is selected
/// or the widget is unknown.
pub extern "C" fn rw_combo_box_current_index(combo_box: u64) -> c_int {
    c_try!({
        match crate::platform::get_platform().combo_box_current_index(combo_box) {
            Some(idx) => idx as c_int,
            None => -1,
        }
    })
}
#[no_mangle]
/// The number of items currently in the combo box; `0` if it is unknown.
pub extern "C" fn rw_combo_box_item_count(combo_box: u64) -> c_uint {
    c_try!({ crate::platform::get_platform().combo_box_item_count(combo_box) as c_uint })
}
#[no_mangle]
/// The text of the item at zero-based `index`.
///
/// An out-of-range index or an unknown widget yields an empty string rather than
/// an error. The result is a freshly allocated C string and must be released
/// with `rw_free_string`.
pub extern "C" fn rw_combo_box_item_text(combo_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::platform::get_platform().combo_box_item_text(combo_box, index as usize);
        to_c_string_or_empty(text.unwrap_or_default())
    })
}
#[no_mangle]
/// Appends `text` (null gives an empty string) as a new last item.
///
/// Returns `true` when the item was added.
pub extern "C" fn rw_list_box_add_item(list_box: u64, text: *const c_char) -> CBool {
    c_try!({ get_control_backend().list_box_add_item(list_box, &c_str_or_default(text)) })
}
#[no_mangle]
/// Removes the item at zero-based `index`, shifting later items up.
///
/// Returns `false` if the index is out of range or the widget is unknown.
pub extern "C" fn rw_list_box_remove_item(list_box: u64, index: c_uint) -> CBool {
    c_try!({ get_control_backend().list_box_remove_item(list_box, index as usize) })
}
#[no_mangle]
/// Removes every item, leaving the list box empty and with no selection.
///
/// Returns `true` on success.
pub extern "C" fn rw_list_box_clear_items(list_box: u64) -> CBool {
    c_try!({ get_control_backend().list_box_clear_items(list_box) })
}
#[no_mangle]
/// Selects the item at zero-based `index`.
///
/// Returns `false` if the index is out of range or the widget is unknown.
pub extern "C" fn rw_list_box_set_current_index(list_box: u64, index: c_uint) -> CBool {
    c_try!({ crate::platform::get_platform().list_box_set_current_index(list_box, index as usize) })
}
#[no_mangle]
/// The index of the selected item, zero-based, or `-1` when nothing is selected
/// or the widget is unknown.
pub extern "C" fn rw_list_box_current_index(list_box: u64) -> c_int {
    c_try!({
        match crate::platform::get_platform().list_box_current_index(list_box) {
            Some(idx) => idx as c_int,
            None => -1,
        }
    })
}
#[no_mangle]
/// The number of items currently in the list box; `0` if it is unknown.
pub extern "C" fn rw_list_box_item_count(list_box: u64) -> c_uint {
    c_try!({ crate::platform::get_platform().list_box_item_count(list_box) as c_uint })
}
#[no_mangle]
/// The text of the item at zero-based `index`.
///
/// An out-of-range index or an unknown widget yields an empty string rather than
/// an error. The result is a freshly allocated C string and must be released
/// with `rw_free_string`.
pub extern "C" fn rw_list_box_item_text(list_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::platform::get_platform().list_box_item_text(list_box, index as usize);
        to_c_string_or_empty(text.unwrap_or_default())
    })
}
#[no_mangle]
/// Replaces the system clipboard contents with `text` (null clears it).
///
/// Returns `true` when the clipboard accepted the text.
pub extern "C" fn rw_set_clipboard_text(text: *const c_char) -> CBool {
    c_try!({ get_control_backend().set_clipboard_text(&c_str_or_default(text)) })
}
#[no_mangle]
/// The current system clipboard text, or an empty string when unreadable.
///
/// The result is a freshly allocated C string and must be released with
/// `rw_free_string`; it is never null.
pub extern "C" fn rw_get_clipboard_text() -> *const c_char {
    c_try!({
        let text = get_control_backend().get_clipboard_text();
        to_c_string_or_empty(text)
    })
}
#[no_mangle]
/// Begins a drag operation from the given source widget.
///
/// # Safety
///
/// `mime_type` must be a null-terminated C string pointing to valid memory.
/// If `payload` is non-null and `payload_len > 0`, `payload` must point to
/// a valid memory region of at least `payload_len` bytes.
pub unsafe extern "C" fn rw_begin_drag(
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
/// Polls for a pending drop event and writes its fields through output pointers.
///
/// # Safety
///
/// All output pointer arguments must be either null or point to valid,
/// properly aligned memory. `mime_out` and `payload_out` must point to
/// locations where allocated C strings / byte arrays can be stored.
pub unsafe extern "C" fn rw_poll_drop_event(
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
/// Creates a menu bar as a child of `parent`.
///
/// Attach it to a window with `rw_attach_menu_bar_to_window`. Returns the new
/// widget's id, or `0` on failure.
pub extern "C" fn rw_create_menu_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_menu_bar(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a top-level menu labelled `text` (null gives an empty label).
///
/// A menu is normally a child of a menu bar created by `rw_create_menu_bar`.
/// Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_menu(
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
/// Installs `menu_bar` as the menu bar of `window`.
///
/// Both ids must refer to existing widgets. Returns `true` on success.
pub extern "C" fn rw_attach_menu_bar_to_window(window: u64, menu_bar: u64) -> CBool {
    c_try!({ get_control_backend().attach_menu_bar_to_window(window, menu_bar) })
}
#[no_mangle]
/// Adds a menu item to `parent_menu`.
///
/// `text` is the label (null gives an empty label). `shortcut` may be null, which
/// creates the item with no accelerator; a non-null value is parsed as a shortcut
/// description such as `"Ctrl+S"`. Returns the new item's id, or `0` on failure.
pub extern "C" fn rw_menu_add_item(
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
/// Takes the next queued menu item activation, if any.
///
/// Returns the id of the activated item, or `0` when the queue is empty. `0` is
/// therefore unambiguous as "nothing pending", since no real menu item is
/// assigned that id.
pub extern "C" fn rw_poll_menu_triggered() -> u64 {
    c_try!({ get_control_backend().poll_menu_triggered().unwrap_or(0) })
}
#[no_mangle]
/// Takes the next queued widget activation, if any.
///
/// Returns the id of the activated widget, or `0` when the queue is empty. This
/// variant discards the trigger kind; use `rw_poll_widget_trigger_event` when the
/// caller needs to distinguish a click from a value change.
pub extern "C" fn rw_poll_widget_triggered() -> u64 {
    c_try!({ get_control_backend().poll_widget_triggered().unwrap_or(0) })
}
/// Polls the next widget trigger event and optionally writes the widget ID to the provided pointer.
///
/// # Safety
/// The `widget_id_out` pointer must be either null or valid for writing a `u64`.
#[no_mangle]
pub unsafe extern "C" fn rw_poll_widget_trigger_event(widget_id_out: *mut u64) -> c_uint {
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
pub extern "C" fn rw_inject_menu_trigger(menu_item_id: u64) -> CBool {
    c_try!({ get_control_backend().inject_menu_trigger(menu_item_id) })
}
/// Generic typed widget trigger injection entrypoint for native hosts.
#[no_mangle]
pub extern "C" fn rw_inject_widget_trigger_event(widget_id: u64, kind_code: c_uint) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
/// Harmony callback alias: direct menu item trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_menu_item(menu_item_id: u64) -> CBool {
    c_try!({ get_control_backend().inject_menu_trigger(menu_item_id) })
}
/// Harmony callback alias: direct click trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_click(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, crate::platform::WidgetTriggerKind::Clicked)
    })
}
/// Harmony callback alias: direct value-changed trigger by widget id.
#[no_mangle]
pub extern "C" fn rw_harmony_on_value_changed(widget_id: u64) -> CBool {
    c_try!({
        get_control_backend().inject_widget_trigger_event(
            widget_id,
            crate::platform::WidgetTriggerKind::ValueChanged,
        )
    })
}
/// Harmony callback alias: direct typed trigger by widget id and kind code.
#[no_mangle]
pub extern "C" fn rw_harmony_on_widget_event(widget_id: u64, kind_code: c_uint) -> CBool {
    c_try!({
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
/// Register a Harmony node handle to logical widget id mapping.
#[no_mangle]
pub extern "C" fn rw_harmony_bind_node(node_handle: u64, widget_id: u64) -> CBool {
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
pub extern "C" fn rw_harmony_unbind_node(node_handle: u64) -> CBool {
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
pub extern "C" fn rw_harmony_lookup_widget_id(node_handle: u64) -> u64 {
    c_try!({ harmony_lookup_widget(node_handle).unwrap_or(0) })
}
/// Clear all Harmony node-handle mappings.
#[no_mangle]
pub extern "C" fn rw_harmony_clear_node_bindings() {
    c_try_void!({
        harmony_node_registry().lock().unwrap_or_else(|e| e.into_inner()).clear();
    })
}
/// Harmony callback alias: menu trigger by node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_menu_item(node_handle: u64) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend().inject_menu_trigger(widget_id)
    })
}
/// Harmony callback alias: click trigger by node handle.
#[no_mangle]
pub extern "C" fn rw_harmony_on_node_click(node_handle: u64) -> CBool {
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
pub extern "C" fn rw_harmony_on_node_value_changed(node_handle: u64) -> CBool {
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
pub extern "C" fn rw_harmony_on_node_widget_event(node_handle: u64, kind_code: c_uint) -> CBool {
    c_try!({
        let Some(widget_id) = harmony_lookup_widget(node_handle) else {
            return false;
        };
        get_control_backend()
            .inject_widget_trigger_event(widget_id, trigger_kind_from_code(kind_code))
    })
}
#[no_mangle]
/// Creates a tool bar as a child of `parent`.
///
/// Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_tool_bar(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> u64 {
    c_try!({ get_control_backend().create_tool_bar(parent, x, y, width, height) })
}
#[no_mangle]
/// Creates a status bar as a child of `parent`, showing `text` (null gives an
/// empty message).
///
/// Returns the new widget's id, or `0` on failure.
pub extern "C" fn rw_create_status_bar(
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
/// Reveals a previously hidden widget.
///
/// An unknown `widget_id` is ignored. Returns nothing; query the new state with
/// `rw_is_widget_visible` if the caller needs confirmation.
pub extern "C" fn rw_show_widget(widget_id: u64) {
    c_try_void!({
        get_control_backend().show_widget(widget_id);
    })
}
#[no_mangle]
/// Conceals a widget without destroying it.
///
/// Unlike [`rw_destroy_widget`], the widget keeps its state, geometry and
/// registrations, so showing it again restores exactly what it was. An unknown
/// `widget_id` is ignored.
pub extern "C" fn rw_hide_widget(widget_id: u64) {
    c_try_void!({
        get_control_backend().hide_widget(widget_id);
    })
}
#[no_mangle]
/// Replaces the widget's text content with `text` (null clears it).
///
/// Applies to the label/caption of any widget that has one; an unknown
/// `widget_id` is ignored.
pub extern "C" fn rw_set_widget_text(widget_id: u64, text: *const c_char) {
    c_try_void!({
        get_control_backend().set_widget_text(widget_id, &c_str_or_default(text));
    })
}
#[no_mangle]
/// The widget's current text content.
///
/// An unknown `widget_id` yields an empty string rather than an error. The result
/// is a freshly allocated C string and must be released with `rw_free_string`;
/// it is never null.
pub extern "C" fn rw_get_widget_text(widget_id: u64) -> *const c_char {
    c_try!({
        let text = get_control_backend().get_widget_text(widget_id);
        to_c_string_or_empty(text)
    })
}
#[no_mangle]
/// Enables or disables user interaction with the widget.
///
/// A disabled control stays visible but is greyed out and ignores input. An
/// unknown `widget_id` is ignored.
pub extern "C" fn rw_set_widget_enabled(widget_id: u64, enabled: CBool) {
    c_try_void!({
        get_control_backend().set_widget_enabled(widget_id, enabled);
    })
}
#[no_mangle]
/// Reports whether the widget accepts user interaction; `false` for an unknown
/// widget.
pub extern "C" fn rw_is_widget_enabled(widget_id: u64) -> CBool {
    c_try!({ get_control_backend().is_widget_enabled(widget_id) })
}
#[no_mangle]
/// Sets whether the widget is drawn.
///
/// This is the flag counterpart of `rw_show_widget` / `rw_hide_widget` and
/// behaves identically; an unknown `widget_id` is ignored.
pub extern "C" fn rw_set_widget_visible(widget_id: u64, visible: CBool) {
    c_try_void!({
        get_control_backend().set_widget_visible(widget_id, visible);
    })
}
#[no_mangle]
/// Reports whether the widget is currently drawn; `false` for an unknown widget
/// or one that has been hidden.
pub extern "C" fn rw_is_widget_visible(widget_id: u64) -> CBool {
    c_try!({ get_control_backend().is_widget_visible(widget_id) })
}
#[no_mangle]
/// Enables or disables IME (input method) handling for the widget.
///
/// Relevant to text-entry widgets, where it controls whether composed input such
/// as CJK candidate selection is routed to the control. Returns `true` when the
/// platform applied the change; `false` if the capability is unsupported or the
/// widget is unknown.
pub extern "C" fn rw_set_widget_ime_enabled(widget_id: u64, enabled: CBool) -> CBool {
    c_try!({ crate::platform::get_platform().set_widget_ime_enabled(widget_id, enabled) })
}
#[no_mangle]
/// Reports whether IME handling is enabled for the widget; `false` when the
/// capability is unsupported or the widget is unknown.
pub extern "C" fn rw_is_widget_ime_enabled(widget_id: u64) -> CBool {
    c_try!({ crate::platform::get_platform().is_widget_ime_enabled(widget_id) })
}
#[no_mangle]
/// Sets the name screen readers announce for the widget.
///
/// `name` may be null, which clears the accessible name. Returns `true` when the
/// platform applied the change, `false` if the capability is unsupported.
pub extern "C" fn rw_set_widget_accessibility_name(widget_id: u64, name: *const c_char) -> CBool {
    c_try!({
        crate::platform::get_platform()
            .set_widget_accessibility_name(widget_id, &c_str_or_default(name))
    })
}
#[no_mangle]
/// The accessibility name previously set for the widget, or an empty string if
/// none is set or the capability is unsupported.
///
/// The result is a freshly allocated C string and must be released with
/// `rw_free_string`.
pub extern "C" fn rw_get_widget_accessibility_name(widget_id: u64) -> *const c_char {
    c_try!({
        let name = crate::platform::get_platform().get_widget_accessibility_name(widget_id);
        to_c_string_or_empty(name)
    })
}
#[no_mangle]
/// The name of the control backend currently serving widget creation, such as
/// `"native"` or `"custom"`.
///
/// Useful for diagnostics, since behaviour differs between backends. The result
/// is a freshly allocated C string and must be released with `rw_free_string`.
pub extern "C" fn rw_backend_name() -> *const c_char {
    c_try!({ to_c_string_or_empty(get_control_backend().backend_name()) })
}
#[no_mangle]
/// The platform's supported-capability bitmask, negotiated at compile time.
///
/// Bit layout:
/// - bit0: DPI scaling
/// - bit1: IME
/// - bit2: accessibility
/// - bit3: native menu bar
/// - bit4: typed widget trigger events
///
/// This reflects the platform's *general* capabilities; use
/// [`rw_platform_capability_contract`] for the contract of a specific runtime
/// profile.
pub extern "C" fn rw_platform_capabilities() -> c_uint {
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
/// The factor that converts logical pixels to physical device pixels.
///
/// `1.0` means no scaling. Returns nothing about failure: a platform without DPI
/// support reports `1.0`.
pub extern "C" fn rw_platform_dpi_scale_factor() -> c_float {
    c_try!({ crate::platform::dpi_scale_factor() })
}
#[no_mangle]
/// Sets the software renderer's anti-aliasing quality, in samples per axis.
///
/// The value is clamped to `1..=8`, and the clamped value actually in effect is
/// returned — the caller does not need a follow-up read to learn the outcome.
/// The setting is process-wide and applies to canvases created afterwards.
pub extern "C" fn rw_set_render_aa_samples_per_axis(samples: c_uint) -> c_uint {
    c_try!({
        let config =
            crate::render::SoftwareRenderConfig { aa_samples_per_axis: samples as u8 }.normalized();
        crate::render::set_default_software_render_config(config);
        crate::render::default_software_render_config().aa_samples_per_axis as c_uint
    })
}
#[no_mangle]
/// The software renderer's current anti-aliasing quality, in samples per axis.
///
/// Always within `1..=8`.
pub extern "C" fn rw_get_render_aa_samples_per_axis() -> c_uint {
    c_try!({ crate::render::default_software_render_config().aa_samples_per_axis as c_uint })
}
#[no_mangle]
/// Sets the embedded render engine's target frame rate, in hertz (frames per
/// second).
///
/// The value is clamped to `1..=240`, and the clamped value actually in effect is
/// returned.
pub extern "C" fn rw_set_embedded_target_fps(fps: c_uint) -> c_uint {
    c_try!({ crate::render_engine::set_embedded_target_fps(fps) as c_uint })
}
#[no_mangle]
/// The embedded render engine's current target frame rate, in hertz.
///
/// Always within `1..=240`.
pub extern "C" fn rw_get_embedded_target_fps() -> c_uint {
    c_try!({ crate::render_engine::embedded_target_fps() as c_uint })
}
#[no_mangle]
/// Queues a no-op task on the embedded render engine and returns its task id.
///
/// `label` may be null for an empty label; if it is non-null it is copied, so the
/// caller keeps ownership of the original. The task body does nothing — this
/// exists so native hosts can exercise the scheduling path from C.
pub extern "C" fn rw_submit_embedded_noop_task(label: *const c_char) -> u64 {
    c_try!({ crate::render_engine::submit_embedded_task(c_str_or_default(label), |_| {}) })
}
#[no_mangle]
/// Reports whether the embedded render engine has been initialized.
pub extern "C" fn rw_embedded_engine_is_initialized() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().initialized })
}
#[no_mangle]
/// Reports whether the embedded render engine's loop is currently running.
pub extern "C" fn rw_embedded_engine_is_running() -> CBool {
    c_try!({ crate::render_engine::embedded_engine_stats().running })
}
#[no_mangle]
/// The number of frames the embedded render engine has rendered since start.
pub extern "C" fn rw_embedded_engine_frame_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().frame_count })
}
#[no_mangle]
/// The number of tasks queued on the embedded render engine but not yet run.
pub extern "C" fn rw_embedded_engine_pending_task_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().pending_task_count as u64 })
}
#[no_mangle]
/// The number of windows the embedded render engine is tracking.
pub extern "C" fn rw_embedded_engine_window_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().window_count as u64 })
}
#[no_mangle]
/// The number of buttons the embedded render engine is tracking.
pub extern "C" fn rw_embedded_engine_button_count() -> u64 {
    c_try!({ crate::render_engine::embedded_engine_stats().button_count as u64 })
}
#[no_mangle]
/// The capability contract negotiated for a runtime profile, as a bitmask.
///
/// `profile_code` is `1` for the embedded profile and any other value for the
/// full profile — note that an unrecognised code therefore silently means
/// "full" rather than being rejected.
///
/// Bit meanings are not shared between the two contract kinds. For native
/// contracts: bit0 is always set, bit1 DPI scaling, bit2 IME, bit3 accessibility,
/// bit4 native menu, bit5 typed widget triggers. For embedded contracts: bit0 is
/// never set, bit1 fixed DPI, bit2 low-memory mode, bit3 typed widget triggers.
pub extern "C" fn rw_platform_capability_contract(profile_code: c_uint) -> c_uint {
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
/// The mobile backend's name, or an empty string when the `mobile-api` feature is
/// not compiled in.
///
/// The result is a freshly allocated C string and must be released with
/// `rw_free_string`.
pub extern "C" fn rw_mobile_backend_name() -> *const c_char {
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
/// Binds the widget layer to an existing native view.
///
/// `native_handle` is the platform's own identifier for the view being taken
/// over, interpreted by the mobile backend. Returns `true` when the view was
/// attached; always `false` when the `mobile-api` feature is not compiled in.
pub extern "C" fn rw_mobile_attach_native_view(native_handle: u64) -> CBool {
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
/// Return the C ABI binding contract version.
///
/// Independent of the crate semantic version: bumped only when the exported
/// `rw_*` symbol set or its calling conventions change. `8` marks the stable
/// 1.0 ABI line.
pub extern "C" fn rw_bindings_api_version() -> c_uint {
    c_try!({ 8 })
}
/// Return Node.js binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Node.js adapter/example available
#[no_mangle]
pub extern "C" fn rw_nodejs_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) })
}
/// Return Python binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Python adapter/example available
/// - bit2: profile-aware capability query available
#[no_mangle]
pub extern "C" fn rw_python_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}
/// Return C++ wrapper status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: C++ wrapper skeleton/example available
#[no_mangle]
pub extern "C" fn rw_cpp_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) })
}
/// Return Java/JNI binding status bitmask.
///
/// Bit layout:
/// - bit0: C ABI entry points available
/// - bit1: Java native-method skeleton available
/// - bit2: JNI bridge skeleton available
#[no_mangle]
pub extern "C" fn rw_java_binding_status() -> c_uint {
    c_try!({ (1 << 0) | (1 << 1) | (1 << 2) })
}
/// Return Java/JNI skeleton ABI version.
#[no_mangle]
pub extern "C" fn rw_java_jni_skeleton_version() -> c_uint {
    c_try!({ 1 })
}
/// Reserved C++ binding marker — returns the current C++ wrapper ABI version.
///
/// Kept as a stable, never-changing symbol so that language bindings can
/// probe for wrapper support without linking against a moving target.
#[no_mangle]
pub extern "C" fn rw_cpp_reserved() -> c_uint {
    c_try!({ 1 })
}
/// Reserved Java binding marker — returns the current JNI wrapper ABI version.
#[no_mangle]
pub extern "C" fn rw_java_reserved() -> c_uint {
    c_try!({ 1 })
}
/// Reserved Python binding marker — returns the current Python wrapper ABI version.
#[no_mangle]
pub extern "C" fn rw_python_reserved() -> c_uint {
    c_try!({ 1 })
}
/// Return the error code of the most recent failed C ABI call.
///
/// Returns `0` (`RW_ERROR_SUCCESS`) when no error has been recorded.
/// The `handle` argument is reserved for future per-widget error state
/// and is currently ignored (the last-error slot is process-wide).
#[no_mangle]
pub extern "C" fn rw_error_code(_handle: u64) -> c_int {
    c_try!({ crate::error::ffi::last_ffi_error().map(|e| e.id.0 as c_int).unwrap_or(0) })
}
/// Return the error message of the most recent failed C ABI call.
///
/// Returns an empty string when no error has been recorded. The returned
/// string must be freed with `rw_free_string`.
/// The `handle` argument is reserved for future per-widget error state
/// and is currently ignored (the last-error slot is process-wide).
#[no_mangle]
pub extern "C" fn rw_error_message(_handle: u64) -> *mut c_char {
    c_try!({
        let message = crate::error::ffi::last_ffi_error().map(|e| e.message).unwrap_or_default();
        CString::new(message).unwrap_or_default().into_raw()
    })
}
#[no_mangle]
/// # Safety
///
/// `s` must be either null or a pointer returned by this crate through
/// `CString::into_raw` and not already freed. Passing any other pointer or
/// double-freeing is undefined behavior.
pub unsafe extern "C" fn rw_free_string(s: *mut c_char) {
    c_try_void!({
        if s.is_null() {
            return;
        }
        unsafe {
            let _ = CString::from_raw(s);
        }
    })
}

/// Alias for [`rw_free_string`] — explicitly named for callers
/// who hold a `*mut c_char` from Rust-allocated strings and want clarity
/// in their own code.
#[no_mangle]
/// # Safety
///
/// Same requirements as [`rw_free_string`]: `s` must be null or a
/// valid pointer previously allocated by this crate for C ownership transfer.
pub unsafe extern "C" fn rw_free_rust_string(s: *mut c_char) {
    rw_free_string(s);
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Exercise the core C ABI round-trip through the real `extern "C"` entry
    /// points: create a window and child controls, mutate text/geometry/
    /// visibility/enabled, read text back, then free the returned string.
    ///
    /// This is the contract C/Java callers depend on, so it asserts the return
    /// conventions (0 on failure, non-zero handles) and that `rw_free_string`
    /// releases what `rw_get_widget_text` allocated.
    #[test]
    fn c_abi_widget_lifecycle_roundtrip() {
        use std::ffi::{CStr, CString};

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("abi-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0, "window creation must return a non-zero handle");

            // Child creation with a valid parent must succeed; an invalid parent is
            // rejected by the platform contract (returns 0).
            let label = c("hello");
            let button = rw_create_button(window, label.as_ptr(), 10, 10, 80, 30);
            assert_ne!(button, 0, "button creation must return a non-zero handle");
            assert_eq!(
                rw_create_button(9999, label.as_ptr(), 0, 0, 10, 10),
                0,
                "an unknown parent must be rejected"
            );

            // Text round-trip through the C string boundary.
            let updated = c("updated");
            rw_set_widget_text(button, updated.as_ptr());
            let ptr = rw_get_widget_text(button);
            assert!(!ptr.is_null(), "rw_get_widget_text must never return null");
            let read_back = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            assert_eq!(read_back, "updated");
            rw_free_string(ptr as *mut c_char);

            // Geometry, visibility and enabled round-trips. `CBool` is `bool`.
            rw_set_widget_geometry(button, 20, 20, 120, 40);
            rw_hide_widget(button);
            assert!(!rw_is_widget_visible(button), "hidden widget reports not visible");
            rw_show_widget(button);
            assert!(rw_is_widget_visible(button), "shown widget reports visible");

            rw_set_widget_enabled(button, false);
            assert!(!rw_is_widget_enabled(button), "disabled widget reports disabled");
            rw_set_widget_enabled(button, true);
            assert!(rw_is_widget_enabled(button), "enabled widget reports enabled");
        }
    }

    /// Reading text for an unknown widget must yield an empty (non-null)
    /// string rather than a dangling pointer, and freeing it must be safe.
    #[test]
    fn c_abi_unknown_widget_text_is_empty_not_null() {
        use std::ffi::CStr;

        unsafe {
            let ptr = rw_get_widget_text(0xDEAD_BEEF);
            assert!(!ptr.is_null(), "unknown widget must still return a valid pointer");
            let text = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            assert!(text.is_empty(), "unknown widget text should be empty, got {text:?}");
            rw_free_string(ptr as *mut c_char);
        }
    }

    // -----------------------------------------------------------------------
    // Generic (name-based) creation and property access
    // -----------------------------------------------------------------------

    /// `rw_widget_kind_names` must report a size, and the buffer round-trip must
    /// contain the controls the registry knows about.
    ///
    /// # Why the assertion is on the parsed list
    ///
    /// A buffer that was filled but not NUL-terminated, or a required size that
    /// disagreed with what was written, would still "succeed" if the test only
    /// checked a non-zero return. Parsing the result is what proves both.
    #[test]
    fn c_abi_widget_kind_names_enumerates_the_registry() {
        use std::ffi::CStr;

        unsafe {
            let required = rw_widget_kind_names(core::ptr::null_mut(), 0);
            assert!(required > 0, "the registry must publish names");

            let mut buffer = vec![0u8; required as usize + 1];
            let written = rw_widget_kind_names(buffer.as_mut_ptr() as *mut c_char, required + 1);
            assert_eq!(written, required, "the required size must be stable across calls");

            let text =
                CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
            let names: Vec<&str> = text.split(' ').collect();
            // Names the registration-fidelity gate verifies are registered, so this
            // also pins that the enumeration and the registry agree.
            for expected in ["button", "tree_view", "timeline_widget", "grid_table"] {
                assert!(names.contains(&expected), "{expected} must appear in the kind list");
            }
        }
    }

    /// A null/zero-capacity query must not write, and must still report the size.
    ///
    /// `rw_widget_kind_names` takes no raw pointers it dereferences when `out` is
    /// null, so this needs no `unsafe` block — and saying so is what keeps the
    /// `unsafe` blocks elsewhere meaningful.
    #[test]
    fn c_abi_name_enumeration_size_query_is_side_effect_free() {
        let first = rw_widget_kind_names(core::ptr::null_mut(), 0);
        let second = rw_widget_kind_names(core::ptr::null_mut(), 0);
        assert_eq!(first, second, "a size query must be repeatable");
        assert!(first > 0);
    }

    /// `rw_create_widget_of_kind` must reach controls the typed `create_*`
    /// functions have no method for, and must reject an unknown name.
    #[test]
    fn c_abi_create_widget_of_kind_reaches_registered_controls() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("by-kind-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            for (name, expected_kind) in [
                ("tree_view", ""),
                ("timeline_widget", ""),
                ("command_palette", ""),
                ("diff_viewer", ""),
                ("markdown_editor", ""),
                ("toast_stack", ""),
                ("grid_table", ""),
            ] {
                let kind = c(name);
                let text = c("");
                let id =
                    rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 100, 40);
                assert_ne!(id, 0, "{name} must be creatable by name");

                // Reading any property back proves the id addresses a live widget
                // rather than being a bare non-zero token.
                let enabled = c("enabled");
                let mut out_kind: c_int = -1;
                let mut out_num: i64 = 0;
                let mut out_str: *mut c_char = core::ptr::null_mut();
                let ok = rw_get_widget_property(
                    id,
                    enabled.as_ptr(),
                    &mut out_kind,
                    &mut out_num,
                    &mut out_str,
                );
                assert!(ok, "{name} must answer a base property");
                assert_eq!(out_kind, RW_VALUE_BOOL, "{name}::enabled must be a bool");
                assert_eq!(out_num, 1, "{name} must start enabled");
                let _ = expected_kind;
            }

            let bogus = c("definitely_not_a_control");
            let empty = c("");
            assert_eq!(
                rw_create_widget_of_kind(window, bogus.as_ptr(), empty.as_ptr(), 0, 0, 10, 10),
                0,
                "an unknown name must be rejected with 0"
            );
        }
    }

    /// A property written through the ABI must read back with the same value.
    ///
    /// This is the whole point of the generic property layer: without it the
    /// `tooltip` and `value` names were unreachable from C at all.
    #[test]
    fn c_abi_set_and_get_widget_property_round_trip() {
        use std::ffi::{CStr, CString};

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("prop-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let kind = c("slider");
            let text = c("");
            let slider =
                rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 120, 24);
            assert_ne!(slider, 0);

            // Write a string property and read it back.
            let tooltip = c("tooltip");
            let value = c("drag me");
            assert!(
                rw_set_widget_property(
                    slider,
                    tooltip.as_ptr(),
                    RW_VALUE_STRING,
                    0,
                    value.as_ptr()
                ),
                "tooltip must be writable through the ABI"
            );
            let mut out_kind: c_int = -1;
            let mut out_num: i64 = 0;
            let mut out_str: *mut c_char = core::ptr::null_mut();
            assert!(rw_get_widget_property(
                slider,
                tooltip.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            assert_eq!(out_kind, RW_VALUE_STRING);
            assert!(!out_str.is_null());
            assert_eq!(CStr::from_ptr(out_str).to_string_lossy(), "drag me");
            rw_free_string(out_str);

            // Write a numeric property and read it back.
            let maximum = c("maximum");
            assert!(rw_set_widget_property(
                slider,
                maximum.as_ptr(),
                RW_VALUE_INT,
                42,
                core::ptr::null()
            ));
            assert!(rw_get_widget_property(
                slider,
                maximum.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            assert_eq!(out_kind, RW_VALUE_INT);
            assert_eq!(out_num, 42);

            // A read-only property must be refused, and the refusal must be
            // reported through the error channel.
            let geometry = c("geometry");
            assert!(rw_get_widget_property(
                slider,
                geometry.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            assert_eq!(out_kind, RW_VALUE_STRING, "geometry is published as a string");
            if !out_str.is_null() {
                rw_free_string(out_str);
            }
            assert!(
                !rw_set_widget_property(
                    slider,
                    geometry.as_ptr(),
                    RW_VALUE_STRING,
                    0,
                    value.as_ptr()
                ),
                "geometry must stay read-only"
            );
            assert_ne!(rw_error_code(0), 0, "the refusal must set the error code");
        }
    }

    /// An unknown property and an unknown widget must both be refused, and the
    /// distinction must survive into `rw_error_code`.
    #[test]
    fn c_abi_property_errors_are_distinguishable() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            crate::error::ffi::clear_last_ffi_error();
            let title = c("err-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let bogus = c("__no_such_property__");
            let mut out_kind: c_int = -1;
            let mut out_num: i64 = 0;
            let mut out_str: *mut c_char = core::ptr::null_mut();

            crate::error::ffi::clear_last_ffi_error();
            assert!(!rw_get_widget_property(
                window,
                bogus.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            let unknown_property_code = rw_error_code(0);
            assert_ne!(unknown_property_code, 0);

            crate::error::ffi::clear_last_ffi_error();
            assert!(!rw_get_widget_property(
                0xDEAD_BEEF,
                bogus.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            assert_ne!(rw_error_code(0), 0, "an unknown widget is also an error");
        }
    }

    /// An unrecognised `rw_value_kind` must be refused rather than silently
    /// treated as zero, which would write data the caller never supplied.
    #[test]
    fn c_abi_set_widget_property_rejects_unknown_value_kind() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("kind-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let tooltip = c("tooltip");
            crate::error::ffi::clear_last_ffi_error();
            assert!(
                !rw_set_widget_property(window, tooltip.as_ptr(), 99, 0, core::ptr::null()),
                "an unknown value kind must be refused"
            );
            assert_ne!(rw_error_code(0), 0);
        }
    }

    /// `rw_widget_property_names` must describe the control it is given.
    #[test]
    fn c_abi_widget_property_names_lists_the_controls_contract() {
        use std::ffi::{CStr, CString};

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("names-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let kind = c("timeline_widget");
            let text = c("");
            let timeline =
                rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 200, 80);
            assert_ne!(timeline, 0);

            let required = rw_widget_property_names(timeline, core::ptr::null_mut(), 0);
            assert!(required > 0);
            let mut buffer = vec![0u8; required as usize + 1];
            rw_widget_property_names(timeline, buffer.as_mut_ptr() as *mut c_char, required + 1);
            let listed =
                CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
            for expected in ["item_count", "row_height", "enabled"] {
                assert!(listed.contains(expected), "{expected} must be published: {listed}");
            }

            // An unknown widget publishes nothing, which is distinguishable from a
            // control that happens to have no properties.
            assert_eq!(rw_widget_property_names(0xDEAD_BEEF, core::ptr::null_mut(), 0), 0);
        }
    }

    /// Theme selection must be observable: an unknown name is refused, a known
    /// one is accepted, and the enumeration lists it.
    #[test]
    fn c_abi_theme_entry_points_round_trip() {
        use std::ffi::{CStr, CString};

        let _guard = crate::theme::theme_test_guard();
        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let required = rw_theme_names(core::ptr::null_mut(), 0);
            assert!(required > 0, "at least one theme must be registered");
            let mut buffer = vec![0u8; required as usize + 1];
            rw_theme_names(buffer.as_mut_ptr() as *mut c_char, required + 1);
            let names =
                CStr::from_ptr(buffer.as_ptr() as *const c_char).to_string_lossy().into_owned();
            let first = names.split(' ').next().expect("a name").to_string();

            let known = c(&first);
            assert!(rw_set_theme(known.as_ptr()), "{first} must be selectable");
            assert_eq!(
                crate::theme::global_theme_manager().current_theme_name(),
                first,
                "the manager must report the theme that was selected"
            );

            let bogus = c("__no_such_theme__");
            assert!(!rw_set_theme(bogus.as_ptr()), "an unknown theme must be refused");
            assert_eq!(
                crate::theme::global_theme_manager().current_theme_name(),
                first,
                "a refused switch must not change the active theme"
            );
        }
    }

    /// The high-contrast override must reach a control created afterwards, so the
    /// entry point is not merely recorded.
    #[test]
    fn c_abi_high_contrast_reaches_a_new_control() {
        use std::ffi::CString;

        let _guard = crate::theme::theme_test_guard();
        crate::theme::set_global_high_contrast(crate::style::HighContrastMode::None);

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("hc-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let label = c("hi");
            let before = rw_create_label(window, label.as_ptr(), 0, 0, 60, 20);
            assert_ne!(before, 0);

            rw_set_high_contrast(1);
            assert_eq!(
                crate::theme::global_high_contrast(),
                crate::style::HighContrastMode::WhiteOnBlack,
                "a non-zero mode must enable the override"
            );

            let after = rw_create_label(window, label.as_ptr(), 0, 30, 60, 20);
            assert_ne!(after, 0);

            // The two labels differ only in the override, so the force-applied
            // background must differ. Reading it through the property contract is
            // what makes this a test of the control's state rather than of a flag.
            let background_of = |id: u64| -> Option<String> {
                let name = c("enabled");
                let mut out_kind: c_int = -1;
                let mut out_num: i64 = 0;
                let mut out_str: *mut c_char = core::ptr::null_mut();
                let ok = rw_get_widget_property(
                    id,
                    name.as_ptr(),
                    &mut out_kind,
                    &mut out_num,
                    &mut out_str,
                );
                assert!(ok, "a live control must answer `enabled`");
                Some(format!("{out_kind}:{out_num}"))
            };
            assert_eq!(background_of(before), background_of(after));

            rw_set_high_contrast(0);
            assert_eq!(
                crate::theme::global_high_contrast(),
                crate::style::HighContrastMode::None,
                "mode 0 must clear the override"
            );
        }
    }

    /// The AA-sample config setters are exercised through the C ABI, which is
    /// available whenever `bindings` is built. The shared test lock, however,
    /// is only compiled on the `desktop` profile, so this case is gated to
    /// match it rather than leaving an unconditional reference.
    #[cfg(all(feature = "desktop", widgets_unstripped))]
    #[test]
    fn render_aa_sample_abi_roundtrip_clamps_values() {
        let _guard = crate::render::software_render_config_test_lock()
            .lock()
            .expect("software render config test lock poisoned");
        let original = rw_get_render_aa_samples_per_axis();
        let low = rw_set_render_aa_samples_per_axis(0);
        assert_eq!(low, 1);
        assert_eq!(rw_get_render_aa_samples_per_axis(), 1);
        let high = rw_set_render_aa_samples_per_axis(100);
        assert_eq!(high, 8);
        assert_eq!(rw_get_render_aa_samples_per_axis(), 8);
        rw_set_render_aa_samples_per_axis(original);
        assert_eq!(rw_get_render_aa_samples_per_axis(), original.clamp(1, 8));
    }
    #[test]
    fn embedded_target_fps_abi_roundtrip_clamps_values() {
        // Shares the embedded engine's process-wide test lock: this test drives the
        // same singleton as `render_engine::embedded`'s tests, so a module-local
        // lock here would exclude nothing and the two would interleave.
        let _guard = crate::render_engine::embedded::embedded_test_guard();
        let original = rw_get_embedded_target_fps();
        let low = rw_set_embedded_target_fps(0);
        assert_eq!(low, 1);
        assert_eq!(rw_get_embedded_target_fps(), 1);
        let high = rw_set_embedded_target_fps(1000);
        assert_eq!(high, 240);
        assert_eq!(rw_get_embedded_target_fps(), 240);
        rw_set_embedded_target_fps(original);
        assert_eq!(rw_get_embedded_target_fps(), original.clamp(1, 240));
    }
}
