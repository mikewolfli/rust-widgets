// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

// `Point` is used by the hit-test API below, which is a core capability of the
// registry rather than a `full_widgets` extra: every profile that can mount a widget
// must be able to answer "which widget is under this point?".
pub(crate) use crate::compat::Rc;
pub(crate) use crate::core::{ObjectId, Point, Rect, Size};
pub(crate) use crate::event::dnd::{DragSession, DropEffect};
pub(crate) use crate::event::{Event, FocusReason};
pub(crate) use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

/// Why a widget could not be mounted onto a host surface.
///
/// Lives here, beside the registry, because that is the layer that knows about
/// widget registration and ownership. `app` re-exports it, so callers that drive
/// mounting through a `WindowHandle` keep the same path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceMountError {
    /// The calling thread has no widget registry — mounting must happen on the
    /// thread that drives the UI.
    NoRegistryOnThread,
    /// This backend has no surface to offer
    /// (`Platform::supports_surfaces` returned `false`). Carries the
    /// backend name for the message.
    UnsupportedByBackend(&'static str),
    /// The backend claims support but refused this particular mount (unknown
    /// parent, wrong parent kind, allocation failure). Carries the backend name.
    RejectedByBackend(&'static str),
    /// A lookup by widget name did not match anything in the widget factory.
    UnknownWidgetName,
}

impl core::fmt::Display for SurfaceMountError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoRegistryOnThread => {
                write!(f, "widgets must be mounted on the UI thread (no registry on this thread)")
            }
            Self::UnsupportedByBackend(backend) => {
                write!(f, "backend '{backend}' has no surface to mount widgets onto")
            }
            Self::RejectedByBackend(backend) => {
                write!(f, "backend '{backend}' refused the mount (see logs for the reason)")
            }
            Self::UnknownWidgetName => {
                write!(f, "the widget factory has no widget registered under that name")
            }
        }
    }
}

impl std::error::Error for SurfaceMountError {}
pub(crate) use crate::widget::Widget;
pub(crate) use std::cell::RefCell;
pub(crate) use std::collections::HashMap;

/// One widget handed to the registry for display by the host surface.
pub(crate) struct Mounted {
    pub(crate) widget: Box<dyn Widget>,
    /// The top-level `Window` this widget lives under, cached when the mount is created (G-106).
    ///
    /// The first field is the **direct parent** the answer was resolved from. An adoption
    /// (`BaseWidget::add_child_linked`) rewrites that parent, so a lookup compares it before
    /// trusting the cached window; a mismatch re-resolves instead of routing a repaint to the
    /// window the widget used to live in. The second field is the resolved window itself, and
    /// `None` means "no window above this widget" — an answer, not an unresolved state.
    pub(crate) top_window: Option<(Option<ObjectId>, Option<ObjectId>)>,
}

/// A tiny set of widget ids used for the registry's per-drive loan markers.
///
/// # Why a vector and not `BTreeSet`
///
/// These sets hold the ids of the **currently driven** widgets, which is almost always one
/// (the widget whose `with_widget`/`with_widget_mut`/`dispatch_event` call is on the stack)
/// and never more than the nesting depth of a callback. A `BTreeSet` paid a tree insert, a
/// tree removal and an allocation per node; the vector is a linear scan over one or two
/// `u64`s, and its capacity survives across drives because the set is reused. The paint
/// path installs four of these per node per frame (`DISPATCHING`, `LENT_MUTATING`,
/// `LENT_EXCLUSIVE`, `LENT_OUT`), so the per-node cost is directly the per-frame cost.
pub(crate) struct IdSet(pub(crate) Vec<ObjectId>);

impl IdSet {
    pub(crate) const fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn insert(&mut self, id: ObjectId) {
        if !self.0.contains(&id) {
            self.0.push(id);
        }
    }

    pub(crate) fn remove(&mut self, id: &ObjectId) -> bool {
        if let Some(index) = self.0.iter().position(|candidate| candidate == id) {
            self.0.remove(index);
            true
        } else {
            false
        }
    }

    pub(crate) fn contains(&self, id: &ObjectId) -> bool {
        self.0.contains(id)
    }
}

// Per-thread widget registry.
//
// `clippy::missing_const_for_thread_local` fires on this block for the
// OpenHarmony target only: `std::collections::HashMap::new()` is not a `const
// fn` on any target, but the lint can resolve `HashMap` on the host and not on
// OpenHarmony's std, so there it suggests a `const` initializer that would not
// compile. The allow is per-cell rather than crate-wide.
thread_local! {
    /// Widgets mounted for display, keyed by their `ObjectId`.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static MOUNTED: RefCell<HashMap<ObjectId, Mounted>> = RefCell::new(HashMap::new());

    /// Ids whose entry is temporarily held **outside** the registry during event dispatch.
    ///
    /// [`dispatch_event`] takes the widget out of `MOUNTED` before calling `handle_event`, so a
    /// handler may re-enter the registry (read its own state, update another control, call
    /// `request_redraw`) without hitting a `RefCell` borrow conflict. While an id is listed here,
    /// [`unregister`] records the removal rather than looking for an entry that is not in the map,
    /// so a handler that destroys the very control being dispatched is honoured instead of being
    /// overwritten by the reinsertion that follows the call.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DISPATCHING: RefCell<IdSet> = RefCell::new(IdSet::new());

    /// Ids unregistered while their dispatch was in progress.
    ///
    /// A tombstone for [`DISPATCHING`]: [`dispatch_event`] skips reinsertion for an id listed here,
    /// then clears the entry. Without it, a handler that called `unregister(id)` would have its
    /// destroy silently undone by the dispatch epilogue putting the widget back.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static DESTROYED_DURING_DISPATCH: RefCell<IdSet> = RefCell::new(IdSet::new());

    /// Widgets currently lent out of [`MOUNTED`] while a frame drives them.
    ///
    /// [`dispatch_event`] and [`with_widget_mut`] take the widget out of `MOUNTED` before they run a
    /// user callback, so the callback may re-enter the registry without a `RefCell` conflict.
    /// `is_mounted` can answer from this loan's presence. Reads of the same mutably-driven widget are
    /// refused while its exclusive borrow is active.
    ///
    /// This maps the id to a pointer for reentrant reads during shared loans. Exclusive loans are
    /// marked in `LENT_EXCLUSIVE`, and no accessor dereferences their pointer while the caller holds
    /// `&mut`. Nested writes to any active loan are refused.
    ///
    /// # Safety
    ///
    /// The pointer is valid only for the dynamic extent of the driving call that installed it: the
    /// owning frame keeps the widget alive on its stack and removes the entry (via [`LentGuard`])
    /// before it returns or unwinds. Readers hold the pointer only long enough to call `&self`,
    /// which cannot outlive that extent because nothing yields it to the caller.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LENT_OUT: RefCell<alloc::collections::BTreeMap<ObjectId, core::ptr::NonNull<dyn Widget>>> =
        RefCell::new(alloc::collections::BTreeMap::new());

    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LENT_EXCLUSIVE: RefCell<IdSet> = RefCell::new(IdSet::new());

    /// Monotonic id source for mounted widgets.
    ///
    /// Starts high so a mounted id cannot collide with a platform-allocated
    /// widget id (those come from `BackendState`, which counts up from 1).
    ///
    /// The counter is seeded per thread but advances in a **globally unique** range:
    /// each thread that wants ids reserves a block with [`reserve_id_block`] and hands
    /// them out locally. A purely per-thread counter would give two threads the same
    /// ids, and some stores that hold widget state are process-wide (the control
    /// backend's, for one), so the second thread's window would read the first thread's
    /// record — a window reporting another window's client size. See [`reserve_id_block`].
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static NEXT_ID: RefCell<ObjectId> = const { RefCell::new(0) };

    /// The end of the id block `NEXT_ID` is handing out from.
    ///
    /// Zero means "no block yet", which is what makes the first allocation reserve one.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static NEXT_ID_LIMIT: RefCell<ObjectId> = const { RefCell::new(0) };

    /// Maps a mounted `Window` widget id to the host window the platform built
    /// for it.
    ///
    /// A window exists in two id spaces: the widget registry's id (what the
    /// library's `create_window` returns and every accessor accepts) and the
    /// platform's own id (what `mount_surface` resolves a parent through, because
    /// only the platform can reach a native `HWND`/`X11` window/`NSWindow`).
    /// Without this link the two never met, so mounting a control on a window the
    /// caller had just created was refused — the library's own window could not
    /// carry the library's own controls.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static HOST_WINDOWS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// The inverse of [`HOST_WINDOWS`]: host id → widget id.
    ///
    /// Platform callbacks hand the backend *its own* window id, and that translation used to be a
    /// linear scan of `HOST_WINDOWS` — paid by every GTK key/pointer/draw callback (thirteen call
    /// sites, `motion_notify` at pointer-event rate). Keeping the reverse direction makes the
    /// lookup O(1). Maintained only by [`set_host_window`] and [`unregister`], which also maintain
    /// `HOST_WINDOWS`, so the two cannot disagree.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static HOST_TO_WIDGET: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// Which library window a **mounted surface** is shown in.
    ///
    /// A surface is registered with no parent (it is not a child in the widget tree —
    /// the platform mounts it as its own drawing area), so `parent()` cannot answer
    /// "which window is this in". Everything that needs the top-level window of a
    /// hovered control — cursor shape, repaint routing — therefore saw `None` for a
    /// mounted surface: the code editor asked for an I-beam and no backend was ever
    /// asked to apply it. The mount path knows the window, so it records it here.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static SURFACE_WINDOWS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// Reverse index for [`surfaces_of`]: the surfaces linked to each window (G-314).
    ///
    /// `surfaces_of` used to filter the whole [`SURFACE_WINDOWS`] table per call, which made
    /// every frame that walks a window's surfaces cost O(total surfaces) instead of
    /// O(this window's surfaces). One map probe answers it now. Maintained by
    /// [`set_surface_window`] and [`unregister`] — the only two places [`SURFACE_WINDOWS`]
    /// changes — so the two cannot disagree, the same discipline [`HOST_TO_WIDGET`] follows
    /// for `HOST_WINDOWS`. A surface that re-links keeps one entry: it is removed from its
    /// previous window's list before being pushed onto the new one.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static SURFACES_BY_WINDOW: RefCell<HashMap<ObjectId, Vec<ObjectId>>> =
        RefCell::new(HashMap::new());

    /// The authoritative keyboard-focus owner for this thread's widget tree.
    ///
    /// Before this existed, [`crate::event::FocusManager`] was fully implemented
    /// (tab order, wrap-around, traversal strategies, a11y callback) but **never
    /// instantiated by any production code** — so pressing Tab moved nothing, and
    /// `Event::FocusGained` / `FocusLost` had no producer anywhere, which made
    /// every control's focus branch unreachable. Holding the manager here, beside
    /// the registry, gives focus one owner that can also see the widgets: routing
    /// a key event to "the focused widget" needs both facts in the same place.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static FOCUS: RefCell<crate::event::FocusManager> =
        RefCell::new(crate::event::FocusManager::new());

    /// The widget the pointer is currently over, for hover enter/leave synthesis.
    ///
    /// Needed because those two events are otherwise unproduced: a backend can see
    /// "the pointer moved inside my surface" but not "it crossed onto another
    /// control", which is a fact about the widget tree. Storing the previous target
    /// is what makes the pair balanced (see `dispatch_hover_transition`).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static HOVERED: RefCell<Option<ObjectId>> = const { RefCell::new(None) };

    /// The cursor last handed to the platform: the hovered widget and the shape.
    ///
    /// A control's pointer intent can change **within** one widget (a code editor is an
    /// I-beam over its text and an arrow over its minimap), so the shape is re-asked on
    /// every motion. This cell is what keeps that from becoming a platform call per
    /// motion event: only a change of widget *or* of shape reaches `set_cursor`.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAST_CURSOR: RefCell<(Option<ObjectId>, crate::platform::CursorIcon)> =
        const { RefCell::new((None, crate::platform::CursorIcon::Default)) };

    /// Host observers of the IME pre-edit delivered to each widget.
    ///
    /// The platform half of IME (`ImeBridge`) *sends* the composition and the widget half
    /// (`Event::ImePreedit`) is what a control handles, but a host had no way to observe the
    /// uncommitted string frame by frame: `deliver_composition` returned a `bool` and nothing
    /// else. Keeping the observers beside the registry gives them the same lifetime and thread
    /// rules as the widget they belong to, and `unregister` drops them with it.
    ///
    /// `Rc<RefCell<..>>` rather than a boxed closure for the same reason as the value callbacks:
    /// [`notify_ime_preedit`] clones the list out for the duration of the call, so a callback
    /// that registers or removes an observer cannot hit an already-borrowed `RefCell`.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static IME_PREEDIT_CALLBACKS: RefCell<HashMap<ObjectId, Vec<ImePreeditCallback>>> =
        RefCell::new(HashMap::new());

    /// Scratch buffer for [`notify_ime_preedit`], reusing the list across notifications.
    ///
    /// A pre-edit notification arrives once per keystroke while a composition is active, and
    /// cloning the registered observers into a fresh `Vec` paid one allocation per keystroke. The
    /// list is taken out for the duration of the call (so a callback may register or clear an
    /// observer without touching the borrowed map), then cleared and returned by
    /// [`ImePreeditBuffer`]'s `Drop`, including when a callback unwinds.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static IME_PREEDIT_SCRATCH: RefCell<Vec<ImePreeditCallback>> =
        RefCell::new(Vec::new());

    /// Whether the last [`tick_animations`] sweep left a **mounted** control in flight.
    ///
    /// Kept across frames because the frame that observes a control *settling* is one the
    /// sweep reports `false` for, and a loop that stopped there would never paint that frame.
    /// See [`animation_bus_needs_another_frame`].
    ///
    /// The trailing `#[allow]` is **not** redundant with the one above it. Clippy accepts
    /// `const { Cell::new(false) }` as a const initializer only on some toolchains — on the
    /// OpenHarmony target it rejects exactly this shape while accepting the `RefCell` ones — so
    /// the spelling above has to stay as it is and the lint is silenced per entry, which is what
    /// every other non-`const` entry in this block already does.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static LAST_SWEEP_HAD_UNSETTLED_MOUNTED: std::cell::Cell<bool> = std::cell::Cell::new(false);

    /// Whether a **host-owned** control was advanced this frame and has not settled.
    ///
    /// A control held as a `Box<dyn Widget>` is not in [`MOUNTED`], so the sweep cannot see
    /// it; its owner reports the fact instead. See
    /// [`animation_bus_note_host_owned_animating`]. The `#[allow]` is a per-target
    /// toolchain difference — see `LAST_SWEEP_HAD_UNSETTLED_MOUNTED` above.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static HOST_OWNED_ANIMATING: std::cell::Cell<bool> = std::cell::Cell::new(false);

    /// The widget that has captured the pointer, if any.
    ///
    /// While a control holds capture it receives every pointer event regardless of
    /// where the pointer is — that is what makes a drag work when the cursor leaves
    /// the control it started on. [`crate::event::PointerCaptureManager`] implemented
    /// this but had no production caller, so a drag could not survive leaving its
    /// origin widget.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static CAPTURE: RefCell<crate::event::PointerCaptureManager> =
        RefCell::new(crate::event::PointerCaptureManager::new());

    /// Stack of currently-active modal dialog ids, innermost last.
    ///
    /// While this stack is non-empty, input outside the top modal dialog's subtree
    /// is suppressed: [`dispatch_event`], [`dispatch_pointer_event`] and
    /// [`focus_widget`] all ask [`modal_blocks`] before acting. This is the one
    /// place modality is enforced, so a modal dialog actually blocks its owner —
    /// the `modal` flag on each dialog widget records the *intent*, and the stack
    /// is what makes that intent real (principle #5: no uninforced flag).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static MODAL: RefCell<alloc::vec::Vec<ObjectId>> = const { RefCell::new(alloc::vec::Vec::new()) };
}

thread_local! {
    /// Mounted controls whose **last observed** answer to `Widget::is_animating` was `true`.
    ///
    /// # Why this registry exists
    ///
    /// The animation sweep used to walk every mounted control and ask each one `is_animating`,
    /// which is a virtual call per control per frame even for a still window (G-06). This set is
    /// the registry the sweep now walks instead: membership *is* the per-control animating bit,
    /// and a control is added when it reports `true` at any point the runtime can observe it
    /// (mount, event dispatch, property write, global sweep, its own tick) and removed when it
    /// reports `false`.
    ///
    /// # Why it cannot leak a settled control
    ///
    /// Every path that can change a mounted control's state goes through one of those observation
    /// points: `with_widget_mut`, `dispatch_event`, `for_each_mounted_widget` (the global-theme
    /// path), `register`, [`tick_animations_sweep`], or `unregister`. A control that settles is
    /// removed on the tick that observed the settle, and a destroyed control is removed by
    /// `unregister` (with a stale-id cleanup in the sweep as a belt-and-braces guard).
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static ANIMATING: RefCell<alloc::collections::BTreeSet<ObjectId>> =
        RefCell::new(alloc::collections::BTreeSet::new());

    /// Candidate ids taken out of [`ANIMATING`] for one sweep. Reused across frames so a still
    /// frame allocates nothing (G-06/G-37).
    pub(crate) static ANIMATION_IDS: RefCell<alloc::vec::Vec<ObjectId>> =
        const { RefCell::new(alloc::vec::Vec::new()) };

    /// Window -> union of animation damage, reused across sweeps (G-07).
    pub(crate) static WINDOW_ANIMATION_DAMAGE: RefCell<alloc::vec::Vec<(ObjectId, Rect)>> =
        const { RefCell::new(alloc::vec::Vec::new()) };
}

/// Records the animating bit observed for `id`.
pub(crate) fn note_animating(id: ObjectId, animating: bool) {
    let _ = ANIMATING.try_with(|set| {
        let mut set = set.borrow_mut();
        if animating {
            set.insert(id);
        } else {
            set.remove(&id);
        }
    });
}

/// How many ids one thread reserves at a time.
///
/// Large enough that the reservation is rare (a thread creating a whole window's worth of
/// controls reserves once) and small enough that the address space is not wasted.
pub(crate) const ID_BLOCK: ObjectId = 0x1_0000;

/// Where the globally unique id space begins.
///
/// Starts high so a mounted id cannot collide with a platform-allocated widget id
/// (those come from `BackendState`, which counts up from 1).
pub(crate) const ID_BASE: ObjectId = 0x5345_4C46_0000_0000;

/// The high-water mark of every block handed out so far.
///
/// Process-wide, and the **only** piece of id allocation that is: the per-thread counter
/// caches what this hands out, so the common path stays a thread-local read with no
/// synchronisation.
pub(crate) static ID_HIGH_WATER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(ID_BASE);

/// Reserves a block of ids for the calling thread and returns the first id in it.
///
/// # Why blocks rather than a plain per-thread counter
///
/// Widgets are `!Send`, so the *registry* is thread-local and a per-thread counter is the
/// obvious fit. But not every store keyed by widget id is thread-local: the control
/// backend keeps its host-side maps process-wide (it is a `OnceLock` singleton). With a
/// per-thread counter two threads mint the **same** ids, so the second thread's window
/// reads the first thread's record — concretely, a newly created window reporting
/// another window's client size. Reserving disjoint blocks makes an id identify one
/// window for the whole process without giving up the lock-free local path.
pub(crate) fn reserve_id_block() -> Option<ObjectId> {
    // `fetch_add` on the high-water mark is the rendezvous point. Uniqueness comes from
    // it being a read-modify-write, so two threads can never be handed overlapping
    // blocks, and no lock is needed on the per-allocation path.
    let start = ID_HIGH_WATER.fetch_add(ID_BLOCK, std::sync::atomic::Ordering::Relaxed);
    // A released thread's ids are not reclaimed: a store may still hold a record for one
    // and a later reader must not find it under a fresh window. Wrapping would do exactly
    // that, so exhaustion is reported instead of reusing the space.
    start.checked_add(ID_BLOCK)?;
    NEXT_ID.try_with(|next| *next.borrow_mut() = start).ok()?;
    NEXT_ID_LIMIT.try_with(|limit| *limit.borrow_mut() = start.wrapping_add(ID_BLOCK)).ok()?;
    Some(start)
}

/// Hands out the next id for this thread, reserving a block when the current one runs out.
///
/// `Err` means this thread has no registry (no UI thread), matching [`register`]'s contract.
pub(crate) fn next_widget_id() -> Result<ObjectId, ()> {
    NEXT_ID
        .try_with(|next| {
            let id = *next.borrow();
            let limit = NEXT_ID_LIMIT.try_with(|limit| *limit.borrow()).unwrap_or(0);
            if id < limit {
                *next.borrow_mut() = id.wrapping_add(1);
                return Ok(id);
            }
            // Exhausted (or never reserved): take the next block. `reserve_id_block`
            // seeds `NEXT_ID`/`NEXT_ID_LIMIT`, so the id to return is the block's first.
            let start = reserve_id_block().ok_or(())?;
            *next.borrow_mut() = start.wrapping_add(1);
            Ok(start)
        })
        .unwrap_or(Err(()))
}
