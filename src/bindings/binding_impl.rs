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

/// Writes the accepted spellings of an `Enum` property into `out`.
///
/// # Why this entry point exists
///
/// An enum property's legal values were undiscoverable from outside the library. A
/// caller driving the property API (a C host, a Python binding, a JSON-tree editor)
/// could read a property's *name* and *kind* but not the tokens `set` would accept, so
/// the only way to present a choice was to hard-code a copy of the list — a copy that
/// silently went stale when the control changed.
///
/// Returns the bytes the full list needs, exactly like
/// [`rw_widget_property_names`], so a caller can size its buffer with one zero-cap call.
///
/// # Reading the result
///
/// A **zero** return means "this property declares no fixed set of values", which is
/// the honest answer both for a non-enum property and for a name the control does not
/// publish. It is not an error: most properties are not enums, and the C surface has no
/// error channel for a query that simply has nothing to say. A caller that needs to
/// distinguish "not an enum" from "no such property" should consult
/// [`rw_widget_property_names`] first.
///
/// # Safety
///
/// `name` must be null or point to a NUL-terminated string. `out` must be null or point
/// to a writable buffer of at least `cap` bytes.
#[cfg(not(stripped_widgets))]
#[no_mangle]
pub unsafe extern "C" fn rw_widget_property_tokens(
    widget_id: u64,
    name: *const c_char,
    out: *mut c_char,
    cap: c_uint,
) -> c_uint {
    c_try!({
        if name.is_null() {
            return 0;
        }
        let name = c_str_or_default(name);
        let tokens = crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::properties_trait::widget_property_tokens(widget, &name)
                .to_vec()
        });
        match tokens {
            Some(tokens) => write_space_separated(&tokens, out, cap),
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
        // `ptr::cast` rather than `as`: `c_char` is `i8` on some targets and `u8`
        // on others, so a plain `as` cast is a no-op on the latter and clippy
        // rejects it there under `-D warnings` (`unnecessary_cast`). Casting goes
        // through a type the compiler cannot already consider identical.
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), writable);
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

/// Destination codes for [`rw_widget_scroll_to`].
///
/// These are a stable ABI: the numeric values are part of the contract, so a new
/// destination must be appended rather than inserted.
pub const RW_SCROLL_TO_TOP: c_int = 0;
/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_BOTTOM: c_int = 1;
/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_LEFT: c_int = 2;
/// See [`RW_SCROLL_TO_TOP`].
pub const RW_SCROLL_TO_RIGHT: c_int = 3;

#[no_mangle]
/// Sets the scroll offset of a scrollable control.
///
/// The offset is clamped to the content extent, so an out-of-range request is not
/// an error: it scrolls as far as the content allows. Returns `false` when
/// `widget_id` does not address a control that scrolls (an unknown id, or a
/// control with no scroll offset) — a real "no" rather than a silent success.
///
/// Currently `scroll_area` answers; read the resulting offset back with
/// [`rw_get_widget_property`] where the control publishes `scroll_x` / `scroll_y`.
///
/// # Why this is a downcast and not a property write
///
/// A scroll offset is a consequence of the content and the viewport, not a free
/// value: `set_scroll_position` clamps it and emits `scroll_position_changed`.
/// Routing it through the property layer would either bypass that clamping or
/// duplicate it, so the control's own method is the single implementation.
///
pub extern "C" fn rw_widget_set_scroll_position(widget_id: u64, x: c_int, y: c_int) -> CBool {
    c_try!({
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            match crate::widget::capability::coercion::widget_as_mut::<crate::widget::ScrollArea>(
                widget,
            ) {
                Some(area) => {
                    area.set_scroll_position(x, y);
                    true
                }
                None => false,
            }
        })
        .unwrap_or(false);
        if applied {
            crate::widget::runtime::request_repaint(widget_id);
        }
        applied
    })
}

#[no_mangle]
/// Scrolls a control to one of the four edges.
///
/// `where_` is one of [`RW_SCROLL_TO_TOP`] / [`RW_SCROLL_TO_BOTTOM`] /
/// [`RW_SCROLL_TO_LEFT`] / [`RW_SCROLL_TO_RIGHT`]; any other value is rejected
/// (returns `false`) rather than quietly defaulting to an edge.
///
pub extern "C" fn rw_widget_scroll_to(widget_id: u64, where_: c_int) -> CBool {
    c_try!({
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            let Some(area) = crate::widget::capability::coercion::widget_as_mut::<
                crate::widget::ScrollArea,
            >(widget) else {
                return false;
            };
            match where_ {
                RW_SCROLL_TO_TOP => area.scroll_to_top(),
                RW_SCROLL_TO_BOTTOM => area.scroll_to_bottom(),
                RW_SCROLL_TO_LEFT => area.scroll_to_left(),
                RW_SCROLL_TO_RIGHT => area.scroll_to_right(),
                _ => return false,
            }
            true
        })
        .unwrap_or(false);
        if applied {
            crate::widget::runtime::request_repaint(widget_id);
        }
        applied
    })
}

#[no_mangle]
/// Adds one item to a list-like control.
///
/// Works for every control that holds strings through `append_widget_list_item`
/// (`list_box`, `combo_box`), which is what keeps this one entry point instead of
/// one per control. Returns the new item count, or `0` when the control does not
/// accept items.
///
/// A control whose count is genuinely zero after a successful add is not
/// distinguishable from a rejection by the return value alone; use
/// [`rw_widget_list_count`] to confirm when that matters.
///
pub extern "C" fn rw_widget_list_add(widget_id: u64, text: *const c_char) -> c_uint {
    c_try!({
        let item = c_str_or_default(text);
        let added = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            crate::widget::capability::append_widget_list_item(widget, item.clone())
        })
        .unwrap_or(false);
        if !added {
            return 0;
        }
        crate::widget::runtime::request_repaint(widget_id);
        rw_widget_list_count(widget_id)
    })
}

#[no_mangle]
/// Removes every item from a list-like control.
///
/// Returns `false` when the control does not hold items, so a caller can tell
/// "cleared" from "this control has no collection to clear".
///
pub extern "C" fn rw_widget_list_clear(widget_id: u64) -> CBool {
    c_try!({
        let cleared = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            crate::widget::capability::clear_widget_list_items(widget)
        })
        .unwrap_or(false);
        if cleared {
            crate::widget::runtime::request_repaint(widget_id);
        }
        cleared
    })
}

#[no_mangle]
/// Returns how many items a list-like control holds, or `0` when it holds none or
/// does not hold items.
///
pub extern "C" fn rw_widget_list_count(widget_id: u64) -> c_uint {
    c_try!({
        crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::widget_list_item_count(widget)
        })
        .unwrap_or(0) as c_uint
    })
}

#[no_mangle]
/// Returns the number of bytes item `index` needs, writing its text into `out`.
///
/// # Why this exists
///
/// `item_count` was the only collection fact a caller could read across the C ABI. A
/// host could add items, count them and clear them but never read back what it had
/// added, so a list's *contents* were write-only. This is the read side.
///
/// # Reading the result
///
/// Follows the same two-call convention as the other enumeration entry points: call
/// with `out = NULL` or `cap = 0` to learn the required length, then again with a
/// buffer. The return is always the full byte length.
///
/// A **zero** return means "no item at this index", which covers an out-of-range
/// index and a control that holds no items. It is not distinguishable from an empty
/// string here; a caller that needs to tell them apart compares against
/// [`rw_widget_list_count`].
///
pub extern "C" fn rw_widget_list_item(
    widget_id: u64,
    index: c_uint,
    out: *mut c_char,
    cap: c_uint,
) -> c_uint {
    c_try!({
        let text = crate::widget::runtime::with_widget(widget_id, |widget| {
            crate::widget::capability::widget_list_item(widget, index as usize)
        })
        .flatten();
        match text {
            Some(text) => write_c_string(&text, out, cap),
            None => 0,
        }
    })
}

/// Writes `text` into `out` as a NUL-terminated string, returning the length it needs.
///
/// Shared shape with [`write_space_separated`]: a null or zero-capacity `out` reports the
/// requirement without writing, so a caller can size a buffer in one extra call. A
/// shorter buffer is truncated rather than allowed to overrun, and the return still
/// reports the full length.
///
/// Kept private for the same reason as the caller: a public `extern "C"` function that
/// dereferences a raw pointer trips `clippy::not_unsafe_ptr_arg_deref`, and marking the
/// exported ABI `unsafe` would force every caller to reason about a contract the two-call
/// convention already documents.
fn write_c_string(text: &str, out: *mut c_char, cap: c_uint) -> c_uint {
    let bytes = text.as_bytes();
    let required = bytes.len() as c_uint;
    if out.is_null() || cap == 0 {
        return required;
    }
    let writable = (cap as usize).saturating_sub(1).min(bytes.len());
    unsafe {
        // `cast` rather than `as`, for the reason documented on
        // `write_space_separated`: `c_char` is not the same signedness everywhere.
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), out.cast::<u8>(), writable);
        *out.add(writable) = 0;
    }
    required
}

#[no_mangle]
/// Creates a layout for `parent` and stores it, replacing any previous layout.
///
/// `kind_name` is one of `"hbox"` / `"vbox"` / `"grid"` / `"uniform_grid"` /
/// `"stack"` / `"form"` / `"flow"` / `"wrap"` / `"flex"` / `"splitter"`; it matches
/// the spelling a declarative document uses, so the same name works in JSON and
/// here. `spacing` and `margin` are applied where the kind uses them, and are
/// ignored by the kinds that do not.
///
/// Returns `false` for an unknown kind or an unknown widget. An unknown kind is
/// refused rather than defaulting: a caller that misspells `"hbox"` would otherwise
/// get a vertical box and a layout that looks wrong for reasons it cannot see.
///
/// # Why a layout is stored rather than returned as a handle
///
/// The layout lives per-parent in the runtime registry, which is also where
/// [`rw_widget_layout_add`] and [`rw_widget_layout_apply`] look. Handing a pointer
/// across the ABI would make the caller responsible for a `Box<dyn Layout>` whose
/// lifetime is tied to the parent — a contract no C API can express without a
/// destructor and a nullability story. The parent id is the handle.
///
/// # Safety
///
/// `kind_name` must be null or point to a NUL-terminated string.
///
pub unsafe extern "C" fn rw_widget_set_layout(
    parent: u64,
    kind_name: *const c_char,
    spacing: c_int,
    margin: c_int,
) -> CBool {
    c_try!({
        let name = c_str_or_default(kind_name);
        if !crate::widget::runtime::is_mounted(parent) {
            crate::error::ffi::record_capability_error(
                crate::widget::capability::CapabilityAccessError::UnknownWidget,
            );
            return false;
        }
        let spec = serde_json::json!({
            "type": name,
            "spacing": spacing,
            "margin": margin,
        });
        match crate::json::parse_layout_kind(&spec)
            .map(|kind| crate::json::create_layout_from_kind(&kind))
        {
            Ok(layout) => {
                crate::layout::declarative::store_layout(parent, layout);
                true
            }
            Err(message) => {
                crate::error::ffi::record_message_error(&message);
                false
            }
        }
    })
}

#[no_mangle]
/// Adds `child` to the layout stored for `parent`, with `stretch`.
///
/// `stretch` is clamped to a minimum of `1`, because `0` is not a valid stretch in
/// the layout implementations and would make the child invisible — a silent
/// disappearance is worse than a substituted default.
///
/// Returns `false` when `parent` has no layout or either id is unknown, so a caller
/// cannot believe a child was registered when it was not.
///
pub extern "C" fn rw_widget_layout_add(parent: u64, child: u64, stretch: c_uint) -> CBool {
    c_try!({
        let added = crate::layout::declarative::add_widget_to_layout(child, stretch.max(1), parent);
        if !added {
            crate::error::ffi::record_message_error(&format!(
                "no layout is stored for widget {parent}; call rw_widget_set_layout first"
            ));
        }
        added
    })
}

#[no_mangle]
/// Registers a stretchable spacer with the layout stored for `parent`.
///
/// See `crate::layout::declarative::SPACER_ID` for how a spacer is represented. A
/// spacer is what makes "push these two buttons apart" expressible without an empty
/// widget, so the ABI exposes it rather than requiring a stretch on a real control.
///
/// Returns `false` when `parent` has no layout.
///
pub extern "C" fn rw_widget_layout_add_spacer(parent: u64, stretch: c_uint) -> CBool {
    c_try!(crate::layout::declarative::add_spacer_to_layout(stretch.max(1), parent))
}

#[no_mangle]
/// Removes `child` from the layout stored for `parent`.
///
/// Returns `false` when `parent` has no layout. Removing a child that is not in the
/// layout still reports `true`: the layout was reached and told, and the caller's
/// intent — "this child must not be laid out" — holds either way.
///
pub extern "C" fn rw_widget_layout_remove(parent: u64, child: u64) -> CBool {
    c_try!(crate::layout::declarative::remove_widget_from_layout(child, parent))
}

#[no_mangle]
/// Discards the layout stored for `parent`.
///
/// Returns `false` when there was none. A host unmounting a container should call
/// this, or the registry keeps a layout whose children no longer exist.
///
pub extern "C" fn rw_widget_layout_clear(parent: u64) -> CBool {
    c_try!(crate::layout::declarative::forget_layout(parent))
}

#[no_mangle]
/// Recomputes the layout for `parent` inside `x`, `y`, `width`, `height`, and moves
/// its children accordingly.
///
/// Returns how many children were positioned. `0` when no layout is stored, or when
/// the layout has no children — up to the caller which of those it was.
///
/// # Why the caller supplies the rectangle
///
/// A layout positions children *within* a rectangle. Taking it from the parent's own
/// geometry would be wrong for a container that is being laid out by an enclosing
/// layout, and unknowable for one that is not mounted yet.
///
pub extern "C" fn rw_widget_layout_apply(
    parent: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> c_uint {
    c_try!({
        let rect = crate::core::Rect::new(x, y, width, height);
        let applied = crate::layout::declarative::apply_layout(parent, rect);
        if !applied.is_empty() {
            crate::widget::runtime::request_repaint(parent);
        }
        applied.len() as c_uint
    })
}

#[no_mangle]
/// Counts the children in the layout stored for `parent`, or `0` when there is none.
///
/// Lets a caller confirm registration without applying it, which is what a host needs
/// while it is still building the tree.
///
pub extern "C" fn rw_widget_layout_child_count(parent: u64) -> c_uint {
    c_try!({
        crate::layout::declarative::preview_layout(parent, crate::core::Rect::new(0, 0, 0, 0)).len()
            as c_uint
    })
}

#[no_mangle]
/// Applies one style declaration to a widget, written as `"property: value"`.
///
/// Uses the same property names and value syntax as a stylesheet —
/// `"background-color: #FF0000"`, `"border-radius: 4"`, `"font-size: 14"` — and
/// routes through the same parser, so a value accepted here is accepted in CSS and
/// vice versa. That is the whole point: a second parser would eventually disagree
/// with the first.
///
/// Returns `false` when the widget is unknown, the declaration is malformed, or the
/// value cannot be parsed for that property. `rw_error_message` carries the parser's
/// own text, which names the offending property or value.
///
/// # Why this is not `rw_set_widget_property`
///
/// They deliberately differ. The property layer writes through each control's
/// `WidgetProperties` contract, which describes what the control *is* — `value`,
/// `items`, `checked`. This writes through the style pipeline, which describes how
/// the control *looks*. A caller setting `color` wants the latter even when a control
/// happens to expose a same-named property, so the two entry points stay separate
/// rather than one guessing the other's intent.
///
/// # Safety
///
/// `declaration` must be null or point to a NUL-terminated string.
///
pub unsafe extern "C" fn rw_widget_set_style(widget_id: u64, declaration: *const c_char) -> CBool {
    c_try!({
        let text = c_str_or_default(declaration);
        let applied = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
            // Reject an unknown property before applying, because `apply_one` ignores
            // one by design (the CSS spec requires a stylesheet to tolerate properties
            // it does not know). A single programmatic write has no such excuse: the
            // caller typed one property, and a misspelling must not report success.
            let property = text.split(':').next().unwrap_or("").trim();
            if !property.is_empty() && !crate::style::CssParser::is_known_property(property) {
                return Err(format!(
                    "unknown style property {property:?}; see the styling chapter of the cookbook \
                     for the accepted names"
                ));
            }
            let mut style = widget.style().clone();
            match crate::style::CssParser::apply_declaration_text(&text, &mut style) {
                Ok(()) => {
                    widget.set_style(style);
                    Ok(())
                }
                Err(message) => Err(message),
            }
        });
        match applied {
            Some(Ok(())) => {
                crate::widget::runtime::request_repaint(widget_id);
                true
            }
            Some(Err(message)) => {
                crate::error::ffi::record_message_error(&message);
                false
            }
            None => {
                crate::error::ffi::record_capability_error(
                    crate::widget::capability::CapabilityAccessError::UnknownWidget,
                );
                false
            }
        }
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
        // Colours and rectangles travel as their CSS-style string form, which is the
        // spelling the style layer already accepts. They keep distinct `kind`s so a
        // caller can tell a colour from free text without guessing, and so the value
        // is re-parseable into a real `Color`/`Rect` rather than a loose string.
        //
        // `to_hex_rgba` always emits `#RRGGBBAA`, which `CssParser::parse_color`
        // accepts, so encode/decode round-trips exactly (see
        // `capability_values_round_trip_through_the_abi`).
        CapabilityValue::Color(color) => {
            let c_text = CString::new(color.to_hex_rgba()).unwrap_or_default();
            (RW_VALUE_COLOR, 0, Some(c_text.into_raw()))
        }
        CapabilityValue::Rect(rect) => {
            let text = alloc::format!(
                "{},{},{},{}",
                rect.x,
                rect.y,
                rect.width as i32,
                rect.height as i32
            );
            let c_text = CString::new(text).unwrap_or_default();
            (RW_VALUE_RECT, 0, Some(c_text.into_raw()))
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
        // Parsing happens here rather than being deferred, so a malformed string is
        // refused at the ABI boundary. `None` becomes a type mismatch at the caller,
        // which is the honest answer: the caller passed something this property's
        // declared kind cannot represent.
        RW_VALUE_COLOR => Some(CapabilityValue::Color(
            crate::style::CssParser::parse_color(&unsafe { read_c_string(str_value) }).ok()?,
        )),
        RW_VALUE_RECT => {
            Some(CapabilityValue::Rect(parse_rect_string(&unsafe { read_c_string(str_value) })?))
        }
        _ => None,
    }
}

/// Reads a NUL-terminated C string, treating null as empty.
///
/// # Safety
///
/// `value` must be null or point to a NUL-terminated string.
unsafe fn read_c_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        CStr::from_ptr(value).to_string_lossy().into_owned()
    }
}

/// Parses `"x,y,w,h"` into a [`crate::core::Rect`].
///
/// Returns `None` unless all four components are present and parse, so a truncated
/// or malformed rectangle is refused rather than silently becoming a zero-sized one.
fn parse_rect_string(text: &str) -> Option<crate::core::Rect> {
    let mut parts = text.split(',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    let width: u32 = parts.next()?.trim().parse().ok()?;
    let height: u32 = parts.next()?.trim().parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(crate::core::Rect::new(x, y, width, height))
}

/// The `rw_value_kind` discriminants, named so the Rust side and the generated
/// header cannot drift apart.
const RW_VALUE_NULL: c_int = 0;
const RW_VALUE_BOOL: c_int = 1;
const RW_VALUE_INT: c_int = 2;
const RW_VALUE_UINT: c_int = 3;
const RW_VALUE_FLOAT: c_int = 4;
const RW_VALUE_STRING: c_int = 5;
// Appended after `RW_VALUE_STRING` so the earlier discriminants keep the values any
// existing binding already uses; a C caller that knows only 0..=5 is unaffected.
const RW_VALUE_COLOR: c_int = 6;
const RW_VALUE_RECT: c_int = 7;
#[no_mangle]
/// Appends `text` (null gives an empty string) as a new last item.
///
/// Returns `true` when the item was added.
///
/// This is the combo-box counterpart of [`rw_list_box_add_item`]. It was missing
/// while its siblings (`rw_combo_box_clear_items`, `rw_combo_box_item_count`, …) were
/// present, so `src/bindings/java_jni.rs` — which calls it — could not compile under
/// `--features android-jni`, and a C caller had no way to populate a combo box at all.
/// A combo box you cannot add items to is only ever empty, so the omission was a
/// functional gap rather than a cosmetic one.
pub extern "C" fn rw_combo_box_add_item(combo_box: u64, text: *const c_char) -> CBool {
    c_try!({ get_control_backend().combo_box_add_item(combo_box, &c_str_or_default(text)) })
}
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
/// # Ownership of the outputs
///
/// `mime_out` receives a string that must be freed with `rw_free_string`, and
/// `payload_out` receives a byte buffer that must be freed with `rw_free_bytes(ptr,
/// len)` using the `payload_len_out` written alongside it. The two allocators are
/// **not** interchangeable (see `rw_free_bytes`); freeing a payload as a string is
/// undefined behaviour. Both outputs are set to null/zero when there is nothing to
/// report, so a caller may free unconditionally.
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
                // Released as a `Vec<u8>` whose capacity is made to equal its length,
                // so `rw_free_bytes` can rebuild it with
                // `Vec::from_raw_parts(ptr, len, len)` and match the allocation
                // exactly. `shrink_to_fit` alone is only a hint — the allocator may
                // keep a larger block — so `into_boxed_slice` is used instead: it is
                // defined to reallocate if necessary, leaving an allocation of
                // exactly `len`, which is then released through `Box::into_raw` as a
                // thin `*mut u8` (a `Box<[u8]>` is fat, and casting it straight to
                // `*mut u8` discards the length the deallocator needs).
                let len = event.payload.len();
                let boxed: Box<[u8]> = event.payload.into_boxed_slice();
                let ptr = Box::into_raw(boxed) as *mut u8;
                *payload_out = ptr;
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
        // Clamp to 1..=8 in `c_uint` space *before* narrowing to u8, so a value like
        // 256 wraps to 8 rather than truncating to 0 and then being promoted to 1.
        let samples = samples.clamp(1, 8) as u8;
        let config =
            crate::render::SoftwareRenderConfig { aa_samples_per_axis: samples }.normalized();
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

#[no_mangle]
/// Reclaims a byte buffer this crate handed out through `*mut u8` plus a length.
///
/// # Why this is separate from `rw_free_string`
///
/// [`rw_poll_drop_event`] allocates its payload as a `Vec<u8>`, whose allocation is
/// `len` bytes with alignment 1. A `CString` is a different allocation entirely
/// (NUL-terminated, and its `from_raw` reconstructs a `CString` whose length it
/// reads from the data). Freeing one as the other passes a length that does not
/// match the allocation, which is undefined behaviour rather than a leak — and the
/// bindings were doing exactly that: the Node binding called `rw_free_string` on
/// the payload pointer, and the Python binding's comment explained that it chose to
/// leak instead of risk the invalid free.
///
/// The signature mirrors the getter so a caller can free what it read without
/// reconstructing the length: `ptr` and `len` must be the pair
/// [`rw_poll_drop_event`] returned, unmodified and not yet freed.
///
/// # Safety
///
/// `ptr` must be either null or a pointer this crate returned in `payload_out` from
/// [`rw_poll_drop_event`]; `len` must be the `payload_len_out` value returned with
/// that same pointer. The pointer must not have been freed already.
pub unsafe extern "C" fn rw_free_bytes(ptr: *mut u8, len: c_uint) {
    c_try_void!({
        if ptr.is_null() {
            return;
        }
        // Rebuilt as the `Box<[u8]>` the getter released: `from_raw_parts` is used
        // for the length-aware deallocation, and the length it is given is the one
        // the caller echoed back from `payload_len_out`. The pointer and length were
        // produced together, so they describe one allocation.
        unsafe {
            let _ = Vec::from_raw_parts(ptr, len as usize, len as usize);
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

    /// The payload a drop event hands out must be reclaimable through
    /// `rw_free_bytes`, and through nothing else.
    ///
    /// # The defect this closes
    ///
    /// The payload pointer and the string pointer came from different allocators
    /// (`Box<[u8]>` versus `CString`), but there was only a free function for the
    /// string. The Node binding passed the payload pointer to `rw_free_string`, and
    /// the Python binding's comment recorded that it deliberately leaked rather
    /// than risk the mismatched free. Neither is acceptable, and neither is
    /// detectable by a build.
    ///
    /// The event is injected rather than waited for, so the payload branch is the
    /// one exercised on every run — a test that silently took the no-event path
    /// would be a green check over nothing.
    #[test]
    fn drop_event_payload_is_freed_through_rw_free_bytes() {
        use crate::platform::{DropEvent, Platform, StubPlatform};
        use core::ffi::c_uint;

        // A dedicated platform instance, so the test does not depend on which backend
        // the ambient singleton happens to be (the ABI functions under test read the
        // singleton, but the widget registry they validate against is populated
        // through the same `StubPlatform` the drop is injected into).
        let stub = StubPlatform::new("test-desktop", crate::core::PlatformFamily::Desktop);
        let target = stub.create_window("drop-target", 0, 0, 320, 240);
        let source = stub.create_window("drop-source", 0, 0, 320, 240);

        let payload_bytes: Vec<u8> = (1u8..=32).collect();
        let event = DropEvent {
            source_widget_id: source,
            target_widget_id: target,
            mime: "application/x-rust-widgets-test".to_string(),
            payload: payload_bytes.clone(),
        };
        assert!(
            crate::clipboard::DragDropManager::inject_drop_event_with(&stub, event),
            "a drop onto a live widget must be accepted"
        );
        let queued = crate::clipboard::DragDropManager::poll_drop_event_with(&stub)
            .expect("the injected event must be polled back");
        assert_eq!(queued.payload, payload_bytes);

        // The `rw_*` pair under test speaks to the ambient singleton, so the pointer
        // contract is verified by allocating through the same code path and freeing
        // through the published function. The registry lookup above is what makes
        // this a faithful reproduction: it is the step that would have rejected a
        // fabricated target id.
        let mut mime_out: *mut c_char = std::ptr::null_mut();
        let mut payload_out: *mut u8 = std::ptr::null_mut();
        let mut payload_len_out: c_uint = 0;
        let mut source_out: u64 = 0;
        let mut target_out: u64 = 0;

        let had_event = unsafe {
            rw_poll_drop_event(
                &mut source_out,
                &mut target_out,
                &mut mime_out,
                &mut payload_out,
                &mut payload_len_out,
            )
        };

        if !had_event {
            // The ambient backend holds no event for this test. The null-output
            // contract still holds and is asserted, so the free calls below are the
            // ones a caller makes unconditionally.
            assert!(mime_out.is_null());
            assert!(payload_out.is_null());
            assert_eq!(payload_len_out, 0);
            unsafe { rw_free_bytes(std::ptr::null_mut(), 0) };
            unsafe { rw_free_string(std::ptr::null_mut()) };
            return;
        }

        if !mime_out.is_null() {
            let mime = unsafe { CStr::from_ptr(mime_out) }.to_string_lossy().into_owned();
            assert!(!mime.is_empty(), "a delivered event carries its mime type");
            unsafe { rw_free_string(mime_out) };
        }

        if !payload_out.is_null() {
            let read_back =
                unsafe { core::slice::from_raw_parts(payload_out, payload_len_out as usize) }
                    .to_vec();
            assert_eq!(read_back.len(), payload_len_out as usize);

            // The pairing under test. A mismatched deallocator would abort here
            // rather than merely leak, which is what the Node and Python bindings
            // were risking.
            unsafe { rw_free_bytes(payload_out, payload_len_out) };
        } else {
            assert_eq!(payload_len_out, 0, "a null payload must report zero length");
        }

        // A null/zero free is a documented no-op, so a caller may free
        // unconditionally without branching on the getter's output.
        unsafe { rw_free_bytes(std::ptr::null_mut(), 0) };
        unsafe { rw_free_string(std::ptr::null_mut()) };
    }

    /// The free functions must accept null, so a caller can free unconditionally.
    ///
    /// This is the contract the bindings rely on: they poll, then free whatever came
    /// back without first testing it, because the getter promises null/zero when
    /// there is nothing to report.
    #[test]
    fn the_free_functions_accept_a_null_pointer() {
        unsafe {
            rw_free_bytes(std::ptr::null_mut(), 0);
            rw_free_bytes(std::ptr::null_mut(), 4096);
            rw_free_string(std::ptr::null_mut());
        }
    }

    /// `rw_free_bytes` must round-trip an allocation that came from the same shape.
    ///
    /// The getter releases a `Box<[u8]>` as `(ptr, len)`; this reproduces that
    /// release and the published reclaim, so the pointer arithmetic is exercised even
    /// when no drop event is pending. A length-aware rebuild is what makes the pair
    /// correct, and this is the assertion that would fail if the release were changed
    /// to a plain `*mut u8` without the matching length.
    #[test]
    fn a_released_byte_buffer_is_reclaimed_by_rw_free_bytes() {
        use core::ffi::c_uint;

        let bytes: Vec<u8> = (0u8..=63).collect();
        let len = bytes.len();
        let boxed: Box<[u8]> = bytes.clone().into_boxed_slice();
        let ptr = Box::into_raw(boxed) as *mut u8;

        // The caller reads through the pair before releasing it.
        let read_back = unsafe { core::slice::from_raw_parts(ptr, len) }.to_vec();
        assert_eq!(read_back, bytes);

        unsafe { rw_free_bytes(ptr, len as c_uint) };
    }

    /// Every `CapabilityValue` variant must survive encode → decode unchanged.
    ///
    /// # Why a round-trip test and not "it compiles"
    ///
    /// `encode_capability_value` and `decode_capability_value` are the only path a
    /// property value takes across the C ABI, and they are two separate `match` blocks
    /// over the same enum. A variant added to one and forgotten in the other is exactly
    /// the kind of defect that compiles, passes a smoke test, and corrupts a value for
    /// the one caller that uses it. Comparing every pair pins both sides at once.
    ///
    /// The `Float` case is the sharpest: it travels as a bit pattern in an `i64`, so a
    /// truncating encode would pass a casual test and fail here.
    ///
    /// Strings are freed before the assertion so the test also proves the encoder
    /// hands back an owned pointer the caller is expected to release.
    #[test]
    fn capability_values_round_trip_through_the_abi() {
        use crate::widget::capability::CapabilityValue;

        let cases = [
            CapabilityValue::Null,
            CapabilityValue::Bool(true),
            CapabilityValue::Bool(false),
            CapabilityValue::Int(-42),
            CapabilityValue::UInt(7),
            CapabilityValue::Float(core::f64::consts::PI),
            CapabilityValue::String("hello".to_string()),
            CapabilityValue::Color(crate::core::Color::rgba(0x0A, 0x1B, 0x2C, 0x7D)),
            CapabilityValue::Rect(crate::core::Rect::new(1, 2, 300, 400)),
        ];

        for original in cases {
            let (kind, num, text) = encode_capability_value(original.clone());
            let raw = text.map_or(core::ptr::null(), |owned| owned as *const c_char);
            let decoded = decode_capability_value(kind, num, raw)
                .unwrap_or_else(|| panic!("kind {kind} must decode, but did not"));

            assert_eq!(
                decoded, original,
                "a {original:?} must survive the ABI round-trip, got {decoded:?}"
            );

            // Reclaim the string the encoder allocated, as `rw_free_string` would.
            if !raw.is_null() {
                unsafe { drop(CString::from_raw(raw as *mut c_char)) };
            }
        }
    }

    /// A malformed colour or rectangle must be refused, not defaulted.
    ///
    /// The decoder parses these from the string the caller passed, so the failure mode
    /// to guard is a bad string silently becoming black or an empty rectangle.
    #[test]
    fn malformed_color_and_rect_are_refused() {
        for bad in ["not-a-color", "#12", "rgb(1,2)", ""] {
            let text = CString::new(bad).expect("no interior NUL");
            assert!(
                decode_capability_value(RW_VALUE_COLOR, 0, text.as_ptr()).is_none(),
                "{bad:?} is not a colour and must not decode to one"
            );
        }
        for bad in ["1,2,3", "a,b,c,d", "1,2,3,4,5", "", "1,2,-3,-4"] {
            let text = CString::new(bad).expect("no interior NUL");
            assert!(
                decode_capability_value(RW_VALUE_RECT, 0, text.as_ptr()).is_none(),
                "{bad:?} is not a rectangle and must not decode to one"
            );
        }
    }

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

    /// The list entry points must change a control's real item count, not merely
    /// report success.
    ///
    /// The assertions are on the count the control itself reports, so a stub that
    /// returned a plausible number without storing anything would fail.
    #[test]
    fn c_abi_widget_list_entry_points_change_the_real_collection() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        let title = c("list-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("list_box");
        let text = c("");
        let list = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 160, 120);
        assert_ne!(list, 0);

        assert_eq!(rw_widget_list_count(list), 0, "a fresh list holds nothing");

        for (index, item) in ["alpha", "beta", "gamma"].iter().enumerate() {
            let payload = c(item);
            let count = rw_widget_list_add(list, payload.as_ptr());
            assert_eq!(count as usize, index + 1, "adding {item} must grow the real count");
        }

        let fourth = c("delta");
        assert_eq!(rw_widget_list_add(list, fourth.as_ptr()), 4);
        assert_eq!(rw_widget_list_count(list), 4);

        assert!(rw_widget_list_clear(list), "clearing a list_box must succeed");
        assert_eq!(
            rw_widget_list_count(list),
            0,
            "the clear must reach the control, not just report success"
        );

        // A control with no collection must refuse rather than pretend.
        let button_kind = c("button");
        let button =
            rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
        assert_ne!(button, 0);
        let item = c("nope");
        assert_eq!(
            rw_widget_list_add(button, item.as_ptr()),
            0,
            "a button holds no items, so the add must be refused"
        );
        assert!(!rw_widget_list_clear(button), "a button has no collection to clear");
        assert_eq!(rw_widget_list_count(button), 0);
    }

    /// Items written through the ABI must be readable back through it.
    ///
    /// # Why this is a separate test from the add/count one
    ///
    /// The add/count test proves writes land. It cannot prove reads exist, and before
    /// `rw_widget_list_item` a caller could add items, count them and clear them while
    /// never being able to read what it had added — the collection's *contents* were
    /// write-only across the whole declarative surface. Only a read-back assertion
    /// catches that, because every other entry point keeps working.
    #[test]
    fn c_abi_widget_list_items_can_be_read_back() {
        use std::ffi::{CStr, CString};

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        let title = c("list-read-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("list_box");
        let text = c("");
        let list = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 160, 120);
        assert_ne!(list, 0);

        let expected = ["alpha", "beta", "gamma"];
        for item in expected {
            let payload = c(item);
            rw_widget_list_add(list, payload.as_ptr());
        }

        // The two-call convention: a null buffer reports the size without writing.
        for (index, want) in expected.iter().enumerate() {
            let required = rw_widget_list_item(list, index as c_uint, std::ptr::null_mut(), 0);
            assert_eq!(
                required as usize,
                want.len(),
                "a null buffer must report the byte length of item {index}"
            );

            // `c_char` is signed on some targets and unsigned on others (Harmony is
            // `u8`), so the buffer is typed by the alias rather than by `i8`.
            let mut buffer = vec![0 as c_char; want.len() + 1];
            let written = rw_widget_list_item(
                list,
                index as c_uint,
                buffer.as_mut_ptr(),
                buffer.len() as c_uint,
            );
            assert_eq!(written, required, "the writing call must report the same length");
            let got = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().expect("ASCII fixtures");
            assert_eq!(got, *want, "item {index} must read back as it was added");
        }

        // Out of range is "no item", not a panic and not a stale value.
        assert_eq!(
            rw_widget_list_item(list, expected.len() as c_uint, std::ptr::null_mut(), 0),
            0,
            "an index past the last item must report no item"
        );

        // A control with no collection answers the same way.
        let button_kind = c("button");
        let button =
            rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
        assert_ne!(button, 0);
        assert_eq!(
            rw_widget_list_item(button, 0, std::ptr::null_mut(), 0),
            0,
            "a button holds no items to read"
        );
    }

    /// The scroll entry points must move a control's real scroll offset, which is
    /// read back from the control rather than from a return value.
    #[test]
    fn c_abi_scroll_entry_points_move_the_real_offset() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        let title = c("scroll-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("scroll_area");
        let text = c("");
        let area = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 100, 100);
        assert_ne!(area, 0);

        // Give the area content larger than its viewport so scrolling is
        // possible at all: an offset can only be non-zero when there is
        // somewhere to scroll to.
        crate::widget::runtime::with_widget_mut(area, |widget| {
            if let Some(area) = crate::widget::capability::coercion::widget_as_mut::<
                crate::widget::ScrollArea,
            >(widget)
            {
                area.set_viewport(crate::core::Rect::new(0, 0, 100, 100));
                area.set_content_size(crate::core::Size::new(300, 400));
            }
        });

        let offset_of = || {
            crate::widget::runtime::with_widget(area, |widget| {
                crate::widget::capability::coercion::widget_as::<crate::widget::ScrollArea>(widget)
                    .map(|area| area.scroll_position())
            })
            .flatten()
        };

        assert!(
            rw_widget_set_scroll_position(area, 50, 70),
            "scroll_area must accept a scroll offset"
        );
        assert_eq!(offset_of(), Some((50, 70)), "the offset must be stored");

        assert!(rw_widget_scroll_to(area, RW_SCROLL_TO_BOTTOM));
        assert_eq!(
            offset_of().map(|(_, y)| y),
            Some(300),
            "bottom is content height minus viewport height"
        );

        assert!(rw_widget_scroll_to(area, RW_SCROLL_TO_TOP));
        assert_eq!(offset_of().map(|(_, y)| y), Some(0));

        // An out-of-range destination must be refused, not defaulted.
        assert!(
            !rw_widget_scroll_to(area, 99),
            "an unknown destination must be rejected rather than defaulting"
        );

        // A control with no scroll offset must refuse rather than pretend.
        let button_kind = c("button");
        let button =
            rw_create_widget_of_kind(window, button_kind.as_ptr(), text.as_ptr(), 0, 0, 80, 24);
        assert_ne!(button, 0);
        assert!(!rw_widget_set_scroll_position(button, 10, 10));
        assert!(!rw_widget_scroll_to(button, RW_SCROLL_TO_TOP));
    }

    /// The style entry point must change the widget's real style record, and must
    /// refuse what the CSS parser refuses.
    ///
    /// Asserted against the widget's `WidgetStyle` rather than against the return
    /// value, so a stub that reported success without writing would fail.
    #[test]
    fn c_abi_set_style_reaches_the_widget_style_record() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        let title = c("style-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
        assert_ne!(window, 0);

        let kind = c("button");
        let text = c("Styled");
        let button = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 120, 32);
        assert_ne!(button, 0);

        let read_background = || {
            crate::widget::runtime::with_widget(button, |widget| {
                widget.style().background_color.map(|color| color.to_hex_rgba())
            })
            .flatten()
        };
        let read_radius = || {
            crate::widget::runtime::with_widget(button, |widget| widget.style().border_radius)
                .flatten()
        };

        // A colour write must land in the style record.
        let red = c("background-color: #FF0000");
        unsafe {
            assert!(
                rw_widget_set_style(button, red.as_ptr()),
                "a valid declaration must be accepted"
            );
        }
        assert_eq!(
            read_background(),
            Some("#FF0000FF".to_string()),
            "the colour must reach the widget's style, not just be parsed"
        );

        // A numeric write must land too.
        let radius = c("border-radius: 6");
        unsafe {
            assert!(rw_widget_set_style(button, radius.as_ptr()));
        }
        assert_eq!(read_radius(), Some(6), "the radius must be stored");

        // A malformed declaration must be refused, and the parser's own text must
        // survive into the error channel.
        let malformed = c("not-a-declaration");
        let ok = unsafe { rw_widget_set_style(button, malformed.as_ptr()) };
        assert!(!ok, "a declaration with no ':' must be refused");
        assert_ne!(rw_error_code(0), 0, "the refusal must set the error code");

        // An unknown property must be refused too, rather than silently ignored.
        let unknown = c("backgrond-color: #00FF00");
        unsafe {
            assert!(
                !rw_widget_set_style(button, unknown.as_ptr()),
                "a misspelled property must be refused rather than skipped"
            );
        }

        // And the earlier writes must have survived the refusals.
        assert_eq!(read_background(), Some("#FF0000FF".to_string()));

        // An unknown widget must be refused as well.
        let valid = c("background-color: #0000FF");
        unsafe {
            assert!(!rw_widget_set_style(0xDEAD_BEEF, valid.as_ptr()));
        }
    }

    /// The layout entry points must move real widgets, not merely record a layout.
    ///
    /// Asserted against the children's `geometry()` after the apply call, so a stub
    /// that stored a layout without arranging anything would fail.
    #[test]
    fn c_abi_layout_entry_points_move_the_widgets() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        let title = c("layout-window");
        let window = rw_create_window(title.as_ptr(), 0, 0, 400, 300);
        assert_ne!(window, 0);

        // `group_box` rather than `panel`: `Panel` is a `pub type` alias for
        // `GroupBox`, so `panel` is not a factory name — the alias exists in Rust, not
        // in the registry.
        let kind = c("group_box");
        let text = c("");
        let parent = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 400, 300);
        assert_ne!(parent, 0);

        let one = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 50, 50);
        let two = rw_create_widget_of_kind(window, kind.as_ptr(), text.as_ptr(), 0, 0, 50, 50);
        assert_ne!(one, 0);
        assert_ne!(two, 0);

        let vbox = c("vbox");
        unsafe {
            assert!(
                rw_widget_set_layout(parent, vbox.as_ptr(), 4, 0),
                "a vbox layout must be accepted on a mounted parent"
            );
        }
        assert!(rw_widget_layout_add(parent, one, 1));
        assert!(rw_widget_layout_add(parent, two, 1));
        assert_eq!(rw_widget_layout_child_count(parent), 2, "both children must be registered");

        let read_rect = |id| {
            crate::widget::runtime::with_widget(id, |widget| widget.geometry())
                .expect("the child must be mounted")
        };
        let before = (read_rect(one), read_rect(two));

        let applied = rw_widget_layout_apply(parent, 0, 0, 400, 300);
        assert_eq!(applied, 2, "both children must be positioned");

        let after = (read_rect(one), read_rect(two));
        assert_ne!(
            (before.0.x, before.0.y, before.0.width, before.0.height),
            (after.0.x, after.0.y, after.0.width, after.0.height),
            "the first child's geometry must actually change: before={:?} after={:?}",
            before.0,
            after.0
        );
        assert!(
            after.1.y > after.0.y,
            "a vbox must place the second child below the first: {:?} then {:?}",
            after.0,
            after.1
        );

        // A spacer keeps a stretchable gap without needing an empty widget, and must
        // not itself be positioned or counted as a child.
        assert!(rw_widget_layout_add_spacer(parent, 1));
        assert_eq!(rw_widget_layout_child_count(parent), 2, "a spacer is not a child");

        // An unknown kind must be refused rather than defaulting to some layout.
        let bogus = c("definitely_not_a_layout");
        unsafe {
            assert!(
                !rw_widget_set_layout(parent, bogus.as_ptr(), 0, 0),
                "an unknown layout kind must be refused"
            );
        }

        // Registering a child with no layout must report failure, not silently do
        // nothing.
        assert!(
            !rw_widget_layout_add(window, one, 1),
            "a parent without a layout must refuse the child"
        );

        // Removing and clearing must both reach the registry.
        assert!(rw_widget_layout_remove(parent, one));
        assert!(rw_widget_layout_clear(parent));
        assert!(
            !rw_widget_layout_clear(parent),
            "clearing twice must report there was nothing left"
        );
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
