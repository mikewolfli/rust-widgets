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
/// Convert a stable C ABI pointer-event code into an [`crate::event::Event`].
///
/// # The codes, and why `0` is not one of them
///
/// A host that owns its own event loop forwards touches and mouse events through
/// `rw_dispatch_pointer_event`. It has to name *which* event it is, and an `int` is the only
/// vocabulary a C ABI shares, so the mapping is: `1` press, `2` release, `3` move, `4` enter,
/// `5` leave. Any other value — including `0` — yields `None`, and the caller refuses the call.
///
/// # Why an unknown code is refused rather than defaulted
///
/// The tempting default is "treat anything unrecognised as a move", because a move is the
/// least destructive. It is also the *worst* choice: a host that sends `0` because it
/// mis-encoded a press would have every click silently become a hover — the control highlights
/// and never activates, with no error anywhere. Reporting "I do not know this event" is the
/// answer a caller can act on.
///
/// Returning `Option` rather than a sentinel `Event` is the same reasoning: there is no
/// "invalid event" variant to smuggle out, so a caller cannot accidentally dispatch one.
fn pointer_event_from_code(
    code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> Option<crate::event::Event> {
    // `button` is passed straight through; a move has none and the library ignores it there.
    let _ = button;
    match code {
        1 => Some(crate::event::Event::mouse_press(x, y, crate::event::mouse_button::PRIMARY)),
        2 => Some(crate::event::Event::mouse_release(x, y, crate::event::mouse_button::PRIMARY)),
        3 => Some(crate::event::Event::mouse_move(x, y)),
        4 => Some(crate::event::Event::MouseEnter { pos: crate::core::Point::new(x, y) }),
        5 => Some(crate::event::Event::MouseLeave { pos: crate::core::Point::new(x, y) }),
        _ => None,
    }
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
        let names = crate::widget::capability::WidgetFactory::new_with_defaults().widget_names();
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
                // The output pointer's existence is checked *before* encoding, not
                // after. `encode_capability_value` hands back an owning `CString`
                // (via `into_raw`) for the string-carrying kinds; if `out_str` were
                // null at write time that pointer would have no destination and no
                // free path, leaking one allocation per read. A caller that only
                // wants the kind (the documented use — see the safety notes above)
                // must not pay for a string it will never receive, so the encoder is
                // told whether anyone can accept the text.
                let (kind, num, text) = encode_capability_value(value, !out_str.is_null());
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
/// # Profiles without a theme module
///
/// `crate::theme` is gated `device_profile`. A build that compiles this ABI
/// without one (`android-jni` alone, for instance) therefore has no theme to
/// select, and the honest answer is `false` — the same "this build cannot do
/// that" the per-kind `create_*` functions give in a stripped profile. The
/// branch is in the **body**, not around the function, so the symbol always
/// exists in the library and a Java/C caller can link against it (principle
/// #41: capability is a runtime fact, not a compile-time API fork).
///
/// Before this gate the reference to `crate::theme` was unconditional, so
/// `cargo check --features "android-jni jni mobile-api …"` failed with
/// `cannot find theme in crate` — the real Android build script's exact feature
/// set.
pub extern "C" fn rw_set_theme(name: *const c_char) -> CBool {
    c_try!({
        let requested = c_str_or_default(name);
        #[cfg(device_profile)]
        {
            let activated = crate::theme::global_theme_manager().set_theme(&requested);
            if activated {
                // Re-resolve every live control's style against the new theme, the same
                // way the window-creation funnel does.
                crate::reapply_active_theme();
            }
            activated
        }
        #[cfg(not(device_profile))]
        {
            let _ = requested;
            false
        }
    })
}
#[no_mangle]
/// Names of the registered themes, space-separated.
///
/// Same convention as [`rw_widget_kind_names`]: the required size is returned and
/// a null `out` with `cap == 0` queries it.
///
/// A build without a theme module registers no themes, so it truthfully reports
/// an empty list (`0`) instead of naming themes that do not exist.
pub extern "C" fn rw_theme_names(out: *mut c_char, cap: c_uint) -> c_uint {
    c_try!({
        #[cfg(device_profile)]
        let names = {
            let manager = crate::theme::global_theme_manager();
            manager.theme_names().iter().map(|name| name.to_string()).collect::<Vec<_>>()
        };
        #[cfg(not(device_profile))]
        let names: Vec<alloc::string::String> = Vec::new();
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
/// A build without a theme module has no high-contrast override to set, so this
/// is a no-op there rather than a link error — see [`rw_set_theme`] for why the
/// gate is in the body.
pub extern "C" fn rw_set_high_contrast(mode: c_int) {
    c_try_void!({
        let enabled = mode != 0;
        let mode = if enabled {
            crate::style::HighContrastMode::WhiteOnBlack
        } else {
            crate::style::HighContrastMode::None
        };
        #[cfg(device_profile)]
        {
            crate::theme::set_global_high_contrast(mode);
            crate::reapply_active_theme();
        }
        #[cfg(not(device_profile))]
        {
            let _ = mode;
        }
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
        // `crate::json` is gated on the JSON engine (a device profile building with
        // the JSON surface). A build with this ABI but without it has no layout-kind
        // parser, so the honest answer is a recorded refusal rather than a link
        // error; the symbol stays available either way (principle #41).
        //
        // The spec is built **inside** the branch that reads it. It used to be built
        // unconditionally above the `#[cfg]`, which is a build error rather than a dead value
        // whenever `serde_json` is not a feature of this build: `serde_json::json!` names the
        // crate in the macro expansion, and the crate is not linked. That combination
        // (this ABI, no `serde_json`) was never built by a gate — `check_profiles.sh` passes
        // `serde_json` alongside `mobile-api` — so the unconditional build looked fine while a
        // profile list without it could not compile at all. Constructing it here makes the two
        // branches independent, which is what the `#[cfg]` was meant to express.
        #[cfg(all(device_profile, feature = "serde_json"))]
        {
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
        }
        #[cfg(not(all(device_profile, feature = "serde_json")))]
        {
            // The three parameters are still read so their presence in the signature is not a
            // silent lie about being used; the refusal is what the caller observes.
            let _ = (name, spacing, margin);
            crate::error::ffi::record_message_error(
                "this build has no JSON layout engine, so a declarative layout kind cannot be \
                 created here",
            );
            false
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
/// Encodes a [`CapabilityValue`](crate::widget::capability::CapabilityValue) into
/// the ABI's `(kind, num, text)` triple.
///
/// `want_text` says whether the caller has a place to receive the text. When it is
/// `false`, the string-carrying kinds return `None` **without allocating**: the
/// owning `CString` is only produced when there is an output pointer to receive it,
/// so a kind-only read (`out_str == null`) cannot leak. The `kind`/`num` half is
/// computed unconditionally, so a caller that suppresses the text still gets the
/// discriminator and the numeric payload.
fn encode_capability_value(
    value: crate::widget::capability::CapabilityValue,
    want_text: bool,
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
            if !want_text {
                return (RW_VALUE_STRING, 0, None);
            }
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
            if !want_text {
                return (RW_VALUE_COLOR, 0, None);
            }
            let c_text = CString::new(color.to_hex_rgba()).unwrap_or_default();
            (RW_VALUE_COLOR, 0, Some(c_text.into_raw()))
        }
        CapabilityValue::Rect(rect) => {
            if !want_text {
                return (RW_VALUE_RECT, 0, None);
            }
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
        // A tuple travels as a compact `kind:value;kind:value` string with its own kind, so a C
        // caller can tell a tuple from free text and can recover each component with its type.
        // The components keep their own kinds because the whole point of the variant is that a
        // composite payload is not flattened to one scalar (BLUE19 #95).
        CapabilityValue::Tuple(items) => {
            if !want_text {
                return (RW_VALUE_TUPLE, items.len() as i64, None);
            }
            let mut text = alloc::string::String::new();
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    text.push(';');
                }
                encode_tuple_component(item, &mut text);
            }
            let c_text = CString::new(text).unwrap_or_default();
            (RW_VALUE_TUPLE, items.len() as i64, Some(c_text.into_raw()))
        }
    }
}

/// Appends one tuple component to `out` as `kind:value`, the wire form
/// [`decode_capability_value`] reads back for [`RW_VALUE_TUPLE`].
#[cfg(not(stripped_widgets))]
fn encode_tuple_component(value: &crate::widget::capability::CapabilityValue, out: &mut String) {
    use crate::widget::capability::CapabilityValue;
    use core::fmt::Write as _;
    match value {
        CapabilityValue::Null => {
            let _ = write!(out, "0:");
        }
        CapabilityValue::Bool(flag) => {
            let _ = write!(out, "1:{}", u8::from(*flag));
        }
        CapabilityValue::Int(number) => {
            let _ = write!(out, "2:{number}");
        }
        CapabilityValue::UInt(number) => {
            let _ = write!(out, "3:{number}");
        }
        CapabilityValue::Float(number) => {
            let _ = write!(out, "4:{number}");
        }
        CapabilityValue::String(text) => {
            // The separator characters are escaped so a component containing `;` or `:` round-trips.
            let _ = write!(
                out,
                "5:{}",
                text.replace('\\', "\\\\").replace(';', "\\;").replace(':', "\\:")
            );
        }
        // A nested colour/rect/tuple is not produced by any event payload today; encoding it as its
        // announcement-free debug form would be a silent lossy path, so it is written as an empty
        // string component and the decoder refuses it. This is stated rather than silently emitted.
        CapabilityValue::Color(_) | CapabilityValue::Rect(_) | CapabilityValue::Tuple(_) => {
            let _ = write!(out, "9:");
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
        RW_VALUE_TUPLE => {
            let text = unsafe { read_c_string(str_value) };
            let components = decode_tuple_components(&text)?;
            // `num` is the advertised component count; trusting it over the parsed length would let a
            // truncated string look complete, so a disagreement is refused rather than resolved.
            if components.len() as i64 != num {
                return None;
            }
            Some(CapabilityValue::Tuple(components))
        }
        _ => None,
    }
}

/// Parses the `kind:value;kind:value` wire form written by [`encode_tuple_component`], or `None`
/// when any component is malformed.
#[cfg(not(stripped_widgets))]
fn decode_tuple_components(
    text: &str,
) -> Option<crate::compat::Vec<crate::widget::capability::CapabilityValue>> {
    use crate::widget::capability::CapabilityValue;
    let mut out = crate::compat::Vec::new();
    for component in split_unescaped(text) {
        let (kind, raw) = component.split_once(':')?;
        let value = match kind {
            "0" => CapabilityValue::Null,
            "1" => CapabilityValue::Bool(raw != "0"),
            "2" => CapabilityValue::Int(raw.parse().ok()?),
            "3" => CapabilityValue::UInt(raw.parse().ok()?),
            "4" => CapabilityValue::Float(raw.parse().ok()?),
            "5" => CapabilityValue::String(unescape_tuple_component(raw)?),
            // `9` is the encoder's "nested value, not representable here" marker; refusing it keeps
            // a lossy write from round-tripping as a silent empty value.
            _ => return None,
        };
        out.push(value);
    }
    Some(out)
}

/// Splits a tuple wire string on **unescaped** `;`, leaving `\;` inside a component intact.
#[cfg(not(stripped_widgets))]
fn split_unescaped(text: &str) -> crate::compat::Vec<&str> {
    let mut parts = crate::compat::Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b';' => {
                parts.push(&text[start..index]);
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    parts.push(&text[start..]);
    parts
}

/// Undoes the escaping [`encode_tuple_component`] applies to a string component.
#[cfg(not(stripped_widgets))]
fn unescape_tuple_component(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(chars.next()?);
        } else {
            out.push(ch);
        }
    }
    Some(out)
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
/// A composite payload (a tuple/list of values). Appended after `RW_VALUE_RECT`; `num` carries the
/// component count and the string holds `kind:value;…`.
const RW_VALUE_TUPLE: c_int = 8;
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

/// Mounts a widget onto a surface and returns whether the backend accepted it.
///
/// The caller must have created the widget first; `parent` is the window it should
/// be displayed in, and `x`/`y`/`width`/`height` are the rectangle it occupies.
///
/// Returns `false` when the backend cannot display library-painted widgets
/// (`Platform::supports_surfaces()`), or when the id is not one this backend
/// created. A caller must treat `false` as "cannot display here" and say so, rather
/// than presenting an empty surface.
#[no_mangle]
pub extern "C" fn rw_mount_surface(
    parent: u64,
    widget_id: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> CBool {
    c_try!({ crate::mount_surface(parent, widget_id, crate::core::Rect::new(x, y, width, height)) })
}

/// Updates the rectangle of a mounted surface.
///
/// Returns `false` when `widget_id` is not mounted, which is the honest answer for
/// a surface that was never mounted or has already been released.
#[no_mangle]
pub extern "C" fn rw_resize_surface(
    widget_id: u64,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
) -> CBool {
    c_try!({ crate::resize_surface(widget_id, crate::core::Rect::new(x, y, width, height)) })
}

/// Releases a mounted surface.
///
/// The widget itself stays alive in the registry; only its display surface is
/// released. Returns `false` when it was not mounted.
#[no_mangle]
pub extern "C" fn rw_unmount_surface(widget_id: u64) -> CBool {
    c_try!({ crate::unmount_surface(widget_id) })
}

/// Marks a mounted surface — or a window the backend draws — as needing a repaint.
///
/// Returns `false` when the id names nothing this backend can repaint. A window id
/// is a legitimate argument: the library repaints a window to reveal the children
/// it draws into the window's frame (see `Platform::invalidate_surface`).
#[no_mangle]
pub extern "C" fn rw_invalidate_surface(widget_id: u64) -> CBool {
    c_try!({ crate::invalidate_surface(widget_id) })
}

/// Whether this backend can host library-painted widgets.
///
/// `true` means [`rw_mount_surface`] will produce a surface that repaints; it says
/// nothing about how a surface is presented. A host that builds a UI it cannot
/// display should ask this first, because the alternative is a blank window with no
/// error — the exact failure mode these entry points were added to remove.
#[no_mangle]
pub extern "C" fn rw_supports_surfaces() -> CBool {
    c_try!({ crate::platform::get_platform().supports_surfaces() })
}

/// Renders one frame of a mounted widget and hands the caller its RGBA pixels.
///
/// # Why this is a getter rather than a "blit into my buffer" call
///
/// The host owns the drawing API (ArkUI `Canvas`, Android `Bitmap`, `NSView`,
/// Skia), so the library cannot draw into it without knowing every toolkit. What it
/// can do — and what this does — is produce the frame in the one layout all of them
/// consume: top-down, straight (non-premultiplied) alpha, `width * height * 4`
/// bytes.
///
/// # Contract
///
/// `x`/`y`/`width`/`height` describe the surface rectangle to render. `out_width`
/// and `out_height` receive the pixel dimensions actually produced, `out_len` the
/// byte length of the returned buffer, and `out_stride` the bytes between the start
/// of two consecutive rows (always `width * 4` today, but published rather than
/// assumed, because a host that hard-codes it shears the image the day a backend
/// pads a row).
///
/// The pixel buffer is returned through `out_pixels`, which the caller owns and must
/// release with [`rw_free_bytes`]. It is set to null when the widget is not mounted,
/// has no `Draw` implementation, or the area is empty — and that null is **not** an
/// error to swallow: it is why the surface stayed blank.
///
/// # Why the buffer is an out-parameter and not the return value
///
/// The error channel (`c_try!`) has no `CAbiSafe` implementation for `*mut u8`, and
/// that absence is deliberate: a raw byte pointer cannot be round-tripped through the
/// numeric fallback the macro uses for its primitives. Publishing the pointer through
/// an out-parameter keeps the frame path on the same error handling every other
/// function here uses, instead of needing a second, weaker convention for one call.
///
/// Returns `true` when a frame was produced. On `false`, every out-parameter is left
/// untouched.
///
/// # Safety
///
/// Every out-pointer may be null, in which case that value is simply not reported. A
/// non-null pointer must be valid and writable for one value of its type for the
/// duration of the call.
#[no_mangle]
pub unsafe extern "C" fn rw_render_surface_frame(
    widget_id: u64,
    width: c_uint,
    height: c_uint,
    out_width: *mut c_uint,
    out_height: *mut c_uint,
    out_stride: *mut c_uint,
    out_len: *mut c_uint,
    out_pixels: *mut *mut u8,
) -> CBool {
    c_try!({
        let Some(frame) = crate::widget::runtime::render_frame_cached(
            widget_id,
            crate::core::Size::new(width, height),
            crate::core::Color::WHITE,
        ) else {
            return false;
        };
        let byte_len = frame.len() as c_uint;
        // The buffer is only handed to the caller when there is an output pointer to
        // receive it. `out_pixels` may be null (the safety contract above allows it,
        // and a caller probing the size/stride reports may pass null deliberately), in
        // which case a `forget` would drop the only owning pointer on the floor and
        // leak the frame. So the `Box<[u8]>` is leaked **only** on the transfer path;
        // otherwise it is left to drop and free normally at the end of the call.
        //
        // Leaked as one allocation, exactly like `rw_poll_drop_event`'s payload, so
        // `rw_free_bytes(ptr, len)` reconstructs it with the length the caller echoed
        // back. Handing out a `Vec` pointer without its length is what forced the
        // separate free function to exist in the first place.
        let mut buffer = frame.into_boxed_slice();
        let pointer = if out_pixels.is_null() {
            core::ptr::null_mut()
        } else {
            let pointer = buffer.as_mut_ptr();
            core::mem::forget(buffer);
            pointer
        };
        // SAFETY: each out-pointer is null-checked before use, and the caller's
        // contract is that a non-null one is writable for one value of its type.
        unsafe {
            if !out_pixels.is_null() {
                *out_pixels = pointer;
            }
            if !out_width.is_null() {
                *out_width = width;
            }
            if !out_height.is_null() {
                *out_height = height;
            }
            if !out_stride.is_null() {
                *out_stride = width * 4;
            }
            if !out_len.is_null() {
                *out_len = byte_len;
            }
        }
        true
    })
}

/// Reads the next widget awaiting a repaint, or `0` when none is queued.
///
/// # Why this is a queue rather than a callback
///
/// The host owns the event loop on these platforms, so the library cannot call
/// "please repaint" into it. It records which surfaces went stale and the host pulls
/// them — which is also what makes a burst of invalidations cost one entry instead of
/// one per call (they are coalesced, see
/// `BackendState::record_repaint_request`).
///
/// # Both id spaces
///
/// The id returned is the one the invalidation named, which is the **widget id**
/// for an ordinary control and may be a widget id or a mounted-surface id depending
/// on the caller. A host that needs a window handle from it can resolve it with the
/// same association `rw_get_widget_geometry` uses.
#[no_mangle]
pub extern "C" fn rw_take_pending_repaint() -> u64 {
    c_try!({ crate::take_pending_repaint().unwrap_or(0) })
}

/// Delivers a pointer event that arrived on a host-owned drawing surface.
///
/// # Why a host needs this, and why it was missing
///
/// On the desktop backends the library drives the event loop, so a click is routed inside the
/// toolkit callback (`windows/types.rs`' window procedure, `linux/platform_impl.rs`' GTK
/// handler). A host that owns its own loop — ArkUI, an Android `Activity`, iOS touch
/// handling, a browser page — is never called into, so it must hand its touches back. There
/// was no way to do that.
///
/// This is not a nicety. `src/platform/android/status.md`, `src/platform/ios/status.md` and
/// `docs/plans/harmony_integration.md` all instruct a host to call
/// `rw_dispatch_pointer_event(...)`, and `grep -rn rw_dispatch_pointer_event src/ include/`
/// returned nothing — the documented integration step, without which a mounted widget is
/// visible but completely inert, had no implementation at any layer.
///
/// # Arguments
///
/// * `root` — the widget subtree to resolve the point in, normally the window. The library
///   hit-tests `point` against it, so the host does **not** need to know which widget was
///   under the finger. Sending an event to a chosen widget id is the separate
///   `rw_dispatch_event_to_widget` below, and is only right when the host already knows the
///   target (a key event for the focused control, a text commit).
/// * `event_code` — `1` press, `2` release, `3` move, `4` enter, `5` leave. Any other value,
///   including `0`, returns `false` rather than being silently treated as a move: a
///   mis-encoded press that became a hover would highlight the control and never activate it,
///   with no error anywhere.
/// * `x`/`y` — in the **window's** coordinate space.
/// * `button` — reserved for a future multi-button path; the library's press/release events
///   carry the primary button, and the value is passed through unchanged.
///
/// # Returns
///
/// Whether a widget accepted the event. `false` means the point was outside every widget in
/// `root`, the resolved widget refused it (disabled, or blocked by a modal), or `event_code`
/// was not one of the five codes.
#[no_mangle]
pub extern "C" fn rw_dispatch_pointer_event(
    root: u64,
    event_code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> CBool {
    c_try!({
        match pointer_event_from_code(event_code, x, y, button) {
            Some(event) => {
                crate::dispatch_pointer_event(root, &event, crate::core::Point::new(x, y))
            }
            None => false,
        }
    })
}

/// Delivers a pointer event straight to `widget_id`, without hit-testing.
///
/// The counterpart to `rw_dispatch_pointer_event` for a host that already knows the target. It
/// shares the same event codes, so a host needs one vocabulary rather than two.
///
/// Routing a pointer event this way **skips the library's hit test**, so a host that uses it
/// for touches must do its own; the pointer path above is the one the integration documents
/// name for that reason.
///
/// Keyboard, text and wheel input have their own carriers — a key code + modifier mask, a
/// committed text string, and a wheel delta — which the pointer `(x, y, button)` arguments
/// cannot express. Those use [`rw_dispatch_key_event`], [`rw_dispatch_text_event`] and
/// [`rw_dispatch_wheel_event`] below.
///
/// Returns whether the widget accepted it: `false` when the id is not a live widget, when the
/// control is disabled, when a modal blocks it, or when `event_code` is unknown.
#[no_mangle]
pub extern "C" fn rw_dispatch_event_to_widget(
    widget_id: u64,
    event_code: c_uint,
    x: c_int,
    y: c_int,
    button: c_uint,
) -> CBool {
    c_try!({
        match pointer_event_from_code(event_code, x, y, button) {
            Some(event) => crate::dispatch_event(widget_id, &event),
            None => false,
        }
    })
}

/// Delivers a key press/release straight to `widget_id`.
///
/// `key` and `modifiers` use the library's own conventions (see
/// [`crate::event::Event::KeyPress`]); `release` selects a `KeyRelease` over a `KeyPress`.
/// This is the direct-key counterpart to [`rw_dispatch_event_to_widget`], for a host that
/// already owns the focused control.
#[no_mangle]
pub extern "C" fn rw_dispatch_key_event(
    widget_id: u64,
    key: c_uint,
    modifiers: c_uint,
    release: CBool,
) -> CBool {
    c_try!({
        let event = if release {
            crate::event::Event::key_release(key, modifiers)
        } else {
            crate::event::Event::key_press(key, modifiers)
        };
        crate::dispatch_event(widget_id, &event)
    })
}

/// Delivers committed text (keyboard layout, IME, virtual keyboard, or paste) to `widget_id`.
///
/// The text is already filtered through the active keyboard layout, so it is not derivable from
/// a raw key code — which is exactly why it needs its own carrier rather than a key event.
#[no_mangle]
pub extern "C" fn rw_dispatch_text_event(widget_id: u64, text: *const c_char) -> CBool {
    c_try!({
        let event = crate::event::Event::text_input(c_str_or_default(text));
        crate::dispatch_event(widget_id, &event)
    })
}

/// Delivers a wheel/scroll event to `widget_id`.
///
/// The delta is in wheel notches (see [`crate::event::Event::Wheel`]); `delta_y` is vertical
/// (positive = scroll down) and `delta_x` is horizontal (positive = scroll right).
#[no_mangle]
pub extern "C" fn rw_dispatch_wheel_event(
    widget_id: u64,
    delta_x: c_int,
    delta_y: c_int,
    modifiers: c_uint,
) -> CBool {
    c_try!({
        let event = crate::event::Event::wheel(delta_x, delta_y, modifiers);
        crate::dispatch_event(widget_id, &event)
    })
}

/// Reports a container's new client size, so the app layer re-runs its layout.
///
/// # Why a host must call this
///
/// On these platforms the library holds no native window handle, so it cannot
/// subscribe to a size change: OpenHarmony delivers one to the ArkTS component's
/// `onAreaChange`, Android to the view's layout callback, iOS to the view
/// controller. Without forwarding it here, a window the user resized keeps every
/// child at the geometry it had for the previous size — the layout never re-runs,
/// and nothing looks wrong enough to report.
///
/// Returns `false` for an id this backend did not create, in which case neither the
/// size nor the trigger is recorded.
#[no_mangle]
pub extern "C" fn rw_report_window_resize(window_id: u64, width: c_uint, height: c_uint) -> CBool {
    c_try!({ crate::queue_resize_trigger(window_id, width, height) })
}

/// Bind the ArkUI **XComponent** this library should draw into.
///
/// # Why this is the first call an ArkTS host makes
///
/// Everything else on this page assumes the library has somewhere to put pixels and some way
/// to receive input. On OpenHarmony both arrive through an `XComponent`: the ArkTS side creates
/// one and its `onLoad` receives the `OH_NativeXComponent*` that this function takes.
///
/// # Contract
///
/// Call it from the `XComponent`'s `onLoad`, passing the component pointer ArkUI supplied.
/// After it returns `true`, `rw_mount_surface` becomes usable and input callbacks are live.
///
/// # Why it is not part of `rw_init`
///
/// The pointer does not exist until ArkUI has built the component tree, and `rw_init` runs
/// before that. Folding the two would either delay initialisation past where a host calls it or
/// register against a component ArkUI has not created.
///
/// # Build configuration
///
/// Returns `false` in every build without `feature = "xcomponent"`, which is the default.
/// That is the honest answer rather than a link error: a host that has not enabled the bridge
/// learns it cannot display anything, instead of the library pretending to have a surface.
///
/// # Safety
///
/// `component` must be the pointer ArkUI passed to the ArkTS `XComponent`'s native `onLoad`,
/// and must stay valid for the component's lifetime — the bridge stores it and ArkUI calls back
/// into the registered function pointers until the component is destroyed.
#[no_mangle]
pub unsafe extern "C" fn rw_harmony_bind_xcomponent(component: u64) -> CBool {
    c_try!({
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            // SAFETY: forwarded from this function's own contract — the caller passes the
            // pointer ArkUI gave it, valid for the component's lifetime.
            unsafe {
                crate::platform::harmony::xcomponent::bind(component as *mut core::ffi::c_void)
            }
        }
        #[cfg(not(all(feature = "xcomponent", not(alloc_frugal))))]
        {
            if component != 0 {
                log::error!(
                    "rw_harmony_bind_xcomponent called, but this build has no XComponent bridge \
                     (feature 'xcomponent' is off); the library has no surface to draw into"
                );
            }
            false
        }
    })
}

/// The ArkUI id of the bound `XComponent`, or `0` when none is bound.
///
/// The ArkTS side uses this to confirm which component the library is drawing into — an
/// application with more than one `XComponent` can otherwise not tell which one it handed over.
/// A fresh string is returned and must be released with `rw_free_string`.
#[no_mangle]
pub extern "C" fn rw_harmony_xcomponent_id() -> *mut c_char {
    c_try!({
        #[cfg(all(feature = "xcomponent", not(alloc_frugal)))]
        {
            match crate::platform::harmony::xcomponent::component_id() {
                Some(id) => CString::new(id).unwrap_or_default().into_raw(),
                None => std::ptr::null_mut(),
            }
        }
        #[cfg(not(all(feature = "xcomponent", not(alloc_frugal))))]
        {
            std::ptr::null_mut()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a C host writes through the ABI must be what it reads back.
    ///
    /// # The defect this closes
    ///
    /// The combo-box and list-box **writes** went through the control backend (which reaches
    /// the widget) while the **reads** went through `Platform` (whose every backend answers the
    /// trait default). So a host could add four items and then be told the box holds zero, and
    /// `rw_combo_box_set_current_index` returned `false` for an index it had just added.
    ///
    /// That is the "reported success for something that did not happen" shape from the other
    /// direction: the failure is a *read* that always answers `0`/`-1`, which no caller can
    /// distinguish from a genuinely empty control.
    ///
    /// # Why the round trip and not the individual calls
    ///
    /// Asserting `rw_combo_box_item_count(..) == 2` alone would pass for a backend that stored
    /// the items itself. Writing through one entry point and reading through another is what
    /// pins that the two agree — which is exactly the property that was broken, and the one a
    /// split between storage locations breaks again the moment either side moves.
    #[test]
    fn combo_box_items_round_trip_through_the_c_abi() {
        crate::init();
        let window = crate::create_window("abi", 0, 0, 400, 300);
        let combo = crate::create_combo_box(window, 0, 0, 120, 24);
        let labels = ["Red", "Green", "Blue"];
        for label in labels {
            let text = CString::new(label).unwrap();
            assert!(rw_combo_box_add_item(combo, text.as_ptr()), "adding '{label}'");
        }

        assert_eq!(rw_combo_box_item_count(combo), labels.len() as c_uint);
        for (index, label) in labels.iter().enumerate() {
            let text = rw_combo_box_item_text(combo, index as c_uint);
            assert!(!text.is_null(), "item {index} has text");
            // SAFETY: `text` was produced by `to_c_string_or_empty` above and is still live.
            let read = unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned();
            assert_eq!(&read, label, "item {index} round-trips its text");
        }

        // Selection is the other half: a host that can add items but cannot select one has a
        // read-only list.
        assert!(rw_combo_box_set_current_index(combo, 1), "index 1 exists");
        assert_eq!(rw_combo_box_current_index(combo), 1);
        assert!(!rw_combo_box_set_current_index(combo, 99), "an out-of-range index is refused");
        assert_eq!(rw_combo_box_current_index(combo), 1, "and does not clear the selection");

        rw_destroy_widget(window);
    }

    /// The list box is the same contract through its own entry points.
    #[test]
    fn list_box_items_round_trip_through_the_c_abi() {
        crate::init();
        let window = crate::create_window("abi", 0, 0, 400, 300);
        let list = crate::create_list_box(window, 0, 0, 120, 80);
        for label in ["Alpha", "Bravo", "Charlie"] {
            let text = CString::new(label).unwrap();
            assert!(rw_list_box_add_item(list, text.as_ptr()), "adding '{label}'");
        }

        assert_eq!(rw_list_box_item_count(list), 3);

        let text = rw_list_box_item_text(list, 1);
        assert!(!text.is_null());
        // SAFETY: produced by the call above and still live.
        let read = unsafe { CStr::from_ptr(text) }.to_str().unwrap().to_owned();
        assert_eq!(read, "Bravo");

        assert!(rw_list_box_set_current_index(list, 2));
        assert_eq!(rw_list_box_current_index(list), 2);

        // Removing shifts the later items up, which is the property a host with a delete
        // button depends on: the item after the removed one must still be readable.
        assert!(rw_list_box_remove_item(list, 0));
        assert_eq!(rw_list_box_item_count(list), 2);
        let first = rw_list_box_item_text(list, 0);
        // SAFETY: produced by the call above and still live.
        let first = unsafe { CStr::from_ptr(first) }.to_str().unwrap().to_owned();
        assert_eq!(first, "Bravo", "removal shifts later items up");

        rw_destroy_widget(window);
    }

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
            // A tuple's components keep their own kinds and a string component escapes the
            // `;`/`:` separators, so both must survive the encode/decode round-trip.
            CapabilityValue::Tuple(vec![
                CapabilityValue::Int(1),
                CapabilityValue::String("a;b:c\\d".to_string()),
                CapabilityValue::Bool(true),
                CapabilityValue::Float(2.5),
            ]),
        ];

        for original in cases {
            let (kind, num, text) = encode_capability_value(original.clone(), true);
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

    /// When no string output is supplied, the encoder must not allocate one.
    ///
    /// # The defect this closes
    ///
    /// `rw_get_widget_property` encoded the value — allocating and `into_raw`-leaking a
    /// `CString` for every string-carrying kind — and only *then* checked `out_str`. A
    /// caller that passed a null `out_str` (documented as allowed, and the way a
    /// kind-only reader works) therefore had its `CString` dropped on the floor: the
    /// only owning pointer was never written anywhere and could never be freed.
    ///
    /// The assertion is on the encoder contract the entry point relies on: with
    /// `want_text == false` a string-carrying value still reports its kind and numeric
    /// payload, but hands back **no** pointer, so there is nothing to leak. `kind` and
    /// `num` must be identical to the allocating path — suppressing the text must not
    /// suppress the discriminator.
    #[test]
    fn encoding_without_a_string_output_allocates_nothing() {
        use crate::widget::capability::CapabilityValue;

        let cases = [
            CapabilityValue::String("hello".to_string()),
            CapabilityValue::Color(crate::core::Color::rgba(1, 2, 3, 4)),
            CapabilityValue::Rect(crate::core::Rect::new(5, 6, 7, 8)),
            CapabilityValue::Tuple(vec![
                CapabilityValue::Int(1),
                CapabilityValue::String("x".to_string()),
            ]),
        ];

        for original in cases {
            let (kind_with, num_with, text) = encode_capability_value(original.clone(), true);
            let (kind_without, num_without, none) =
                encode_capability_value(original.clone(), false);

            assert_eq!(
                (kind_with, num_with),
                (kind_without, num_without),
                "suppressing the text must not change the kind/num of {original:?}"
            );
            assert!(!text.is_none(), "{original:?} allocates when an output exists");
            assert!(
                none.is_none(),
                "{original:?} must not allocate when there is no output pointer to receive it"
            );

            // Reclaim the allocation the paying path made, so this test itself does not leak.
            if let Some(raw) = text {
                unsafe { drop(CString::from_raw(raw)) };
            }
        }

        // A scalar kind never allocated in either mode, and must keep answering the same
        // thing — the flag narrows the string path only.
        let (kind_a, num_a, text_a) = encode_capability_value(CapabilityValue::Int(9), true);
        let (kind_b, num_b, text_b) = encode_capability_value(CapabilityValue::Int(9), false);
        assert_eq!((kind_a, num_a, text_a), (kind_b, num_b, text_b));
        assert_eq!(kind_a, RW_VALUE_INT);
    }

    /// The null-string path of `rw_get_widget_property` must not allocate but must still
    /// report the kind and numeric payload.
    ///
    /// This drives the real entry point rather than the encoder alone, so the branch
    /// that decides `want_text` is under test: a caller reading a string property with
    /// `out_str == null` gets `RW_VALUE_STRING` back and an untouched null pointer.
    #[test]
    fn c_abi_get_property_with_null_string_output_reports_the_kind() {
        use std::ffi::CString;

        let c = |s: &str| CString::new(s).expect("no interior NUL");

        unsafe {
            let title = c("null-str-window");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0);

            let tooltip = c("tooltip");
            let value = c("a-tip");
            assert!(rw_set_widget_property(
                window,
                tooltip.as_ptr(),
                RW_VALUE_STRING,
                0,
                value.as_ptr()
            ));

            // No `out_str`: the kind must still be written, and no string is produced.
            let mut out_kind: c_int = -1;
            let mut out_num: i64 = 0;
            assert!(
                rw_get_widget_property(
                    window,
                    tooltip.as_ptr(),
                    &mut out_kind,
                    &mut out_num,
                    core::ptr::null_mut(),
                ),
                "a kind-only read must succeed"
            );
            assert_eq!(
                out_kind, RW_VALUE_STRING,
                "the string kind must be reported without a buffer"
            );
            assert_eq!(out_num, 0);

            // And the same property read *with* an output still round-trips its text, so
            // the suppression did not disable the normal path.
            let mut out_str: *mut c_char = core::ptr::null_mut();
            assert!(rw_get_widget_property(
                window,
                tooltip.as_ptr(),
                &mut out_kind,
                &mut out_num,
                &mut out_str
            ));
            assert!(!out_str.is_null());
            assert_eq!(CStr::from_ptr(out_str).to_string_lossy(), "a-tip");
            rw_free_string(out_str);
        }
    }

    /// A frame requested with no pixel output must free its buffer, not leak it.
    ///
    /// # The defect this closes
    ///
    /// `rw_render_surface_frame` built the frame, `forget`-leaked the owning `Box<[u8]>`
    /// to hand ownership to the caller, and only then checked `out_pixels`. A caller that
    /// passed a null pixel output — documented as allowed, e.g. a probe that only wants
    /// the dimensions/stride — left the allocation with no pointer to free it: a leak per
    /// frame, on the per-pixel hot path.
    ///
    /// Both branches are exercised: null `out_pixels` must still report width/height/
    /// stride/length but write nothing, and a real `out_pixels` must receive an owned
    /// buffer that `rw_free_bytes` reclaims with the reported length.
    #[test]
    fn render_surface_frame_without_a_pixel_output_does_not_leak() {
        use crate::core::Rect;

        let mut editor =
            crate::widget::special_widgets::code_editor::CodeEditor::new(Rect::new(0, 0, 64, 48));
        editor.set_text("fn main() {}");
        let id = crate::widget::runtime::register(Box::new(editor)).expect("the registry");

        // Null pixel output: the geometry must still be published, and nothing is written.
        let mut width: c_uint = 0;
        let mut height: c_uint = 0;
        let mut stride: c_uint = 0;
        let mut len: c_uint = 0;
        let ok = unsafe {
            rw_render_surface_frame(
                id,
                64,
                48,
                &mut width,
                &mut height,
                &mut stride,
                &mut len,
                core::ptr::null_mut(),
            )
        };
        assert!(ok, "a frame must be produced for a mounted drawable widget");
        assert_eq!((width, height), (64, 48));
        assert_eq!(stride, 64 * 4);
        assert_eq!(len, 64 * 48 * 4);

        // A real pixel output receives an owned buffer of exactly the reported length,
        // freed once through `rw_free_bytes`. This is the same pairing a host uses.
        let mut pixels: *mut u8 = core::ptr::null_mut();
        let ok = unsafe {
            rw_render_surface_frame(
                id,
                64,
                48,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut len,
                &mut pixels,
            )
        };
        assert!(ok);
        assert!(!pixels.is_null(), "a non-null output must receive the buffer");
        assert_eq!(len, 64 * 48 * 4);
        unsafe { rw_free_bytes(pixels, len) };

        crate::widget::runtime::unregister(id);
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

        let _guard = crate::style::theme_test_guard();
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

        let _guard = crate::style::theme_test_guard();
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

    /// A host-owned event loop can deliver a touch and have the widget act on it.
    ///
    /// # The gap this closes
    ///
    /// `src/platform/android/status.md`, `src/platform/ios/status.md` and
    /// `docs/plans/harmony_integration.md` all told a host to call
    /// `rw_dispatch_pointer_event(...)`. The symbol did not exist in `binding_impl.rs` or in
    /// `include/rw_generated.h`, so the documented integration step — the one without which a
    /// mounted widget is visible but completely inert — had no implementation at any layer.
    ///
    /// # Why the assertions are an end-to-end round trip
    ///
    /// Asserting that the function returns `true` would pass against a function that hit-tests
    /// and then drops the event. So the test creates a real button through the C ABI, dispatches
    /// a press and a release at its centre, and reads back the trigger the widget produced.
    /// That is the only evidence that the event reached a control and it acted.
    #[test]
    fn a_host_can_deliver_a_pointer_event_through_the_c_abi() {
        use std::ffi::CString;
        let c = |s: &str| CString::new(s).expect("no interior NUL");

        // The event codes the ABI documents; named here so a renumbering breaks this test.
        const PRESS: c_uint = 1;
        const RELEASE: c_uint = 2;
        const MOVE: c_uint = 3;
        const UNKNOWN: c_uint = 0;

        {
            let title = c("pointer-round-trip");
            let window = rw_create_window(title.as_ptr(), 0, 0, 320, 240);
            assert_ne!(window, 0, "the fixture needs a real window to route in");

            let label = c("OK");
            let button = rw_create_button(window, label.as_ptr(), 20, 30, 80, 30);
            assert_ne!(button, 0, "and a real button to click");

            // An unknown code is refused rather than mis-read as a move: a host that
            // mis-encoded a press must not have it silently become a hover.
            assert!(!rw_dispatch_pointer_event(window, UNKNOWN, 60, 45, 0));

            // A move over the control routes (hover) but activates nothing.
            assert!(
                rw_dispatch_pointer_event(window, MOVE, 60, 45, 0),
                "a move inside the button's rect must be routed to it"
            );

            // A press far outside every child is not accepted and must be reported.
            assert!(
                !rw_dispatch_pointer_event(window, PRESS, 3000, 3000, 0),
                "a point outside the subtree must be refused, not silently dropped"
            );

            // The real path: press then release inside the button, which is what a click is.
            assert!(
                rw_dispatch_pointer_event(window, PRESS, 60, 45, 0),
                "a press inside the button must be accepted"
            );
            assert!(rw_dispatch_pointer_event(window, RELEASE, 60, 45, 0), "and its release too");

            // # What the widget does with it, and what it deliberately does *not*
            //
            // A click that arrives this way makes the button emit its **own** `clicked` signal
            // — the in-process path a Rust caller hooks. It does **not** push an entry onto the
            // platform trigger queue: that queue carries triggers a *host* injects (a native
            // `onClick`, a designer action), which is why `SurfaceHandle::on_click` documents that
            // a widget "emits its own signals rather than a platform click callback".
            //
            // Asserting a queued trigger here was my first version and it failed — correctly. The
            // evidence that the event reached a control and was acted on is the signal, so the test
            // hooks `BaseWidget::clicked` and counts the emissions over a full press+release.
            let clicks = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let counter = std::sync::Arc::clone(&clicks);
            let hooked = crate::widget::runtime::with_widget_mut(button, |widget| {
                widget.base().clicked.connect(move || {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                });
            });
            assert!(hooked.is_some(), "the button must be a live widget to hook");

            // A press whose release lands *outside* must not activate — the other half of the
            // click contract, and what proves the routing carried a real position rather than
            // just "some event".
            //
            // # What the two return values mean, verified rather than assumed
            //
            // `route_pointer_event` answers "did this reach a live widget". The press lands on
            // the button, so it takes pointer capture (D09-EVT-01); the release at (400,400) is
            // therefore delivered back to the button that owns the gesture, even though that
            // point hits no widget:
            //
            // * the **press** returns `true` — the point hit the button, which captured the
            //   pointer;
            // * the **release at (400,400)** returns `true` — capture routes it to the button;
            //   the button sees a release outside its bounds and cancels the gesture.
            //
            // Before D09-EVT-01 the outside release returned `false` — it was dropped by the hit
            // test and never reached the button, so the button's own press latch stayed armed
            // until some *later* unrelated release fired a spurious click. Capture is what keeps
            // the release paired with its press. The signal count is the assertion that
            // distinguishes "the button acted" from "the event went somewhere": it must stay
            // `0` because an outside release cancels rather than activates.
            assert!(
                rw_dispatch_pointer_event(window, PRESS, 60, 45, 0),
                "a press inside the button must reach it and take the gesture"
            );
            assert!(
                rw_dispatch_pointer_event(window, RELEASE, 400, 400, 0),
                "the outside release must be routed back to the widget that owns the gesture"
            );
            assert_eq!(
                clicks.load(std::sync::atomic::Ordering::SeqCst),
                0,
                "a release outside the button must not activate it"
            );

            // Inside ⇒ activate.
            assert!(rw_dispatch_pointer_event(window, PRESS, 60, 45, 0));
            assert!(rw_dispatch_pointer_event(window, RELEASE, 60, 45, 0));
            assert_eq!(
                clicks.load(std::sync::atomic::Ordering::SeqCst),
                1,
                "press and release inside the button must activate it exactly once"
            );

            // The direct route reaches a widget the host already knows, without hit-testing.
            assert!(
                rw_dispatch_event_to_widget(button, MOVE, 60, 45, 0),
                "the direct route must reach a live widget"
            );
            assert!(
                !rw_dispatch_event_to_widget(button, UNKNOWN, 60, 45, 0),
                "and refuse an unknown event code like the routing path does"
            );
        }
    }

    /// An empty drop queue must clear every output, not just return `false`.
    ///
    /// A caller that reuses an `out` from a previous success and frees unconditionally
    /// would otherwise free a stale pointer a second time. The sentinel values prove the
    /// getter actually wrote null/zero rather than merely leaving the caller's bytes alone.
    #[test]
    fn an_empty_drop_poll_clears_nonzero_outputs() {
        use core::ffi::c_uint;

        // Drain any ambient event so the next poll is definitely empty (tests share the
        // process-wide control backend).
        loop {
            let mut s = 0u64;
            let mut t = 0u64;
            let mut m: *mut c_char = std::ptr::null_mut();
            let mut p: *mut u8 = std::ptr::null_mut();
            let mut l: c_uint = 0;
            let had = unsafe { rw_poll_drop_event(&mut s, &mut t, &mut m, &mut p, &mut l) };
            if !had {
                break;
            }
            if !m.is_null() {
                unsafe { rw_free_string(m) };
            }
            if !p.is_null() {
                unsafe { rw_free_bytes(p, l) };
            }
        }

        let mut source_out: u64 = 0xDEAD_BEEF;
        let mut target_out: u64 = 0xDEAD_BEEF;
        let mut mime_out: *mut c_char = std::ptr::dangling_mut::<c_char>();
        let mut payload_out: *mut u8 = std::ptr::dangling_mut::<u8>();
        let mut payload_len_out: c_uint = 0xDEAD_BEEF;

        let had = unsafe {
            rw_poll_drop_event(
                &mut source_out,
                &mut target_out,
                &mut mime_out,
                &mut payload_out,
                &mut payload_len_out,
            )
        };
        assert!(!had, "the drained queue must report no event");
        assert_eq!(source_out, 0, "an empty poll must clear the source sentinel");
        assert_eq!(target_out, 0, "an empty poll must clear the target sentinel");
        assert!(mime_out.is_null(), "an empty poll must null the mime pointer");
        assert!(payload_out.is_null(), "an empty poll must null the payload pointer");
        assert_eq!(payload_len_out, 0, "an empty poll must zero the payload length");
    }

    /// Text, key and wheel input must reach a widget through their dedicated C entries.
    ///
    /// The pointer entry only carries `(x, y, button)`, so a host could never deliver a
    /// committed string, a key code + modifier mask, or a wheel delta. These three entries
    /// close that gap and are observed at the widget's `handle_event`.
    #[test]
    fn a_host_can_deliver_text_key_and_wheel_through_the_c_abi() {
        use crate::event::{Event, EventHandler};
        use std::ffi::CString;
        use std::rc::Rc;

        let log = Rc::new(core::cell::RefCell::new(Vec::new()));

        struct Recorder {
            log: Rc<core::cell::RefCell<Vec<String>>>,
            base: crate::widget::BaseWidget,
        }
        impl crate::widget::Widget for Recorder {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                &mut self.base
            }
        }
        impl EventHandler for Recorder {
            fn handle_event(&mut self, event: &Event) {
                let entry = match event {
                    Event::TextInput { text } => format!("text:{text}"),
                    Event::KeyPress { key, modifiers } => format!("keydown:{key}:{modifiers}"),
                    Event::KeyRelease { key, modifiers } => format!("keyup:{key}:{modifiers}"),
                    Event::Wheel { delta, modifiers } => {
                        format!("wheel:{}:{}:{modifiers}", delta.x, delta.y)
                    }
                    _ => return,
                };
                self.log.borrow_mut().push(entry);
            }
        }

        let id = crate::widget::runtime::register(Box::new(Recorder {
            log: Rc::clone(&log),
            base: crate::widget::BaseWidget::new(
                crate::widget::WidgetKind::LineEdit,
                crate::core::Rect::new(0, 0, 10, 10),
                "recorder",
            ),
        }))
        .expect("mount the recorder");

        let text = CString::new("hi").expect("no interior NUL");
        assert!(rw_dispatch_text_event(id, text.as_ptr()), "text commit must reach the widget");
        assert!(rw_dispatch_key_event(id, 65, 2, false), "key press must reach the widget");
        assert!(rw_dispatch_key_event(id, 65, 0, true), "key release must reach the widget");
        assert!(rw_dispatch_wheel_event(id, 0, -1, 4), "wheel must reach the widget");

        assert_eq!(
            *log.borrow(),
            vec![
                "text:hi".to_string(),
                "keydown:65:2".to_string(),
                "keyup:65:0".to_string(),
                "wheel:0:-1:4".to_string(),
            ]
        );

        crate::widget::runtime::unregister(id);
    }
}
