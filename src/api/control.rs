//! The crate-root procedural control API: widget state, event queues, menus,
//! chrome, drag/drop and IME.

#[cfg(full_widgets)]
use crate::app;
#[cfg(all(not(alloc_frugal), widgets_unstripped))]
use crate::backend_for_kind;
#[cfg(not(alloc_frugal))]
use crate::platform::{DropEvent, WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(full_widgets)]
use crate::widget::widget_trait::Widget;
#[cfg(all(not(alloc_frugal), not(widgets_unstripped)))]
use crate::NO_SUCH_CONTROL;
#[cfg(not(alloc_frugal))]
use crate::{control_backend, platform, widget};

/// Show a widget by its object id.
///
/// Routed through the control backend, which is the one mechanism that owns the
/// control (BLUE15 #55): the platform supplies the surface, not the control.
#[cfg(not(alloc_frugal))]
pub fn show_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, true);
}
/// Hide a widget by its object id.
#[cfg(not(alloc_frugal))]
pub fn hide_widget(widget_id: crate::core::ObjectId) {
    control_backend::get_control_backend().set_widget_visible(widget_id, false);
}

/// Announce that a widget was **closed**, by emitting its
/// [`BaseWidget::closed`](crate::widget::BaseWidget::closed) signal **and** its published `closed`
/// signal when the control declares one.
///
/// # Why this is the join
///
/// A closeable control's own `close`/`dismiss` path and the handle layer's `close` are two entry
/// points to one lifecycle fact. Both must reach the same signal, or a handler registered through
/// one route misses a close produced by the other.
///
/// # Why both signals are emitted
///
/// A control may publish `closed` through a signal of its own (`Window::closed`) in addition to the
/// base one. The JSON/designer route binds `events:{"closed":…}` to the control's **published**
/// signal via `event_signal_dyn`, while the handle layer's `on_close` connects to `base.closed`. If
/// this generic close path emitted only one of them, whichever route used the other would stay
/// silent — which is exactly how a `closed` handler came to fire for one route and not the other.
/// Emitting both means either listener fires from whichever path closed the control.
///
/// Returns `true` when a mounted widget received it. A `false` means the id addresses no live
/// widget — the same honest answer [`hide_widget`] gives by doing nothing observable.
#[cfg(not(alloc_frugal))]
pub fn close_widget(widget_id: crate::core::ObjectId) -> bool {
    let mounted = crate::widget::runtime::with_widget_mut(widget_id, |widget| {
        widget.base().closed.emit();
        // The published `closed`, when it is a signal of the control's own rather than the base one
        // already emitted above. `unit` refs over `base.closed` are the same signal, so re-emitting
        // would just run the base slots twice; comparing identities is not possible through the
        // erased ref, so this relies on each control publishing a *distinct* field (as `Window`
        // does). A control whose published `closed` is already `base.closed` is unaffected by the
        // extra emit because `Signal::emit` on the same signal during its own emit pass skips the
        // slots already on the stack (see `Signal::emit`'s re-entrancy rule).
        if let Some(reference) = widget.event_signal_dyn("closed") {
            reference.emit_unit();
        }
    })
    .is_some();
    if mounted {
        // This is the close announcement every platform funnels through (`crate::close_widget` is
        // the one path its own comments name), yet only the Linux backend also injected
        // `WidgetTriggerKind::Closed` — so a window closed by `WM_CLOSE`, `windowShouldClose:`
        // or a programmatic `WindowHandle::close` left the handle-side callback tables (and the
        // values they capture) alive for the rest of the process. Cleaning them here gives every
        // close path the same teardown semantics; the Linux trigger path is idempotent
        // (LYT-X06).
        #[cfg(full_widgets)]
        crate::app::remove_callbacks(widget_id);
    }
    mounted
}
/// Set geometry of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_geometry(
    widget_id: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    control_backend::get_control_backend().set_widget_geometry(widget_id, x, y, width, height);
}
/// Read a widget's rectangle, or `None` when the id addresses nothing.
#[cfg(not(alloc_frugal))]
pub fn widget_geometry(widget_id: crate::core::ObjectId) -> Option<(i32, i32, u32, u32)> {
    control_backend::get_control_backend().get_widget_geometry(widget_id)
}
/// Set text of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_text(widget_id: crate::core::ObjectId, text: &str) {
    control_backend::get_control_backend().set_widget_text(widget_id, text);
}
/// Get text of a widget.
#[cfg(not(alloc_frugal))]
pub fn get_widget_text(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_text(widget_id)
}
/// Set enabled state of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_enabled(widget_id: crate::core::ObjectId, enabled: bool) {
    control_backend::get_control_backend().set_widget_enabled(widget_id, enabled);
}
/// Check if a widget is enabled.
#[cfg(not(alloc_frugal))]
pub fn is_widget_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_enabled(widget_id)
}
/// Set visibility of a widget.
#[cfg(not(alloc_frugal))]
pub fn set_widget_visible(widget_id: crate::core::ObjectId, visible: bool) {
    control_backend::get_control_backend().set_widget_visible(widget_id, visible);
    // Hiding a modal releases the scrim plane so the modal below it (or the page) takes over
    // (DLG-001). The call is a no-op for an id that never claimed the plane. Gated because the
    // dialog layer only exists in a build with the full widget set.
    #[cfg(full_widgets)]
    if !visible {
        crate::widget::dialog::pop_modal_layer(widget_id);
    }
}
/// Check if a widget is visible.
#[cfg(not(alloc_frugal))]
pub fn is_widget_visible(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_visible(widget_id)
}

/// Set a widget's primary numeric value (slider, progress bar, spin box, ...).
///
/// Returns `false` when the widget exposes no numeric value, so callers never
/// mistake "unsupported" for "set to 0".
#[cfg(not(alloc_frugal))]
pub fn set_widget_value(widget_id: crate::core::ObjectId, value: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["value"], value)
}

/// Read a widget's primary numeric value.
#[cfg(not(alloc_frugal))]
pub fn widget_value(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["value"])
}

/// Set a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn set_widget_range(widget_id: crate::core::ObjectId, min: f64, max: f64) -> bool {
    let names: &[&str] = &["minimum", "min", "min_value"];
    let max_names: &[&str] = &["maximum", "max", "max_value"];
    widget::capability::widget_access::write_number(widget_id, names, min)
        && widget::capability::widget_access::write_number(widget_id, max_names, max)
}

/// Read a widget's `(min, max)` range.
#[cfg(not(alloc_frugal))]
pub fn widget_range(widget_id: crate::core::ObjectId) -> Option<(f64, f64)> {
    let min = widget::capability::widget_access::read_number(
        widget_id,
        &["minimum", "min", "min_value"],
    )?;
    let max = widget::capability::widget_access::read_number(
        widget_id,
        &["maximum", "max", "max_value"],
    )?;
    Some((min, max))
}

/// Set a widget's selection index (combo box, list box, tab widget).
///
/// `None` clears the selection, which is what these controls store as `Null` —
/// distinct from selecting index 0.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selected_index(widget_id: crate::core::ObjectId, index: Option<usize>) -> bool {
    let names: &[&str] = &["selected_index", "current_index", "current_row", "active_index"];
    let value = match index {
        Some(index) => crate::widget::capability::CapabilityValue::UInt(index as u64),
        None => crate::widget::capability::CapabilityValue::Null,
    };
    widget::capability::widget_access::write_first(widget_id, names, &value)
}

/// Read a widget's selection index.
#[cfg(not(alloc_frugal))]
pub fn widget_selected_index(widget_id: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(
        widget_id,
        &["selected_index", "current_index", "current_row", "active_index"],
    )
}

/// Set a widget's checked state (check box, radio button, toggle button).
#[cfg(not(alloc_frugal))]
pub fn set_widget_checked(widget_id: crate::core::ObjectId, checked: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["checked"],
        &crate::widget::capability::CapabilityValue::Bool(checked),
    )
}

/// Read a widget's checked state, or `None` when it is not checkable.
#[cfg(not(alloc_frugal))]
pub fn is_widget_checked(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["checked"])
}

/// Set a widget's increment step (slider, spin box, scroll bar).
#[cfg(not(alloc_frugal))]
pub fn set_widget_step(widget_id: crate::core::ObjectId, step: f64) -> bool {
    widget::capability::widget_access::write_number(widget_id, &["single_step", "step"], step)
}

/// Read a widget's increment step.
#[cfg(not(alloc_frugal))]
pub fn widget_step(widget_id: crate::core::ObjectId) -> Option<f64> {
    widget::capability::widget_access::read_number(widget_id, &["single_step", "step"])
}

/// Set a progress-style widget's indeterminate (busy) state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_indeterminate(widget_id: crate::core::ObjectId, indeterminate: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["indeterminate"],
        &crate::widget::capability::CapabilityValue::Bool(indeterminate),
    )
}

/// Read a progress-style widget's indeterminate state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_indeterminate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["indeterminate"])
}

/// Set a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn set_widget_read_only(widget_id: crate::core::ObjectId, read_only: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["read_only"],
        &crate::widget::capability::CapabilityValue::Bool(read_only),
    )
}

/// Read a text-entry widget's read-only state.
#[cfg(not(alloc_frugal))]
pub fn is_widget_read_only(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["read_only"])
}

/// Set a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn set_widget_max_length(widget_id: crate::core::ObjectId, max_length: u32) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["max_length"],
        &crate::widget::capability::CapabilityValue::UInt(max_length as u64),
    )
}

/// Read a text-entry widget's maximum accepted length.
#[cfg(not(alloc_frugal))]
pub fn widget_max_length(widget_id: crate::core::ObjectId) -> Option<u32> {
    widget::capability::widget_access::read_index(widget_id, &["max_length"])
        .and_then(|v| u32::try_from(v).ok())
}

/// Apply or clear a window state (maximised, minimised, full-screen, ...).
///
/// Returns `false` when the id is not a window or the backend cannot honour the
/// state on its toolkit.
#[cfg(not(alloc_frugal))]
pub fn set_window_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
    on: bool,
) -> bool {
    platform::get_platform().set_window_state(widget_id, flag, on)
}

/// Read a window state, or `None` when the id is not a window.
#[cfg(not(alloc_frugal))]
pub fn is_window_in_state(
    widget_id: crate::core::ObjectId,
    flag: platform::WindowStateFlag,
) -> Option<bool> {
    platform::get_platform().is_window_in_state(widget_id, flag)
}

/// Set a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn set_window_min_size(widget_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    platform::get_platform().set_window_min_size(widget_id, width, height)
}

/// Read a window's minimum content size.
#[cfg(not(alloc_frugal))]
pub fn window_min_size(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    platform::get_platform().window_min_size(widget_id)
}

/// Set a window's icon from a file path.
#[cfg(not(alloc_frugal))]
pub fn set_window_icon(widget_id: crate::core::ObjectId, path: &str) -> bool {
    platform::get_platform().set_window_icon(widget_id, path)
}

/// Read a window's icon path, if one was set.
#[cfg(not(alloc_frugal))]
pub fn window_icon(widget_id: crate::core::ObjectId) -> Option<String> {
    platform::get_platform().window_icon(widget_id)
}

/// Set a text entry's selection range as `(start, end)` character offsets.
///
/// # Current support
///
/// No control publishes a selection range as a property (the text controls track
/// it internally and expose `cursor_position`), so this reports `false` rather
/// than pretending the range was applied. Routing it through the property
/// contract means publishing the property on the text controls is the only change
/// needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_selection(widget_id: crate::core::ObjectId, start: u32, end: u32) -> bool {
    let names: &[&str] = &["selection"];
    // The pair travels as the historical "start,end" spelling used by the text
    // controls' own setter, so a control that publishes `selection` parses one
    // string rather than needing two properties kept in step.
    widget::capability::widget_access::write_first(
        widget_id,
        names,
        &crate::widget::capability::CapabilityValue::String(format!("{start},{end}")),
    )
}

/// Read a text entry's selection range, or `None` when nothing is selected.
#[cfg(not(alloc_frugal))]
pub fn widget_selection(widget_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let raw = widget::capability::widget_access::read_text(widget_id, &["selection"])?;
    let (start, end) = raw.split_once(',')?;
    Some((start.trim().parse().ok()?, end.trim().parse().ok()?))
}

/// Set a text entry's placeholder (cue) text.
#[cfg(not(alloc_frugal))]
pub fn set_widget_placeholder(widget_id: crate::core::ObjectId, text: &str) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["placeholder_text", "placeholder"],
        &crate::widget::capability::CapabilityValue::String(text.to_string()),
    )
}

/// Read a text entry's placeholder text.
#[cfg(not(alloc_frugal))]
pub fn widget_placeholder(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["placeholder_text", "placeholder"])
}

/// Set a text entry's echo mode.
///
/// # Current support
///
/// No control publishes an echo property yet, so this reports `false` on every
/// widget rather than pretending the mode was applied. It is routed through the
/// property contract — the place a control would express it — so that adding the
/// property to the text controls is the only change needed to make it work.
#[cfg(not(alloc_frugal))]
pub fn set_widget_echo_mode(widget_id: crate::core::ObjectId, mode: platform::EchoMode) -> bool {
    let token = match mode {
        platform::EchoMode::Normal => "normal",
        platform::EchoMode::Password => "password",
        platform::EchoMode::NoEcho => "no_echo",
    };
    widget::capability::widget_access::write_first(
        widget_id,
        &["echo_mode"],
        &crate::widget::capability::CapabilityValue::String(token.to_string()),
    )
}

/// Read a text entry's echo mode.
#[cfg(not(alloc_frugal))]
pub fn widget_echo_mode(widget_id: crate::core::ObjectId) -> Option<platform::EchoMode> {
    match widget::capability::widget_access::read_text(widget_id, &["echo_mode"])?.as_str() {
        "normal" => Some(platform::EchoMode::Normal),
        "password" => Some(platform::EchoMode::Password),
        "no_echo" => Some(platform::EchoMode::NoEcho),
        _ => None,
    }
}

/// Apply a slider's creation-time orientation.
#[cfg(not(alloc_frugal))]
pub fn set_slider_orientation(
    widget_id: crate::core::ObjectId,
    orientation: crate::core::Orientation,
) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["orientation"],
        &crate::widget::capability::CapabilityValue::String(
            widget::capability::orientation_to_str(orientation).to_string(),
        ),
    )
}

/// Read a slider's orientation.
#[cfg(not(alloc_frugal))]
pub fn slider_orientation(widget_id: crate::core::ObjectId) -> Option<crate::core::Orientation> {
    let token = widget::capability::widget_access::read_text(widget_id, &["orientation"])?;
    widget::capability::expect_orientation(crate::widget::capability::CapabilityValue::String(
        token,
    ))
    .ok()
}

/// Set a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn set_widget_tristate(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    widget::capability::widget_access::write_first(
        widget_id,
        &["tristate_enabled"],
        &crate::widget::capability::CapabilityValue::Bool(enabled),
    )
}

/// Read a checkable control's tri-state mode.
#[cfg(not(alloc_frugal))]
pub fn is_widget_tristate(widget_id: crate::core::ObjectId) -> Option<bool> {
    widget::capability::widget_access::read_flag(widget_id, &["tristate_enabled"])
}

/// Put a radio button into a named mutually-exclusive group.
///
/// An empty group clears the membership, which these controls store as `Null`.
#[cfg(not(alloc_frugal))]
pub fn set_widget_group(widget_id: crate::core::ObjectId, group: &str) -> bool {
    let value = if group.is_empty() {
        crate::widget::capability::CapabilityValue::Null
    } else {
        crate::widget::capability::CapabilityValue::String(group.to_string())
    };
    widget::capability::widget_access::write_first(widget_id, &["group_id", "group"], &value)
}

/// Read a radio button's group name.
#[cfg(not(alloc_frugal))]
pub fn widget_group(widget_id: crate::core::ObjectId) -> Option<String> {
    widget::capability::widget_access::read_text(widget_id, &["group_id", "group"])
}

/// Set a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn set_widget_scroll_position(widget_id: crate::core::ObjectId, x: i32, y: i32) -> bool {
    let x_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_x"],
        x as f64,
    );
    let y_ok = widget::capability::widget_access::write_number(
        widget_id,
        &["scroll_position_y"],
        y as f64,
    );
    x_ok && y_ok
}

/// Read a scrollable container's scroll offset.
#[cfg(not(alloc_frugal))]
pub fn widget_scroll_position(widget_id: crate::core::ObjectId) -> Option<(i32, i32)> {
    let x = widget::capability::widget_access::read_number(widget_id, &["scroll_position_x"])?;
    let y = widget::capability::widget_access::read_number(widget_id, &["scroll_position_y"])?;
    Some((x as i32, y as i32))
}
// ComboBox operations
/// Appends an item with the given text to a combo box.
///
/// Returns `false` when the backend refuses — which includes being handed an id
/// that is not a combo box.
#[cfg(not(alloc_frugal))]
pub fn combo_box_add_item(combo_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().combo_box_add_item(combo_box, text)
}
/// Removes every item from a combo box, leaving it empty and with nothing
/// selected. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn combo_box_clear_items(combo_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().combo_box_clear_items(combo_box)
}
/// Selects the combo box item at `index`.
///
/// Routed through the property contract as the `current_index` property, so this
/// reports `false` when the widget does not publish that property. An
/// out-of-range `index` is **not** reliably reported as a failure — read
/// [`combo_box_current_index`] back to confirm what took effect.
#[cfg(not(alloc_frugal))]
pub fn combo_box_set_current_index(combo_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        combo_box,
        &["current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The combo box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_current_index(combo_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(combo_box, &["current_index"])
}
/// How many items a combo box holds.
///
/// Reports `0` for an unknown id as well as for a genuinely empty list, so it
/// cannot be used to tell "empty" from "not a combo box".
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_count(combo_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(combo_box, &["item_count"]).unwrap_or(0)
}
/// The text of the combo box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn combo_box_item_text(combo_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().combo_box_item_text(combo_box, index)
}
// ListBox operations
/// Appends an item with the given text to a list box. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_add_item(list_box: crate::core::ObjectId, text: &str) -> bool {
    control_backend::get_control_backend().list_box_add_item(list_box, text)
}
/// Removes the list box item at `index`, shifting later items down.
///
/// Returns `false` when the backend refuses. As with the combo box, an
/// out-of-range index is not reliably distinguished from a refusal — re-read
/// [`list_box_item_count`] to check.
#[cfg(not(alloc_frugal))]
pub fn list_box_remove_item(list_box: crate::core::ObjectId, index: usize) -> bool {
    control_backend::get_control_backend().list_box_remove_item(list_box, index)
}
/// Removes every item from a list box. Returns `false` when the backend refuses.
#[cfg(not(alloc_frugal))]
pub fn list_box_clear_items(list_box: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().list_box_clear_items(list_box)
}
/// Selects the list box item at `index`.
///
/// Tries the `current_row` property first and falls back to `current_index`,
/// because both spellings are published by list controls in this library.
/// Returns `false` when neither is published; an out-of-range `index` is not
/// reliably reported, so read [`list_box_current_index`] back to confirm.
#[cfg(not(alloc_frugal))]
pub fn list_box_set_current_index(list_box: crate::core::ObjectId, index: usize) -> bool {
    widget::capability::widget_access::write_first(
        list_box,
        &["current_row", "current_index"],
        &crate::widget::capability::CapabilityValue::UInt(index as u64),
    )
}
/// The list box's selected index, or `None` when nothing is selected or the id
/// is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_current_index(list_box: crate::core::ObjectId) -> Option<usize> {
    widget::capability::widget_access::read_index(list_box, &["current_row", "current_index"])
}
/// How many items a list box holds. Reports `0` for an unknown id as well as for
/// an empty list.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_count(list_box: crate::core::ObjectId) -> usize {
    widget::capability::widget_access::read_index(list_box, &["item_count"]).unwrap_or(0)
}
/// The text of the list box item at `index`, or `None` when the index is out of
/// range or the id is unknown.
#[cfg(not(alloc_frugal))]
pub fn list_box_item_text(list_box: crate::core::ObjectId, index: usize) -> Option<String> {
    control_backend::get_control_backend().list_box_item_text(list_box, index)
}
// Event polling
/// Takes the id of the next widget that has been activated, or `None` when no
/// activation is queued.
///
/// This is a queue, not a state: each successful call removes one event, so a
/// loop calling it until `None` drains the pending activations. Use
/// [`poll_widget_trigger_event`] when the kind of activation matters.
#[cfg(not(alloc_frugal))]
pub fn poll_widget_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_widget_triggered()
}
/// Takes the next queued widget activation together with what kind it was, or
/// `None` when the queue is empty. Drains the same queue as
/// [`poll_widget_triggered`].
#[cfg(not(alloc_frugal))]
pub fn poll_widget_trigger_event() -> Option<WidgetTriggerEvent> {
    control_backend::get_control_backend().poll_widget_trigger_event()
}

/// Blocks the calling thread until the framework signals work, or `timeout` elapses.
///
/// # Why hosts need this
///
/// The trigger queues are pull-based ([`poll_menu_triggered`],
/// [`poll_widget_trigger_event`]), and a host that drains them used to sleep a fixed
/// interval between polls. That cost up to one interval of latency and woke the thread
/// at that rate even while the window was still. Enqueuing a trigger (and every event
/// post) now wakes the framework's wait channel, so a host can block here and be
/// scheduled as soon as work exists. The timeout is the backstop for state that has no
/// wake source, such as a client-size change observed by polling.
///
/// Returns `true` when a wake was observed, `false` on timeout.
#[cfg(not(alloc_frugal))]
pub fn wait_for_host_events(timeout: std::time::Duration) -> bool {
    let observed = platform::frame_wait::generation();
    platform::frame_wait::wait_after(observed, timeout);
    platform::frame_wait::generation() != observed
}

/// Delivers every queued widget trigger, in order, until the queue is empty.
///
/// Returns the number of events consumed.
///
/// # Why this lives at the crate root rather than in `crate::app`
///
/// Every platform backend's event loop needs to call it — GTK's timeout, the Win32
/// message pass, Wayland's idle step, Android's polling loop — and `crate::app` is gated
/// `full_widgets` while the trigger queue is available to every profile. A backend that
/// called `crate::app::drain_triggers` therefore compiled on `desktop` and failed on
/// `mini` with `cannot find 'app' in 'crate'`, which is the same class of defect as a call
/// into a gated `create_*`: one unpicked profile hides it completely
/// (`tools/check_profiles.sh` runs the profile matrix; a `desktop` build never sees it).
///
/// The trigger queue is not a widget-set feature — it is how a backend reports "the user
/// resized the window" — so the drain sits beside the queue it drains, at the widest gate
/// its dependency has.
///
/// # Why it is defined in every profile, not gated
///
/// The queue exists in every profile except `alloc_frugal` (`mini`). Gating the function
/// would mean gating each of the **eleven** backend call sites, and a gate that has to be
/// repeated eleven times is a gate that will be forgotten in one of them — which is
/// exactly how this function came to be missing from the crate root in the first place.
/// Defining it unconditionally moves the condition to one place, and `alloc_frugal`
/// answers `0` because there is genuinely nothing to drain there.
///
/// # Termination
///
/// Drains until the queue reports empty, so a tick's cost is bounded by the number of
/// events that arrived since the last tick. A backend that produced events faster than it
/// consumed them would starve its own loop, which is why the queue is drained to empty
/// rather than to a fixed count: "drained" is the only state from which the next tick's
/// backlog is knowable.
/// Dispatches **every** queued trigger belonging to `widget_id`, and leaves other widgets' queued.
///
/// # Why this exists next to [`drain_triggers`]
///
/// [`drain_triggers`] is the host's drain: it takes the front of the **process-wide** queue until it is
/// empty, which is what a single-threaded event loop wants. A test wants something narrower -- it
/// drives one widget and needs that widget's events delivered without its result depending on what
/// another test queued first. The queue is process-wide and the test runner shares a thread pool, so
/// `drain_triggers` gives no such guarantee: a test can dispatch dozens of other tests' events and
/// never reach its own.
///
/// This removes and dispatches only the events whose `widget_id` matches, so it neither consumes nor
/// discards anyone else's work, and it preserves the order among one widget's own events. It is one
/// widget's share of the same drain, not a second implementation of it: the dispatch call is the one
/// [`drain_triggers`] makes, and the queue operation is the targeted pop beside the FIFO one.
///
/// Returns how many events it dispatched.
#[cfg(not(alloc_frugal))]
pub fn drain_widget_triggers_for(widget_id: crate::core::ObjectId) -> usize {
    let mut dispatched = 0usize;
    while let Some(event) =
        control_backend::get_control_backend().pop_widget_trigger_event_for(widget_id)
    {
        #[cfg(full_widgets)]
        {
            app::dispatch_trigger(event.widget_id, event.kind);
        }
        #[cfg(not(full_widgets))]
        {
            // No router in this profile. Popping it above is what removes it from the queue; naming
            // it keeps the binding exercised in every build.
            let _ = (event.widget_id, event.kind);
        }
        dispatched += 1;
    }
    dispatched
}

/// Drain the whole trigger queue, dispatching each event to its widget.
///
/// This is the host's entry point for step 1 of a frame: everything the backend has
/// reported since the last call is delivered, and the return value is how many events
/// that was.
///
/// # Why this is in the crate root rather than in `app`
///
/// It used to live in `app`, which is gated on `full_widgets`. A backend that
/// called `crate::app::drain_triggers` therefore compiled on `desktop` and failed on
/// `mini` with `cannot find 'app' in 'crate'`, which is the same class of defect as a call
/// into a gated `create_*`: one unpicked profile hides it completely
/// (`tools/check_profiles.sh` runs the profile matrix; a `desktop` build never sees it).
///
/// The trigger queue is not a widget-set feature — it is how a backend reports "the user
/// resized the window" — so the drain sits beside the queue it drains, at the widest gate
/// its dependency has.
///
/// # Why it is defined in every profile, not gated
///
/// The queue exists in every profile except `alloc_frugal` (`mini`). Gating the function
/// would mean gating each of the **eleven** backend call sites, and a gate that has to be
/// repeated eleven times is a gate that will be forgotten in one of them — which is
/// exactly how this function came to be missing from the crate root in the first place.
/// Defining it unconditionally moves the condition to one place, and `alloc_frugal`
/// answers `0` because there is genuinely nothing to drain there.
///
/// # Termination
///
/// Drains until the queue reports empty, so a tick's cost is bounded by the number of
/// events that arrived since the last tick. A backend that produced events faster than it
/// consumed them would starve its own loop, which is why the queue is drained to empty
/// rather than to a fixed count: "drained" is the only state from which the next tick's
/// backlog is knowable.
pub fn drain_triggers() -> usize {
    #[cfg(alloc_frugal)]
    {
        // This profile has no trigger queue, so there is nothing queued and nothing to
        // deliver. `0` is the truthful count, not a failure.
        0
    }
    #[cfg(not(alloc_frugal))]
    {
        // Only a build with a router can dispatch, so only that build counts. Naming the
        // two configurations separately keeps `mut` off the binding that never moves —
        // the compiler then checks exactly the profile-conditional fact, instead of a
        // `#[allow(unused_mut)]` suppressing it in one profile and the other profile's
        // correctness going unchecked.
        #[cfg(full_widgets)]
        let mut dispatched = 0usize;
        #[cfg(not(full_widgets))]
        let dispatched = 0usize;
        while let Some(event) = poll_widget_trigger_event() {
            #[cfg(full_widgets)]
            {
                app::dispatch_trigger(event.widget_id, event.kind);
                dispatched += 1;
            }
            #[cfg(not(full_widgets))]
            {
                // No router in this profile: `app` is gated `full_widgets`, so there is no
                // `dispatch_trigger` to hand the event to. The event is still **removed**
                // from the queue (the `poll` above did that), which is the part the queue
                // owes its callers, and it is *not* counted as dispatched.
                //
                // # Why the count must stay `0` here
                //
                // This used to increment `dispatched` unconditionally, so `embedded` —
                // which is a stripped profile with the trigger queue present but no router
                // — reported `1` for a `Resized` event that reached no layout. The return
                // value is the host's only signal that dispatch happened, and a count that
                // is indistinguishable from a real delivery is the "reported success for
                // something that did not happen" shape this crate names elsewhere. The
                // honest answer for a profile with no router is "I consumed it and
                // delivered nothing", i.e. zero, and the log line says which.
                log::debug!(
                    "drain_triggers: consumed a {:?} trigger for widget {} in a build with no \
                     widget router (profile has no device class); nothing was dispatched",
                    event.kind,
                    event.widget_id
                );
                let _ = (event.widget_id, event.kind);
            }
        }
        dispatched
    }
}
/// Queues an activation of `widget_id` as if the user had performed it, for
/// tests and for driving the UI from outside the event loop.
///
/// Returns `false` when the backend will not accept the injected event, in
/// which case nothing is queued and no later poll will report it.
#[cfg(not(alloc_frugal))]
pub fn inject_widget_trigger_event(
    widget_id: crate::core::ObjectId,
    kind: WidgetTriggerKind,
) -> bool {
    control_backend::get_control_backend().inject_widget_trigger_event(widget_id, kind)
}
/// Reports that a container's client area became `width` by `height`, and queues a
/// `Resized` trigger so a host re-runs its layout.
///
/// # Who calls this
///
/// The platform backends call it from their window-resize callback (GTK
/// `size-allocate`, Win32 `WM_SIZE`, AppKit `windowDidResize:`), and a host that owns
/// its event loop may call it for a resize it learns about by other means. It is the
/// one entry point for "the user resized this window", which no `set_widget_geometry`
/// call covers — the program was not the one changing the size.
///
/// The reported size is retained (see [`window_client_size`]) and a
/// [`WidgetTriggerKind::Resized`] event is queued. The two are deliberately separate:
/// the event is consumed by polling, while the size keeps answering afterwards.
///
/// Returns `false` when `window_id` addresses nothing, so a stale id cannot inject a
/// phantom resize.
#[cfg(not(alloc_frugal))]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    control_backend::get_control_backend().queue_resize_trigger(window_id, width, height)
}
/// Reports a container resize in an allocation-frugal (`mini`) build.
///
/// The signature is identical to the full build's (rule #21: a profile change must
/// not change the API). `mini` runs without a platform singleton, so there is no
/// window whose resize could be observed and nothing to queue — answering `false`
/// reports that honestly instead of accepting a resize that could never be delivered.
#[cfg(alloc_frugal)]
pub fn queue_resize_trigger(window_id: crate::core::ObjectId, width: u32, height: u32) -> bool {
    let _ = (window_id, width, height);
    false
}
/// The client size last reported for `window_id`, or `None` when none was.
///
/// The readback half of [`queue_resize_trigger`]: a host that receives a `Resized`
/// event learns *which* window changed, and asks here *how big* it now is. Falls back
/// to the window's current geometry, which is what it still has if the user has never
/// resized it.
#[cfg(not(alloc_frugal))]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    control_backend::get_control_backend().window_client_size(window_id)
}
/// The last reported client size in an allocation-frugal (`mini`) build.
///
/// Always `None`: the same signature as the full build, but a `mini` build keeps no
/// window records, so no size was ever reported to answer with.
#[cfg(alloc_frugal)]
pub fn window_client_size(window_id: crate::core::ObjectId) -> Option<(u32, u32)> {
    let _ = window_id;
    None
}
// Clipboard
/// Replaces the platform clipboard's text contents.
///
/// Returns `false` when the clipboard could not be written — including when
/// another application holds it, which is a normal, transient condition. This
/// function takes only text; see [`platform_clipboard`] for the rich backend.
#[cfg(not(alloc_frugal))]
pub fn set_clipboard_text(text: &str) -> bool {
    let written = platform::get_platform().set_clipboard_text(text);
    // APP-012: a clipboard write is an application-level fact. Publishing it only on success means
    // a subscriber never sees a change that did not happen; a host that holds no subscription pays
    // one empty-list check.
    #[cfg(full_widgets)]
    if written {
        app::publish_application_event(&app::ApplicationEvent::ClipboardChanged {
            text: crate::compat::String::from(text),
        });
    }
    written
}
/// The platform clipboard's text contents.
///
/// Returns an empty string when the clipboard holds no text, holds a non-text
/// format, or cannot be read — the three cases are **not** distinguished here,
/// so an empty result does not mean the copy succeeded with empty text.
#[cfg(not(alloc_frugal))]
pub fn get_clipboard_text() -> String {
    platform::get_platform().get_clipboard_text()
}
/// The platform clipboard's text, or `None` when it holds no text.
///
/// Where [`get_clipboard_text`] answers `""` for an empty clipboard, a clipboard holding an
/// image or a file list, *and* a failed read, this reports the one fact a paste handler needs:
/// `Some(text)` when there is text to paste, `None` when there is not. It does not by itself
/// distinguish "empty" from "non-text" — use [`platform_clipboard`] for that — but it is the
/// difference between "paste this" and "there is nothing text-shaped to paste".
#[cfg(not(alloc_frugal))]
pub fn clipboard_text() -> Option<String> {
    platform::get_platform().clipboard_text()
}
/// Returns the platform's rich clipboard backend, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_clipboard() -> Option<&'static dyn crate::platform::clipboard::RichClipboardBackend>
{
    platform::get_platform().clipboard_backend()
}
/// Asks the user for a path, using the picker the host's own desktop provides.
///
/// # Why this is a host-level call and not a dialog widget
///
/// [`widget::dialog::file_dialog`] is painted by this library and filled by the **host**: it has no
/// directory listing, so a host without one of its own can browse nothing with it. This call goes
/// to the operating system's picker instead, which browses the filesystem itself.
///
/// # What the three answers mean
///
/// `Ok(Some(path))` the user chose that path; `Ok(None)` the user cancelled (a complete
/// interaction, not a failure); `Err(`[`platform::PathPickerError::Unavailable`]`)` this backend has
/// a picker but cannot present it here (no display session, or a thread the toolkit forbids a modal
/// window on); `Err(`[`platform::PathPickerError::Unsupported`]`)` this backend has none, which
/// [`platform::capabilities`]'.`file_dialog` reports **before** the call so a host can pick its
/// fallback without provoking the failure.
///
/// # This blocks
///
/// The picker is modal: it owns the user's input until it is answered, and the event loop keeps
/// running while it is up. Call it from the thread that owns the loop.
#[cfg(not(alloc_frugal))]
pub fn choose_path(
    purpose: crate::platform::PathPurpose,
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    platform::get_platform().choose_path(purpose, title)
}
/// Runs `work` on the event loop's own thread, at or after `after` from now.
///
/// # What a host uses this for
///
/// A host that is not driving the loop sometimes has to get work *onto* it: ending a session after
/// a deadline (a smoke test that must terminate itself, without a watcher thread and without a
/// process kill), refreshing a status line, polling something the toolkit wants read on its own
/// thread. Every toolkit has a timer API for that, and none of them may be created from another
/// thread — which is exactly where such a host is standing.
///
/// # The answer, and its precision
///
/// Returns whether a loop in this process will run the work: `true` when a loop has ticked, so it
/// reaches the queue on its next frame. `false` means no loop has ticked yet; the work stays
/// queued, and a loop started afterwards runs it — so "schedule the ending, then run the loop"
/// works from a single thread.
///
/// `after` is a **lower bound**: work runs on the first frame at or after its deadline, so the
/// granularity is one frame interval (~16 ms). Repeating work is done by having the callback call
/// this again; there is deliberately no repeating form (one mechanism, one interval).
///
/// See `platform::main_thread` for why the frame tick is the
/// executor and what a panicking callback does.
#[cfg(not(alloc_frugal))]
pub fn schedule_on_main(after: std::time::Duration, work: impl FnOnce() + Send + 'static) -> bool {
    platform::main_thread::schedule_in(after, work)
}
/// Asks the user to pick an existing file — [`choose_path`] with
/// [`PathPurpose::OpenFile`](crate::platform::PathPurpose::OpenFile).
///
/// The three named entry points below are the vocabulary a host asks in; they are one call with
/// one implementation, so they cannot drift apart (principles #54/#101).
#[cfg(not(alloc_frugal))]
pub fn open_path(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::OpenFile, title)
}
/// Asks the user to name a path to write — [`choose_path`] with
/// [`PathPurpose::SaveFile`](crate::platform::PathPurpose::SaveFile).
#[cfg(not(alloc_frugal))]
pub fn save_path(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::SaveFile, title)
}
/// Asks the user to pick a directory — [`choose_path`] with
/// [`PathPurpose::SelectDirectory`](crate::platform::PathPurpose::SelectDirectory).
#[cfg(not(alloc_frugal))]
pub fn select_directory(
    title: &str,
) -> Result<Option<std::path::PathBuf>, crate::platform::PathPickerError> {
    choose_path(crate::platform::PathPurpose::SelectDirectory, title)
}
// Menu operations
/// Creates a native menu bar as a child of `parent`.
///
/// On macOS the bar only becomes the application's main menu once it is attached
/// with [`attach_menu_bar_to_window`]. Add menus with [`create_menu`] and items
/// with [`menu_add_item`].
#[cfg(not(alloc_frugal))]
pub fn create_menu_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::MenuBar).create_menu_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Adds a menu under a menu bar (or another menu, for a submenu).
///
/// `parent` must be the **menu bar** for a top-level menu, or a **menu** for a nested one:
/// the parent determines both where the menu belongs structurally and, for a bar, the title
/// that gets painted in the strip. A menu whose parent is a bar also appears in the bar's
/// entry list; a context menu is created under the window it belongs to and starts hidden.
///
/// The menu is a popup and starts hidden. Open it with [`menu_open_at`] (or
/// [`MenuHandle::open_at`](crate::app::MenuHandle::open_at)) when the host decides it should
/// appear — including from the bar's
/// [`triggered_entry`](crate::MenuBar::triggered_entry) event, whose payload carries the
/// entry's rectangle.
///
/// Returns `0` on a profile that does not ship menus.
#[cfg(not(alloc_frugal))]
pub fn create_menu(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::Menu).create_menu(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Attaches a menu bar to a window.
///
/// Returns `false` when the backend cannot attach it.
#[cfg(not(alloc_frugal))]
pub fn attach_menu_bar_to_window(
    window: crate::core::ObjectId,
    menu_bar: crate::core::ObjectId,
) -> bool {
    control_backend::get_control_backend().attach_menu_bar_to_window(window, menu_bar)
}
/// Adds an item to a menu, returning the item's id.
///
/// `shortcut` is **display text** and is neither parsed nor registered as an
/// accelerator — use [`format_shortcut`] to render a platform-correct string
/// from a typed shortcut declaration. The returned id is what
/// [`poll_menu_triggered`] reports once the item is chosen.
#[cfg(not(alloc_frugal))]
pub fn menu_add_item(
    parent_menu: crate::core::ObjectId,
    text: &str,
    shortcut: Option<&str>,
) -> crate::core::ObjectId {
    control_backend::get_control_backend().menu_add_item(parent_menu, text, shortcut)
}
/// Whether the running platform can put an icon on a **native** menu item.
///
/// Ask this before building an icon for a native menu: `false` means the OS surface has no place for
/// one, so the caller keeps its text instead of dropping an icon it already resolved. That
/// distinction — *this surface cannot present an icon* versus *the icon failed to load* — is what
/// makes the two reportable apart (blue27 §19.11, §19.13 I15).
///
/// The self-drawn menus (`Menu`, `MenuBar`, `MenuButton`, `DropdownMenu`) do not need this: they are
/// widgets and take a typed source through their own `icon_source` property. This answers only for
/// the native menu bar the platform owns.
///
/// `false` on every desktop backend today; see
/// [`Platform::supports_menu_item_icons`](crate::platform::Platform::supports_menu_item_icons) for
/// what each platform would have to verify before answering `true`.
#[cfg(not(alloc_frugal))]
pub fn menu_item_icons_supported() -> bool {
    control_backend::get_control_backend().supports_menu_item_icons()
}

/// Legacy mechanism-vocabulary spelling of [`menu_item_icons_supported`].
#[cfg(not(alloc_frugal))]
#[deprecated(
    note = "use `menu_item_icons_supported`; the question is about menu items, not a mechanism"
)]
pub fn native_menu_item_icons_supported() -> bool {
    menu_item_icons_supported()
}
/// Renders a shortcut the way the current operating system writes it.
///
/// macOS returns `⌘⇧Z`-style glyphs; Windows and Linux return
/// `Ctrl+Shift+Z`. Use this for any label the user reads (menu text, tooltips,
/// the shortcut column of a command palette) so one shortcut declaration reads
/// natively on every platform.
///
/// For the host-independent notation (useful in serialized keymaps and tests),
/// use [`crate::shortcut::Shortcut::format_shortcut`] together with
/// [`crate::shortcut::format_shortcut_for_platform`] instead.
///
/// ```
/// use rust_widgets::shortcut::{
///     format_shortcut_for_platform, Key, PlatformShortcutStyle, Shortcut,
/// };
///
/// let shortcut = Shortcut::primary(Key::Z);
/// let mac = format_shortcut_for_platform(&shortcut, PlatformShortcutStyle::Mac);
/// let desktop = format_shortcut_for_platform(&shortcut, PlatformShortcutStyle::Desktop);
/// assert_eq!(mac, "⌘Z");
/// assert_eq!(desktop, "Ctrl+Z");
///
/// // The host's spelling is the same table, chosen at run time: one implementation, two
/// // notations. No `cfg(target_os)` is needed to state what this function does.
/// let shown = rust_widgets::format_shortcut(&shortcut);
/// assert_eq!(shown, format_shortcut_for_platform(&shortcut, PlatformShortcutStyle::current()));
/// assert!(shown == mac || shown == desktop);
/// ```
#[cfg(not(alloc_frugal))]
pub fn format_shortcut(shortcut: &crate::shortcut::Shortcut) -> String {
    platform::get_platform().format_shortcut(shortcut)
}
/// Takes the id of the next menu item the user activated, or `None` when none is
/// queued.
///
/// Like the widget version this drains a queue, so a loop calling it until
/// `None` collects every pending activation. The id returned is the one
/// [`menu_add_item`] handed back.
#[cfg(not(alloc_frugal))]
pub fn poll_menu_triggered() -> Option<crate::core::ObjectId> {
    control_backend::get_control_backend().poll_menu_triggered()
}
/// Returns the accelerator text bound to a menu item, if any.
///
/// Read from the menu item's own `shortcut` property, so the answer describes the
/// item the library drew rather than a host-side copy that could drift.
#[cfg(not(alloc_frugal))]
pub fn menu_item_shortcut(menu_item: crate::core::ObjectId) -> Option<String> {
    control_backend::get_control_backend().menu_item_shortcut(menu_item)
}

/// Shows a menu popup with its top-left corner at `position`.
///
/// `position` is in the **window's client coordinates** — the space a menu bar's
/// [`MenuBar::entry_rect`](crate::MenuBar::entry_rect) reports — and the popup is clamped
/// into `viewport` so it never opens partly off-screen. Opening also moves the menu to the
/// end of its window's child list, so a popup paints above, and receives clicks before, any
/// control created after it.
///
/// This is the self-drawn counterpart of a native `NSMenu`/`HMENU` popup: without it a
/// [`create_menu`] widget starts hidden (a popup is not a panel) and has no public way to
/// become visible. Use [`menu_close`] to dismiss it.
///
/// Returns `false` when `menu` is not a mounted menu, when the viewport is empty, or when
/// the id addresses another kind of widget.
///
/// ```rust,no_run
/// use rust_widgets::core::{Point, Rect};
///
/// let window = rust_widgets::create_window("Menu", 0, 0, 640, 480);
/// let bar = rust_widgets::create_menu_bar(window, 0, 0, 640, 28);
/// let file = rust_widgets::create_menu(bar, "File", 0, 0, 160, 0);
/// rust_widgets::menu_add_item(file, "Open", Some("Ctrl+O"));
/// assert!(rust_widgets::menu_open_at(file, 0, 28, Rect::new(0, 0, 640, 480)));
/// assert!(rust_widgets::menu_close(file));
/// ```
#[cfg(full_widgets)]
pub fn menu_open_at(
    menu: crate::core::ObjectId,
    x: i32,
    y: i32,
    viewport: crate::core::Rect,
) -> bool {
    if viewport.width == 0 || viewport.height == 0 {
        return false;
    }
    crate::widget::runtime::open_context_menu(menu, crate::core::Point::new(x, y), viewport)
}

/// Hides an open menu popup.
///
/// Returns `false` when `menu` is not a mounted menu or another kind of widget. The
/// menu's own `hidden`/`about_to_hide` signals fire through [`crate::Menu`]'s close path,
/// so a caller that subscribed to them sees the same lifecycle fact a click-outside would
/// produce.
#[cfg(full_widgets)]
pub fn menu_close(menu: crate::core::ObjectId) -> bool {
    let hidden = crate::widget::runtime::with_widget_mut(menu, |widget| {
        let Some(menu) = (widget as &mut dyn std::any::Any).downcast_mut::<crate::Menu>() else {
            return false;
        };
        menu.hide();
        true
    })
    .unwrap_or(false);
    if hidden {
        crate::widget::runtime::request_repaint_subtree(menu);
        crate::widget::runtime::clear_popup_grab();
    }
    hidden
}
/// Returns the backend's opaque handle for a widget, when it has one.
///
/// Named for the **intent** ("the backend's own handle for this control"), not for
/// the mechanism. The value is opaque: it is whatever object the backend uses to
/// represent the control, and its meaning is entirely backend-specific. It exists so
/// host code and integration tests can reach the underlying object for things the
/// cross-platform API does not model.
///
/// This was `native_handle`, which named a mechanism in the public API — the same
/// vocabulary the surface API deliberately avoids (`src/lib.rs`'s mount/resize
/// helpers, and principle #52). The old name remains as a deprecated alias so an
/// existing call site gets a warning rather than a build break.
///
/// Returns `None` when the widget is unknown, or when the backend created it in
/// state-only mode (for example off the UI thread) and therefore holds no object to
/// return.
#[cfg(not(alloc_frugal))]
pub fn backend_handle(widget: crate::core::ObjectId) -> Option<usize> {
    platform::get_platform().get_native_handle(widget)
}
/// Queues activation of a menu item as if the user had chosen it, for tests.
///
/// Returns `false` when the backend will not accept the injected event.
#[cfg(not(alloc_frugal))]
pub fn inject_menu_trigger(menu_item_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().inject_menu_trigger(menu_item_id)
}
// ToolBar and StatusBar
/// Creates a tool bar strip as a child of `parent`.
///
/// The returned id can be wrapped in an `app::ToolBarHandle` for the
/// typed operations (adding actions, changing orientation).
///
/// Returns `0` on a profile that does not ship the tool bar.
#[cfg(not(alloc_frugal))]
pub fn create_tool_bar(
    parent: crate::core::ObjectId,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::ToolBar).create_tool_bar(parent, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
/// Creates a status bar as a child of `parent`, with `text` as its message.
///
/// Returns `0` on a profile that does not ship the status bar.
#[cfg(not(alloc_frugal))]
pub fn create_status_bar(
    parent: crate::core::ObjectId,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> crate::core::ObjectId {
    #[cfg(widgets_unstripped)]
    {
        backend_for_kind(widget::WidgetKind::StatusBar)
            .create_status_bar(parent, text, x, y, width, height)
    }
    #[cfg(not(widgets_unstripped))]
    {
        let _ = (parent, text, x, y, width, height);
        NO_SUCH_CONTROL
    }
}
// Drag and Drop
/// Starts a drag of `payload` out of `source_widget_id`, advertising it as
/// `mime`.
///
/// `payload` is copied into the platform's drag object and is not retained by
/// this library, so the caller may free it once this returns. Returns `false`
/// when the backend cannot begin a drag — for instance when a drag is already in
/// progress — in which case no drop event will follow.
#[cfg(not(alloc_frugal))]
pub fn begin_drag(source_widget_id: crate::core::ObjectId, mime: &str, payload: &[u8]) -> bool {
    platform::get_platform().begin_drag(source_widget_id, mime, payload)
}
/// Takes the next completed drop, or `None` when none is pending.
///
/// Polling a queue: each successful call removes one event. A drop is only
/// reported after the user has released, so a drag in progress yields `None`.
#[cfg(not(alloc_frugal))]
pub fn poll_drop_event() -> Option<DropEvent> {
    let event = platform::get_platform().poll_drop_event();
    // APP-012: a completed drop is broadcast at the moment it is dequeued, so every subscriber sees
    // the same one report rather than each polling the platform queue independently.
    #[cfg(full_widgets)]
    if let Some(event) = &event {
        app::publish_application_event(&app::ApplicationEvent::DropCompleted {
            event: event.clone(),
        });
    }
    event
}
/// Queues a drop event as if the user had performed it, for tests.
///
/// Returns `false` when the backend will not accept the injected event, in which
/// case a later poll will not report it.
#[cfg(not(alloc_frugal))]
pub fn inject_drop_event(event: DropEvent) -> bool {
    platform::get_platform().inject_drop_event(event)
}
// IME and Accessibility
/// Enables or disables input-method (IME) input for a text-entry widget.
///
/// Returns `false` when the backend refuses — typically because the id is not a
/// text-entry control, or because the platform has no IME support to switch.
#[cfg(not(alloc_frugal))]
pub fn set_widget_ime_enabled(widget_id: crate::core::ObjectId, enabled: bool) -> bool {
    control_backend::get_control_backend().set_widget_ime_enabled(widget_id, enabled)
}
/// Whether input-method input is enabled for a widget.
///
/// Reports `false` both for "enabled is off" and for "this id is unknown or not
/// a text control", so it cannot distinguish the two.
#[cfg(not(alloc_frugal))]
pub fn is_widget_ime_enabled(widget_id: crate::core::ObjectId) -> bool {
    control_backend::get_control_backend().is_widget_ime_enabled(widget_id)
}
/// Returns the platform's IME bridge, if available.
#[cfg(not(alloc_frugal))]
pub fn platform_ime_bridge() -> Option<&'static dyn crate::platform::ime::ImeBridge> {
    platform::get_platform().ime_bridge()
}
/// Sets the accessible name of a widget, as reported to assistive technology.
///
/// This is the label a screen reader announces — distinct from the visible text,
/// which is often too terse or ambiguous to read aloud. Returns `false` when the
/// backend refuses.
#[cfg(not(alloc_frugal))]
pub fn set_widget_accessibility_name(widget_id: crate::core::ObjectId, name: &str) -> bool {
    control_backend::get_control_backend().set_widget_accessibility_name(widget_id, name)
}
/// The accessible name set by [`set_widget_accessibility_name`].
///
/// Returns an empty string when no name was set, when the id is unknown, or when
/// the backend refused — the cases are not distinguished, so an empty result
/// does not prove the widget is nameless to assistive technology.
#[cfg(not(alloc_frugal))]
pub fn get_widget_accessibility_name(widget_id: crate::core::ObjectId) -> String {
    control_backend::get_control_backend().get_widget_accessibility_name(widget_id)
}
