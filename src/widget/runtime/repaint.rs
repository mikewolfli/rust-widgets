// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

/// Asks the platform to repaint a mounted widget.
///
/// Call this after mutating a widget through [`with_widget_mut`] so the change
/// becomes visible. Backends route it to their own invalidation (`setNeedsDisplay:`
/// on macOS, `InvalidateRect` on Windows, `queue_draw` on GTK); a backend that
/// never mounted the widget simply does nothing.
///
/// # It is the `Full` arm of the same submission `mark_dirty_rect` makes
///
/// A control in [`RepaintMode::Full`] -- the default, and the mode a control starts in
/// -- repaints everything, so it has no rectangle to record and reaches the platform
/// here directly. Both arms therefore count against the frame at their one point of
/// submission; doing it in a wrapper instead would leave the dirty-rect arm counted
/// twice.
pub fn request_repaint(id: ObjectId) {
    request_repaint_because(id, RepaintReason::Explicit);
}

/// Submits a **partial** repaint for `id`, bounded by `rect`.
///
/// Use this instead of [`request_repaint`] when a control knows which pixels it just
/// changed. The rectangle should be the control's painted box grown by
/// [`DAMAGE_MARGIN`](crate::widget::metrics::DAMAGE_MARGIN);
/// [`painted_box_with_damage_margin`] computes exactly that, because a focus ring or a
/// blurred halo paints outside the rectangle the layout gave the control.
///
/// # It cannot regress the whole-control behaviour
///
/// The submission goes through [`mark_dirty_rect`], so damage is recorded only for a
/// mounted control in [`RepaintMode::Dirty`] or [`RepaintMode::Adaptive`]. When that
/// lookup refuses -- [`RepaintMode::Full`], which stays the default, or a mounted control
/// with no tracker -- the call falls back to the whole-control invalidation it replaces,
/// so a caller that has not opted into damage tracking keeps the behaviour it had before.
///
/// A control that was never mounted is a no-op: there is no surface to invalidate, and
/// no damage state to record. `id` is the registry id returned by [`register`], not the
/// widget's own `BaseWidget::id()`.
pub fn request_repaint_rect(id: ObjectId, rect: Rect) {
    request_repaint_rect_because(id, rect, RepaintReason::Explicit);
}

/// [`request_repaint_rect`] with an explicit cause.
///
/// `reason` names why the pixels changed and is reported when the call has to fall back
/// to the whole-control path. When the damage is recorded instead, [`mark_dirty_rect`]
/// accounts the submission as [`RepaintReason::State`]: a submitted damage rectangle is a
/// state change by construction, and keeping that accounting in one place is what stops
/// the frame ledger from counting the same submission twice.
pub fn request_repaint_rect_because(id: ObjectId, rect: Rect, reason: RepaintReason) {
    // An id that was never mounted has neither a surface nor a damage state, so neither
    // arm below could do anything useful; returning here keeps a stray submission from
    // reaching the platform at all.
    if !is_mounted(id) {
        return;
    }
    if !mark_dirty_rect(id, rect) {
        request_repaint_because(id, reason);
    }
}

/// Submits a narrow repaint from inside a control's `tick`.
///
/// # Why the rectangle is known but the rest is deferred
///
/// A `tick` runs with the control lent out of the registry (see [`with_widget_mut`]), so the
/// runtime cannot look up the box the repaint is bounded by or the window above the control
/// while the tick is on the stack. The control does know its own box, so it hands the rectangle
/// over now; the platform submission and the window invalidation happen in
/// `flush_deferred_damage`, which the frame driver calls as soon as the sweep releases the
/// registry.
///
/// A control outside a tick should call [`request_repaint_rect`] instead: this path costs one
/// queue entry per control per frame.
pub fn request_repaint_rect_deferred(id: ObjectId, rect: Rect, reason: RepaintReason) {
    // One entry per control per frame: a later rectangle supersedes an earlier one, and the
    // damage tracker coalesces whatever the two of them recorded anyway. This is the **same
    // merge policy** as `submit_animation_repaints` below (the other queue that folds per-frame
    // submissions); a change to either must be mirrored in the other (G-104).
    let _ = DEFERRED_DAMAGE.try_with(|queue| {
        let mut queue = queue.borrow_mut();
        let existing =
            DEFERRED_DAMAGE_INDEX.try_with(|index| index.borrow().get(&id).copied()).ok().flatten();
        if let Some(at) = existing {
            // The index can only be stale if the queue was replaced by a caller that did not
            // go through `flush_deferred_damage`; verify the entry before writing through it.
            if let Some(entry) = queue.get_mut(at) {
                if entry.0 == id {
                    entry.1 = rect;
                    entry.2 = reason;
                    return;
                }
            }
        }
        let at = queue.len();
        queue.push((id, rect, reason));
        let _ = DEFERRED_DAMAGE_INDEX.try_with(|index| {
            index.borrow_mut().insert(id, at);
        });
    });
}

/// Flushes the damage queued by [`request_repaint_rect_deferred`].
///
/// Called after the animation sweep, and before a frame's repaint counts are taken, so a
/// submission made from inside a `tick` is attributed to the frame that made it. Cheap when the
/// queue is empty: one `try_with` and a length check.
pub(crate) fn flush_deferred_damage() {
    let queued = DEFERRED_DAMAGE
        .try_with(|queue| core::mem::take(&mut *queue.borrow_mut()))
        .unwrap_or_default();
    // The index describes the queue that was just taken. It is cleared in the same breath so a
    // later submission cannot merge into an index that no longer names a live entry (G-104).
    let _ = DEFERRED_DAMAGE_INDEX.try_with(|index| index.borrow_mut().clear());
    if queued.is_empty() {
        return;
    }
    for (id, rect, reason) in queued {
        request_repaint_rect_because(id, rect, reason);
        // A window that paints its own child list has to be told separately: the control inside
        // it has no surface of its own there, so the narrow call above reaches nothing. The
        // rectangle is the control's, not the window's, which keeps the window's own layer
        // validation scoped to the pixels that actually changed.
        if let Some(window) = top_level_window_of(id) {
            request_repaint_rect_because(window, rect, reason);
        }
    }
}

/// Submits the repaints for one animation sweep, aggregating by window.
///
/// # What is per control and what is per window
///
/// Each advanced control keeps its own submission: a control can be a **mounted surface** with its
/// own platform invalidation, and its per-widget damage is what an incremental render of that
/// surface reads. What is hoisted is the *window* half. Every advanced control used to walk the
/// parent chain a second time and submit the same rectangle to its window, so N controls in one
/// window produced N identical window submissions and N layer invalidations. This unions their
/// boxes first and tells the window once, which is the same pixels with one submission (G-07).
///
/// A windowless control still takes the normal path (it is its own target). A control whose box
/// cannot be read falls back to the subtree request, exactly as before.
pub(crate) fn submit_animation_repaints(moved: &[ObjectId]) {
    let mut windows: alloc::vec::Vec<(ObjectId, Rect)> =
        WINDOW_ANIMATION_DAMAGE.with(|slot| core::mem::take(&mut *slot.borrow_mut()));
    windows.clear();
    for &id in moved {
        let Some(rect) = painted_box_with_damage_margin(id) else {
            request_repaint_subtree_because(id, RepaintReason::Animation);
            continue;
        };
        // The control's own rectangle, and its surface when it has one.
        request_repaint_rect_because(id, rect, RepaintReason::Animation);
        if let Some(window) = top_level_window_of(id) {
            // Same per-window merge policy as `request_repaint_rect_deferred` above: one entry
            // per window per frame, later geometry wins. The window list is small, so this scan
            // is kept and the two sites are cross-referenced rather than allowed to drift (G-104).
            match windows.iter_mut().find(|(candidate, _)| *candidate == window) {
                Some((_, union)) => {
                    let merged = union.union(&rect);
                    *union = merged;
                }
                None => windows.push((window, rect)),
            }
        }
    }
    for &(window, rect) in &windows {
        request_repaint_rect_because(window, rect, RepaintReason::Animation);
    }
    WINDOW_ANIMATION_DAMAGE.with(|slot| {
        let mut slot = slot.borrow_mut();
        windows.clear();
        if windows.capacity() > slot.capacity() {
            *slot = windows;
        }
    });
}

/// The rectangle to hand [`request_repaint_rect`] for `id`: its painted box grown by
/// [`DAMAGE_MARGIN`](crate::widget::metrics::DAMAGE_MARGIN) on every side.
///
/// # Why the geometry is the base
///
/// A library-painted control paints inside the rectangle it was given, and the one
/// rectangle the runtime can read back for every widget kind is [`geometry_of`]. The
/// margin turns that rectangle into the full area the control can affect, including a
/// focus ring or a blurred halo that extends past it.
///
/// Returns `None` when `id` is not mounted, so a caller can skip a submission that would
/// be refused anyway.
pub fn painted_box_with_damage_margin(id: ObjectId) -> Option<Rect> {
    geometry_of(id).map(grow_by_damage_margin)
}

/// Grows `rect` by [`DAMAGE_MARGIN`](crate::widget::metrics::DAMAGE_MARGIN) on all four
/// sides, saturating instead of wrapping at the coordinate limits.
pub(crate) fn grow_by_damage_margin(rect: Rect) -> Rect {
    let margin = crate::widget::metrics::DAMAGE_MARGIN;
    let offset = margin as i32;
    Rect::new(
        rect.x.saturating_sub(offset),
        rect.y.saturating_sub(offset),
        rect.width.saturating_add(margin).saturating_add(margin),
        rect.height.saturating_add(margin).saturating_add(margin),
    )
}

/// Epoch of the last global-input change (see [`arm_global_repaint`]).
///
/// Only the two ends of the mechanism read it: the arming function (device profiles only) and
/// `flush_global_repaint`'s non-test arm. A *test* build compares against the thread-local
/// generation instead, so in a stripped test build — no device profile, hence no arming site —
/// nothing would reference it and the dead-code lint would fire.
#[cfg(any(device_profile, not(test)))]
pub(crate) static GLOBAL_REPAINT_EPOCH: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// The global-repaint epoch this loop thread last repainted everything for.
    pub(crate) static GLOBAL_REPAINT_SEEN: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };

    /// The global-repaint generation this thread has observed.
    ///
    /// Per-widget layers live on the thread that painted them, so a theme reset only has to
    /// invalidate that thread's layers. [`arm_global_repaint`] bumps this beside the process-wide
    /// atomic: the atomic wakes every thread, while this counter is what the layer cache on the
    /// painting thread compares against. A cross-thread arming is still honoured — the next
    /// [`drive_frame`] flushes it through `mark_dirty_rect`, which bumps the tree epoch.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static GLOBAL_REPAINT_LOCAL: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// Arms one full repaint of every window and mounted surface for the next frame.
///
/// # Why this is armed rather than performed here
///
/// The callers are global-input mutations (the active theme, the text scale, the
/// locale): every control paints differently afterwards, but no control's own state
/// changed, so nothing in the widget layer would ask for a frame. The theme mutation
/// runs while the theme manager's **non-reentrant** mutex is held, and invalidating a
/// surface there could re-enter that lock through a synchronous platform paint, so the
/// mutation only bumps this epoch and wakes the loop; [`drive_frame`] performs the
/// invalidation outside the lock on the thread that owns the widgets.
///
/// A still, event-driven host therefore repaints after a theme change instead of
/// keeping the frame it already drew until the user happens to move the mouse.
///
/// Every arming site is a device-profile one (the theme manager, the theme crossfade in
/// `drive_frame`, `segmented_button`), so this is `device_profile` rather than
/// `any(device_profile, test)`: a stripped test build has no caller, and the wider gate only made
/// the three items it shares with `flush_global_repaint` read as dead code there.
#[cfg(device_profile)]
pub(crate) fn arm_global_repaint() {
    GLOBAL_REPAINT_EPOCH.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let _ = GLOBAL_REPAINT_LOCAL.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
    crate::platform::frame_wait::wake();
}

/// The global-repaint generation layers on this thread must match.
pub(crate) fn global_repaint_generation() -> u64 {
    GLOBAL_REPAINT_LOCAL.try_with(|epoch| epoch.get()).unwrap_or(0)
}

/// The epoch a theme mutation bumps, so `theme::manager` can assert it armed the repaint.
///
/// The only reader is `theme::manager`'s test module, which a stripped build does not compile.
#[cfg(all(test, device_profile))]
pub(crate) fn global_repaint_epoch_for_test() -> u64 {
    // The thread-local generation, for the same reason `flush_global_repaint` compares against it:
    // a concurrent test's theme mutation must not make "exactly one repaint" unassertable.
    global_repaint_generation()
}

/// Performs the full repaint `arm_global_repaint` requested, once per epoch.
pub(crate) fn flush_global_repaint() {
    // A test build compares against the **thread-local** epoch. The process-wide counter is
    // bumped by any test's theme mutation, which would make a repaint arrive between two
    // frames of an unrelated test and break an assertion about "exactly one repaint". The
    // production path keeps the process-wide epoch, because the thread that arms the repaint
    // need not be the thread that draws.
    #[cfg(test)]
    let epoch = global_repaint_generation();
    #[cfg(not(test))]
    let epoch = GLOBAL_REPAINT_EPOCH.load(core::sync::atomic::Ordering::Relaxed);
    let last = GLOBAL_REPAINT_SEEN.with(|seen| seen.replace(epoch));
    if epoch == last {
        return;
    }
    // A backend can honour a window or a mounted surface; a control inside either is
    // covered by that ancestor's repaint (the same split `request_repaint_subtree`
    // makes). `surface_window_for` reads its own thread-local, so it is safe inside the
    // registry borrow; the parent walk is not, hence the two-phase collection.
    let mut targets: alloc::vec::Vec<(ObjectId, Rect)> = alloc::vec::Vec::new();
    let mut maybe_windowless: alloc::vec::Vec<(ObjectId, Rect)> = alloc::vec::Vec::new();
    let _ = MOUNTED.try_with(|map| {
        for (id, entry) in map.borrow().iter() {
            let rect = entry.widget.geometry();
            if entry.widget.kind() == crate::widget::WidgetKind::Window
                || surface_window_for(*id).is_some()
            {
                targets.push((*id, rect));
            } else {
                maybe_windowless.push((*id, rect));
            }
        }
    });
    for (id, rect) in maybe_windowless {
        if top_level_window_of(id).is_none() {
            targets.push((id, rect));
        }
    }
    for (id, rect) in targets {
        mark_dirty_rect(id, rect);
        request_repaint_because(id, RepaintReason::Explicit);
    }
}

/// How much of a frame the render loop repaints.
///
/// # Why this is a policy rather than a switch
///
/// Damage tracking is not unconditionally faster. It wins when a few controls
/// change and the window is large, and it *loses* when the whole surface is moving
/// — an animation, a scroll, a video — because then the union of damage rects grows
/// to the full frame while the tracker still pays to merge and clip regions. A
/// boolean would force every caller to know which case it is in; this type lets the
/// library decide, and lets `Adaptive` learn from what actually happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepaintMode {
    /// Repaint the whole surface every frame, ignoring damage. The historical
    /// behaviour, and the default.
    #[default]
    Full,
    /// Repaint only the damage regions, merging overlapping ones.
    Dirty,
    /// Start in [`Self::Dirty`] and fall back to [`Self::Full`] for a frame whose
    /// damage covers most of the surface.
    ///
    /// The fallback is per-frame rather than sticky: a window that animates for a
    /// second and then settles returns to damage-tracked repaints on its own,
    /// which a one-way latch could not do.
    Adaptive,
}

/// Damage tracked since the last frame, and the policy that decides how to use it.
///
/// Held per mounted widget, beside the widget itself, so a damage region cannot
/// outlive the control it describes.
pub(crate) struct RepaintState {
    pub(crate) mode: RepaintMode,
    pub(crate) tracker: crate::performance::DirtyRegionTracker,
    /// Consecutive frames whose damage covered too much of the surface to be worth
    /// regioning, while the mode was [`RepaintMode::Adaptive`].
    ///
    /// # What this counter is for
    ///
    /// `Adaptive` and `Dirty` are not the same policy, and before this counter they
    /// behaved identically: both delegated to `render_dirty_regions`, which decides
    /// per frame whether to fall back. The difference is what each mode knows.
    ///
    /// `Dirty` means the caller has already decided that damage tracking is worth it,
    /// so every frame is measured against the damage. `Adaptive` means the caller does
    /// not know, and expects the library to work it out — so a run of frames whose
    /// damage covers the surface is evidence that regioning cannot pay off *here*, and
    /// measuring each of them again is work whose answer is already known.
    ///
    /// After [`ADAPTIVE_LARGE_DAMAGE_RUN`] such frames the next frame skips the
    /// regioning entirely and paints whole. One frame with small damage resets the
    /// counter, so settling back into a few changing controls returns to regioning on
    /// its own rather than staying latched.
    pub(crate) large_damage_run: u32,
    /// Reusable buffer for the region rectangles of one incremental frame (G-72/G-102).
    ///
    /// `render_frame_incremental_cow` needs the damage rectangles twice (once to measure
    /// coverage, once to hand them to `render_dirty_regions_into`), and the `_into` form writes
    /// them into a caller-owned buffer. Holding that buffer here means a frame loop keeps its
    /// capacity across frames instead of allocating a fresh `Vec` per frame. It is taken with
    /// `mem::take` for the duration of the render — the fields are only borrowed together for
    /// short bookkeeping — and returned through [`DirtyRectScratch`] on every exit path.
    pub(crate) dirty_scratch: alloc::vec::Vec<Rect>,
}

/// Consecutive over-threshold frames in [`RepaintMode::Adaptive`] before it stops
/// measuring and paints whole.
///
/// Three, because two could be a coincidence — a resize legitimately damages
/// everything once or twice — while a fourth measurement of the same answer is pure
/// overhead. The counter is reset by any frame whose damage is small, so the cost of
/// guessing wrong here is one extra full paint.
pub const ADAPTIVE_LARGE_DAMAGE_RUN: u32 = 3;

impl RepaintState {
    pub(crate) fn new() -> Self {
        Self {
            mode: RepaintMode::default(),
            tracker: crate::performance::DirtyRegionTracker::new(),
            large_damage_run: 0,
            dirty_scratch: alloc::vec::Vec::new(),
        }
    }
}

/// Takes the reusable region buffer out of `id`'s repaint state (G-72).
///
/// The state is left with an empty buffer; [`return_dirty_scratch`] puts the grown one back. A
/// missing state (an unmounted widget) yields a fresh, empty buffer, which is correct because the
/// caller is about to paint something the registry cannot describe.
pub(crate) fn take_dirty_scratch(id: ObjectId) -> alloc::vec::Vec<Rect> {
    REPAINT
        .try_with(|map| {
            map.try_borrow_mut().ok().and_then(|mut map| {
                map.get_mut(&id).map(|state| core::mem::take(&mut state.dirty_scratch))
            })
        })
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Returns a region buffer whose capacity is worth keeping to `id`'s repaint state (G-72).
pub(crate) fn return_dirty_scratch(id: ObjectId, mut buffer: alloc::vec::Vec<Rect>) {
    let _ = REPAINT.try_with(|map| {
        if let Ok(mut map) = map.try_borrow_mut() {
            if let Some(state) = map.get_mut(&id) {
                if buffer.capacity() > state.dirty_scratch.capacity() {
                    buffer.clear();
                    state.dirty_scratch = buffer;
                }
            }
        }
    });
}

/// Owns the region buffer for one incremental render and returns it on every exit path (G-72).
///
/// The render has several early returns (unmounted, no `Draw`, full paint, nothing damaged), so
/// the buffer is returned from `Drop` rather than from each one. `try_borrow_mut` in
/// [`return_dirty_scratch`] keeps the drop safe even if a callback unwinds while a `REPAINT`
/// borrow is still live: the buffer is then simply dropped instead of panicking.
pub(crate) struct DirtyRectScratch {
    pub(crate) id: ObjectId,
    pub(crate) buffer: Option<alloc::vec::Vec<Rect>>,
}

impl DirtyRectScratch {
    pub(crate) fn take(id: ObjectId) -> Self {
        Self { id, buffer: Some(take_dirty_scratch(id)) }
    }

    pub(crate) fn buffer(&mut self) -> &mut alloc::vec::Vec<Rect> {
        // `None` only while `Drop` is taking the buffer out.
        self.buffer.get_or_insert_with(alloc::vec::Vec::new)
    }
}

impl Drop for DirtyRectScratch {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            return_dirty_scratch(self.id, buffer);
        }
    }
}

thread_local! {
    /// Idle dirty-region tracker swapped in for a widget's own while the damage is painted
    /// (ENG-A3).
    ///
    /// Both trackers keep the capacity their previous frames grew, so the steady state neither
    /// constructs a tracker per frame nor copies its regions one by one.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DIRTY_TRACKER_SPARE: RefCell<Option<crate::performance::DirtyRegionTracker>> =
        const { RefCell::new(None) };
}

/// Takes the idle tracker, or a fresh one when the slot is empty (a re-entrant render).
pub(crate) fn take_dirty_tracker_spare() -> crate::performance::DirtyRegionTracker {
    DIRTY_TRACKER_SPARE
        .try_with(|spare| spare.borrow_mut().take())
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Returns a spent tracker to the idle slot, keeping its capacity for the next frame.
pub(crate) fn return_dirty_tracker_spare(tracker: crate::performance::DirtyRegionTracker) {
    let _ = DIRTY_TRACKER_SPARE.try_with(|spare| {
        if let Ok(mut spare) = spare.try_borrow_mut() {
            *spare = Some(tracker);
        }
    });
}

/// Owns the swapped-out dirty tracker for one incremental render and returns it on every exit
/// path (ENG-A3).
///
/// The tracker is emptied by the paint itself; the guard exists because the render has several
/// early returns between the swap and that point. `try_borrow_mut` in
/// [`return_dirty_tracker_spare`] keeps the drop safe if a callback unwinds while the slot is
/// borrowed.
pub(crate) struct DirtyTrackerScratch {
    pub(crate) tracker: Option<crate::performance::DirtyRegionTracker>,
}

impl DirtyTrackerScratch {
    pub(crate) fn new(tracker: crate::performance::DirtyRegionTracker) -> Self {
        Self { tracker: Some(tracker) }
    }

    pub(crate) fn get(&mut self) -> &mut crate::performance::DirtyRegionTracker {
        // `None` only while `Drop` is taking the tracker out.
        self.tracker.get_or_insert_default()
    }
}

impl Drop for DirtyTrackerScratch {
    fn drop(&mut self) {
        if let Some(tracker) = self.tracker.take() {
            return_dirty_tracker_spare(tracker);
        }
    }
}

/// Records that a widget asked to be repainted, resolving its registry id.
///
/// # Why the id needs resolving
///
/// A widget knows its own `BaseWidget::id()` (its `Object`'s counter), but the runtime
/// registry keys widgets by an id it allocates in [`register`]. The two spaces are
/// **not** the same number: a freshly built control reports `1` while the registry
/// handed out `0x5345_4C46_0000_0001`. So a damage record keyed on the widget's own id
/// would be filed under a widget that does not exist, be silently dropped, and leave
/// partial repaint unreachable — which is exactly what happened before
/// [`register`] started recording the mapping.
///
/// # Why a lookup rather than passing the id down
///
/// [`BaseWidget::request_redraw`](crate::widget::BaseWidget::request_redraw) takes
/// `&self` and is called from ~1000 places that have no idea what the registry called
/// them. Threading a registry id through all of them is what would make partial repaint
/// a list someone must keep complete; resolving it here keeps the producer at one
/// chokepoint.
///
/// Returns `true` when the damage was recorded (i.e. the widget opted in and is
/// registered).
pub(crate) fn mark_widget_damage(own_id: ObjectId, rect: Rect) -> bool {
    let Some(registry_id) = registry_id_of(own_id) else {
        // Not registered yet: a control being constructed, or one a test built and never
        // mounted. There is no surface to repaint, so there is nothing to record — and
        // nothing to remember either, because these run before `register`.
        return false;
    };
    mark_dirty_rect(registry_id, rect)
}

/// The registry id for a widget that reported `own_id` as its `BaseWidget::id()`.
///
/// `None` when the widget was never registered. See `mark_widget_damage` for why the
/// two id spaces exist and why this lookup is necessary.
pub fn registry_id_of(own_id: ObjectId) -> Option<ObjectId> {
    OWN_IDS.try_with(|map| map.borrow().get(&own_id).copied()).ok().flatten()
}

/// Enables damage-tracked repaint for `id`, returning whether it took effect.
///
/// # The auto-decision entry point
///
/// [`RepaintMode::Adaptive`] is the mode to use when you do not know whether regioning
/// pays off: the library consults the damage each frame and stops measuring after a run
/// of frames whose damage covers the surface — an animation, a scroll, a video — where
/// the union of regions grows to the full frame anyway. It resumes regioning as soon as
/// one frame reports small damage, so a window that animates briefly and then settles
/// returns to partial repaints on its own.
///
/// Returns `false` when `id` is not a mounted widget, which is the same condition
/// [`set_repaint_mode`] reports.
pub fn set_repaint_mode_adaptive(id: ObjectId) -> bool {
    set_repaint_mode(id, RepaintMode::Adaptive)
}

/// Whether damage-tracked repaint is worth enabling for `id` — the library's own
/// decision, so a caller does not have to guess.
///
/// # The judgement, and why it is this shape
///
/// Damage tracking is not unconditionally faster. It wins when a few controls change in
/// a large surface, and it *loses* on a small one: the tracker still pays to merge, sort
/// and clip regions, and once damage covers most of the frame the clip discards work
/// that was already done — so the regioning costs more than the pixels it saves.
///
/// The decision is deliberately **two-sided**:
///
/// * **Too few controls** — with one control, damage is that control's whole rect, which
///   is the whole frame; regioning buys nothing and costs the bookkeeping.
/// * **Too small a surface** — below [`AUTO_REPAINT_MIN_PIXELS`] the per-frame cost of
///   merging and clipping regions is a meaningful fraction of just painting everything.
///
/// # Why it is safe to say "yes" by default
///
/// Saying yes selects [`RepaintMode::Adaptive`], which is **self-correcting**: a frame
/// whose damage covers too much of the surface falls back to a whole paint for that
/// frame, and a run of such frames stops the measurement entirely until the damage
/// shrinks. So a wrong "yes" costs a bounded amount of bookkeeping on the frames that
/// follow, never a wrong frame — and the pixels are identical either way, which
/// `an_incremental_repaint_matches_a_full_repaint` asserts.
///
/// # Who calls it
///
/// [`register`] does, once per control, the moment it is mounted. That is the only
/// automatic call site: it is the first point at which the control's geometry and its
/// children are both known, and it costs the caller nothing. Every control on a device
/// profile therefore answers this question exactly once, without a host having to know
/// its own frame pattern in advance.
///
/// Nothing is enabled for a control the library judged not worth it: it keeps `Full`,
/// and `mark_widget_damage` is a single map lookup that returns `false`. A host that
/// disagrees can call [`set_repaint_mode`] directly and get exactly what it asked for,
/// which is why the threshold is a named constant rather than a magic number.
pub fn should_track_damage(id: ObjectId) -> bool {
    // A record is only worth keeping for a control that ever asked to be repainted,
    // because a surface nothing redraws gains nothing from a region clip and still pays
    // the bookkeeping. The flag is read off the widget, not from a table keyed by
    // registry id: a control whose constructor prepares its first frame asks *before*
    // `register` runs, when no registry id exists to file the request against.
    if !has_ever_requested_redraw(id) {
        return false;
    }

    let Some(geometry) = geometry_of(id) else {
        return false;
    };
    let pixels = u64::from(geometry.width) * u64::from(geometry.height);
    if pixels < AUTO_REPAINT_MIN_PIXELS {
        return false;
    }

    // A control with no children is one rect: damage is the whole control, so the clip
    // cannot narrow anything. Counted through the widget rather than the tree so a
    // caller need not build one.
    let has_children =
        read_widget(id, |widget| !widget.base().children().is_empty()).unwrap_or(false);
    has_children
}

/// Pixels below which damage tracking is not offered by [`should_track_damage`].
///
/// A quarter megapixel — roughly 500×500. Below it the merge/sort/clip bookkeeping is a
/// measurable share of the cost of painting the whole surface, and the surface is small
/// enough that painting it whole is already cheap. The value is a judgement rather than a
/// measurement, so it is named and exposed: a caller who disagrees can call
/// [`set_repaint_mode`] directly and get exactly what it asked for.
pub const AUTO_REPAINT_MIN_PIXELS: u64 = 250_000;

/// Whether `id`'s widget has ever asked to be repainted.
///
/// The flag is read from the widget rather than kept in a table keyed by registry id,
/// because a request can arrive before there is a registry id to file it against — a
/// constructor that prepares its own first frame calls `request_redraw` while the
/// control is still being built. A table could only be written after `register`, so such
/// a control would be judged "silent" for the rest of its life. Asking the widget
/// answers the same question the request actually made.
pub(crate) fn has_ever_requested_redraw(id: ObjectId) -> bool {
    read_widget(id, |widget| widget.has_ever_requested_redraw()).unwrap_or(false)
}

/// Enables damage tracking for `id` **if the library judges it worthwhile**, returning
/// whether it did.
///
/// This is the one-call form of [`should_track_damage`] + [`set_repaint_mode_adaptive`],
/// for a host that wants the benefit without analysing its own frame pattern:
///
/// ```no_run
/// # use rust_widgets::widget::runtime::*;
/// # fn example(window: rust_widgets::core::ObjectId) {
/// if enable_damage_tracking_if_useful(window) {
///     // partial repaints from here on; `render_frame_incremental` uses them
/// }
/// # }
/// ```
///
/// A `false` return is not an error — it means the whole-frame path is already the right
/// answer for this widget, so there is nothing to gain and no bookkeeping to pay for.
pub fn enable_damage_tracking_if_useful(id: ObjectId) -> bool {
    if !should_track_damage(id) {
        return false;
    }
    set_repaint_mode_adaptive(id)
}

/// Sets the repaint policy for a mounted widget.
///
/// Returns `false` when `id` is not mounted, so a caller can tell "policy set" from
/// "there was nothing to set it on". Switching to [`RepaintMode::Full`] clears any
/// accumulated damage, so a later switch back to [`RepaintMode::Dirty`] does not
/// repaint a region that stopped being interesting several frames ago.
pub fn set_repaint_mode(id: ObjectId, mode: RepaintMode) -> bool {
    let mounted = MOUNTED.try_with(|m| m.borrow().contains_key(&id)).unwrap_or(false);
    if !mounted {
        return false;
    }
    let _ = REPAINT.try_with(|map| {
        let mut map = map.borrow_mut();
        let state = map.entry(id).or_insert_with(RepaintState::new);
        state.mode = mode;
        if mode == RepaintMode::Full {
            state.tracker.clear();
        }
    });
    true
}

/// The repaint policy in force for `id`.
///
/// A widget with no recorded policy reports [`RepaintMode::Full`], which is what it
/// actually gets, rather than a "not configured" state a caller would have to
/// translate.
pub fn repaint_mode(id: ObjectId) -> RepaintMode {
    REPAINT
        .try_with(|map| map.borrow().get(&id).map(|state| state.mode))
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Marks `rect` as needing repaint on the next frame.
///
/// A no-op for a widget in [`RepaintMode::Full`], because that mode repaints
/// everything anyway and accumulating rects it will never read would be waste.
///
/// # It also tells the platform
///
/// Tracking damage and repainting it are two halves of one feature. The tracker says
/// *what* to redraw when the library renders the frame; the platform call says *when*
/// to redraw at all. Without the second half a caller would record damage that nothing
/// ever asked to be shown.
///
/// The platform call is [`crate::platform::Platform::invalidate_surface_rect`], which
/// narrows the repaint when the backend can. When it cannot — and the default cannot —
/// the whole control is invalidated instead, because a correct repaint is worth more
/// than a narrow one.
///
/// Returns `true` when the damage was recorded.
///
/// # The region count is bounded here (D09-RENDER-01)
///
/// The tracker a widget accumulates damage in has a `max_regions` limit, but that limit only
/// took effect in `optimize()` — which this module never calls — so a widget invalidating many
/// small rectangles in one frame grew the tracker without bound. Downstream that meant a
/// quadratic `merge`, and up to one full widget-tree traversal per region. The bound is applied
/// at the point damage is recorded: past the limit, the tracked regions are folded into their
/// single bounding rectangle, which keeps one region and loses no pixels. `merge` and the
/// per-region repaint then each stay bounded.
///
/// The fold happens rather than an early return because dropping damage would be a wrong
/// picture — see [`crate::performance::DirtyRegionTracker::add_coalescing_beyond_capacity`].
pub fn mark_dirty_rect(id: ObjectId, rect: Rect) -> bool {
    // Any damage submission is evidence that some surface's content changed, which is what the
    // tree-frame cache compares against (see `render_frame_tree_cached`). Bumped even when this
    // widget has no tracker, because a window-level cache is not the per-widget policy. The
    // window-scoped bump (G-311) mirrors it: only this widget's window loses its cached frame.
    note_renderable_change();
    note_renderable_change_for(id);
    // The layer cache is narrower: only the layers this rectangle touches can now be stale.
    invalidate_widget_layers_in(id, rect);
    let recorded = REPAINT
        .try_with(|map| {
            let mut map = map.borrow_mut();
            let Some(state) = map.get_mut(&id) else {
                return false;
            };
            if state.mode == RepaintMode::Full {
                return false;
            }
            state.tracker.add_coalescing_beyond_capacity(rect);
            true
        })
        .ok()
        .unwrap_or(false);

    if recorded {
        // Narrow when the backend can express it; otherwise fall back to invalidating
        // the whole control. `request_repaint` is the same call the `Full` path uses,
        // so the fallback needs no special handling here.
        if !crate::invalidate_surface_rect(id, rect) {
            crate::invalidate_surface(id);
        }
        // Account for it against this frame, so `drive_frame`'s `repaints_submitted`
        // reports what the frame actually cost rather than a second bookkeeping pass
        // that would have to re-derive it. A damage rectangle is a state change by
        // construction: the caller is telling the library which pixels it just invalidated.
        note_frame_repaint_reason(id, RepaintReason::State);
    }
    recorded
}

/// The damage accumulated for `id` since the last frame, as rectangles.
///
/// Exposed so a backend can hand the regions to its own compositor instead of
/// using `render_frame_incremental` — a GPU backend wants scissor rects, not a
/// software repaint.
///
/// This convenience form allocates its result. A caller that asks once per frame should use
/// [`dirty_rects_into`] with a buffer it owns, which is what the incremental render path does.
pub fn dirty_rects(id: ObjectId) -> Vec<Rect> {
    let mut out = alloc::vec::Vec::new();
    let _ = dirty_rects_into(id, &mut out);
    out
}

/// Buffer-reusing form of [`dirty_rects`]; returns whether any damage was recorded.
///
/// `out` is cleared first. The emptiness predicate is answered from the tracker directly, so an
/// undamaged widget costs one map lookup and no tracker clone or merge (G-102). This is the
/// predicate the no-damage fast path in `render_frame_cached_shared` needs, and the buffer form
/// the incremental render needs; keeping both here is what stops the two from drifting.
pub fn dirty_rects_into(id: ObjectId, out: &mut alloc::vec::Vec<Rect>) -> bool {
    out.clear();
    if !has_dirty_rects(id) {
        return false;
    }
    DIRTY_OUT.with(|tracker| {
        let mut tracker = tracker.borrow_mut();
        tracker.clear();
        let _ = REPAINT.try_with(|map| {
            if let Some(state) = map.borrow().get(&id) {
                for region in state.tracker.regions() {
                    tracker.add(region.rect);
                }
            }
        });
        tracker.merge();
        out.extend(tracker.regions().iter().map(|region| region.rect));
    });
    !out.is_empty()
}

/// Whether `id` has damage recorded since the last frame.
///
/// `merge()` never removes a region (it only unions overlapping ones), so the tracker's
/// non-emptiness is exactly `dirty_rects(id).is_empty()` — without the clone and merge (G-102).
pub(crate) fn has_dirty_rects(id: ObjectId) -> bool {
    REPAINT
        .try_with(|map| map.borrow().get(&id).is_some_and(|state| !state.tracker.is_empty()))
        .unwrap_or(false)
}
