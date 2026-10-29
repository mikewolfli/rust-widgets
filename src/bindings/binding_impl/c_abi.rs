// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Core of the stable C ABI: lifecycle, widget construction, properties, menus, and memory.
use super::*;
use crate::c_try;
use crate::c_try_void;
use crate::control_backend::get_control_backend;
use alloc::ffi::CString;
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::c_uint;

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

#[no_mangle]
/// Drives **one frame** of the library without starting a blocking native loop.
///
/// # Why this exists alongside [`rw_run`]
///
/// [`rw_run`] hands the thread to the platform's own loop and does not return until
/// the loop exits. That is the right shape for a C host whose main thread is free to
/// block, but it is unusable from a host that must keep control of its own main
/// thread — a Python `example.py` that wants to poll triggers, an editor plug-in, a
/// test harness. The previous advice ("call `rw_run` before your polling loop") could
/// not work: control never returns to the polling loop (D09-PY-04).
///
/// This entry point is that host's frame step. It performs exactly the frame a
/// platform loop performs internally — drain the trigger queue, advance animations by
/// `delta_ms`, apply pending translations, and report repaint demand — through the
/// same single frame driver (`crate::drive_frame`), so there is one frame contract
/// and not two. A host that owns its loop calls this once per frame instead of
/// calling `rw_run`, and then reads the `rw_poll_*` functions for the events this
/// frame dispatched.
///
/// # What it does not do
///
/// It does **not** pump a toolkit's event source (a GTK/X11 event queue, a Win32
/// message pump). A backend whose user input arrives from such a source still needs
/// [`rw_run`] (or the host's own native message loop) to receive that input; a host
/// that only calls this and never pumps its toolkit sees no pointer input. The method
/// is the *frame* step, not the *input* source — the same division
/// `Platform::run` documents for a backend whose loop is externally driven.
///
/// Returns whether another frame is owed (an animation is still settling). A host
/// that ignores the answer still receives every queued trigger; it simply may not
/// paint the final settle frame of an animation until its next call. In the
/// allocation-frugal profile there is no animation bus, so the honest answer is
/// always `false` — the same answer `crate::drive_frame` gives there.
pub extern "C" fn rw_pump_frame(delta_ms: c_uint) -> CBool {
    c_try!({ crate::drive_frame(delta_ms).needs_another_frame })
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
            // SAFETY: every out pointer was null-checked individually and only non-null pointers are dereferenced for writing.
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
        // The capability registry — and therefore the factory that enumerates it —
        // exists only with a full device build (`full_widgets`). A build that has
        // this ABI but no registry truthfully reports an empty list rather than
        // failing to link; the gate is in the body so the symbol always exists
        // (principle #41).
        #[cfg(full_widgets)]
        let names = crate::widget::capability::WidgetFactory::shared().widget_names();
        #[cfg(not(full_widgets))]
        let names: Vec<&str> = Vec::new();
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
#[no_mangle]
pub unsafe extern "C" fn rw_widget_property_tokens(
    widget_id: u64,
    name: *const c_char,
    out: *mut c_char,
    cap: c_uint,
) -> c_uint {
    c_try!({
        // The gate lives in the **body**, not around the function: `rw_generated.h` declares
        // this entry point unconditionally, and a `stripped_widgets` build that dropped the
        // symbol would fail to link for a host that compiled against the header. Zero is this
        // entry point's documented "no fixed set of values" answer, which is honest here — a
        // stripped build publishes no property tokens at all (same shape as `rw_set_theme`).
        #[cfg(stripped_widgets)]
        {
            let _ = (widget_id, name, out, cap);
            0
        }
        #[cfg(not(stripped_widgets))]
        {
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
        }
    })
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
                // The output pointer's existence is checked *before* encoding, not
                // after. `encode_capability_value` hands back an owning `CString`
                // (via `into_raw`) for the string-carrying kinds; if `out_str` were
                // null at write time that pointer would have no destination and no
                // free path, leaking one allocation per read. A caller that only
                // wants the kind (the documented use — see the safety notes above)
                // must not pay for a string it will never receive, so the encoder is
                // told whether anyone can accept the text.
                let (kind, num, text) = encode_capability_value(value, !out_str.is_null());
                // SAFETY: each out-pointer is null-checked before the store, as the C ABI permits callers to pass null.
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
/// # Why this goes through the crate-level function and not `Platform`
///
/// It used to call `platform::get_platform().combo_box_set_current_index(..)`, which every
/// backend answers with the trait default — `false`. The combo box **is** a library-painted
/// control whose selection lives on the widget itself (`ComboBox::current_index`), and the
/// crate-level [`crate::combo_box_set_current_index`] is the function that reaches it. The
/// Platform trait's copy is a leftover from the native-control era, when a host could own the
/// control and its selection for real.
///
/// The effect of the wrong route was a **split brain** that reads as success: `rw_combo_box_add_item`
/// worked (it already went through the control backend), so a C/Python/Java host added its items,
/// then `rw_combo_box_set_current_index` returned `false` and `rw_combo_box_current_index`
/// answered `-1` forever. The host had populated a combo box it could never select in and never
/// read back.
///
/// Returns `false` if the index is out of range or the widget is unknown.
pub extern "C" fn rw_combo_box_set_current_index(combo_box: u64, index: c_uint) -> CBool {
    c_try!({ crate::combo_box_set_current_index(combo_box, index as usize) })
}

#[no_mangle]
/// The index of the selected item, zero-based, or `-1` when nothing is selected
/// or the widget is unknown.
pub extern "C" fn rw_combo_box_current_index(combo_box: u64) -> c_int {
    c_try!({
        // The crate-level accessor, not `Platform`'s: a combo box is library-painted, so its
        // selection lives on the widget. See `rw_combo_box_set_current_index` for the split-brain
        // this routing fixes.
        match crate::combo_box_current_index(combo_box) {
            Some(index) => index as c_int,
            None => -1,
        }
    })
}

#[no_mangle]
/// The number of items currently in the combo box; `0` if it is unknown.
pub extern "C" fn rw_combo_box_item_count(combo_box: u64) -> c_uint {
    c_try!({ crate::combo_box_item_count(combo_box) as c_uint })
}

#[no_mangle]
/// The text of the item at zero-based `index`.
///
/// An out-of-range index or an unknown widget yields an empty string rather than
/// an error. The result is a freshly allocated C string and must be released
/// with `rw_free_string`.
pub extern "C" fn rw_combo_box_item_text(combo_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::combo_box_item_text(combo_box, index as usize);
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
    c_try!({ crate::list_box_set_current_index(list_box, index as usize) })
}

#[no_mangle]
/// The index of the selected item, zero-based, or `-1` when nothing is selected
/// or the widget is unknown.
pub extern "C" fn rw_list_box_current_index(list_box: u64) -> c_int {
    c_try!({
        match crate::list_box_current_index(list_box) {
            Some(idx) => idx as c_int,
            None => -1,
        }
    })
}

#[no_mangle]
/// The number of items currently in the list box; `0` if it is unknown.
pub extern "C" fn rw_list_box_item_count(list_box: u64) -> c_uint {
    c_try!({ crate::list_box_item_count(list_box) as c_uint })
}

#[no_mangle]
/// The text of the item at zero-based `index`.
///
/// An out-of-range index or an unknown widget yields an empty string rather than
/// an error. The result is a freshly allocated C string and must be released
/// with `rw_free_string`.
pub extern "C" fn rw_list_box_item_text(list_box: u64, index: c_uint) -> *const c_char {
    c_try!({
        let text = crate::list_box_item_text(list_box, index as usize);
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
            // SAFETY: the caller guarantees `payload` points to at least `payload_len` readable bytes; null or zero-length was short-circuited above.
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
            // An empty queue must still clear every output: a caller that reuses an
            // `out` from a previous success (and frees it unconditionally) would
            // otherwise see the stale pointer/length and double-free. The getter's
            // contract is null/zero when there is nothing to report.
            // SAFETY: each out-pointer is null-checked before the store, as the C ABI permits null outputs.
            unsafe {
                if !source_out.is_null() {
                    *source_out = 0;
                }
                if !target_out.is_null() {
                    *target_out = 0;
                }
                if !mime_out.is_null() {
                    *mime_out = std::ptr::null_mut();
                }
                if !payload_out.is_null() {
                    *payload_out = std::ptr::null_mut();
                }
                if !payload_len_out.is_null() {
                    *payload_len_out = 0;
                }
            }
            return false;
        };
        // SAFETY: each out-pointer is null-checked before the store, as the C ABI permits null outputs.
        unsafe {
            if !source_out.is_null() {
                *source_out = event.source_widget_id;
            }
            if !target_out.is_null() {
                *target_out = event.target_widget_id;
            }
            if !mime_out.is_null() {
                let cs = c_string_or_empty(event.mime);
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
        c_string_or_empty(message).into_raw()
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
        // SAFETY: `s` was produced by this module's `into_raw`, so returning it to `CString` balances the same allocation.
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

// ── Widget surfaces ─────────────────────────────────────────────────────────
//
// The four functions below are what a host needs to **display** a widget on a
// platform that owns its own pixels but does not paint library widgets itself.
//
// # Why they exist, and why they were missing
//
// `docs/plans/harmony_integration.md` documents the ArkTS flow as "mount a
// surface, pull the pending repaint, render the frame, blit it", and
// `src/platform/harmony/status.md` claimed the same ("the ArkTS side pulls
// frames"). Neither was reachable: `Platform::mount_surface` and
// `invalidate_surface` are Rust trait methods with no C ABI entry point, and there
// was **no** function that produced frame pixels at all. So the Harmony backend
// reported `supports_surfaces() == true` — correctly, the queue exists — while a
// host holding only the C ABI could not mount anything.
//
// These are not Harmony-only: `AndroidMobilePlatform`, `IosMobilePlatform` and
// `MacOSObjc2Platform` answer `supports_surfaces() == true` over the same
// record-plus-queue store, so the same four calls are how any of them is driven
// from outside the process.
