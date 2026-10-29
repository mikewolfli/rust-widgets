// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Hands ownership of `widget` to the registry and returns its display id.
///
/// Returns `None` when the calling thread has no registry — i.e. it is not the
/// thread that drives the UI. Callers must surface that as "cannot display
/// here" rather than dropping the widget silently.
pub fn register(widget: Box<dyn Widget>) -> Option<ObjectId> {
    let id = next_widget_id();
    let Ok(id) = id else { return None };
    let stored = MOUNTED.try_with(|map| {
        map.borrow_mut().insert(id, Mounted { widget, top_window: None });
    });
    if stored.is_err() {
        return None;
    }
    finish_registration(id);
    Some(id)
}

/// The bookkeeping every registration path owes a freshly mounted control (G-321).
///
/// `register` and `register_with_id` used to repeat these steps verbatim after their
/// registry inserts, and the copy was one more place the two entry points could drift.
/// One body answers for both: once the control is in [`MOUNTED`], the id is the only
/// difference between the two paths, so it is the only parameter.
pub(crate) fn finish_registration(id: ObjectId) {
    // Record the widget's own id -> the registry id, so a producer that only has the
    // former (notably `request_redraw`, called from `&self`) can address the tracker.
    // Read through `with_widget` because the id lives on the widget, and this runs
    // before any caller can hold a borrow.
    if let Some(own_id) = with_widget(id, |widget| widget.base().id()) {
        let _ = OWN_IDS.try_with(|map| {
            map.borrow_mut().insert(own_id, id);
        });
    }
    // Join the tab order if the control says it is focusable. Asked of the widget
    // itself (see `Widget::is_focusable`) rather than of a table of kinds, so a
    // control the library has never heard of participates by answering `true`.
    let focusable = read_widget(id, |widget| widget.is_focusable()).unwrap_or(false);
    if focusable {
        register_focusable(id);
    }
    // Ask the auto-decision whether regioning pays off for this control. This is the
    // only automatic call site: `register` runs as soon as a control is mounted, so a
    // widget reaches the auto-decision exactly once, at the moment its geometry and its
    // children are both known — and a control that mounts *as* a child, or whose
    // constructor mounts children, is judged again on its own mount.
    //
    // The cost of the question is two map probes and one thread-local geometry read;
    // the cost of the answer is only ever paid when the answer is `yes`, because the
    // mode it selects is the self-correcting one. See `should_track_damage`.
    let _ = enable_damage_tracking_if_useful(id);
    // Accessibility submit point 1 of BLUE24 §6.3: a control that has just been mounted is given a
    // node in the accessibility tree, derived from the control itself through `A11yState::from_widget`
    // (that is stage ①, which already existed). Without this the three platform bridges had zero
    // production callers, so a screen reader on any of them received nothing.
    //
    // Read through `with_widget` rather than keeping a borrow: the derive reads the same property
    // contract `accessible_value` does, and both go through the registry this function has just
    // written to.
    if let Some(state) = with_widget(id, crate::platform::accessibility::A11yState::from_widget) {
        crate::widget::a11y_submit::submit_mounted(id, &state);
    }
    // Seed the animation registry with the control's initial state: a spinner or a control that
    // mounts mid-transition is already animating before any event reaches it (G-06).
    if read_widget(id, |widget| widget.is_animating()).unwrap_or(false) {
        note_animating(id, true);
    }
    // Resolve and record the window this control lives under while the mount is fresh (G-106).
    // The parent chain is stable for the mount's lifetime; a later adoption changes the direct
    // parent and the lookup notices (see `top_level_window_of`).
    let _ = top_level_window_of(id);
    // G-404: join the theme switch notification. The subscription is what replaces the full
    // registry walk `crate::reapply_active_theme` used to perform on every switch: each mounted
    // control records itself when the manager emits `theme_changed`, and the drain re-merges
    // exactly the notified set. See `THEME_RESTYLE_PENDING` for why the slot only records intent
    // instead of restyling synchronously.
    #[cfg(device_profile)]
    subscribe_widget_to_theme_changes(id);
    // RT-X02: record the tooltip managers so a hover transition can iterate only them.
    #[cfg(full_widgets)]
    if read_widget(id, |widget| widget.kind() == crate::widget::WidgetKind::TooltipManager)
        .unwrap_or(false)
    {
        note_tooltip_manager(id, true);
    }
}

// ---------------------------------------------------------------------------
// `theme_changed` subscription (G-404)
// ---------------------------------------------------------------------------

// Restyle requests recorded by the controls' `theme_changed` subscriptions (G-404).
//
// Every mounted control subscribes to the theme manager's `theme_changed` signal at
// registration, and a switch — `set_theme`, `set_appearance`, the high-contrast override, a
// system-accent refresh, a loaded-and-activated theme — marks each subscriber here. The merge
// does **not** happen in the slot: every emit site runs while the caller holds the theme mutex
// (`global_theme_manager` guards each `&mut self` emit method), and re-resolving a style takes
// that same non-reentrant mutex, so restyling inside the slot would deadlock. The slot only
// records intent; [`drain_pending_theme_restyle`] performs the merge where no theme lock is
// held — at the next frame, or in the switch path's own completion call.
#[cfg(device_profile)]
thread_local! {
    /// Ids whose subscription fired since the last drain. Insert-only and drained whole, so a
    /// control notified twice before one drain is styled once.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static THEME_RESTYLE_PENDING: RefCell<IdSet> = RefCell::new(IdSet::new());

    /// Each control's `theme_changed` connection, so tearing the control down disconnects its
    /// slot. Without this, a host that creates and destroys controls accumulates slots that
    /// fire for ids that resolve to nothing, and the signal's slot list grows without bound.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static THEME_SLOT_HANDLES: RefCell<HashMap<ObjectId, crate::signal::ConnectionHandle>> =
        RefCell::new(HashMap::new());
}

/// Subscribes the freshly mounted `id` to the theme manager's `theme_changed` signal.
///
/// The slot cannot capture the widget — slots are `Send + Sync + 'static`, and widgets are
/// `!Send` thread-local state — so it captures the registry id and records intent only.
#[cfg(device_profile)]
pub(crate) fn subscribe_widget_to_theme_changes(id: ObjectId) {
    let handle = crate::theme::global_theme_manager()
        .on_theme_changed()
        .connect_ref(move |_| mark_theme_restyle_pending(id));
    let _ = THEME_SLOT_HANDLES.try_with(|map| {
        map.borrow_mut().insert(id, handle);
    });
}

/// The `theme_changed` slot body: record that `id` needs its theme re-applied.
///
/// Only thread-local state is touched, so the slot is safe to run while the emitting caller
/// holds the theme mutex (no lock it holds can be re-entered here) and safe against a
/// re-entrant emit (the set is never borrowed across a call that could emit).
#[cfg(device_profile)]
pub(crate) fn mark_theme_restyle_pending(id: ObjectId) {
    let _ = THEME_RESTYLE_PENDING.try_with(|set| set.borrow_mut().insert(id));
}

/// Disconnects `id`'s `theme_changed` slot. Called from [`unregister`], the single removal
/// funnel, so a destroyed control stops occupying a slot on the manager's signal.
///
/// The disconnect takes the theme mutex, because the signal lives on the manager. No
/// unregister caller holds that mutex — the theme layer resolves styles under short
/// lock-and-drop windows and never tears a widget down — so the acquire cannot self-deadlock.
#[cfg(device_profile)]
pub(crate) fn disconnect_widget_theme_subscription(id: ObjectId) {
    let handle = THEME_SLOT_HANDLES.try_with(|map| map.borrow_mut().remove(&id)).ok().flatten();
    if let Some(handle) = handle {
        crate::theme::global_theme_manager().on_theme_changed().disconnect(handle);
    }
}

/// Re-applies the active theme to every control whose `theme_changed` subscription fired since
/// the last drain, and clears the pending set.
///
/// Runs **outside** the theme mutex — both callers guarantee it: the frame step (`drive_frame`)
/// and the switch paths' own completion call (`crate::reapply_active_theme`), which the FFI
/// entry points and the demos invoke after the manager guard is dropped. The merge is
/// `theme::apply_active_theme`, the same one a newly created control gets, through
/// [`with_widget_mut`] so a control destroyed by its own restyle unwinds by the dispatch rules
/// instead of aliasing the registry.
#[cfg(device_profile)]
pub(crate) fn drain_pending_theme_restyle() {
    let pending = THEME_RESTYLE_PENDING
        .try_with(|set| core::mem::replace(&mut *set.borrow_mut(), IdSet::new()))
        .unwrap_or_else(|_| IdSet::new());
    for id in pending.0 {
        // An id that is gone (destroyed between the notification and this drain) or that belongs
        // to another thread's registry resolves to nothing here and is skipped — which is
        // precisely the set the previous full walk used to restyle.
        with_widget_mut(id, crate::theme::apply_active_theme);
    }
}

/// Hands ownership of `widget` to the registry under `declared_id`, a **reserved** id.
///
/// # Why a caller needs to choose the id
///
/// `register` assigns an id from the runtime's own counter. A caller that holds a document's
/// `"id"` — the JSON layout loader — needs the control to be mounted under *that* id, or every
/// name lookup resolves to an id the runtime never issued. Reserving the id is what makes a
/// declarative document's names and the runtime's registry the same id space.
///
/// # Why it can fail
///
/// The id may already be taken. Returning a *different* id would be worse than failing: the
/// caller would record the control under a name whose handle addresses somebody else's widget.
/// `Err(occupied)` therefore names the id that was in the way, and the caller decides — it
/// must not silently fall back to a fresh id.
///
/// A `declared_id` of `0` is refused for the same reason: `0` is the API's "no widget"
/// sentinel, so mounting a live control there would make it unreachable by design.
pub fn register_with_id(
    widget: Box<dyn Widget>,
    declared_id: ObjectId,
) -> Result<ObjectId, ObjectId> {
    if declared_id == 0 {
        return Err(0);
    }
    // A host with no widget runtime cannot hold the control; say so rather than
    // reporting a success that `with_widget` would then deny.
    if MOUNTED.try_with(|_| ()).is_err() {
        return Err(declared_id);
    }

    // The reservation has to be atomic with respect to the duplicate check: the map is
    // thread-local, so a single borrow covers both the "is it free?" question and the
    // "then it is mine" answer. Asking and then inserting would let a nested mount take
    // the id in between, which is exactly the collision this function exists to prevent.
    let reserved = MOUNTED.try_with(|map| {
        let mut map = map.borrow_mut();
        if map.contains_key(&declared_id) {
            return Err(declared_id);
        }
        map.insert(declared_id, Mounted { widget, top_window: None });
        Ok(declared_id)
    });
    match reserved {
        Ok(Ok(id)) => {
            let _ = id;
        }
        Ok(Err(occupied)) => return Err(occupied),
        Err(_) => return Err(declared_id),
    }

    // The reservation must not be undone by a later `next_widget_id` handing the same id to
    // another producer, so the allocator is told the id is spent: if the counter is sitting on
    // exactly this id, it advances past it. Ids below the counter are already spent, and an id
    // far above it is reserved correctly on its own (the counter will walk up to it and find it
    // occupied), so advancing past it here would only skip ids for no reason.
    let _ = NEXT_ID.try_with(|next| {
        let counter = next.borrow();
        if *counter == declared_id {
            drop(counter);
            *next.borrow_mut() = declared_id.wrapping_add(1);
        }
    });

    // Every step `register` does after the insert, from the shared body (G-321) so the two
    // entry points cannot drift. See `finish_registration` for why each one exists.
    finish_registration(declared_id);
    Ok(declared_id)
}

/// Removes a mounted widget, dropping it. Returns whether it was present.
pub fn unregister(id: ObjectId) -> bool {
    // A destroyed control must not keep requesting frames through the animation registry.
    note_animating(id, false);
    // G-404: the control's `theme_changed` slot must not outlive it — otherwise a host that
    // recycles controls accumulates slots that fire for ids that resolve to nothing. Done
    // first so both removal paths below (the dispatch tombstone and the direct removal) leave
    // no subscription behind.
    #[cfg(device_profile)]
    disconnect_widget_theme_subscription(id);
    // G-304: best-effort reclamation of the dead id from its parent's child list, so a long-lived
    // container does not accumulate ids that resolve to nothing. `children()` documents the
    // traversal obligation (skip unresolvable ids) because this lookup can miss while the widget is
    // lent out for mutation; both together keep the list from growing without bound. Both id spaces
    // are removed because a parent's list may hold either the registry id or the widget's own id.
    let own_id = read_widget(id, |widget| widget.base().id());
    let parent = read_widget(id, |widget| widget.parent()).flatten();
    if let Some(parent) = parent {
        let _ = with_widget_mut(parent, |parent| {
            let base = parent.base_mut();
            base.remove_child(id);
            if let Some(own_id) = own_id {
                if own_id != id {
                    base.remove_child(own_id);
                }
            }
        });
    }
    // Drop any host-window association with the widget it belonged to, so a
    // recycled or reused id cannot inherit a stale host window.
    let removed_host = HOST_WINDOWS.try_with(|map| map.borrow_mut().remove(&id)).ok().flatten();
    if let Some(host) = removed_host {
        // Only drop the reverse entry while it still names this widget: a host that has since
        // been re-associated with another widget belongs to that one now.
        let _ = HOST_TO_WIDGET.try_with(|rev| {
            let mut rev = rev.borrow_mut();
            if rev.get(&host) == Some(&id) {
                rev.remove(&host);
            }
        });
    }
    // The reverse index must lose the link with the forward one (G-314), or `surfaces_of`
    // would answer for a window with a surface that no longer exists.
    let _ = SURFACE_WINDOWS.try_with(|map| {
        if let Some(window) = map.borrow_mut().remove(&id) {
            let _ = SURFACES_BY_WINDOW.try_with(|index| {
                if let Some(list) = index.borrow_mut().get_mut(&window) {
                    list.retain(|linked| *linked != id);
                }
            });
        }
    });
    // A widget that is gone must not stay in the tab order: `focus_next` would
    // hand focus to an id that resolves to nothing, and the key that triggered
    // the move would be swallowed. The manager also clears focus if this id owned
    // it, so the next Tab starts from the beginning rather than from a dead id.
    let _ = FOCUS.try_with(|focus| focus.borrow_mut().unregister_focusable(id));
    // Likewise it must not stay the hover target, or the next pointer move would
    // send `MouseLeave` to an id that no longer exists and skip the real one.
    let _ = HOVERED.try_with(|hovered| {
        let mut hovered = hovered.borrow_mut();
        if *hovered == Some(id) {
            *hovered = None;
        }
    });
    // A host observer of this widget's IME pre-edit would otherwise outlive the control it
    // observes and keep its captured state alive for the rest of the thread.
    clear_ime_preedit_callbacks(id);
    // RT-X02: a destroyed manager must leave the hover set, or a later pointer transition would
    // look it up and find nothing.
    #[cfg(full_widgets)]
    note_tooltip_manager(id, false);
    // A host-registered repeating timer would otherwise outlive the control: the worker would
    // keep waking and posting `Event::Timer` to an id that no longer resolves. The loop owns the
    // timer set, so the purge goes through the shared registry rather than a manager method
    // (G-901). The dialog/full-widget profiles compile `runtime` but not the purge registry's
    // profile, so the call is gated to the profile that owns both.
    #[cfg(full_widgets)]
    let _ = crate::event::timer::stop_timers_for_target_everywhere(id);
    // Object ids are reused (see `create_widgets_base`): a tracker that still holds the dead
    // widget's last press at the same position would turn a fresh widget's first click into a
    // double-click (EV-X03). Same registry pattern as the timer purge above.
    #[cfg(full_widgets)]
    crate::event::double_click::forget_double_click_for_target_everywhere(id);
    // And it must not keep pointer capture: routing would send every subsequent
    // pointer event to an id that resolves to nothing.
    let _ = CAPTURE.try_with(|capture| {
        let mut capture = capture.borrow_mut();
        if capture.capturing_widget() == Some(id) {
            capture.release_capture();
        }
    });
    // A destroyed popup cannot own the next outside press. The grab is keyed by registry id,
    // so both id spaces are checked (a control that destroys itself reports its own id).
    #[cfg(not(alloc_frugal))]
    if let Some(grab) = popup_grab() {
        if grab == id || registry_id_of(id) == Some(grab) {
            clear_popup_grab();
        }
    }
    // A destroyed control must not leave its modality behind. A dead modal id is on no live
    // widget's ancestor chain, so `modal_blocks` answers `true` for *every* id and input stays
    // blocked for the rest of the session. All three id spaces are popped because the stack may
    // hold any of them: the passed id itself, its registry counterpart (`registry_id_of` only
    // searches one way, so a registry id cannot be mapped back to an own id that way), and the
    // widget's own id, which is the only handle `Dialog::open` pushes.
    let _ = exit_modal(id);
    if let Some(registry) = registry_id_of(id) {
        if registry != id {
            let _ = exit_modal(registry);
        }
    }
    if let Some(own) = own_id {
        if own != id {
            let _ = exit_modal(own);
        }
    }
    // The cached frame and any pending damage describe this widget, so both must go
    // with it: a reused id would otherwise start from the previous control's pixels,
    // and a stale damage rect would repaint a region that no longer exists. Bumping the
    // frame epochs first is what makes the removal effective for a *child*: the tree-frame
    // caches are keyed by the rendering window, so `forget_cached_frame(id)` here is a no-op
    // for them, and without the bump a destroyed child would keep its pixels and its point-hit
    // entry. The global bump also invalidates the point-hit cache (TREE_FRAME_EPOCH).
    note_renderable_change();
    note_renderable_change_for(id);
    forget_cached_frame(id);
    let _ = REPAINT.try_with(|map| map.borrow_mut().remove(&id));
    // And it cannot stay classified as an open overlay, or a later window would keep painting
    // it above siblings that should cover it (G-316).
    note_overlay_maybe_changed(Some(id));
    // And the own-id reverse mapping, which keeps the table bounded to live widgets and
    // keeps damage attribution correct: a stale own -> dead registry mapping would file damage
    // against an id nothing resolves to.
    if let Some(own_id) = own_id {
        let _ = OWN_IDS.try_with(|map| map.borrow_mut().remove(&own_id));
    }
    // Accessibility submit point 2 of BLUE24 §6.3: the control is gone, so its node must be too.
    // Posted only when the widget was actually present, so an `unregister` of an id that was never
    // mounted does not tear down a node a live control still owns.
    //
    // The dispatching case is separate because the entry is **not** in `MOUNTED`: [`dispatch_event`]
    // took it out so a handler could re-enter the registry. Recording a tombstone lets the dispatch
    // epilogue skip reinsertion, which is what makes a handler's `unregister` of the very control it
    // is handling actually destroy that control instead of being undone a moment later.
    if DISPATCHING.try_with(|set| set.borrow().contains(&id)).unwrap_or(false) {
        let _ = DESTROYED_DURING_DISPATCH.try_with(|set| {
            set.borrow_mut().insert(id);
        });
        crate::widget::a11y_submit::submit_unmounted(id);
        return true;
    }
    let removed = MOUNTED.try_with(|map| map.borrow_mut().remove(&id).is_some()).unwrap_or(false);
    if removed {
        crate::widget::a11y_submit::submit_unmounted(id);
    }
    removed
}

/// Records the host window the platform built for the mounted window `id`.
///
/// Called on the creation path once `Platform::create_window` has returned, so
/// that [`host_window_for`] can resolve a library window id to the id the host
/// layer understands. Storing an association for an id that is not mounted is a
/// programming error and is ignored, because a mapping to a window that does not
/// exist could only produce a mount onto nothing.
pub fn set_host_window(id: ObjectId, host: ObjectId) -> bool {
    let stored = HOST_WINDOWS.try_with(|map| {
        if MOUNTED.try_with(|m| m.borrow().contains_key(&id)).unwrap_or(false) {
            let mut map = map.borrow_mut();
            // Re-associating a widget with a new host must stop advertising the old host, or a
            // stale `HOST_TO_WIDGET` entry would answer lookups for a window this widget no
            // longer owns.
            if let Some(previous) = map.insert(id, host) {
                if previous != host {
                    let _ = HOST_TO_WIDGET.try_with(|rev| rev.borrow_mut().remove(&previous));
                }
            }
            let _ = HOST_TO_WIDGET.try_with(|rev| {
                rev.borrow_mut().insert(host, id);
            });
            true
        } else {
            false
        }
    });
    stored.unwrap_or(false)
}

/// Returns the host window associated with a mounted window widget `id`.
///
/// `None` means "this id is not a window the platform built a host object for" —
/// either it is not a window at all, or the backend has no host windows to build
/// (a state-only backend), in which case the caller must report that it cannot
/// display rather than guessing.
pub fn host_window_for(id: ObjectId) -> Option<ObjectId> {
    HOST_WINDOWS.try_with(|map| map.borrow().get(&id).copied()).unwrap_or(None)
}

/// The widget id whose host window is `host`.
///
/// The inverse of [`host_window_for`], and the translation a platform backend needs
/// whenever an OS callback hands it *its own* window id: the widget registry keys
/// everything by the widget id, and `queue_resize_trigger` refuses an id that
/// addresses no widget. A backend that reported the platform id instead would have
/// every resize silently dropped.
///
/// Returns `None` when no widget names this host, which is correct for a window the
/// platform built before any widget was associated with it.
pub fn widget_id_for_host_window(host: ObjectId) -> Option<ObjectId> {
    HOST_TO_WIDGET.try_with(|map| map.borrow().get(&host).copied()).unwrap_or(None)
}

/// Returns whether `id` refers to a mounted self-drawn widget.
///
/// # Why a `try_borrow` and not a `borrow`
///
/// This is asked from inside the paint path — [`crate::widget::draw_bridge::draw_of`] uses it to
/// tell a control the registry owns from one the host owns — and the paint path is reached while
/// [`for_each_mounted_widget`] already holds the registry mutably. A plain `borrow()` panics there
/// ("RefCell already mutably borrowed"), turning an ordinary question into a crash.
///
/// Answering `true` on a borrow conflict is the safe reading rather than a guess: the only caller
/// holding the registry open is the sweep iterating it, so the widget *is* mounted. The alternative
/// answer would make the paint path advance a control the frame loop is also advancing — the
/// double-step this function exists to prevent.
pub fn is_mounted(id: ObjectId) -> bool {
    let direct = MOUNTED
        .try_with(|map| map.try_borrow().map(|map| map.contains_key(&id)).unwrap_or(true))
        .unwrap_or(false);
    if direct {
        return true;
    }
    // A widget lent out to its own driving callback is still mounted: answering `false` here would
    // tell that callback its object had been destroyed, which is exactly the "cannot tell a live
    // control from a gone one" confusion BLUE-issue E-18 names. The check is by **registry** id,
    // matching how the loan is keyed.
    if LENT_OUT.with(|map| map.borrow().contains_key(&id)) {
        return true;
    }
    // The paint path asks with the widget's *own* id (`BaseWidget::id()`), which is a second id
    // space, not the registry key. Resolve it so the answer is the same whichever id the caller
    // holds. This also restores the previous behaviour where the enclosing `with_widget_mut` borrow
    // made the direct check answer `true`; now the answer is derived from the actual loan rather
    // than from a borrow conflict.
    if let Some(registry_id) = registry_id_of(id) {
        let mounted = MOUNTED
            .try_with(|map| {
                map.try_borrow().map(|map| map.contains_key(&registry_id)).unwrap_or(true)
            })
            .unwrap_or(false);
        if mounted || LENT_OUT.with(|map| map.borrow().contains_key(&registry_id)) {
            return true;
        }
    }
    false
}

/// Returns whether the widget mounted under `id` can carry a tri-state value.
///
/// Asked of the widget itself rather than of a table of kinds: checkability is a
/// capability of the control's own property set, so any type that declares
/// `checked` in [`crate::widget::WidgetProperties::property_names`] answers
/// `true` here — including a third-party widget the library has never heard of.
/// Hosts use this to decide whether a `set_widget_tristate` request is
/// meaningful (principle #37: a request the control cannot honour is refused,
/// never silently stored).
pub fn widget_is_checkable(id: ObjectId) -> bool {
    with_widget(id, |widget| {
        crate::widget::capability::properties_trait::widget_property_names(widget)
            .is_some_and(|names| names.contains(&"checked"))
    })
    .unwrap_or(false)
}

/// Returns the number of widgets mounted on this thread.
pub fn mounted_count() -> usize {
    MOUNTED.try_with(|map| map.borrow().len()).unwrap_or(0)
}

/// Runs `f` against every widget mounted on this thread.
///
/// # Why this exists
///
/// Some operations change a *global* fact rather than one control — switching the
/// active theme, or toggling the high-contrast override — and those have to reach
/// the controls that already exist, not just the ones created afterwards.
/// Enumerating here keeps the registry's internals private: a caller asks for the
/// sweep rather than reaching into the map.
///
/// The visit order is unspecified, and `f` must not mount or unmount widgets (the
/// map is borrowed for the duration).
pub fn for_each_mounted_widget<R>(mut f: impl FnMut(ObjectId, &mut dyn Widget) -> R) {
    let _ = MOUNTED.try_with(|map| {
        for (id, entry) in map.borrow_mut().iter_mut() {
            f(*id, entry.widget.as_mut());
            // A global operation (theme application, contrast override) is one of the few ways a
            // control's animation state changes without a `with_widget_mut` call. Observing the
            // bit here keeps the animation registry complete for that path (G-06).
            note_animating(*id, entry.widget.is_animating());
        }
    });
}
