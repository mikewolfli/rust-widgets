// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Returns the geometry of a mounted widget, or `None` when it is not mounted.
///
/// A widget currently being driven by a mutable callback **is** mounted, but its geometry is not
/// re-read through an aliasing shared reference. Reads of other mounted widgets remain available.
pub fn geometry_of(id: ObjectId) -> Option<Rect> {
    let mounted = MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| entry.widget.geometry()))
        .ok()
        .flatten();
    mounted.or_else(|| lent_out_widget(id, |widget| widget.geometry()))
}

/// The origin a control's own coordinates are relative to.
///
/// # Why a window's origin is `(0, 0)`
///
/// A window's geometry is recorded in **screen** coordinates — `create_window` mounts it
/// at the `x`/`y` the caller asked the operating system for — while every control inside
/// it is placed in **client** coordinates. Those are two different spaces, and using the
/// window's screen position as the tree's origin mixes them.
///
/// The mixture was not harmless. `render_frame_tree` subtracts this origin from every
/// widget before drawing, so a window created at `(100, 100)` shifted each child by
/// `(-100, -100)`: a combo box at client `(20, 142)` was drawn at `(-80, 42)`, outside the
/// canvas and therefore clipped away. The window's own fill used its absolute rect and
/// came back to `(0, 0)`, so the frame was **a correct background with no controls on
/// it** — controls whose style was resolved correctly, whose geometry was correct, and
/// which were still invisible. The window's border stroke additionally landed across the
/// whole frame, which is why a corner of the window read as the border colour.
///
/// So a root contributes no offset: its children's coordinates already start at the
/// window's top-left. A window created at `(0, 0)` behaved by accident before; every
/// window behaves now.
///
/// # Why a non-window keeps its own position
///
/// A control that paints itself is addressed in **its parent's** space, and
/// [`render_frame`] is called for that control's own surface, so its rectangle must be
/// translated back to its own origin. That is the case this subtraction exists for.
pub(crate) fn frame_origin(id: ObjectId) -> (i32, i32) {
    match MOUNTED.try_with(|map| {
        map.borrow().get(&id).map(|entry| (entry.widget.kind(), entry.widget.geometry()))
    }) {
        Ok(Some((crate::widget::WidgetKind::Window, _))) | Ok(None) | Err(_) => (0, 0),
        Ok(Some((_, geometry))) => (geometry.x, geometry.y),
    }
}

/// Updates the geometry of a mounted widget. Returns whether it was found.
///
/// # Why this detaches the widget before calling `set_geometry`
///
/// A container with a content model places its own children when its box changes: a dialog
/// writes its content child's rectangle, a tab widget its current page, a splitter its panes.
/// Those writes go through the public geometry entry point, which is this function. Holding
/// the registry borrow across the widget call (the previous form) made the first such control
/// panic with `RefCell already mutably borrowed` — the borrow was held for the parent while
/// the child write asked for it again.
///
/// [`with_widget_mut`] is the discipline every other drive of a widget already uses: the entry
/// is taken out of the map for the duration, so a `set_geometry` that re-enters the registry
/// (to place children, or from a host callback) sees a free map. It also brings the same
/// nested-mutation refusal as `dispatch_event`, rather than aliasing a live `&mut`.
pub fn set_geometry(id: ObjectId, geometry: Rect) -> bool {
    let mut changed = false;
    let found = with_widget_mut(id, |widget| {
        changed = widget.geometry() != geometry;
        widget.set_geometry(geometry);
    })
    .is_some();
    // A moved or resized subtree no longer matches any layer captured at its previous box. The
    // tree-frame cache compares the same epoch, so one bump keeps both caches honest.
    if found && changed {
        note_renderable_change();
        // The per-window frame epoch follows the global one, so the window this control lives in
        // repaints its cached frame while the other windows keep theirs (G-311).
        note_renderable_change_for(id);
        // A resize can leave a parent's layer rect unchanged while the child inside it shrinks,
        // so the layers are dropped wholesale rather than by rectangle.
        invalidate_widget_layers();
    }
    found
}

/// Runs `f` with mutable access to a mounted widget.
///
/// # Why the widget is detached before `f` runs
///
/// The borrow is scoped to the *map operation*, never to `f`. A property setter runs as `f` and emits
/// the control's signal synchronously, so a user callback runs while `f` is still on the stack; if the
/// `RefCell` were borrowed for all of `f`, that callback's read of any control (its own property, or
/// another control) hit `already borrowed` and panicked. This is the property-write path's version of
/// the defect `dispatch_event` fixed for input (BLUE-issue E-17).
///
/// While `f` runs, other controls remain accessible and `is_mounted(id)` remains true. Reads and
/// writes of this same widget through another registry path are refused while the active `&mut`
/// live `&mut`.
///
/// Returns `None` when `id` is not mounted, or when a nested call targets the id this call is already
/// mutating.
pub fn with_widget_mut<R>(id: ObjectId, f: impl FnOnce(&mut dyn Widget) -> R) -> Option<R> {
    // A nested mutable access to a widget already lent out for mutation would alias the live `&mut`,
    // so it is refused explicitly. The outer call's marker is a mutable-borrow marker distinct from a
    // read-only lent entry; see `LENT_MUTATING`.
    if lent_mutating(id) {
        return None;
    }
    let taken = MOUNTED.try_with(|map| map.borrow_mut().remove(&id)).ok().flatten();
    let mut mounted = taken?;
    // Mark the id in-flight for the duration, exactly as `dispatch_event` does, so a callback that
    // **destroys** this control (a property setter that closes its own window) has its `unregister`
    // recorded as a tombstone rather than silently undone by the reinsertion below. The guard clears
    // the marker on both the normal return and an unwind.
    let mut in_flight = InFlightGuard::install(id);
    let _lent = LentGuard::install(
        id,
        mounted.widget.as_mut(),
        /* mutating = */ true,
        /* exclusive = */ true,
    );
    let result = f(mounted.widget.as_mut());
    // Observe the animation bit while the control is still ours: a setter that started a fade
    // registers the control here, before the next sweep looks for work (G-06).
    let animating = mounted.widget.is_animating();
    drop(_lent);
    let destroyed = in_flight.finish();
    note_animating(id, animating && !destroyed);
    if !destroyed {
        // Reinsert unless a callback replaced this id while `f` ran (the callback's own registration
        // wins); a tombstone means the callback destroyed it, so it must not come back.
        let _ = MOUNTED.try_with(|map| {
            let mut map = map.borrow_mut();
            map.entry(id).or_insert(mounted);
        });
    }
    Some(result)
}

/// Owns the in-flight marker for one registry drive that is not a dispatch, so a destroy inside the
/// drive still sticks and the marker is cleared on an unwind.
///
/// The input dispatch path has [`DispatchGuard`]; `with_widget_mut` needs the same bookkeeping for
/// the same reason (a property setter can emit a signal whose callback destroys the control), so the
/// two share the `DISPATCHING`/`DESTROYED_DURING_DISPATCH` mechanism rather than each inventing one.
pub(crate) struct InFlightGuard {
    pub(crate) id: ObjectId,
    pub(crate) finished: bool,
}

impl InFlightGuard {
    pub(crate) fn install(id: ObjectId) -> Self {
        let _ = DISPATCHING.try_with(|set| {
            set.borrow_mut().insert(id);
        });
        Self { id, finished: false }
    }

    /// Clears the marker and consumes any destroy tombstone, reporting whether the control was
    /// destroyed during the drive.
    pub(crate) fn finish(&mut self) -> bool {
        self.finished = true;
        let _ = DISPATCHING.try_with(|set| {
            set.borrow_mut().remove(&self.id);
        });
        DESTROYED_DURING_DISPATCH.try_with(|set| set.borrow_mut().remove(&self.id)).unwrap_or(false)
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if !self.finished {
            let _ = DISPATCHING.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
            let _ = DESTROYED_DURING_DISPATCH.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
        }
    }
}

/// Runs `f` with shared access to a mounted widget.
///
/// # Why the widget is detached before `f` runs (BLUE-issue E-30)
///
/// The borrow is scoped to the *map lookup*, never to `f`. A caller may read a widget's state and then
/// synchronously trigger its signal — a legitimate `&self` operation — and the slot that runs is a user
/// callback that may update **another** control (set a `Label`, hide a `Panel`). When `f` ran inside
/// `map.borrow().get(...).map(|entry| f(...))`, that callback's `with_widget_mut` (for a different id)
/// hit `already borrowed` on the same `RefCell` and panicked. Detaching first means the registry is free
/// while `f` runs, exactly as [`with_widget_mut`] and [`dispatch_event`] already do for their entries.
///
/// While `f` runs the entry is lent out **read-only**, so a nested read of this same id still resolves
/// (see `LENT_OUT`) and a nested **write** to it is refused. Reads are unavailable while an outer
/// mutable operation or event dispatch holds the widget.
///
/// Returns `None` when `id` is not mounted, or when a nested call targets the id this call is reading.
pub fn with_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
    // A read may nest a read of the same id (a getter that calls a getter), but a read must not be
    // turned into a write of the same id while this shared borrow is live. `with_widget_mut` already
    // refuses nested mutation of a lent-out id via `lent_mutating`; a read loan is recorded the same way
    // so the refusal is symmetric.
    if lent_present(id) {
        if lent_exclusive(id) {
            return None;
        }
        // Already lent out (an outer read of this same id): resolve through the loan, no re-detach.
        return lent_out_widget(id, f);
    }
    let taken = MOUNTED.try_with(|map| map.borrow_mut().remove(&id)).ok().flatten();
    let mounted = taken?;
    let mut in_flight = InFlightGuard::install(id);
    let _lent = LentGuard::install(
        id,
        mounted.widget.as_ref(),
        /* mutating = */ true,
        /* exclusive = */ false,
    );
    // `mutating = true` marks the id as *unavailable for nested mutation* for the loan's duration.
    // The pointer itself is shared (`&dyn Widget`), so reads still resolve through `LENT_OUT`.
    let result = f(mounted.widget.as_ref());
    drop(_lent);
    let destroyed = in_flight.finish();
    if !destroyed {
        let _ = MOUNTED.try_with(|map| {
            let mut map = map.borrow_mut();
            map.entry(id).or_insert(mounted);
        });
    }
    Some(result)
}

/// Runs `f` with shared access to a mounted widget **without detaching it from the registry**.
///
/// [`with_widget`] has to detach, because its `f` may emit a signal whose callback mounts,
/// unmounts or writes a control — operations that need the registry to itself. A pure read needs
/// none of that, and the detachment costs a `remove`, an `insert` and two bookkeeping guards:
/// roughly ten hash operations to answer what one lookup already holds.
///
/// This is the read-only counterpart the hot paths use: hit-testing every child of a node,
/// walking `parent()` links per modal layer, and copying a root's child list. They were doing the
/// detach/renter dance for a `Rect`, an `ObjectId` or a `bool`.
///
/// # The contract `f` must keep
///
/// `f` may only **read**. It must not mount, unmount, mutate or emit: a signal whose callback did
/// any of those would take a mutable borrow of the registry while this call holds a shared one.
/// Nested [`read_widget`] calls are fine (shared borrows compose), and so are ordinary getters.
/// When in doubt, use [`with_widget`] — it is the general form and pays for the freedom.
///
/// A control currently lent out for mutation answers `None`; one lent out for reading resolves
/// through the loan. Those are the same visibility rules [`with_widget`] obeys.
pub fn read_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
    if lent_present(id) {
        return lent_out_widget(id, f);
    }
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| f(entry.widget.as_ref())))
        .ok()
        .flatten()
}

/// Whether `id` currently has a lent-out entry (read or mutating).
pub(crate) fn lent_present(id: ObjectId) -> bool {
    // The table is empty for almost every read: the detach/renter dance only fills it while a
    // callback runs. `is_empty` short-circuits the hash probe on the hot path (G-317).
    LENT_OUT.with(|map| {
        let map = map.borrow();
        !map.is_empty() && map.contains_key(&id)
    })
}

/// Runs `f` with a **read-only** view of a widget currently lent out of [`MOUNTED`].
///
/// Returns `None` when no widget is lent out under `id`. See [`LENT_OUT`] for why this exists and why
/// no mutating accessor may use it.
pub(crate) fn lent_out_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
    if lent_exclusive(id) {
        return None;
    }
    // The pointer is copied out under a short borrow, then dereferenced with no `RefCell` borrow held:
    // the borrowed-from frame keeps the widget alive for this whole call (it will not return until the
    // callback does), so the pointer is valid here. `f` runs to completion before any other code can
    // invalidate it, so the shared reference never escapes this call.
    let ptr = LENT_OUT.with(|map| map.borrow().get(&id).copied());
    // SAFETY: the entry exists only while the owning frame is on the stack with the widget alive; the
    // shared reference produced here is consumed entirely within `f` and cannot outlive this call.
    ptr.map(|ptr| unsafe { f(ptr.as_ref()) })
}

/// Whether `id` is currently lent out **for mutation**.
///
/// A read-only lent entry may be read re-entrantly; a mutating one may not, because a nested write
/// would alias the `&mut` the driving frame holds. [`with_widget_mut`] refuses such a nested write by
/// answering `None` — an explicit "not available for writing right now" rather than an aliasing bug
/// or a silent no-op.
pub(crate) fn lent_mutating(id: ObjectId) -> bool {
    LENT_MUTATING.with(|set| set.borrow().contains(&id))
}

pub(crate) fn lent_exclusive(id: ObjectId) -> bool {
    LENT_EXCLUSIVE.with(|set| set.borrow().contains(&id))
}

/// Marks a widget as lent out of [`MOUNTED`] for the guard's lifetime, restoring the read-only path
/// and (for a mutating loan) clearing the nested-write refusal on drop.
///
/// A `Drop` guard rather than code after the call, so the loan is undone on **both** the normal return
/// and an unwind out of a user callback. A callback that panics during dispatch or a property write no
/// longer leaves the widget permanently lent out and unreachable — the shape BLUE-issue E-19 records
/// for dispatch (it applied equally to `with_widget_mut`).
pub(crate) struct LentGuard {
    pub(crate) id: ObjectId,
    pub(crate) mutating: bool,
    pub(crate) exclusive: bool,
}

impl LentGuard {
    pub(crate) fn install(
        id: ObjectId,
        widget: &dyn Widget,
        mutating: bool,
        exclusive: bool,
    ) -> Self {
        let ptr: core::ptr::NonNull<dyn Widget> = core::ptr::NonNull::from(widget);
        let _ = LENT_OUT.try_with(|map| {
            map.borrow_mut().insert(id, ptr);
        });
        if mutating {
            let _ = LENT_MUTATING.try_with(|set| {
                set.borrow_mut().insert(id);
            });
        }
        if exclusive {
            let _ = LENT_EXCLUSIVE.try_with(|set| {
                set.borrow_mut().insert(id);
            });
        }
        Self { id, mutating, exclusive }
    }
}

impl Drop for LentGuard {
    fn drop(&mut self) {
        let _ = LENT_OUT.try_with(|map| {
            map.borrow_mut().remove(&self.id);
        });
        if self.mutating {
            let _ = LENT_MUTATING.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
        }
        if self.exclusive {
            let _ = LENT_EXCLUSIVE.try_with(|set| {
                set.borrow_mut().remove(&self.id);
            });
        }
    }
}

// Ids lent out for **mutation**, so a nested write to the same id can be refused.
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LENT_MUTATING: RefCell<IdSet> = RefCell::new(IdSet::new());
}

// The menu popup that currently owns the pointer's **outside press**.
//
// A popup is modal for one gesture: the click that dismisses it belongs to the menu, not to the
// control that happens to be underneath. Routing only delivers an event to the widget under the
// pointer, so without this the outside click went to the control below and the menu stayed open —
// measured on the GTK demo, where clicking the canvas under an open File menu selected a list row
// and left the menu painted.
//
// This is deliberately not pointer *capture*: capture makes the popup the target of every move
// and release too, and the dispatcher releases capture when a new press starts. A grab is
// one-shot — it claims the dismissing press, then it is gone.
#[cfg(not(alloc_frugal))]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static POPUP_GRAB: core::cell::Cell<Option<ObjectId>> = const { core::cell::Cell::new(None) };
}

/// Records `menu` as the popup that owns the next outside press.
#[cfg(not(alloc_frugal))]
pub(crate) fn set_popup_grab(menu: ObjectId) {
    // Two id spaces exist: `BaseWidget::id()` (what a control sees as its own id) and the
    // registry key (what `with_widget`/hit-testing use). A control reports its own id while the
    // dispatcher holds registry ids, so normalise here rather than making every call site know
    // the difference. `open_context_menu` already passes a registry id, for which the lookup
    // answers `None` and the fallback keeps it unchanged.
    let menu = registry_id_of(menu).unwrap_or(menu);
    // A popup becoming the grab owner is a popup-layer membership change (G-316); grabs are
    // rare next to frames, so the whole flag cache is dropped rather than reasoned about.
    note_overlay_maybe_changed(None);
    let _ = POPUP_GRAB.try_with(|slot| slot.set(Some(menu)));
}

/// Forgets the popup grab, if there is one.
#[cfg(not(alloc_frugal))]
pub(crate) fn clear_popup_grab() {
    // Releasing the grab means the popup went away (G-316); see `set_popup_grab`.
    note_overlay_maybe_changed(None);
    let _ = POPUP_GRAB.try_with(|slot| slot.set(None));
}

/// Forgets the popup grab **only when it belongs to `popup`**.
///
/// Used when a control closes itself: a popup that was already superseded by another must not
/// clear the newer popup's grab on its way out.
#[cfg(not(alloc_frugal))]
pub(crate) fn clear_popup_grab_for(popup: ObjectId) {
    let popup = registry_id_of(popup).unwrap_or(popup);
    let _ = POPUP_GRAB.try_with(|slot| {
        if slot.get() == Some(popup) {
            // Only a real release changes popup-layer membership (G-316).
            note_overlay_maybe_changed(None);
            slot.set(None);
        }
    });
}

/// The current popup grab, if any.
#[cfg(not(alloc_frugal))]
pub(crate) fn popup_grab() -> Option<ObjectId> {
    POPUP_GRAB.try_with(|slot| slot.get()).ok().flatten()
}

/// Where a press that an open popup's grab governs should be routed.
///
/// * `Ok(Some(popup))` — the press is inside the popup; the caller hit-tests **from the popup**
///   so a list that extends past its parent's rectangle still receives its row clicks.
/// * `Ok(None)` — no grab applies to this window, so the press is routed normally.
/// * `Err(handled)` — the press was outside the popup and has already been delivered to it
///   (which is what hides it); the caller must return `handled` unchanged.
#[cfg(not(alloc_frugal))]
pub(crate) fn popup_press_route(
    root: ObjectId,
    point: Point,
    event: &Event,
) -> Result<Option<ObjectId>, bool> {
    let Some(popup) = popup_grab() else {
        return Ok(None);
    };
    // A grab belongs to the window whose popup set it. A press in another window must be routed
    // normally, not swallowed by this one — and the check also keeps two windows in one process
    // (or two tests on one thread) from interfering.
    if top_level_window_of(popup) != Some(root) {
        if !read_widget(popup, |widget| widget.is_visible()).unwrap_or(false) {
            clear_popup_grab();
        }
        return Ok(None);
    }
    // The popup closed itself (an item was chosen, Escape, a programmatic close).
    if !read_widget(popup, |widget| widget.is_visible()).unwrap_or(false) {
        clear_popup_grab();
        return Ok(None);
    }
    if widget_accepts_point(popup, point) {
        return Ok(Some(popup));
    }
    let handled = dispatch_event(popup, event);
    // The control's own outside-press arm hides it. Clearing here is what makes the grab one-shot
    // even when it chooses to stay open for a padded click.
    clear_popup_grab();
    Err(handled)
}

/// The popup that owns **non-press** pointer input at `point`, if any.
///
/// While a grab is active the popup owns the pointer for the window, so a move or a wheel over
/// its list has to reach it even though the list extends past its parent's rectangle — otherwise
/// the row highlight the pointer produces would stop at the container's own edge, and the press
/// that *is* routed correctly would have no preceding hover. Unlike [`popup_press_route`] this
/// neither dispatches an outside press nor clears the grab: only a press ends the grab.
#[cfg(not(alloc_frugal))]
pub(crate) fn popup_hover_origin(root: ObjectId, point: Point) -> Option<ObjectId> {
    let popup = popup_grab()?;
    if top_level_window_of(popup) != Some(root) {
        return None;
    }
    if !read_widget(popup, |widget| widget.is_visible()).unwrap_or(false) {
        return None;
    }
    widget_accepts_point(popup, point).then_some(popup)
}
