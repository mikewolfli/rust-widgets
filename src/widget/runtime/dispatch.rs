// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// A host callback observing the IME pre-edit updates delivered to one widget.
///
/// `&str` is the uncommitted composition and `usize` its cursor position (the same shape
/// [`crate::event::Event::ImePreedit`] carries). The string is borrowed for the duration of the
/// call: a host that needs to keep it copies it.
pub(crate) type ImePreeditCallback = Rc<RefCell<dyn FnMut(&str, usize)>>;

/// Registers a host observer for the IME pre-edit delivered to `widget_id`.
///
/// Returns `false` when the id addresses no mounted widget, which is the same "a handler that
/// can never run" answer [`register_value_callback`](crate::app::handle) gives: the caller can
/// report it instead of holding a subscription that silently never fires.
///
/// The only caller is [`WidgetHandle::on_ime_preedit`](crate::app::WidgetHandle::on_ime_preedit),
/// and the `app` layer is `full_widgets`-only. The stripped profiles still compile this module
/// (the widget registry is what they run on), so the function has no consumer there — hence the
/// per-item `dead_code` allowance rather than a crate-wide one.
#[cfg_attr(not(full_widgets), allow(dead_code))]
pub(crate) fn register_ime_preedit_callback<F: FnMut(&str, usize) + 'static>(
    widget_id: ObjectId,
    callback: F,
) -> bool {
    if !accepts_ime_input(widget_id) {
        return false;
    }
    let stored: ImePreeditCallback = Rc::new(RefCell::new(callback));
    let _ = IME_PREEDIT_CALLBACKS.try_with(|map| {
        map.borrow_mut().entry(widget_id).or_default().push(stored);
    });
    true
}

/// Owns the taken pre-edit scratch buffer and returns it to its thread-local slot on drop.
///
/// The guard exists so a callback that unwinds cannot permanently steal the buffer: the `Drop`
/// runs during unwinding, clears the borrowed `Rc`s, and installs the allocation back where the
/// next notification finds it. A re-entrant notification took its own buffer (the slot was
/// empty), so the slot may already hold one by the time this guard runs; in that case this
/// buffer is simply dropped rather than replacing it.
pub(crate) struct ImePreeditBuffer(Option<Vec<ImePreeditCallback>>);

impl ImePreeditBuffer {
    /// Takes the thread's scratch buffer, or answers `None` when the thread has no slot.
    pub(crate) fn take() -> Self {
        Self(IME_PREEDIT_SCRATCH.try_with(|slot| core::mem::take(&mut *slot.borrow_mut())).ok())
    }

    /// The reusable list, when the thread had a slot to take it from.
    pub(crate) fn entries(&mut self) -> Option<&mut Vec<ImePreeditCallback>> {
        self.0.as_mut()
    }
}

impl Drop for ImePreeditBuffer {
    fn drop(&mut self) {
        let Some(mut buffer) = self.0.take() else { return };
        // The `Rc`s must not outlive the notification: keeping them would retain observers (and
        // their captures) after the control was unregistered or the host cleared them.
        buffer.clear();
        let _ = IME_PREEDIT_SCRATCH.try_with(|slot| {
            if let Ok(mut slot) = slot.try_borrow_mut() {
                if slot.is_empty() {
                    *slot = buffer;
                }
            }
        });
    }
}

/// Tells every observer of `widget_id` about one uncommitted composition update.
///
/// The entries are `Rc::clone`d into the thread's reusable scratch list before the callbacks run,
/// so a callback may register another observer, destroy the widget, or call
/// [`clear_ime_preedit_callbacks`] without hitting an already-borrowed `RefCell`. Cloning out is
/// still required for that reason; only the per-notification `Vec` allocation is gone. Returns
/// whether at least one observer ran.
pub(crate) fn notify_ime_preedit(widget_id: ObjectId, text: &str, cursor: usize) -> bool {
    let mut scratch = ImePreeditBuffer::take();
    let Some(callbacks) = scratch.entries() else { return false };
    callbacks.clear();
    let found = IME_PREEDIT_CALLBACKS
        .try_with(|map| match map.borrow().get(&widget_id) {
            Some(entries) => {
                callbacks.extend(entries.iter().cloned());
                true
            }
            None => false,
        })
        .unwrap_or(false);
    if !found || callbacks.is_empty() {
        return false;
    }
    for callback in callbacks.iter() {
        if let Ok(mut observer) = callback.try_borrow_mut() {
            observer(text, cursor);
        } else {
            log::warn!(
                "the IME pre-edit observer for widget {widget_id} is already running; refusing \
                 to re-enter it rather than panicking"
            );
        }
    }
    true
}

/// Drops every IME pre-edit observer registered for `widget_id`.
pub(crate) fn clear_ime_preedit_callbacks(widget_id: ObjectId) {
    let _ = IME_PREEDIT_CALLBACKS.try_with(|map| {
        map.borrow_mut().remove(&widget_id);
    });
}

/// Places the platform IME candidate window at the focused control's caret.
///
/// The query is the control's own answer ([`crate::widget::Widget::input_method_query`]);
/// coordinates are window-relative, which is what the platform bridge expects. A control with
/// no insertion point, a widget that is not focused, and a build without an IME bridge all
/// answer `false` without touching the platform.
#[cfg(not(alloc_frugal))]
pub fn sync_ime_candidate(widget_id: ObjectId) -> bool {
    let Some((focused, query)) = read_widget(widget_id, |widget| {
        (widget.base().focus_reason().is_some(), widget.input_method_query())
    }) else {
        return false;
    };
    if !focused {
        return false;
    }
    let Some(query) = query else {
        return false;
    };
    let Some(bridge) = crate::platform::platform_facts().ime_bridge() else {
        return false;
    };
    bridge.set_candidate_window_position(crate::platform::ime::ImeCandidatePosition {
        x: query.caret_rect.x,
        y: query.caret_rect.y + query.caret_rect.height as i32,
    });
    true
}

/// [`sync_ime_candidate`] for a build with no widget registry or platform IME.
///
/// There is no registry to query and no bridge to position, so the honest answer is `false`.
#[cfg(alloc_frugal)]
pub fn sync_ime_candidate(_widget_id: ObjectId) -> bool {
    false
}

/// Forwards a platform event into a mounted widget's `EventHandler`.
///
/// Returns whether the widget was found. Backends call this to deliver raw
/// platform input; coordinate translation into widget-local space is the
/// widget's responsibility (see `position_at_point`).
///
/// A modal dialog blocks delivery to anything outside its own subtree: when a modal
/// is active and `id` is not the modal (or one of its descendants), the event is
/// dropped and `false` is returned — the same "not found" answer a hit outside any
/// widget gives, so the backend treats it as unowned input rather than an error.
pub fn dispatch_event(id: ObjectId, event: &Event) -> bool {
    if modal_blocks(id) {
        return false;
    }
    // Detach the widget from the registry for the duration of the call.
    //
    // # The re-entrancy defect this closes
    //
    // A control's `handle_event` emits its own signal synchronously when it detects the user action
    // (a `Button` emitting `clicked` on pointer release). A slot on that signal is a user callback,
    // and a user callback legitimately reads or updates widgets: it may read the button's own state,
    // set text on a `Label`, or close a window. Every one of those goes through `with_widget` /
    // `with_widget_mut`, which borrow the very `RefCell` this function used to hold mutably across
    // `handle_event` — so the callback panicked with `already borrowed`, not only for the control
    // being dispatched but for any control in the same registry. `request_redraw` (called by nearly
    // every setter) re-enters too, via `top_level_window_of`, so even a callback that touched nothing
    // could trip it.
    //
    // Taking the entry out before the call and putting it back after means the borrow is held only
    // for the two map operations, never across user code. A handler that destroys the control (or
    // remounts under the same id) is respected: see the tombstone check below.
    let taken = MOUNTED.try_with(|map| map.borrow_mut().remove(&id)).ok().flatten();
    let Some(mut mounted) = taken else {
        return false;
    };
    // Mark the id in-flight so `unregister` records a tombstone instead of searching the map.
    let _ = DISPATCHING.try_with(|set| {
        set.borrow_mut().insert(id);
    });
    // Mark the widget as exclusively lent out. `is_mounted` can still report its presence, but
    // accessors must not dereference its pointer while `handle_event` holds `&mut Widget`.
    let mut guard = DispatchGuard::install(id, mounted.widget.as_mut());

    // The handler runs outside the registry borrow. Its return type is `()`, and the contract of
    // this function answers "was the widget found?" — `true`, because the entry was present when we
    // detached it.
    //
    // # Why the cleanup is a guard and not code after the call (BLUE-issue E-19)
    //
    // A callback may unwind (the host `catch_unwind`s it). Plain cleanup after `handle_event` would
    // then be skipped: the id would stay in `DISPATCHING`, a tombstone would never be consumed, and
    // the widget would never be reinserted — so `is_mounted` answered `false` while `unregister`
    // still reported the id present. [`DispatchGuard`] runs the same cleanup on the normal return and
    // on an unwind, so the registry is consistent either way.
    guard.run(&mut mounted, event);

    // An event handler is the main way an animation starts (hover, press, toggle), so the bit is
    // observed here before the control is reinserted (G-06).
    let animating = mounted.widget.is_animating();
    let destroyed = guard.finish();
    note_animating(id, animating && !destroyed);
    if destroyed {
        // A handler unregistered this control. The cleanup inside `unregister` ran while the
        // mount was still out of `MOUNTED` (this dispatch detached it), so the own-id reverse
        // map could not be resolved there and needs finishing here. Without it every
        // self-destroying handler accumulates one `OWN_IDS` entry, and the stale own -> dead
        // registry mapping misattributes later damage. Reading the own id is still possible
        // because `guard.finish()` returned the widget to us. The rest of `unregister`'s
        // cleanup (focus, hover, capture, cached frame, modal, popup grab) did run, because it
        // is keyed by the registry id that the handler passed.
        let own = mounted.widget.base().id();
        if own != id {
            let _ = OWN_IDS.try_with(|map| map.borrow_mut().remove(&own));
        }
        // Dropping `mounted` here is what actually frees the control.
        return true;
    }

    // Put it back unless a handler remounted something under the same id in the meantime — in which
    // case the handler's own registration wins and is left untouched.
    let _ = MOUNTED.try_with(|map| {
        let mut map = map.borrow_mut();
        map.entry(id).or_insert(mounted);
    });
    // A caret-moving event (typing, arrows, an IME update, or a click that places the insertion
    // point) may have changed where the caret is. Keep the platform candidate window anchored to
    // the control's own answer. This is gated by event kind (G-110): the old unconditional call
    // paid a registry read, a `focus_reason` check, an `input_method_query` virtual call and a
    // platform-bridge lookup for every delivered event, pointer motion included.
    if event_can_move_caret(event) {
        let _ = sync_ime_candidate(id);
    }
    true
}

// A `String` buffer reused across single-character text-input dispatches (G-606).
//
// A committed character reaches a control as `Event::TextInput { text }`, and that variant owns
// its `String`. Building one per keystroke allocated and freed a buffer for every character the
// user typed. The pool holds one buffer that a dispatch borrows for the call and hands back
// immediately afterwards, so after the first keystroke the hot path allocates nothing. A nested
// dispatch (a handler that types into another control) finds the slot empty and allocates for
// that one call; that is what keeps re-entrancy safe without a lock, and it is also the "only
// allocate when the content changes" rule the plan asks for: the buffer grows once and is reused.
thread_local! {
    pub(crate) static TEXT_INPUT_BUFFER: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// Delivers a single committed character to `id` as [`Event::TextInput`], reusing the pooled
/// payload buffer.
///
/// The multi-character entry points (paste, IME commit, a backend that reads a whole UTF-16
/// string at once) keep using [`Event::text_input`]; this is the one-code-point hot path.
///
/// Returns whether the widget was found, with the same contract as [`dispatch_event`].
pub fn dispatch_text_char(id: ObjectId, ch: char) -> bool {
    let mut buffer = TEXT_INPUT_BUFFER
        .try_with(|slot| slot.borrow_mut().take())
        .ok()
        .flatten()
        .unwrap_or_default();
    buffer.clear();
    buffer.push(ch);
    let event = Event::TextInput { text: buffer };
    let delivered = dispatch_event(id, &event);
    // Hand the payload back so the next keystroke reuses it instead of allocating.
    if let Event::TextInput { text } = event {
        let _ = TEXT_INPUT_BUFFER.try_with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                *slot = Some(text);
            }
        });
    }
    delivered
}

/// Whether `event` can move a text control's caret, which is what [`sync_ime_candidate`]
/// anchors the platform candidate window to.
///
/// Keyboard, text, and IME events can; so can a pointer **press/release**, which places the
/// insertion point where the user clicked, and a focus change, which moves the insertion point
/// to a different control. Pointer **motion** cannot, and it is the high-frequency event the old
/// unconditional sync ran on (G-110).
pub(crate) fn event_can_move_caret(event: &Event) -> bool {
    matches!(
        event,
        Event::KeyPress { .. }
            | Event::KeyRelease { .. }
            | Event::KeyDown(_)
            | Event::KeyUp(_)
            | Event::TextInput { .. }
            | Event::ImePreedit { .. }
            | Event::ImeCommit { .. }
            | Event::MousePress { .. }
            | Event::MouseRelease { .. }
            | Event::MouseDown(_)
            | Event::MouseUp(_)
            | Event::MouseDoubleClick { .. }
            | Event::FocusGained { .. }
            | Event::FocusLost
    )
}

/// Owns the in-flight bookkeeping for one [`dispatch_event`] call, so it is undone even on an unwind.
///
/// Installs the exclusive loan ([`LENT_OUT`]), marks the id in [`DISPATCHING`], and on completion
/// clears the marker, consumes any destroy tombstone, and — for a surviving widget — removes the loan
/// so the entry can be reinserted by the caller. See BLUE-issue E-18 (self-reads) and E-19 (unwind).
pub(crate) struct DispatchGuard {
    pub(crate) id: ObjectId,
    pub(crate) finished: bool,
}

impl DispatchGuard {
    pub(crate) fn install(id: ObjectId, widget: &mut dyn Widget) -> Self {
        let ptr: core::ptr::NonNull<dyn Widget> = core::ptr::NonNull::from(widget);
        let _ = LENT_OUT.try_with(|map| {
            map.borrow_mut().insert(id, ptr);
        });
        let _ = LENT_EXCLUSIVE.try_with(|set| {
            set.borrow_mut().insert(id);
        });
        Self { id, finished: false }
    }

    /// Runs the event handler. Separated from construction so the loan is installed before the call
    /// and definitely removed after it (or during an unwind).
    pub(crate) fn run(&mut self, mounted: &mut Mounted, event: &Event) {
        mounted.widget.handle_event(event);
        // A context-menu request is delivered *in addition to* the event's ordinary handling, so a
        // control that starts a selection on a secondary press keeps doing so. The hook lives on
        // `Widget` rather than on `EventHandler` because it answers a question about the control's
        // intent (what menu, at which point), not about the raw event. The event itself is still
        // passed to `handle_event` first, which is what keeps the delivery additive.
        if let Event::ContextMenu { pos } = event {
            let _ = mounted.widget.context_menu_event(*pos);
        }
    }

    /// Completes the dispatch bookkeeping, returning whether a handler destroyed the control.
    pub(crate) fn finish(&mut self) -> bool {
        let _ = DISPATCHING.try_with(|set| {
            set.borrow_mut().remove(&self.id);
        });
        let destroyed = DESTROYED_DURING_DISPATCH
            .try_with(|set| set.borrow_mut().remove(&self.id))
            .unwrap_or(false);
        // The loan is released here on the normal path; the `Drop` below is the unwind path.
        let _ = LENT_OUT.try_with(|map| {
            map.borrow_mut().remove(&self.id);
        });
        let _ = LENT_EXCLUSIVE.try_with(|set| {
            set.borrow_mut().remove(&self.id);
        });
        self.finished = true;
        destroyed
    }
}

impl Drop for DispatchGuard {
    fn drop(&mut self) {
        // On the normal path `finish` already ran; this branch is the unwind path (a panicking
        // callback), where we still clear the loan and the in-flight marker so the registry is left
        // consistent. The tombstone is intentionally left for `unregister` to have recorded, and is
        // consumed by the next dispatch cleanup or the destroy itself.
        if !self.finished {
            let _ = LENT_OUT.try_with(|map| {
                map.borrow_mut().remove(&self.id);
            });
            let _ = LENT_EXCLUSIVE.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
            let _ = DISPATCHING.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
        }
    }
}

/// Tells `id` and every mounted descendant that their container became `width` by `height`.
///
/// # Why a subtree and not just the resized control
///
/// [`Event::Resize`] documents "the new content size in logical pixels", and the control whose
/// box actually changed is rarely the one that cares: a layout manager resizes the window, but
/// the code editor inside it is what must re-measure its visible rows. Delivering only to the
/// window would leave every nested control holding the geometry it computed for the old size —
/// exactly the staleness the event exists to announce. Each descendant receives the size of the
/// **resized container**, not of itself: the event answers "my world changed", and a control
/// reads its own rectangle through [`crate::widget_geometry`] if it needs the narrower fact.
///
/// # Why this is not `dispatch_event` in a loop by the caller
///
/// Walking the mounted tree requires the registry, which lives here. A caller that reimplemented
/// the walk would be a second, drifting definition of "the subtree".
///
/// Returns the number of controls the event reached, so a caller can tell "nothing was mounted"
/// from "everything was told".
#[cfg(not(alloc_frugal))]
pub fn dispatch_resize(id: ObjectId, width: u32, height: u32) -> usize {
    let event = Event::resize(width, height);
    let mut reached = 0;
    // The descendants are collected before delivery so a handler that mounts or destroys
    // controls cannot make the walk observe a half-updated registry.
    let mut targets = alloc::vec::Vec::new();
    collect_subtree_ids(id, &mut targets);
    for target in targets {
        if dispatch_event(target, &event) {
            reached += 1;
        }
    }
    reached
}

/// Collects `id` and every mounted descendant into `out`, parents before children.
///
/// The traversal goes through [`crate::widget::Widget::children`] (via [`with_child_ids`]), which is
/// the single definition of "the mounted tree" — the same source the hit-testing and damage walks
/// use, so the three cannot disagree about what a subtree is.
///
/// # Why this is a loop and not a recursion
///
/// It used to recurse over [`direct_children_of`], which allocates a `Vec` per node — once per
/// event for every widget in the subtree a broadcast reaches. An explicit work stack lets each
/// node's list come from the shared child-id buffer, because the list is consumed before the walk
/// moves on; a recursive version holds it while it descends, which would make the buffer busy at
/// every level and buy nothing.
#[cfg(not(alloc_frugal))]
pub(crate) fn collect_subtree_ids(id: ObjectId, out: &mut alloc::vec::Vec<ObjectId>) {
    out.push(id);
    // Reversed pushes so `pop` yields the first child first: `out` stays "parents before children,
    // siblings in declaration order", exactly as the recursive form produced.
    let mut work: alloc::vec::Vec<ObjectId> = alloc::vec::Vec::new();
    with_child_ids(id, |children| {
        work.extend(children.iter().rev().copied());
    });
    while let Some(current) = work.pop() {
        out.push(current);
        with_child_ids(current, |children| {
            work.extend(children.iter().rev().copied());
        });
    }
}

/// An input fact a platform backend knows about a control it drew itself.
///
/// # Why a native backend reports rather than the library probing
///
/// A native control's hover, press and focus are the *platform's* facts: GTK knows them
/// through `enter-notify-event`, Win32 through `WM_MOUSEMOVE`, AppKit through `mouseEntered:`.
/// The library cannot discover them — it does not own the pixels (BLUE24 §7.1) — but it can
/// be *told*, and this enum is the vocabulary.
///
/// # Why "report" is not "guess"
///
/// The backend already has these facts, because it must handle input to work at all. So this
/// is a report of something known, not an inference: the backend is the authority on its own
/// control's state, exactly as [`crate::widget::BaseWidget`] is the authority on a
/// self-painted control's.
///
/// # Why this is not the whole [`crate::event::Event`] vocabulary
///
/// Because the platform is *not* delivering an event to the library — the control is native
/// and its own toolkit consumed the event. What crosses the boundary is only the state that
/// the library needs to answer `widget_state()`, which is what makes a theme's
/// `"<kind>:hover"` apply to a native control. A backend that wants to deliver a real event
/// uses [`dispatch_event`] instead; the two are complementary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateFact {
    /// The pointer entered (`true`) or left (`false`) the control.
    Hovered(bool),
    /// The control is being held down (`true`) or was released (`false`).
    Pressed(bool),
    /// The control gained (`true`) or lost (`false`) keyboard focus.
    Focused(bool),
    /// The control was enabled (`true`) or disabled (`false`).
    Enabled(bool),
}

/// Reports an input fact a platform backend observed about a control it drew itself.
///
/// See [`StateFact`] for why a backend reports rather than the library probing. Returns
/// whether the fact was applied, i.e. whether `id` is a mounted control; a backend that
/// reports on a control the library does not know about learns that immediately rather than
/// silently having the fact dropped.
///
/// # What it changes
///
/// It writes the same [`crate::widget::BaseWidget`] fields that a self-painted control's own
/// event path writes, so from that point on `widget_state()` is as true for a native control
/// as for a self-painted one and the theme's `<kind>:<state>` overrides apply to both worlds.
/// A control that has already been told the same fact is **not** changed again, so a backend
/// that reports on every mouse move costs one comparison rather than a restyle per sample.
pub fn report_state(id: ObjectId, fact: StateFact) -> bool {
    let Some(changed) = with_widget_mut(id, |widget| {
        let base = widget.base_mut();
        match fact {
            StateFact::Hovered(hovered) => {
                if base.is_hovered() == hovered {
                    return false;
                }
                base.set_hovered(hovered);
            }
            StateFact::Pressed(pressed) => {
                if base.is_pressed() == pressed {
                    return false;
                }
                base.set_pressed(pressed);
            }
            StateFact::Focused(focused) => {
                let already = base.focus_reason().is_some();
                if already == focused {
                    return false;
                }
                // `Programmatic` because the library did not observe the navigation that
                // moved focus -- the native toolkit did. Claiming `Tab` would assert the user
                // pressed Tab, and claiming `Pointer` would suppress the ring; "the
                // application (here: the platform) moved focus" is the honest reading, and it
                // is the variant that keeps the ring's meaning intact.
                base.set_focus_reason(if focused {
                    Some(crate::event::FocusReason::Programmatic)
                } else {
                    None
                });
            }
            StateFact::Enabled(enabled) => {
                if base.is_enabled() == enabled {
                    return false;
                }
                base.set_enabled(enabled);
            }
        }
        // A state change the theme can see is a state change it must re-resolve. Done in this
        // same registry drive: the previous version detached the widget a second time through
        // `with_widget_mut(id, reapply_active_theme_state)`, paying another map remove/insert and
        // a second pair of guards for the change it had just made.
        crate::style::reapply_active_theme_state(widget);
        true
    }) else {
        return false;
    };

    // A state change the theme can see is a repaint worth requesting, and it is also a change
    // a screen reader should hear about (BLUE24 §6.3's submit points 4/5 are the same event
    // for a self-painted control). Reporting the fact is therefore not complete until both
    // have been told, which is why they are here rather than at each backend's call site: a
    // backend that forgot one would make its control the only one whose hover neither painted
    // nor announced.
    if changed {
        // The theme was already re-resolved inside the drive above (it needs the same `&mut`),
        // so the remaining work is the repaint and the screen-reader announcement.
        request_repaint_because(id, RepaintReason::State);
        crate::widget::a11y_submit::submit_state_changed(id);
    }
    true
}

/// Opens `menu_id` as a context menu at `position`, clamped to `viewport`.
///
/// This is the one operation a right-click needs, and it lives here rather than in
/// each widget because a context menu is opened *at a pointer position on another
/// widget*: the target widget knows the position, the menu owns the popup, and the
/// registry is the only thing that can reach both.
///
/// Returns `false` when `menu_id` is not a mounted menu, or when it is not a menu
/// at all — the honest answer, reported instead of a silent no-op, so a caller
/// wiring a right-click handler learns immediately that it pointed at the wrong id.
///
/// Gated with the menu widget itself: a profile that compiles `Menu` out has no
/// context menu to open, and reporting that at compile time is more useful than a
/// function that could only ever return `false`.
#[cfg(full_widgets)]
pub fn open_context_menu(menu_id: ObjectId, position: Point, viewport: Rect) -> bool {
    // Only one popup is open at a time. A bar click on another title must close the menu already
    // showing; without this, every clicked title added another popup and they stacked. Clicking
    // the **same** title again toggles its menu closed, which is what a menu-bar title does.
    if let Some(previous) = popup_grab() {
        clear_popup_grab();
        if previous == menu_id {
            if hide_popup(previous) {
                request_repaint_subtree(previous);
                return true;
            }
        } else if hide_popup(previous) {
            request_repaint_subtree(previous);
        }
    }
    // A popup must be the window's **last** child before it is shown: the renderer paints
    // siblings in order and the hit test walks them in reverse, so a menu created before a
    // later control would be painted underneath it and miss every click that overlapped it.
    // Reparenting here rather than at creation keeps the caller's order irrelevant.
    bring_popup_to_front(menu_id);
    let opened = with_widget_mut(menu_id, |widget| {
        let Some(menu) = (widget as &mut dyn core::any::Any).downcast_mut::<crate::Menu>() else {
            log::warn!(
                "widget::runtime: id {menu_id} is not a menu, so a context menu cannot be opened \
                 at {position:?}"
            );
            return false;
        };
        menu.open_at(position, viewport);
        true
    })
    .unwrap_or_else(|| {
        log::warn!("widget::runtime: no widget is mounted as id {menu_id}");
        false
    });
    if opened {
        // `Menu::open_at` changes visibility and geometry; nothing else submits the frame.
        request_repaint_subtree(menu_id);
        // The popup owns the click that dismisses it (see `POPUP_GRAB`).
        set_popup_grab(menu_id);
    }
    opened
}

/// Hides `menu` if it is a mounted menu, reporting whether anything was hidden.
#[cfg(full_widgets)]
pub(crate) fn hide_popup(menu: ObjectId) -> bool {
    with_widget_mut(menu, |widget| {
        (widget as &mut dyn core::any::Any).downcast_mut::<crate::Menu>().map(|menu| menu.hide())
    })
    .flatten()
    .is_some()
}

/// Makes `popup` the last child of the window it belongs to, so it paints and hit-tests
/// above every sibling.
///
/// Returns `false` when the id is not mounted, has no window above it, or cannot be
/// detached from its current parent — a standalone popup mounted as its own surface has no
/// sibling order to change, and changing nothing is reported rather than claimed.
#[cfg(full_widgets)]
pub(crate) fn bring_popup_to_front(popup: ObjectId) -> bool {
    let Some(window) = top_level_window_of(popup) else {
        return false;
    };
    if window == popup {
        return false;
    }
    let Some(parent) = read_widget(popup, |widget| widget.parent()) else {
        return false;
    };
    let already_last = parent == Some(window)
        && read_widget(window, |widget| widget.children().last().copied() == Some(popup))
            .unwrap_or(false);
    if already_last {
        return true;
    }
    if let Some(parent) = parent {
        // A popup that was created under its invoker (a menu under a menu bar) must leave
        // that child list first; otherwise two parents claim the same id and the tree walk
        // paints it twice.
        with_widget_mut(parent, |widget| widget.remove_child(popup));
    }
    let linked = with_widget_mut(window, |widget| {
        widget.remove_child(popup);
        widget.add_child(popup);
    })
    .is_some();
    if !linked {
        return false;
    }
    with_widget_mut(popup, |widget| widget.set_parent(Some(window))).is_some()
}

/// Opens the context menu at the pointer position of a secondary-button press.
///
/// Convenience for the common wiring: a widget forwards its right-clicks here and
/// the menu appears where the user clicked. Returns `false` for any event that is
/// not a secondary-button press, or when the menu cannot be opened — so a caller
/// can pass every event through without testing the button itself.
#[cfg(full_widgets)]
pub fn open_context_menu_for_event(menu_id: ObjectId, event: &Event, viewport: Rect) -> bool {
    let Event::MousePress { pos, button, .. } = event else {
        return false;
    };
    if *button != crate::event::mouse_button::SECONDARY {
        return false;
    }
    open_context_menu(menu_id, *pos, viewport)
}
