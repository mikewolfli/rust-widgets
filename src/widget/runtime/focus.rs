// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

// ---------------------------------------------------------------------------
// Keyboard focus
// ---------------------------------------------------------------------------

/// Returns the widget that currently owns keyboard focus, if any.
pub fn focused_widget() -> Option<ObjectId> {
    FOCUS.try_with(|focus| focus.borrow().focused_widget()).unwrap_or(None)
}

/// Returns whether `id` currently owns keyboard focus.
pub fn has_focus(id: ObjectId) -> bool {
    FOCUS.try_with(|focus| focus.borrow().has_focus(id)).unwrap_or(false)
}

/// Whether `id` may take keyboard focus from a pointer press.
///
/// The widget must be a mounted, enabled control that reports
/// `Widget::is_focusable`. A disabled control stays in the
/// tab order (so the user can move past it) but must not *receive* focus, which is why enabled
/// state is checked here rather than in [`Widget::is_focusable`].
pub fn widget_wants_keyboard_focus(id: ObjectId) -> bool {
    read_widget(id, |widget| widget.is_enabled() && widget.is_focusable()).unwrap_or(false)
}

/// Whether `id` is a text-entry control that should own the input method's key stream.
///
/// A toolkit IM context must only consume keys while a control that accepts committed text owns
/// focus; filtering keys for a button would swallow its activation. The decision is derived from
/// the widget's kind, so every backend asks the same question and a new entry control opts in
/// once (`WidgetKind::accepts_ime_input`).
pub fn accepts_ime_input(id: ObjectId) -> bool {
    read_widget(id, |widget| widget.is_enabled() && widget.kind().accepts_ime_input())
        .unwrap_or(false)
}

/// Moves keyboard focus to `id`, telling the previously focused widget it lost it.
///
/// Delivers the `FocusLost` / `FocusGained` pair itself rather than expecting each
/// backend to synthesise them: focus is a library-level concept here, and relying on
/// four separate backends to emit a matching pair is how the events ended up with no
/// producer at all (see [`crate::event::FocusManager`]).
///
/// The platform's IME bridge is told about the change too, so a candidate window knows
/// which control is being edited. Without it the bridge's focused-widget field stayed
/// `None` forever and the IME had nowhere to place its candidates.
///
/// Returns whether focus moved. Already-focused ids report `false`, so a repeated
/// click does not re-emit the pair.
pub fn focus_widget(id: ObjectId) -> bool {
    focus_widget_with_reason(id, FocusReason::Programmatic)
}

/// Moves keyboard focus to `id`, recording *why* it moved.
///
/// [`focus_widget`] is this function with [`FocusReason::Programmatic`], which is the
/// honest default for a caller that has no user gesture to point at: an explicit
/// `focus_widget` call is the application choosing a control, not the user navigating.
///
/// The reason travels with the `FocusGained` payload so an individual control can
/// decide whether to paint a focus ring **at the moment it handles the event**. A
/// shared `current_focus_reason()` query would be the wrong shape: by the time a third
/// control asked, the answer could describe a move that had already been superseded,
/// and a draw pass happens later still.
///
/// Returns whether focus moved. Already-focused ids report `false`, so a repeated
/// click does not re-emit the pair.
pub fn focus_widget_with_reason(id: ObjectId, reason: FocusReason) -> bool {
    // Read the outgoing owner *before* the manager switches, so the pair is
    // `previous -> id` and not `id -> id`.
    let previous = focused_widget();
    let moved = FOCUS
        .try_with(|focus| {
            let mut focus = focus.borrow_mut();
            if focus.focused_widget() == Some(id) {
                return false;
            }
            // Focus on an unmounted id would be an unobservable lie: the key
            // router would find nothing to deliver to. Refuse it instead.
            if !is_mounted(id) {
                return false;
            }
            // A modal dialog owns focus while it is up: focusing a widget outside
            // its subtree would let the keyboard type into a window the user cannot
            // otherwise reach, so it is refused the same way an unmounted id is.
            if modal_blocks(id) {
                return false;
            }
            focus.set_focus(id);
            true
        })
        .unwrap_or(false);
    if !moved {
        return false;
    }
    // Outside the manager borrow: both deliveries reach into MOUNTED, and a widget
    // handling `FocusLost` may itself read focus state.
    if let Some(previous) = previous {
        notify_ime_focus_out(previous);
        let _ = dispatch_event(previous, &Event::FocusLost);
    }
    notify_ime_focus_in(id);
    let _ = dispatch_event(id, &Event::FocusGained { reason });
    true
}

/// Focuses `id` because a pointer press landed on it.
///
/// The reason is [`FocusReason::Pointer`], which suppresses the focus ring: the
/// pointer already shows the user where they are, and a ring under the cursor reads
/// as a stuck highlight. This is the entry point pointer routing should call instead
/// of [`focus_widget`], so no call site has to remember the distinction.
pub fn focus_on_pointer_press(id: ObjectId) -> bool {
    focus_widget_with_reason(id, FocusReason::Pointer)
}

/// Focuses `id` because a keyboard accelerator or shortcut activated it.
///
/// `Shortcut` **does** draw the ring: the user is on the keyboard, and if the control
/// is now the keyboard's target the ring is how that fact becomes visible.
pub fn focus_on_keyboard_activation(id: ObjectId) -> bool {
    focus_widget_with_reason(id, FocusReason::Shortcut)
}

/// Tells the platform's IME bridge that `id` is now being edited.
///
/// No-op when the build has no platform singleton (`mini`) or the backend binds no
/// IME bridge — the honest "this host has no IME" case, not a silent failure.
pub(crate) fn notify_ime_focus_in(id: ObjectId) {
    #[cfg(not(alloc_frugal))]
    if let Some(bridge) = crate::platform::platform_facts().ime_bridge() {
        bridge.focus_in(id);
    }
    #[cfg(alloc_frugal)]
    let _ = id;
}

/// Tells the platform's IME bridge that `id` is no longer being edited.
pub(crate) fn notify_ime_focus_out(id: ObjectId) {
    #[cfg(not(alloc_frugal))]
    if let Some(bridge) = crate::platform::platform_facts().ime_bridge() {
        bridge.focus_out(id);
    }
    #[cfg(alloc_frugal)]
    let _ = id;
}

/// Clears keyboard focus, telling the previous owner it lost it.
pub fn clear_focus() -> bool {
    let previous = focused_widget();
    let cleared = FOCUS
        .try_with(|focus| {
            let mut focus = focus.borrow_mut();
            if focus.focused_widget().is_none() {
                return false;
            }
            focus.clear_focus();
            true
        })
        .unwrap_or(false);
    if !cleared {
        return false;
    }
    if let Some(previous) = previous {
        notify_ime_focus_out(previous);
        let _ = dispatch_event(previous, &Event::FocusLost);
    }
    true
}

/// Moves focus to the next focusable widget in tab order (wraps around).
///
/// Returns the newly focused id. `forward == false` moves backwards (Shift-Tab).
/// The `FocusLost` / `FocusGained` pair is delivered for the moved pair, which is
/// what makes a Tab press observable to the controls themselves.
pub fn focus_next(forward: bool) -> Option<ObjectId> {
    let previous = focused_widget();
    // The tab order keeps disabled and hidden controls in it on purpose (see `is_focusable`), so
    // the **step** skips them. Delivering `FocusGained` to a control that refuses input left the
    // key swallowed with no visible focus change, and asked the IME for a candidate window over
    // a control the user cannot type into.
    let next = FOCUS
        .try_with(|focus| {
            let mut focus = focus.borrow_mut();
            focus.focus_next_matching(forward, |id| {
                read_widget(id, |widget| widget.is_enabled() && widget.is_visible())
                    .unwrap_or(false)
            })
        })
        .ok()
        .flatten()?;
    if let Some(previous) = previous.filter(|&p| p != next) {
        notify_ime_focus_out(previous);
        let _ = dispatch_event(previous, &Event::FocusLost);
    }
    notify_ime_focus_in(next);
    let reason = if forward { FocusReason::Tab } else { FocusReason::BackTab };
    let _ = dispatch_event(next, &Event::FocusGained { reason });
    Some(next)
}

/// Registers `id` as focusable (adds it to the tab order).
pub fn register_focusable(id: ObjectId) {
    let _ = FOCUS.try_with(|focus| focus.borrow_mut().register_focusable(id));
}

/// Returns the tab order as it currently stands.
pub fn focusable_widgets() -> Vec<ObjectId> {
    FOCUS.try_with(|focus| focus.borrow().focusable_widgets().to_vec()).unwrap_or_default()
}

/// Runs `f` with the focus manager, for the backends and the a11y bridge.
pub fn with_focus_manager<R>(f: impl FnOnce(&mut crate::event::FocusManager) -> R) -> Option<R> {
    FOCUS.try_with(|focus| f(&mut focus.borrow_mut())).ok()
}

/// Chooses how Tab orders the focusable widgets.
///
/// `TabOrder` (the default) is registration order; `RowMajor` and `ColumnMajor`
/// re-sort by the positions recorded via [`set_focusable_position`]. Returns `false`
/// on a thread with no focus manager, which is the same honest-answer rule the rest
/// of this module follows.
///
/// The strategy was fully implemented in [`crate::event::FocusManager`] but never
/// exposed, so a grid-like UI had no way to ask for row-major Tab movement.
pub fn set_focus_traversal(strategy: crate::event::FocusTraversalStrategy) -> bool {
    FOCUS
        .try_with(|focus| {
            focus.borrow_mut().set_traversal_strategy(strategy);
        })
        .is_ok()
}

/// Records where a focusable widget sits, for row/column-major traversal.
///
/// Reorders immediately when a non-default strategy is active, so a caller can set
/// positions in any order. No-op for a widget that is not focusable, because a
/// position the traversal will never consult is not worth storing.
pub fn set_focusable_position(id: ObjectId, x: i32, y: i32) -> bool {
    FOCUS
        .try_with(|focus| {
            let mut focus = focus.borrow_mut();
            if !focus.focusable_widgets().contains(&id) {
                return false;
            }
            focus.set_widget_position(id, x, y);
            true
        })
        .unwrap_or(false)
}
