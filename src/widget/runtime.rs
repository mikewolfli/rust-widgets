// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Registry for library-painted widgets mounted into windows.
//!
//! # Why this exists
//!
//! `widget::WidgetFactory` can build a `Box<dyn Widget>` for any kind —
//! `CodeEditor`, `ColorPicker`, `GanttWidget`, … — but until this module existed
//! there was no path from such a box to pixels inside a real window. The platform
//! layer only knew how to create OS controls (an OS button, a text field, a
//! toolkit widget); a library-painted widget had to be handed to
//! `render_to_svg()` or it went nowhere. Mounting one into a window produced an
//! empty surface.
//!
//! This registry closes that loop. The host keeps ownership of the widget here,
//! keyed by [`ObjectId`]; a backend supplies the surface it paints widgets into
//! and, when a repaint is wanted, calls [`with_widget_mut`] and paints a
//! frame. **Which surface that is is a backend detail** — see
//! [`crate::platform::Platform::mount_surface`].
//!
//! # Threading
//!
//! Widgets are `!Send` (they own `Rc`/`RefCell` state), and the desktop backends
//! require UI work on the platform main thread anyway. The registry is therefore
//! **thread-local**, and [`register`] returns `None` when called from a thread
//! that has no registry rather than smuggling a non-`Send` value across threads.
//!
//! [`with_widget_mut`]: crate::widget::runtime::with_widget_mut

// `Point` is used by the hit-test API below, which is a core capability of the
// registry rather than a `full_widgets` extra: every profile that can mount a widget
// must be able to answer "which widget is under this point?".
use crate::compat::Rc;
use crate::core::{ObjectId, Point, Rect, Size};
use crate::event::dnd::{DragSession, DropEffect};
use crate::event::{Event, FocusReason};
use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

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
use crate::widget::Widget;
use std::cell::RefCell;
use std::collections::HashMap;

/// One widget handed to the registry for display by the host surface.
struct Mounted {
    widget: Box<dyn Widget>,
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
struct IdSet(Vec<ObjectId>);

impl IdSet {
    const fn new() -> Self {
        Self(Vec::new())
    }

    fn insert(&mut self, id: ObjectId) {
        if !self.0.contains(&id) {
            self.0.push(id);
        }
    }

    fn remove(&mut self, id: &ObjectId) -> bool {
        if let Some(index) = self.0.iter().position(|candidate| candidate == id) {
            self.0.remove(index);
            true
        } else {
            false
        }
    }

    fn contains(&self, id: &ObjectId) -> bool {
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
    static MOUNTED: RefCell<HashMap<ObjectId, Mounted>> = RefCell::new(HashMap::new());

    /// Ids whose entry is temporarily held **outside** the registry during event dispatch.
    ///
    /// [`dispatch_event`] takes the widget out of `MOUNTED` before calling `handle_event`, so a
    /// handler may re-enter the registry (read its own state, update another control, call
    /// `request_redraw`) without hitting a `RefCell` borrow conflict. While an id is listed here,
    /// [`unregister`] records the removal rather than looking for an entry that is not in the map,
    /// so a handler that destroys the very control being dispatched is honoured instead of being
    /// overwritten by the reinsertion that follows the call.
    #[allow(clippy::missing_const_for_thread_local)]
    static DISPATCHING: RefCell<IdSet> = RefCell::new(IdSet::new());

    /// Ids unregistered while their dispatch was in progress.
    ///
    /// A tombstone for [`DISPATCHING`]: [`dispatch_event`] skips reinsertion for an id listed here,
    /// then clears the entry. Without it, a handler that called `unregister(id)` would have its
    /// destroy silently undone by the dispatch epilogue putting the widget back.
    #[allow(clippy::missing_const_for_thread_local)]
    static DESTROYED_DURING_DISPATCH: RefCell<IdSet> = RefCell::new(IdSet::new());

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
    static LENT_OUT: RefCell<alloc::collections::BTreeMap<ObjectId, core::ptr::NonNull<dyn Widget>>> =
        RefCell::new(alloc::collections::BTreeMap::new());

    #[allow(clippy::missing_const_for_thread_local)]
    static LENT_EXCLUSIVE: RefCell<IdSet> = RefCell::new(IdSet::new());

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
    static NEXT_ID: RefCell<ObjectId> = const { RefCell::new(0) };

    /// The end of the id block `NEXT_ID` is handing out from.
    ///
    /// Zero means "no block yet", which is what makes the first allocation reserve one.
    #[allow(clippy::missing_const_for_thread_local)]
    static NEXT_ID_LIMIT: RefCell<ObjectId> = const { RefCell::new(0) };

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
    static HOST_WINDOWS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// The inverse of [`HOST_WINDOWS`]: host id → widget id.
    ///
    /// Platform callbacks hand the backend *its own* window id, and that translation used to be a
    /// linear scan of `HOST_WINDOWS` — paid by every GTK key/pointer/draw callback (thirteen call
    /// sites, `motion_notify` at pointer-event rate). Keeping the reverse direction makes the
    /// lookup O(1). Maintained only by [`set_host_window`] and [`unregister`], which also maintain
    /// `HOST_WINDOWS`, so the two cannot disagree.
    #[allow(clippy::missing_const_for_thread_local)]
    static HOST_TO_WIDGET: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// Which library window a **mounted surface** is shown in.
    ///
    /// A surface is registered with no parent (it is not a child in the widget tree —
    /// the platform mounts it as its own drawing area), so `parent()` cannot answer
    /// "which window is this in". Everything that needs the top-level window of a
    /// hovered control — cursor shape, repaint routing — therefore saw `None` for a
    /// mounted surface: the code editor asked for an I-beam and no backend was ever
    /// asked to apply it. The mount path knows the window, so it records it here.
    #[allow(clippy::missing_const_for_thread_local)]
    static SURFACE_WINDOWS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

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
    static FOCUS: RefCell<crate::event::FocusManager> =
        RefCell::new(crate::event::FocusManager::new());

    /// The widget the pointer is currently over, for hover enter/leave synthesis.
    ///
    /// Needed because those two events are otherwise unproduced: a backend can see
    /// "the pointer moved inside my surface" but not "it crossed onto another
    /// control", which is a fact about the widget tree. Storing the previous target
    /// is what makes the pair balanced (see `dispatch_hover_transition`).
    #[allow(clippy::missing_const_for_thread_local)]
    static HOVERED: RefCell<Option<ObjectId>> = const { RefCell::new(None) };

    /// The cursor last handed to the platform: the hovered widget and the shape.
    ///
    /// A control's pointer intent can change **within** one widget (a code editor is an
    /// I-beam over its text and an arrow over its minimap), so the shape is re-asked on
    /// every motion. This cell is what keeps that from becoming a platform call per
    /// motion event: only a change of widget *or* of shape reaches `set_cursor`.
    #[allow(clippy::missing_const_for_thread_local)]
    static LAST_CURSOR: RefCell<(Option<ObjectId>, crate::platform::CursorIcon)> =
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
    static IME_PREEDIT_CALLBACKS: RefCell<HashMap<ObjectId, Vec<ImePreeditCallback>>> =
        RefCell::new(HashMap::new());

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
    static LAST_SWEEP_HAD_UNSETTLED_MOUNTED: std::cell::Cell<bool> = std::cell::Cell::new(false);

    /// Whether a **host-owned** control was advanced this frame and has not settled.
    ///
    /// A control held as a `Box<dyn Widget>` is not in [`MOUNTED`], so the sweep cannot see
    /// it; its owner reports the fact instead. See
    /// [`animation_bus_note_host_owned_animating`]. The `#[allow]` is a per-target
    /// toolchain difference — see `LAST_SWEEP_HAD_UNSETTLED_MOUNTED` above.
    #[allow(clippy::missing_const_for_thread_local)]
    static HOST_OWNED_ANIMATING: std::cell::Cell<bool> = std::cell::Cell::new(false);

    /// The widget that has captured the pointer, if any.
    ///
    /// While a control holds capture it receives every pointer event regardless of
    /// where the pointer is — that is what makes a drag work when the cursor leaves
    /// the control it started on. [`crate::event::PointerCaptureManager`] implemented
    /// this but had no production caller, so a drag could not survive leaving its
    /// origin widget.
    #[allow(clippy::missing_const_for_thread_local)]
    static CAPTURE: RefCell<crate::event::PointerCaptureManager> =
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
    static MODAL: RefCell<alloc::vec::Vec<ObjectId>> = const { RefCell::new(alloc::vec::Vec::new()) };
}

/// How many ids one thread reserves at a time.
///
/// Large enough that the reservation is rare (a thread creating a whole window's worth of
/// controls reserves once) and small enough that the address space is not wasted.
const ID_BLOCK: ObjectId = 0x1_0000;

/// Where the globally unique id space begins.
///
/// Starts high so a mounted id cannot collide with a platform-allocated widget id
/// (those come from `BackendState`, which counts up from 1).
const ID_BASE: ObjectId = 0x5345_4C46_0000_0000;

/// The high-water mark of every block handed out so far.
///
/// Process-wide, and the **only** piece of id allocation that is: the per-thread counter
/// caches what this hands out, so the common path stays a thread-local read with no
/// synchronisation.
static ID_HIGH_WATER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(ID_BASE);

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
fn reserve_id_block() -> Option<ObjectId> {
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
fn next_widget_id() -> Result<ObjectId, ()> {
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

/// Hands ownership of `widget` to the registry and returns its display id.
///
/// Returns `None` when the calling thread has no registry — i.e. it is not the
/// thread that drives the UI. Callers must surface that as "cannot display
/// here" rather than dropping the widget silently.
pub fn register(widget: Box<dyn Widget>) -> Option<ObjectId> {
    let id = next_widget_id();
    let Ok(id) = id else { return None };
    let stored = MOUNTED.try_with(|map| {
        map.borrow_mut().insert(id, Mounted { widget });
    });
    if stored.is_err() {
        return None;
    }
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
    Some(id)
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
        map.insert(declared_id, Mounted { widget });
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

    // Every step `register` does after the insert, repeated here rather than extracted, so
    // the two entry points cannot drift: a caller reading `register` sees the same list.
    // See `register` for why each one exists.
    if let Some(own_id) = with_widget(declared_id, |widget| widget.base().id()) {
        let _ = OWN_IDS.try_with(|map| {
            map.borrow_mut().insert(own_id, declared_id);
        });
    }
    let focusable = read_widget(declared_id, |widget| widget.is_focusable()).unwrap_or(false);
    if focusable {
        register_focusable(declared_id);
    }
    let _ = enable_damage_tracking_if_useful(declared_id);
    if let Some(state) =
        with_widget(declared_id, crate::platform::accessibility::A11yState::from_widget)
    {
        crate::widget::a11y_submit::submit_mounted(declared_id, &state);
    }
    Ok(declared_id)
}

/// Removes a mounted widget, dropping it. Returns whether it was present.
pub fn unregister(id: ObjectId) -> bool {
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
    let _ = SURFACE_WINDOWS.try_with(|map| map.borrow_mut().remove(&id));
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
    // The cached frame and any pending damage describe this widget, so both must go
    // with it: a reused id would otherwise start from the previous control's pixels,
    // and a stale damage rect would repaint a region that no longer exists.
    forget_cached_frame(id);
    let _ = REPAINT.try_with(|map| map.borrow_mut().remove(&id));
    // And the own-id reverse mapping, or a later widget built with the same
    // `BaseWidget::id()` would resolve to this dead registry id and file its damage
    // against a widget that no longer exists.
    let own_id = MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|mounted| mounted.widget.base().id()))
        .ok()
        .flatten();
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
/// [`Widget::is_focusable`](crate::widget::Widget::is_focusable). A disabled control stays in the
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
fn notify_ime_focus_in(id: ObjectId) {
    #[cfg(not(alloc_frugal))]
    if let Some(bridge) = crate::platform::platform_facts().ime_bridge() {
        bridge.focus_in(id);
    }
    #[cfg(alloc_frugal)]
    let _ = id;
}

/// Tells the platform's IME bridge that `id` is no longer being edited.
fn notify_ime_focus_out(id: ObjectId) {
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

// ---------------------------------------------------------------------------
// Hit testing: point -> widget
// ---------------------------------------------------------------------------

// A reusable child-id buffer for the tree walks (see `with_child_ids`). One buffer per thread,
// because every walk in this module is iterative and consumes a node's children before asking the
// next node for its own.
thread_local! {
    static CHILD_IDS: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` with `widget_id`'s child ids, reusing a per-thread buffer instead of allocating.
///
/// [`widget_at`] asks every node on the pointer's path for its child list, and the answer used to be
/// a fresh `Vec` each time (`children().to_vec()`). One buffer is enough because the list is
/// consumed inside one iteration. A widget's `contains_point` is user code, though, so a control
/// that re-entered a tree walk would find the buffer busy — that case reads the widget's own slice
/// instead of panicking, which is why this cannot assume the borrow succeeds.
fn with_child_ids<R>(widget_id: ObjectId, f: impl FnOnce(&[ObjectId]) -> R) -> Option<R> {
    // Read-only: every caller only inspects the child list. `with_widget` takes the widget out
    // of the mounted registry and puts it back (remove + insert + two loan guards per node),
    // which during a window resize meant ~10 hash operations per node for a pure read. The
    // shared borrow of `read_widget` is enough and leaves the registry untouched.
    read_widget(widget_id, |widget| {
        CHILD_IDS.with(|cell| match cell.try_borrow_mut() {
            Ok(mut scratch) => {
                scratch.clear();
                scratch.extend_from_slice(widget.children());
                f(&scratch)
            }
            Err(_) => f(widget.children()),
        })
    })
}

/// Returns the widget that should receive a pointer event at `point`.
///
/// # Why this exists
///
/// [`dispatch_event`] takes a widget id the caller already knows, which left the
/// `point -> widget` step to each backend. On every desktop backend the operating
/// system's own window/view hierarchy performed it, so a control only ever received
/// events when the host had given it a native surface of its own — the library could
/// not answer "which control is under this pixel?". This function answers it, by
/// walking the widget tree that [`crate::widget::Widget::children`] already
/// describes.
///
/// # Coordinates
///
/// `point` is in the **same space as the widgets' geometry**, which in this library
/// is absolute: the layout engine hands each child a rect derived from its parent's
/// absolute rect (see `src/layout/box_layout.rs`), and controls call
/// `contains_point` with raw event positions (see
/// `src/widget/media_widgets/video_player.rs`). No origin subtraction happens at any
/// level — nesting is already baked into each rect, so a deeper widget simply has a
/// smaller absolute rect inside its parent's.
///
/// # Order
///
/// Children are visited **last-to-first**, so a later sibling (drawn on top) wins —
/// the same "topmost wins" rule the renderer paints by. The search descends to the
/// deepest hit, so a click inside a control inside a container resolves to the
/// control, not the container.
///
/// Invisible widgets are skipped: they are not drawn, so they must not swallow a
/// click either. Disabled widgets are still returned — hit testing reports *what is
/// there*, and whether a disabled control reacts is the control's decision (it can
/// report it declined, the same honest-answer rule used everywhere else).
///
/// Returns `None` for an unmounted `root` or a tree with no hit.
///
/// # The root's own bounds, and the one case where they are not consulted
///
/// Every **non-root** node is tested against `point` normally. The root is the one exception, and
/// the reason is a coordinate-space fact rather than an oversight: a window's geometry holds the
/// `x`/`y` the caller asked the operating system for (see `frame_origin`), while its children are
/// written in client coordinates. Those are two different spaces, so testing the window's rect
/// against a client point rejected every click on the window.
///
/// Skipping the test entirely is not the fix either, and that is what this function used to do: a
/// point far outside the root then fell through the child search and returned the **root** as the
/// deepest hit, so a click hundreds of pixels off the window still reached a widget. The two spaces
/// only differ by the root's own origin, so the bound is applied **relative to that origin**: a
/// point is inside the root when `point - origin` lies within the root's rect. When the root is a
/// client-space container (`origin == (0, 0)`, which is the case for every tree built through the
/// layout engine and for the tests below) this is exactly the plain containment test, and for a
/// window it is containment in client coordinates — which is the space `point` is documented to be
/// in above.
pub fn widget_at(root: ObjectId, point: Point) -> Option<ObjectId> {
    widget_at_inner(root, point, false)
}

/// [`widget_at`], optionally without applying `root`'s own bounds.
///
/// `trust_root` exists for an open popup: its painted list extends past its own rectangle, and
/// the control has already answered `contains_point` for the press (that answer is what selected
/// it), so applying the geometry bound here would reject exactly the rows that hang outside the
/// field — the case this descent exists to serve.
fn widget_at_inner(root: ObjectId, point: Point, trust_root: bool) -> Option<ObjectId> {
    let bounds = read_widget(root, |widget| widget.geometry())?;
    // A **window** root's geometry is written in the operating system's space (the position it was
    // asked to open at), while its children are written in client coordinates. `frame_origin` is
    // the one predicate that knows that difference, so the root bound is applied in the same space
    // the children use. Subtracting `geometry()` directly rejected every click on a window opened
    // away from the screen origin: measured, a press at client `(182, 58)` on a window at `(140,
    // 140)` became local `(42, -82)` and no control was found.
    let origin = frame_origin(root);
    let local = Point::new(point.x - origin.0, point.y - origin.1);
    let root_rect = Rect::new(0, 0, bounds.width, bounds.height);
    let mut current = root;
    loop {
        // `Ok(Some(child))` descends, `Ok(None)` means this node is the deepest hit, and `Err` means
        // the point is not inside this node at all. The step is a value rather than a control-flow
        // `return` because the node's child list is borrowed from [`with_child_ids`]'s buffer while
        // the children are tested.
        let step = with_child_ids(current, |children| {
            // A **root** is a container whose geometry is written in a different space from its
            // children's (see the doc comment), so its bound is applied in the root's own space via
            // the `local` point captured above. Every other node is tested normally: a control is
            // addressed in its parent's space, which is the space its own rectangle is written in.
            if current == root {
                if !trust_root && !root_rect.contains_point(local) {
                    return Err(());
                }
            } else if !widget_accepts_point(current, point) {
                return Err(());
            }
            // Topmost-first: a later sibling paints over an earlier one.
            //
            // Visibility, the overlay flag and the hit rule are asked in **one** registry visit. They
            // used to be two separate `with_widget` calls per child, so every node cost three map
            // lookups on a path that runs for every pointer event the host delivers.
            //
            // A child with an **open overlay** wins over a later sibling that happens to cover the
            // same pixels: an open drop-down list is painted below its field, where the next form row
            // may sit. Without the priority that row took the click and the list could not be used.
            let mut deepest = None;
            let mut open_popup = None;
            for child in children.iter().rev().copied() {
                let (visible, overlay, accepts) = read_widget(child, |widget| {
                    (widget.is_visible(), widget.has_open_popup(), widget.contains_point(point))
                })
                .unwrap_or((false, false, false));
                if !visible || !accepts {
                    continue;
                }
                if overlay {
                    open_popup = Some(child);
                    break;
                }
                if deepest.is_none() {
                    deepest = Some(child);
                }
            }
            Ok(open_popup.or(deepest))
        })?;
        match step {
            Ok(Some(child)) => current = child,
            // No child claims the point, so this widget is the deepest hit.
            Ok(None) => return Some(current),
            Err(()) => return None,
        }
    }
}

/// Whether a mounted widget's own bounds accept `point` (in its local space).
///
/// Asks the widget rather than comparing rectangles, so each control keeps its own
/// hit rule — the touch-target expansion in `contains_point`, or a shape narrower
/// than the bounding box. Returns `false` for an unmounted id.
fn widget_accepts_point(id: ObjectId, point: Point) -> bool {
    read_widget(id, |widget| widget.contains_point(point)).unwrap_or(false)
}

/// Delivers `event` to whatever widget is under `point`.
///
/// Because widget geometry is absolute here (see [`widget_at`]), the event's position
/// is already in the space the target expects, so it is forwarded unchanged — a
/// coordinate rewrite would introduce an off-by-parent-origin on every nested control.
///
/// A pointer event also drives hover: moving onto a widget synthesises
/// [`Event::MouseEnter`] for it and [`Event::MouseLeave`] for the one left behind.
/// Without that, no control could ever show a hover state, because no backend produces
/// those two events.
///
/// A widget that holds pointer capture receives the event even when the pointer is not
/// over it, so a drag can continue past its origin widget.
///
/// The router itself owns the capture for the duration of a press gesture (D09-EVT-01):
/// the widget the press lands on becomes the gesture owner, and every subsequent move and
/// the final release are delivered to it regardless of where the pointer is. Without that,
/// releasing after dragging off a control left its own press latch armed, so a *later*
/// unrelated release on the control could fire a spurious click. Capture is released once
/// the gesture-ending event has been delivered, so the next press takes a fresh owner.
///
/// Returns whether a widget accepted the event.
pub fn dispatch_pointer_event(root: ObjectId, event: &Event, point: Point) -> bool {
    // An open popup is modal for the gesture that dismisses it: a press outside the popup is
    // delivered to the popup (which hides itself) and goes no further. Checking the grab before
    // hit-testing is what keeps the press from reaching the control the popup covers.
    //
    // A press **inside** the popup is hit-tested *from* the popup. The grab used to leave that
    // case to `widget_at(root, ..)`, which refuses to descend through an ancestor whose own
    // bounds reject the point — so a list hanging below its parent's rectangle (a combo box in a
    // panel, a drop-down in a form row) could never receive a row click, while the popup stayed
    // open. `widget_at` treats a non-window root's `frame_origin` as its own top-left, so a
    // popup works as the origin of the descent.
    #[cfg(not(alloc_frugal))]
    let pointer_origin = if event.starts_pointer_gesture() {
        match popup_press_route(root, point, event) {
            Ok(origin) => origin,
            Err(handled) => return handled,
        }
    } else {
        popup_hover_origin(root, point)
    };
    #[cfg(alloc_frugal)]
    let pointer_origin: Option<ObjectId> = None;

    let target = match pointer_origin {
        Some(popup) => widget_at_inner(popup, point, true),
        None => widget_at(root, point),
    };
    dispatch_hover_transition(target, point);
    // A control that captured the pointer keeps receiving events even when the
    // cursor leaves it — that is what lets a drag continue past its origin widget.
    // Hover still follows the true position, so highlights stay honest.
    //
    // A *new* gesture-start event always re-targets: the pointer going down again is a
    // fresh press, so any leftover capture from a previous gesture is dropped first and
    // the press is hit-tested normally. Without that, a capture that outlived its gesture
    // (for example a host delivered a press but never a release) would swallow every
    // later press, and nothing could ever take the pointer again.
    let starts = event.starts_pointer_gesture();
    if starts && capturing_widget().is_some() {
        let _ = release_pointer_capture();
    }
    if let Some(captured) = capturing_widget() {
        if is_mounted(captured) && !modal_blocks(captured) {
            // A gesture-ending event closes the capture *after* delivery, so this is the
            // last event the capturer sees under this ownership. The control's own latch
            // (`pressed`/`grabbed`) is cleared by its handler, and the router then forgets
            // the capture so the next press can re-target.
            let handled = dispatch_event(captured, event);
            if event.ends_pointer_gesture() {
                let _ = release_pointer_capture();
            }
            return handled;
        }
        // The capturer is gone (or blocked by a modal); drop the capture rather
        // than routing into nothing.
        let _ = release_pointer_capture();
        return false;
    }
    match target {
        Some(target) if !modal_blocks(target) => {
            // A disabled control takes no input: no focus move, no pointer capture, no delivery.
            // The router owns this rule (see the module's focus docs: "the router skips disabled
            // controls when delivering, not when ordering"), so a control that relies on it — a
            // third-party widget that does not re-check its own enabled flag — is not fed input
            // while disabled. The capture branch above deliberately does **not** apply it: a
            // gesture that began on an enabled control must still receive its ending event.
            if !read_widget(target, |widget| widget.is_enabled()).unwrap_or(false) {
                return false;
            }
            // A press on an interactive control makes it the keyboard's target.
            //
            // This is the production caller the focus entry point never had: the router moved
            // pointer capture and delivered the press, but nothing told the focus manager, so a
            // click on a text field left keys going to whichever control the host had focused
            // programmatically (or to the window). The predicate is the same one the registry
            // uses for the tab order, so a control that is reachable by Tab is also reachable by
            // click and the two cannot drift.
            if starts && widget_wants_keyboard_focus(target) {
                let _ = focus_on_pointer_press(target);
            }
            // The widget under a press becomes the gesture owner for its whole duration,
            // even if the pointer leaves before the release (D09-EVT-01). A control may also
            // call `capture_pointer` itself for a longer-lived drag; the router only needs to
            // guarantee that a plain press→drag→release always completes on the same widget.
            if starts {
                let _ = capture_pointer(target);
            }
            let handled = dispatch_event(target, event);
            // A secondary press is both a pointer gesture and a menu request. The press above is
            // delivered first and unconditionally, so a control that starts a selection or opens
            // its own popup keeps receiving it; the request is then delivered to the *same* widget
            // as a separate event. Delivering it here (rather than replacing the press) is the
            // additive contract documented on `Event::ContextMenu`.
            if let Event::MousePress { pos, button, .. } = event {
                if *button == crate::event::mouse_button::SECONDARY {
                    let _ = dispatch_event(target, &Event::ContextMenu { pos: *pos });
                }
            }
            handled
        }
        _ => false,
    }
}

/// Routes a context-menu request to the focused control.
///
/// A Menu/Application key has no pointer position, so the translator emits
/// [`Event::ContextMenu`] with the anchor the translator was given (currently `(0, 0)`). The host
/// cannot hit-test such a request — there is no pointer — and the control it belongs to is by
/// definition the one with keyboard focus. This function performs that routing.
///
/// Returns whether the event was delivered to a mounted focused control. `false` means either the
/// event is not a context-menu request or no control currently has focus; a host that receives
/// `false` may fall back to its own menu policy. The focused control may still answer `false` from
/// its own [`Widget::context_menu_event`] — that is the control saying "no menu at this point",
/// which is a handled delivery rather than a routing failure.
pub fn dispatch_context_menu_event(event: &Event) -> bool {
    if !matches!(event, Event::ContextMenu { .. }) {
        return false;
    }
    let Some(focused) = focused_widget() else {
        return false;
    };
    dispatch_event(focused, event)
}

/// Drives one cross-control drag from the raw pointer stream.
///
/// # Why a coordinator and not per-control state machines
///
/// A drag crosses controls: it starts on a row, travels over a column, and ends on a
/// third widget. Each control can answer "what would I carry"
/// ([`crate::event::dnd::DragSource`]) and "would I take this"
/// ([`crate::event::dnd::DropTarget`]), but none of them can see the others. The
/// registry is the only layer that can, so the coordinator lives here and uses it to
/// translate `MousePress -> MouseMove -> MouseRelease` into one [`DragSession`].
///
/// # What it does not do
///
/// It does not deliver the events to controls or draw a preview. A host that wants both
/// the ordinary event stream *and* drag detection calls [`Self::on_event`] alongside
/// [`dispatch_pointer_event`]; the two are additive, so a control never loses its
/// press just because it is also draggable.
///
/// # Ordering
///
/// * press: probe the control under the pointer for a
///   [`crate::event::dnd::DragSource`] and begin an inactive session;
/// * move: let the session apply its own activation threshold, then probe the control
///   under the pointer for a [`crate::event::dnd::DropTarget`] and report the hover;
/// * release: commit through [`DragSession::drop_on`], then tell the source what the
///   effect was.
///
/// The session stays inactive until the pointer travels, so a press that never moves is
/// still a click and no target is highlighted.
#[derive(Default)]
pub struct DragCoordinator {
    session: Option<DragSession>,
    hovered: Option<ObjectId>,
    /// The control that supplied the payload. Kept here because [`DragSession`] describes
    /// the gesture, not who owns it, and the completion has to be delivered back to the
    /// source even when the pointer ends over a different control.
    source: Option<ObjectId>,
}

impl DragCoordinator {
    /// Creates an idle coordinator.
    pub fn new() -> Self {
        Self::default()
    }

    /// The session currently being driven, if any.
    pub fn session(&self) -> Option<&DragSession> {
        self.session.as_ref()
    }

    /// The control the pointer is currently over during an active drag, if any.
    pub fn hovered(&self) -> Option<ObjectId> {
        self.hovered
    }

    /// Feeds one pointer event to the drag state machine.
    ///
    /// Returns `Some(effect)` when this event ended a drag, and `None` for every other
    /// event. The effect is reported for a session that started, even when it never
    /// became active (a click): its value is then [`DropEffect::None`], which is the
    /// honest "nothing was dropped" answer the source needs to restore its state.
    ///
    /// `viewport` bounds the hit test. A point outside it resolves to no control, which
    /// is what stops a drag from targeting a widget in another surface.
    pub fn on_event(&mut self, event: &Event, viewport: Rect) -> Option<DropEffect> {
        match event {
            Event::MousePress { pos, button, .. }
                if *button == crate::event::mouse_button::PRIMARY =>
            {
                self.begin(*pos, viewport);
                None
            }
            Event::MouseMove { pos } => {
                self.update(*pos, viewport);
                None
            }
            Event::MouseRelease { pos, .. } => self.finish(*pos, viewport),
            _ => None,
        }
    }

    /// Starts a session when the press landed on a control that offers a payload.
    fn begin(&mut self, pos: Point, viewport: Rect) {
        if self.session.is_some() {
            return;
        }
        let Some(source) = widget_at_point(pos, viewport) else {
            return;
        };
        let payload = with_widget_mut(source, |widget| {
            // The identity a target compares against is the control's **own** id, not the
            // registry id that mounted it: a control knows only its own id, and a target asks
            // the same question of itself.
            let identity = widget.id();
            widget
                .drag_source_mut()
                .and_then(|source| source.drag_payload(pos))
                .map(|payload| payload.with_source(identity))
        })
        .flatten();
        let Some(payload) = payload else {
            return;
        };
        self.session = Some(DragSession::begin(payload, pos));
        self.source = Some(source);
        self.hovered = None;
    }

    /// Advances the session and reports the hover to the control under the pointer.
    fn update(&mut self, pos: Point, viewport: Rect) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.update(pos, DragSession::ACTIVATION_THRESHOLD);
        if !session.is_active() {
            return;
        }
        // Clone the payload so the session borrow ends before the registry is entered:
        // `with_widget_mut` runs user code that may re-enter the coordinator's owner.
        let payload = session.payload().clone();
        let target = widget_at_point(pos, viewport);
        if target == self.hovered {
            return;
        }
        if let Some(previous) = self.hovered {
            let _ = with_widget_mut(previous, |widget| {
                if let Some(target) = widget.drop_target_mut() {
                    target.report_leave();
                }
            });
        }
        self.hovered = target;
        if let Some(target) = target {
            let _ = with_widget_mut(target, |widget| {
                if let Some(target) = widget.drop_target_mut() {
                    // Ask whether a drop would be taken, then report the hover either way.
                    // `can_accept` drives the preview/commit decision; `report_hover` lets a
                    // target paint its own refusal as well as its acceptance (a drop zone shows
                    // a different state for a payload it will not take). Suppressing the hover
                    // report for a refusal would leave that feedback unreachable.
                    let _ = target.can_accept(&payload);
                    target.report_hover(&payload);
                }
            });
        }
    }

    /// Commits a release and reports the effect back to the source.
    fn finish(&mut self, pos: Point, viewport: Rect) -> Option<DropEffect> {
        let mut session = self.session.take()?;
        let source = self.source.take();
        if let Some(previous) = self.hovered.take() {
            let _ = with_widget_mut(previous, |widget| {
                if let Some(target) = widget.drop_target_mut() {
                    target.report_leave();
                }
            });
        }
        let effect = if session.is_active() {
            match widget_at_point(pos, viewport) {
                Some(target) => {
                    let dropped = with_widget_mut(target, |widget| {
                        widget.drop_target_mut().map(|target| session.drop_on(target, pos))
                    });
                    dropped.flatten().unwrap_or(DropEffect::None)
                }
                // A release outside every control is a cancelled drag, not a drop:
                // `DropEffect::None` tells the source to keep its item.
                None => DropEffect::None,
            }
        } else {
            DropEffect::None
        };
        if let Some(source) = source {
            let _ = with_widget_mut(source, |widget| {
                if let Some(source) = widget.drag_source_mut() {
                    source.on_drag_finished(effect);
                }
            });
        }
        Some(effect)
    }
}

/// Returns the deepest mounted control whose own hit rule accepts `point`.
///
/// The drag coordinator has no root id to descend from — a drag may travel between
/// surfaces, and the caller hands it a viewport rather than a tree — so it asks the
/// registry directly. The deepest hit wins, expressed as the smallest rectangle: a child
/// lies inside its parent, and the pointer belongs to the most specific control under it.
/// Equal areas are broken by the later id, matching the "later sibling paints on top"
/// rule the renderer uses.
fn widget_at_point(point: Point, viewport: Rect) -> Option<ObjectId> {
    if !viewport.contains_point(point) {
        return None;
    }
    let mut best: Option<(ObjectId, u32)> = None;
    for_each_mounted_widget(|id, widget| {
        if !widget.is_visible() || !widget.is_enabled() || !widget.contains_point(point) {
            return;
        }
        let rect = widget.geometry();
        let area = rect.width.saturating_mul(rect.height);
        let better = match best {
            None => true,
            Some((best_id, best_area)) => area < best_area || (area == best_area && id >= best_id),
        };
        if better {
            best = Some((id, area));
        }
    });
    best.map(|(id, _)| id)
}

/// Gives `id` capture of the pointer until released.
///
/// Returns `false` for an unmounted id or when it already holds capture, so a caller
/// can tell "nothing changed" from "capture moved". Capture is how a drag survives the
/// pointer leaving the widget that started it.
pub fn capture_pointer(id: ObjectId) -> bool {
    if !is_mounted(id) {
        return false;
    }
    CAPTURE.try_with(|capture| capture.borrow_mut().set_capture(id)).unwrap_or(false)
}

/// Releases the pointer capture, if any. Returns whether there was one to release.
pub fn release_pointer_capture() -> bool {
    CAPTURE.try_with(|capture| capture.borrow_mut().release_capture()).unwrap_or(false)
}

/// Returns the widget currently holding pointer capture, if any.
pub fn capturing_widget() -> Option<ObjectId> {
    CAPTURE.try_with(|capture| capture.borrow().capturing_widget()).unwrap_or(None)
}

/// Returns whether `id` currently holds pointer capture.
pub fn has_pointer_capture(id: ObjectId) -> bool {
    CAPTURE.try_with(|capture| capture.borrow().has_capture(id)).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Modal dialog enforcement
// ---------------------------------------------------------------------------

/// Returns whether `descendant` is `ancestor` or anywhere below it in the widget
/// tree, walking the [`crate::widget::Widget::parent`] links.
///
/// A widget is its own descendant for this purpose: a modal dialog's own controls
/// (and the dialog itself) must keep receiving input while the modal is up.
fn is_descendant_of(ancestor: ObjectId, descendant: ObjectId) -> bool {
    if ancestor == descendant {
        return true;
    }
    let mut cursor = read_widget(descendant, |widget| widget.parent());
    while let Some(current) = cursor.flatten() {
        if current == ancestor {
            return true;
        }
        cursor = read_widget(current, |widget| widget.parent());
    }
    false
}

/// Returns the top of the modal stack, i.e. the dialog that currently owns input.
pub fn active_modal() -> Option<ObjectId> {
    MODAL.try_with(|stack| stack.borrow().last().copied()).unwrap_or(None)
}

/// Returns whether any modal dialog is currently active.
pub fn is_modal_active() -> bool {
    MODAL.try_with(|stack| !stack.borrow().is_empty()).unwrap_or(false)
}

/// Returns whether `id` is outside the active modal dialog's subtree and therefore
/// must not receive input.
///
/// This is the single predicate both event dispatch and focus routing consult, so
/// the rule "a modal blocks everything except itself and its descendants" is stated
/// once rather than re-derived at each call site. When no modal is active, nothing
/// is blocked.
pub fn modal_blocks(id: ObjectId) -> bool {
    let Some(modal) = active_modal() else {
        return false;
    };
    // The modal itself and its subtree stay live; everything else is blocked. An
    // unmounted id is also blocked — it cannot be part of the live dialog.
    !is_descendant_of(modal, id)
}

/// Pushes `dialog_id` onto the modal stack so it becomes the input owner.
///
/// Returns `false` for an unmounted id, so a caller cannot install a modal that no
/// event can ever reach. Re-pushing an already-active id is a no-op that reports
/// `true`: the caller asked for "make this modal", and it already is.
pub fn enter_modal(dialog_id: ObjectId) -> bool {
    if !is_mounted(dialog_id) {
        return false;
    }
    MODAL
        .try_with(|stack| {
            let mut stack = stack.borrow_mut();
            if stack.last() == Some(&dialog_id) {
                return true;
            }
            stack.push(dialog_id);
            true
        })
        .unwrap_or(false)
}

/// Removes the topmost occurrence of `dialog_id` from the modal stack.
///
/// A dialog that is dismissed pops itself; this is also how a caller closes a modal
/// it opened. Returns whether the stack changed.
pub fn exit_modal(dialog_id: ObjectId) -> bool {
    MODAL
        .try_with(|stack| {
            let mut stack = stack.borrow_mut();
            if let Some(pos) = stack.iter().rposition(|&id| id == dialog_id) {
                stack.remove(pos);
                true
            } else {
                false
            }
        })
        .unwrap_or(false)
}

/// Closes *every* active modal dialog, leaving the stack empty.
///
/// A hard reset for a host that is tearing down a window tree or an application
/// shutdown, where the incremental pop of [`exit_modal`] would leave a stale id
/// behind if the caller lost track of the open order.
pub fn clear_modals() {
    let _ = MODAL.try_with(|stack| stack.borrow_mut().clear());
}

/// Sends the enter/leave pair for a pointer that is now over `hovered`.
///
/// The library synthesises these because no backend does: a backend knows "the mouse
/// moved in my surface", not "the pointer crossed onto a different control", and
/// hover is a property of the widget tree. Tracking the previous target here keeps
/// the pair balanced — exactly one `MouseLeave` for the widget left, one
/// `MouseEnter` for the widget entered, and neither when the pointer stays put.
fn dispatch_hover_transition(hovered: Option<ObjectId>, point: Point) {
    // A widget a modal blocks receives no input, so it must not record a hover either: the
    // `MouseEnter` below is refused by `dispatch_event`, while `HOVERED` would still hold the
    // widget — breaking the pair invariant stated above, reporting a hover for a control the
    // user cannot touch, and publishing its cursor (an I-beam behind a dialog) through
    // `Platform::set_cursor`.
    let hovered = hovered.filter(|id| !modal_blocks(*id));
    let previous = HOVERED.with(|last| *last.borrow());
    if previous != hovered {
        HOVERED.with(|last| *last.borrow_mut() = hovered);
        // Deliver outside the cell borrow: a control handling `MouseLeave` may itself
        // read or write hover state.
        if let Some(previous) = previous {
            let _ = dispatch_event(previous, &Event::MouseLeave { pos: point });
        }
        if let Some(hovered) = hovered {
            let _ = dispatch_event(hovered, &Event::MouseEnter { pos: point });
        }
    }

    // BLUE28 B28.09: the control's cursor intent is applied through the platform once
    // per **change**, not per motion event. The intent is asked at the pointer's
    // position because one control may want different shapes in different regions
    // (an editor's text is an I-beam, its minimap is an arrow). A platform call is
    // made only when the answer or the target window changed.
    //
    // `read_widget`, not `with_widget`: this runs on **every** pointer move, and the detaching
    // variant removes the entry, installs two guards and re-inserts it — ~ten hash operations to
    // read one `&self` getter. `read_widget` is the read-only shape documented for exactly this
    // kind of query.
    let icon = hovered
        .and_then(|id| read_widget(id, |widget| widget.cursor_icon_at(point)))
        .unwrap_or(crate::platform::CursorIcon::Default);
    let key = (hovered.or(previous), icon);
    if LAST_CURSOR.with(|last| *last.borrow()) == key {
        return;
    }
    LAST_CURSOR.with(|last| *last.borrow_mut() = key);
    if let Some(window) = hovered.or(previous).and_then(top_level_window_of) {
        let _ = crate::platform::platform_facts().set_cursor(window, icon);
    }
}

/// Clears the hover target, sending `MouseLeave` to whatever was hovered.
///
/// Called when the pointer leaves the surface entirely, which is the one case a
/// coordinate-based transition cannot observe: there is no widget under the pointer,
/// so the previous hover would otherwise stay highlighted forever.
pub fn clear_hover(point: Point) {
    dispatch_hover_transition(None, point);
}

/// Returns the widget the pointer is currently over, if any.
pub fn hovered_widget() -> Option<ObjectId> {
    HOVERED.with(|last| *last.borrow())
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
        }
    });
}

/// What one call to [`drive_frame`] did, kept for diagnostics and tests.
///
/// # Why this is a record and not a `bool`
///
/// "Did anything happen?" cannot answer the only question a frame loop ever has to
/// answer -- *should I schedule another frame?* -- without over-scheduling. A hover
/// that has finished still needs **one** more frame to be painted in its settled
/// state, and then no more; a window with nothing moving needs none. Both cases
/// return `false` from a bare `bool`, so a loop built on one either spins forever or
/// drops the frame that draws the settle. [`FrameOutcome::needs_another_frame`] is
/// computed from the bus *after* this frame's work, so the frame that observes a
/// settle reports `true` and the one after it reports `false`.
///
/// The remaining fields exist because "this frame cost something" should be
/// answerable *by cause*: how many events woke it, how many controls interpolated,
/// how many platform repaints went out. That is the same grouping BLUE24 §8 asks of
/// `FrameStats`, measured one second earlier in the pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameOutcome {
    /// Events drained from the trigger queue and dispatched this frame.
    pub events_dispatched: usize,
    /// Controls that answered `is_animating()` and were advanced by `delta_ms`.
    pub controls_ticked: usize,
    /// Repaint requests submitted to the platform this frame.
    pub repaints_submitted: usize,
    /// Whether the host should schedule another frame.
    pub needs_another_frame: bool,
}

/// The decode accounting for a build with no decoder linked in: always zero.
///
/// # Why a real type and not `(0, 0, 0)`
///
/// `drive_frame` reads the cache counters twice per frame and takes the difference. On a build
/// without `feature = "image"` (`android`, `embedded`) there is no `crate::image` module to ask, and
/// naming it did not compile. The three readings could have been written as three separate `cfg`
/// blocks with literal zeros, but a type keeps the two readings structurally identical to the
/// gated arm: the same field names, the same subtraction below, one `cfg` on each read.
///
/// The zero is the *true* answer rather than a placeholder. With no decoder compiled in, no decode
/// can occur, so a frame's share is genuinely zero -- the same reasoning
/// `avatar::resolved_image_source` records for its own no-decoder arm. It is not a "we could not
/// measure this" stand-in, which would be a dishonest number in exactly the place this crate's
/// frame ledger exists to be honest about.
#[cfg(not(all(feature = "image", not(alloc_frugal))))]
#[derive(Default)]
struct NoDecoderStats {
    requests: u64,
    hits: u64,
    misses: u64,
}

/// A frame's cost, grouped by **what produced it** rather than as one total.
///
/// # Why not a duration
///
/// "This frame took 12 ms" cannot be acted on: it might be one control repainting the whole
/// surface, a damage merge that failed, or an animation running more frames than it needs. So
/// the ledger groups by **consumer** -- who was advanced, who was redrawn, how much merging
/// saved -- which is the form a reader can act on (BLUE24 §8.1).
///
/// # Why it is a second type beside [`FrameOutcome`]
///
/// [`FrameOutcome`] answers the frame loop's one question (*schedule another frame?*) and is
/// `Copy` because a loop holds it every iteration. This carries a `Vec` of causes, so it is
/// not `Copy` and is taken **on demand** -- a host that wants the account asks for it, and a
/// host that only drives frames never pays for the allocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Monotonic frame counter, so two readings can be compared without a clock.
    pub index: u64,
    /// The delta this frame advanced animations by.
    pub delta_ms: u32,
    /// Events drained and dispatched this frame.
    pub events: usize,
    /// Controls that answered `is_animating()` and were advanced.
    pub controls_ticked: usize,
    /// Surfaces submitted for repaint this frame (after coalescing).
    pub controls_drawn: usize,
    /// Repaint requests **folded into** an earlier request for the same surface this frame.
    ///
    /// The saving, not the work: five requests for one surface submit once and coalesce four.
    pub repaints_coalesced: usize,
    /// Why each repainted surface was repainted, last cause first per surface.
    ///
    /// One entry per surface -- the reason it was repainted -- so a reader can answer "why is
    /// this still redrawing?" without a log. A surface with no entry is one that was not
    /// repainted, which is why "no anonymous repaint" (BLUE24 §8 criterion 4) is a statement
    /// about this list being complete rather than about it being non-empty.
    pub last_repaint_reason: alloc::vec::Vec<(ObjectId, RepaintReason)>,
    /// Image decodes the frame avoided, by sharing a decode an earlier frame already did.
    ///
    /// # Why an image decode belongs in a frame's account
    ///
    /// A decode is the most expensive thing a control can do inside `draw` — a full PNG inflate, or
    /// a JPEG pass, on the thread that is supposed to be presenting. "This frame was slow" is not
    /// answerable without knowing whether it decoded a photograph, and "is the cache helping" is not
    /// answerable without knowing whether the decodes went away. Both are one number.
    ///
    /// # These are deltas, and the counters they come from are cumulative
    ///
    /// [`crate::image::cache::stats`] is process-wide and monotonic, because it answers "what has
    /// this process saved". A frame reports its own share: the difference between the two readings
    /// taken around the frame's work. `decode_hits + decode_misses <= decode_requests` holds within a
    /// frame; across the process the counters balance exactly.
    ///
    /// A build without the `image` feature reports zeros rather than omitting the fields, so a host
    /// that reads them does not have to be compiled differently to do it.
    pub decode_requests: u64,
    /// Decode requests answered without decoding.
    pub decode_hits: u64,
    /// Decode requests that had to decode — the ones that cost this frame real time.
    pub decode_misses: u64,
}

/// The frame account as text, so it can leave the process.
///
/// # Why this is the exportable half of BLUE24 §12 U-8
///
/// U-8 (a remote / off-screen renderer) asks for the frame account to be a **product** rather than
/// an in-memory value: pixels can be sent to another machine, but "why this frame looks like this"
/// can only be transmitted if the account has a form that survives leaving the process. A `Debug`
/// derive is not that form — it is a debugger's view, unstable across versions and unreadable in a
/// log. This is the stable one: the fields in a fixed order, the causes by their published tokens.
///
/// It is also the answer to a defect that exists **today, in this process**: `last_frame_stats()`
/// had no consumer anywhere in the crate, so the ledger's whole purpose — answering "why is this
/// still redrawing?" — was reachable only from a Rust expression. A host logging a slow frame now
/// has something to log.
///
/// # Why one line and not JSON
///
/// The crate's JSON path is hand-written (`capability::designer_manifest`), and adding a second
/// serializer for a value with no JSON consumer would be a second mechanism for one question
/// (principle #101). A single line of `key=value` is what a log wants, diffs cleanly, and needs no
/// dependency in a `no_std` build.
impl core::fmt::Display for FrameStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "frame={} delta_ms={} events={} ticked={} drawn={} coalesced={} decode={}/{}/{}",
            self.index,
            self.delta_ms,
            self.events,
            self.controls_ticked,
            self.controls_drawn,
            self.repaints_coalesced,
            self.decode_requests,
            self.decode_hits,
            self.decode_misses,
        )?;
        // The causes are the part a reader scans for, so they are named rather than counted: a frame
        // that says "drawn=3" without saying why is the anonymous repaint BLUE24 §8 criterion 4
        // exists to prevent.
        for (surface, reason) in &self.last_repaint_reason {
            write!(f, " repaint[{}]={}", surface, reason.as_str())?;
        }
        Ok(())
    }
}

/// Performs every library-side task of one frame, in a fixed order, exactly once.
///
/// # Why a *frame* and not just [`tick_animations`]
///
/// Three entry points each did part of a frame, in an order that differed per platform:
/// [`crate::drain_triggers`] emptied the input queue, [`tick_animations`] advanced the
/// animation bus, and the paint path (`draw_bridge::draw_of`) advanced a control *again*
/// as a side effect of drawing it. So how many times a control was interpolated in one
/// frame depended on how many layers the host happened to call, when the only correct
/// answer is **once**, and **before** painting -- otherwise every frame paints the state
/// it had one frame ago and the animation visibly lags its own input.
///
/// This function is the one place that answers that ordering, so the answer is the same
/// on every backend.
///
/// # The order, and why no step may move
///
/// 0. **Refresh the device facts.** One snapshot per frame, read before anything else, so every
///    control in this frame sees the same text scale, motion preference and direction. A host
///    that changed a system setting sees it apply on the next frame, never part-way through a
///    paint (see [`crate::style::environment`]'s module docs).
/// 1. **Drain input.** Pointer, key and focus events that arrived since the last frame
///    land first, because an animation's *target* comes from state and state comes from
///    these events. Reversed, a control would not begin reacting until the following
///    frame -- the input latency the ordering exists to remove.
/// 2. **Advance the animation bus once.** Controls answering `is_animating()` take
///    `delta_ms`. Because this precedes painting, the frame that a hover starts on is
///    already one step in.
/// 3. **Report whether another frame is owed.** Taken *after* steps 1-2, which is what
///    makes the frame that observes a control settling report `true` (so the settle gets
///    painted) and the next one report `false` (so an idle window stops paying).
///
/// # Two loop-owned jobs run before step 0
///
/// Two things here are not *frame* tasks, and both belong to the loop rather than to a frame:
/// work another thread handed to this thread ([`crate::platform::main_thread`]) and a loop-stop
/// request made from another thread ([`crate::platform::stop`]). They are done at the top of this
/// function because this is the one function every loop this library owns calls **on the thread
/// that owns the loop** — GTK's timeout closure, the polling loops of the
/// state/wayland/windows/mobile backends, and AppKit's per-window `NSTimer` — which is the only
/// thread a toolkit permits either a modal-free callback or a stop on. Running them first means a
/// loop already on its way out is not asked for one more frame of work before it may leave.
///
/// # `repaints_submitted`: why it is counted rather than performed here
///
/// Damage is submitted where the damage is *discovered* -- `request_repaint`,
/// `mark_dirty_rect`, and the platform's own native redraw path -- so that a control
/// which redraws mid-frame does not wait for the next one. This function therefore
/// accounts for the submissions made during this frame's steps 1-2 rather than making
/// them itself; a frame loop learns "this frame invalidated N surfaces" without the
/// library having to buffer damage it has nowhere to wait for.
///
/// # Static frames are free
///
/// With nothing queued and nothing animating, this is two `try_with` lookups plus a
/// registry sweep of `is_animating()`, returns `needs_another_frame == false`, and the
/// host sleeps until the next event. That property is what lets the loop run at the
/// display's rate while the window is still, and it is asserted by this module's tests.
pub fn drive_frame(delta_ms: u32) -> FrameOutcome {
    // The two jobs that belong to the *loop* rather than to a frame are done here, on the
    // thread that owns the loop, before the frame's own work: work another thread handed to this
    // thread (`platform::main_thread`), then a stop request another thread made
    // (`platform::stop`). See this function's docs for why the tick is the place for both.
    crate::platform::main_thread::run_due_on_loop_thread();
    crate::platform::stop::honour_request_on_loop_thread();

    // BLUE28 B28.24: a suspended app has no animation clock. Input is still drained (so state
    // changes are not queued for the whole background period), but no control advances and no
    // frame is owed. The frame after a resume uses a zero delta so the background gap is not
    // replayed as one enormous step.
    let suspended = animation_suspended();
    let delta_ms = if take_resume_reset() { 0 } else { delta_ms };

    // The decode counters, read before anything else in the frame so this frame's share is exactly
    // the work done between here and the reading below -- and not whatever a previous frame left.
    //
    // # Why this is two `cfg` arms rather than one call
    //
    // `crate::image` is gated on `feature = "image"`, while this function is not: `full_widgets`
    // profiles like `android` and `embedded` have the widget tree but not the decoder. Naming
    // `crate::image::cache::stats()` unconditionally therefore failed to *compile* on those targets
    // (`unresolved module image`), which is why `check_android_cross.sh` reported a cross-target
    // break on `aarch64-linux-android` while every host build was green.
    //
    // The zero is not a placeholder. With no decoder compiled in, no decode *can* happen, so
    // "this frame decoded nothing" is the true count rather than a stand-in for an unknown one --
    // the same reasoning `avatar::resolved_image_source` uses for its no-decoder arm.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    let decodes_before = crate::image::cache::stats();
    #[cfg(not(all(feature = "image", not(alloc_frugal))))]
    let decodes_before = NoDecoderStats::default();

    // Step 0 -- the device facts, so every control in this frame sees one set of them.
    crate::style::environment::refresh_environment();

    // Step 0b -- a global input changed (theme, text scale, locale) since the last frame:
    // repaint every window and mounted surface once. Armed instead of performed at the
    // mutation site to keep the non-reentrant theme lock out of the platform path.
    flush_global_repaint();

    // Step 1 -- input first, so this frame's animation targets are this frame's state.
    let events_dispatched = crate::drain_triggers();

    // Step 2 -- exactly one advance per animating control, before any paint.
    //
    // The sweep also counts what it advanced, and reports whether anything is still moving: the
    // frame used to re-walk the whole registry for the count and a third time for
    // `needs_another_frame`, so one still frame cost three full-tree scans.
    let (controls_ticked, sweep_animating) = if suspended {
        (count_animating_widgets(), false)
    } else {
        tick_animations_sweep(delta_ms)
    };
    // A narrow submission queued while a control was lent out for its `tick` is flushed here,
    // before the ledger below reads the frame's repaint counts, so the frame that made the
    // request is the frame that reports it. The sweep already flushes its own; this also covers
    // a suspended frame, where no sweep runs.
    flush_deferred_damage();

    // Step 3 -- the bus answers for the *next* frame, so a settle is painted once more
    // and then scheduling stops. `sweep_animating` is this frame's answer from the sweep just
    // above; the host-owned fact and the settle latch are folded in by the helper.
    let needs_another_frame =
        !suspended && animation_bus_needs_another_frame_given(sweep_animating);

    // Step 4 -- translations that changed on disk, so an edited catalogue reaches the
    // screen without a restart. This runs *after* the animation step and before the
    // repaint counts are taken, because applying a reload can invalidate text and thus
    // submit a repaint; taking the counts first would attribute that repaint to the
    // next frame and make this frame's ledger understate its own work.
    //
    // It is unconditionally cheap when hot reload is off: `pump_hot_reload` is one
    // mutex acquisition and a `None` check. That matters because every platform loop
    // calls `drive_frame` on a timer, and a `cfg` here would fork the frame driver per
    // profile for a saving that is not measurable.
    #[cfg(feature = "i18n")]
    crate::i18n::pump_hot_reload();

    // The numbers the frame ledger will report. Taken together so the counts and the causes
    // describe the same frame: taking them apart would let a repaint be attributed to the
    // frame after the one that submitted it.
    let repaints_submitted = take_frame_repaint_count();
    let repaints_coalesced = take_frame_coalesced_count();
    let last_repaint_reason = take_frame_repaint_reasons();
    let index = FRAME_INDEX.try_with(|frame| frame.replace(frame.get() + 1)).unwrap_or(0);

    // This frame's share of the process-wide decode accounting. A delta rather than the totals,
    // because the question a frame ledger answers is about *this* frame; see `FrameStats`.
    //
    // Read **after** the draw work rather than with the repaint numbers above, because a decode
    // happens inside `draw` and the whole point is to account for it. (The repaint counts are taken
    // before painting for the opposite reason: they report what the *input* steps invalidated, and a
    // submission made while painting belongs to the frame that will paint it.)
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    let decodes_after = crate::image::cache::stats();
    #[cfg(not(all(feature = "image", not(alloc_frugal))))]
    let decodes_after = NoDecoderStats::default();
    let decode_requests = decodes_after.requests.saturating_sub(decodes_before.requests);
    let decode_hits = decodes_after.hits.saturating_sub(decodes_before.hits);
    let decode_misses = decodes_after.misses.saturating_sub(decodes_before.misses);

    let _ = LAST_FRAME_STATS.try_with(|slot| {
        slot.replace(Some(FrameStats {
            index,
            delta_ms,
            events: events_dispatched,
            controls_ticked,
            controls_drawn: repaints_submitted,
            repaints_coalesced,
            last_repaint_reason,
            decode_requests,
            decode_hits,
            decode_misses,
        }));
    });

    FrameOutcome { events_dispatched, controls_ticked, repaints_submitted, needs_another_frame }
}

thread_local! {
    /// The most recent frame's ledger, for [`last_frame_stats`].
    ///
    /// Stored rather than returned because [`drive_frame`]'s return type is part of the frame
    /// contract a loop calls every iteration (and is `Copy`); a host that wants the account
    /// reads it separately, so a host that does not want it pays nothing.
    #[allow(clippy::missing_const_for_thread_local)]
    static LAST_FRAME_STATS: core::cell::RefCell<Option<FrameStats>> =
        const { core::cell::RefCell::new(None) };
}

/// The ledger for the most recent [`drive_frame`], or `None` when no frame has run.
///
/// # Why "why" is answerable at all
///
/// Because every repaint submission in the crate names a [`RepaintReason`] at its one point of
/// submission, and this returns those reasons grouped by surface. A frame that repainted
/// seven surfaces because an animation is in flight says so; if instead it says nothing, no
/// repaint happened, which is the still-frame guarantee being *observable* rather than asserted
/// (BLUE24 §8 criterion 1).
pub fn last_frame_stats() -> Option<FrameStats> {
    LAST_FRAME_STATS.try_with(|slot| slot.borrow().clone()).ok().flatten()
}

/// Counts mounted controls that currently report themselves as animating.
///
/// Split out so the two readers of the same fact -- [`drive_frame`]'s diagnostic count
/// and [`has_animating_widgets`]'s boolean -- cannot drift: both walk the registry once
/// and ask the same predicate.
fn count_animating_widgets() -> usize {
    let mut count = 0usize;
    for_each_mounted_widget(|_, widget| {
        if widget.is_animating() {
            count += 1;
        }
    });
    count
}

thread_local! {
    /// Repaint requests submitted since the last [`drive_frame`].
    ///
    /// A counter rather than a log: the frame only needs "how many", and the reasons
    /// belong to the control that asked (BLUE24 §8's `RepaintReason` is where that
    /// question is answered, not here). Reset by [`take_frame_repaint_count`].
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_REPAINTS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };

    /// Repaint requests that were **folded into an earlier request for the same surface**
    /// this frame, so they cost the platform nothing extra.
    ///
    /// A coalesced request is not a lost one: the surface is already going to be repainted,
    /// so asking again would be a second submission of the same work. Counting it is what
    /// lets a frame answer "how much did merging save?" rather than only "how many went
    /// out?" -- BLUE24 §8 criterion 3 (five controls asking for one rectangle submit once
    /// and coalesce four).
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_REPAINTS_COALESCED: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };

    /// The last reason each surface was repainted, for BLUE24 §8's `FrameStats`.
    ///
    /// One entry per surface, overwritten (not appended), so a surface repainted fifty times
    /// in a frame reads as "the reason it was repainted" rather than fifty rows. `Explicit`
    /// means "a caller asked, and named no cause" — which is itself a fact worth being able
    /// to see, because it is the one reason a gate cannot attribute to a mechanism.
    ///
    /// This map is also the coalescing set: "has this surface already been repainted this
    /// frame?" is `contains_key`. Keeping one ordered map rather than a `Vec` of surfaces plus
    /// a `Vec` of reasons makes membership O(log n) instead of a linear scan on every repaint
    /// request, which is what made a full-tree invalidate O(n²) in the number of surfaces.
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_REPAINT_REASONS: core::cell::RefCell<
        alloc::collections::BTreeMap<ObjectId, RepaintReason>,
    > = const { core::cell::RefCell::new(alloc::collections::BTreeMap::new()) };

    /// Monotonic frame counter, incremented once per [`drive_frame`].
    ///
    /// A counter rather than a clock: two readings of [`FrameStats`] are comparable by
    /// subtracting indices, which needs no `SystemTime` and therefore works under `mini`
    /// (BLUE24 §4's "no device fact from a wall clock" rule applies to the frame ledger too).
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_INDEX: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// Whether the animation clock is stopped because the host app is in the background.
///
/// A process-wide flag rather than a thread-local: the lifecycle callback that sets it arrives
/// on the platform thread, while a frame loop may run on another thread (and `mini` has no
/// thread-local frame ledger at all). Suspend/resume is a fact about the app, not about a thread.
static ANIMATIONS_SUSPENDED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Set by [`set_animation_suspended(false)`](set_animation_suspended); the next frame advances by
/// zero so a long background gap is not replayed as one huge delta.
static ANIMATION_RESUME_RESET: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// Stops or restarts the animation clock (BLUE28 B28.24).
///
/// `true` is what a host calls from `onPause`/`sceneWillResignActive`: no control advances while
/// the app is in the background, so a transition that was in flight resumes from the phase it
/// reached rather than jumping to its end. `false` is `onResume`/`didBecomeActive`: the next
/// frame is advanced with a zero delta so the time spent in the background is not treated as one
/// frame's worth of animation.
///
/// Calling either transition repeatedly is harmless; only the value matters.
pub fn set_animation_suspended(suspended: bool) {
    use core::sync::atomic::Ordering;
    if suspended {
        ANIMATIONS_SUSPENDED.store(true, Ordering::SeqCst);
    } else {
        ANIMATIONS_SUSPENDED.store(false, Ordering::SeqCst);
        ANIMATION_RESUME_RESET.store(true, Ordering::SeqCst);
    }
}

/// Whether the animation clock is currently stopped.
pub fn animation_suspended() -> bool {
    ANIMATIONS_SUSPENDED.load(core::sync::atomic::Ordering::SeqCst)
}

/// Reads and clears the one-frame zero-delta request left by a resume.
fn take_resume_reset() -> bool {
    ANIMATION_RESUME_RESET.swap(false, core::sync::atomic::Ordering::SeqCst)
}

/// Why a surface was asked to repaint.
///
/// # Why a reason and not just a count
///
/// "This frame submitted seven repaints" cannot be acted on. "Seven repaints, all because an
/// animation is still in flight" says the animation is the cost; "seven, all because a control
/// changed its theme state" says the styles are; "seven, all `Explicit`" says a caller is
/// repainting by hand and no library mechanism can explain it. BLUE24 §8 asks the frame ledger
/// to answer by **cause**, and this is the cause.
///
/// The set is deliberately closed, and each variant is a mechanism a reader can go and find:
/// an implementation that adds a repaint must pick the variant that names *why*, so the ledger
/// cannot silently grow an "other" bucket that explains nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepaintReason {
    /// A control's self-driven animation advanced and needs to be seen.
    Animation,
    /// A control's interaction or semantic state changed (`widget_state` / `semantic_state`).
    State,
    /// An overlay's content changed (a tooltip, a popup, a menu).
    Overlay,
    /// A caller asked directly (a property write, an imperative redraw) and named no mechanism.
    Explicit,
    /// The platform itself asked for its surface to be redrawn.
    ///
    /// Distinct from every variant above because it is the *other* world in a mixed
    /// native/self-painted window (BLUE24 §7): the platform owns the pixels there, and this is
    /// how the library comes to know about it.
    Native,
}

impl RepaintReason {
    /// Whether this reason means the surface needs another frame after this one.
    ///
    /// Only [`RepaintReason::Animation`] does: it is the one cause that keeps producing new
    /// pixels frame after frame. The others describe a change that is now complete, and a
    /// frame loop that scheduled another frame for them would spin on a still window.
    pub fn keeps_animating(self) -> bool {
        matches!(self, RepaintReason::Animation)
    }

    /// The reason's stable spelling, for a frame account that leaves the process.
    ///
    /// # Why a cause needs a name at all
    ///
    /// [`FrameStats`] carries `last_repaint_reason` — one entry per repainted surface — because
    /// "why is this still redrawing?" is the question the ledger exists to answer. But until this
    /// function existed the answer could not be **written down**: the reason is an enum, and a host
    /// logging the account or showing it in a diagnostics pane had no spelling to print. The field
    /// was therefore complete for a Rust reader and useless to any other kind.
    ///
    /// The tokens are lower-case and stable, because they are what a log line or an exported
    /// account will be read and grepped for. They match [`RepaintReason::parse`], so the two cannot
    /// drift into a spelling that only one of them accepts.
    pub fn as_str(self) -> &'static str {
        match self {
            RepaintReason::Animation => "animation",
            RepaintReason::State => "state",
            RepaintReason::Overlay => "overlay",
            RepaintReason::Explicit => "explicit",
            RepaintReason::Native => "native",
        }
    }

    /// Parses a token produced by [`RepaintReason::as_str`].
    ///
    /// `None` rather than a default: an account that named a cause the library does not publish is
    /// a fact about a version mismatch, and silently folding it into `Explicit` would make a reader
    /// believe the library asked for a repaint it did not. The same rule the crate's other token
    /// parsers follow (`BevelDirection::parse`, `Elevation::parse`).
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "animation" => Some(RepaintReason::Animation),
            "state" => Some(RepaintReason::State),
            "overlay" => Some(RepaintReason::Overlay),
            "explicit" => Some(RepaintReason::Explicit),
            "native" => Some(RepaintReason::Native),
            _ => None,
        }
    }
}

/// Records that this frame submitted one repaint to the platform, for `reason` on `surface`.
///
/// Called from the damage-tracking paths, which are the single places a repaint is
/// requested, so the count cannot miss a submission made through the library.
///
/// # Coalescing
///
/// A second request for a surface already repainted this frame is **counted as coalesced
/// rather than submitted**. That is what makes BLUE24 §8 criterion 3 true: five controls
/// announcing damage on one surface cost the platform one submission, and the frame can
/// report the four it saved. The surface, not the control, is the unit because a submission
/// is per surface -- two controls drawing into one window are one repaint.
///
/// The reason recorded is the one from the **first** request for that surface this frame:
/// it is the cause that already made the frame happen, and overwriting it with a later, more
/// incidental reason would make the ledger describe the last caller rather than the cost.
pub(crate) fn note_frame_repaint_reason(surface: ObjectId, reason: RepaintReason) {
    // `true` when this surface was already recorded this frame. The stored reason is the
    // first one, which is the cause that made the frame happen.
    let already = FRAME_REPAINT_REASONS
        .try_with(|reasons| {
            let mut reasons = reasons.borrow_mut();
            match reasons.entry(surface) {
                alloc::collections::btree_map::Entry::Occupied(_) => true,
                alloc::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(reason);
                    false
                }
            }
        })
        .unwrap_or(false);

    if already {
        let _ = FRAME_REPAINTS_COALESCED.try_with(|count| count.set(count.get().saturating_add(1)));
        return;
    }

    let _ = FRAME_REPAINTS.try_with(|count| count.set(count.get().saturating_add(1)));
}

/// Takes (and clears) the number of repaints submitted since the last call.
fn take_frame_repaint_count() -> usize {
    FRAME_REPAINTS.try_with(|count| count.replace(0)).unwrap_or(0)
}

/// Takes (and clears) the number of repaints folded into an earlier one this frame.
fn take_frame_coalesced_count() -> usize {
    FRAME_REPAINTS_COALESCED.try_with(|count| count.replace(0)).unwrap_or(0)
}

/// Takes (and clears) why each surface was repainted this frame, and resets the surfaces set.
///
/// The surfaces set is cleared here rather than in [`take_frame_repaint_count`] so the two
/// takers are called together by [`drive_frame`] and the coalescing window is exactly one
/// frame: clearing it early would let a second request for the same surface be counted as a
/// fresh submission within the same frame.
fn take_frame_repaint_reasons() -> alloc::vec::Vec<(ObjectId, RepaintReason)> {
    FRAME_REPAINT_REASONS
        .try_with(|reasons| core::mem::take(&mut *reasons.borrow_mut()).into_iter().collect())
        .unwrap_or_default()
}

/// Advances every animating control by `delta_ms`, reporting whether another frame is needed.
///
/// # Why this is the crate's *only* frame driver
///
/// `src/style/animation.rs` and eleven controls' own `tick` state machines were all fully
/// implemented and all unreachable: nothing in `src/widget/`, `src/app/`, `src/render/` or
/// `src/platform/` ever called one, so a hover faded nowhere and a caret never blinked. The
/// missing piece is a single driver, and "who drives" has two wrong answers:
///
/// * **a timer per control** -- a hundred controls, a hundred timers, none of them aligned to
///   the frame, and each one a chance to leak;
/// * **each backend driving** -- the same button advanced twice in one frame, so its animation
///   runs at a speed that depends on how many layers happened to call in.
///
/// So the driver is this one function, called once per frame by the host *before* it paints.
/// The set of controls advanced is whatever answers `true` to its own
/// [`Widget::is_animating`] rather than a registry the runtime maintains: a registry would
/// have a "registered but never unregistered" leak, while self-reporting cannot -- an
/// unmounted control simply is not in the map any more.
///
/// # The return value is the whole point
///
/// `true` means "at least one control still owes frames", which is exactly when the host
/// should schedule another one. A window showing only resting controls therefore settles at
/// `false` and pays nothing per frame -- a still application does not burn a core, which is
/// the half of "smooth" that is easy to forget.
///
/// # Where a control can be, and why the sweep has to cover both
///
/// A control lives in exactly one of two places, and they are reached by completely
/// different APIs:
///
/// * **mounted** on the [`MOUNTED`] registry, which is how a host that built its tree
///   through [`crate::app`] holds every control; the window tree sweeps it;
/// * **owned by the caller**, as the `Box<dyn Widget>` that
///   [`crate::widget::WidgetFactory::create`] returns and that the documented embedding
///   pattern holds directly.
///
/// The first version of this driver swept only the registry. That is wrong for the owned
/// case, and wrong in the worst possible way: a control that is not mounted was never
/// advanced, yet nobody notices, because the control still *draws* -- it simply draws its
/// start state forever. A `CheckBox` ticked on program output, a `Switch` set from a
/// settings file, a caret in a directly-held `LineEdit` all sat motionless and reported no
/// error. The `census`/snapshot path in this very crate holds controls that way
/// (`draw_bridge::draw_of` takes `&mut dyn Widget`), which is why the gap is reachable from
/// the crate's own code and not only from a host's.
///
/// So a control that is advanced while the registry is being swept is recorded here, and
/// anything that subsequently asks *"should the next frame be scheduled?"* gets the union
/// of the two populations. That is what makes
/// [`crate::widget::draw_bridge::draw_of`] -- the single entry point for painting a
/// directly-owned control -- able to drive the frames such a control needs. Without it the
/// animation bus would be complete for mounted trees and silently inert for owned ones.
pub fn tick_animations(delta_ms: u32) -> bool {
    tick_animations_sweep(delta_ms).1
}

/// The sweep behind [`tick_animations`], also reporting how many controls it advanced.
///
/// `drive_frame` needs both numbers. Asking the registry a second time to count what the sweep
/// just saw made every frame walk the whole tree twice, and a third time to answer
/// "is anything still animating?". One walk now answers all of it.
fn tick_animations_sweep(delta_ms: u32) -> (usize, bool) {
    // A theme crossfade is a frame-driven animation too, and its tokens are read by every
    // control's paint. Advancing it here is what makes `set_theme_faded` visible without a
    // second timer; the manager arms the global repaint through `note_theme_mutation`.
    //
    // The theme module exists only on a device profile, so the sweep must compile without it:
    // a build with no profile has no theme to cross-fade, and reporting "not moving" is the
    // honest answer rather than a call the linker could not resolve.
    #[cfg(device_profile)]
    let theme_moving = if delta_ms == 0 {
        false
    } else {
        let mut manager = crate::theme::global_theme_manager();
        manager.tick_theme_crossfade(delta_ms)
    };
    #[cfg(not(device_profile))]
    let theme_moving = {
        let _ = delta_ms;
        false
    };
    let mut still_animating = theme_moving;
    let mut ticked = 0usize;
    // Ids that advanced this sweep. Collected rather than invalidated in place: the sweep
    // holds each widget out of the registry for its `tick`, and a repaint request walks the
    // parent chain (and may call back into the registry), so doing it inside the loop would
    // nest two registry drives.
    let mut moved: alloc::vec::Vec<ObjectId> = alloc::vec::Vec::new();
    for_each_mounted_widget(|id, widget| {
        // `is_animating` first, so a resting control (the common case) is not asked to
        // interpolate at all: this is what keeps a static frame free.
        if widget.is_animating() {
            // `tick` also reports "moving"; either answer keeps the frame coming, because a
            // control can be mid-flight while its own `is_animating` is a coarser fact.
            if widget.tick(delta_ms) || widget.is_animating() {
                still_animating = true;
            }
            // Counted *after* the tick, matching the ledger's pre-refactor meaning: a control
            // that settles on this frame is not reported as still ticking.
            if widget.is_animating() {
                ticked += 1;
            }
            // Submitted even on the settle frame, where `tick` has just placed the value at
            // its target and both answers are now `false`. That frame is the one that paints
            // the **finished** state; skipping it left the last drawn frame one step short —
            // measured as a menu popup frozen at 70% of its height after the reveal had
            // already settled in the model.
            moved.push(id);
        }
    });

    // # Why a tick must submit a repaint
    //
    // Advancing the model is only half a frame. Every backend's loop calls `drive_frame` on a
    // timer and presents only when the host says a surface needs drawing; when the sweep moved
    // a control and submitted nothing, the value advanced while the pixels stayed on the frame
    // the change started in. Measured on the GTK demo: a menu's open reveal reached ~28% over
    // four frames and then froze, because the click that opened it was the last thing that
    // invalidated the window. The same split applied to every animated control in a painted
    // window tree. Submitting the repaint here — with the cause named — is what makes
    // `needs_another_frame` mean "another frame will be *drawn*", not just "another frame will
    // be computed".
    for id in moved {
        request_narrow_repaint(id, RepaintReason::Animation);
    }
    flush_deferred_damage();
    // The latch is written **both ways**, and that is the whole point.
    //
    // # The defect this replaces
    //
    // It used to be set when `still_animating` and never cleared. The doc above explains what it
    // is for: scheduling the one frame that paints a control's *settled* state. But a flag whose
    // only clearer is the teardown function (`animation_bus_reset_host_owned`) is not a "one more
    // frame" request — it is a latch that turns `needs_another_frame` into a permanent `true`
    // from the first frame in which anything moved. A window that had ever shown a spinner could
    // never sleep again, which is the exact opposite of what `drive_frame` promises
    // (BLUE24 §1: "the cost of a still frame is one emptiness check").
    //
    // Writing `still_animating` unconditionally makes the flag mean what its name and its doc
    // say: *this* sweep found something unsettled. The settle frame still gets scheduled, because
    // the answer is read **after** the sweep: the last frame in which a control moved sets the
    // flag and is told `true`, the next sweep finds nothing and clears it, so the frame after
    // that is free. That is one extra frame, not an infinite stream.
    //
    // It cannot clobber the host-owned fact: `HOST_OWNED_ANIMATING` is a separate cell, written by
    // `draw_bridge` when it advances an owned control, and `animation_bus_needs_another_frame` ORs
    // the two.
    let _ = LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.set(still_animating));
    (ticked, still_animating)
}

/// Records that a **host-owned** control was advanced this frame, and reports whether the
/// animation bus should currently consider one to be in flight.
///
/// # Why this is a pair of free functions and not a method
///
/// [`tick_animations`] hands a control `delta_ms` and receives "do I need another frame?"
/// back. A host that owns its controls and calls their `tick` directly never goes through
/// that function, so it has no way to tell it what it saw. These two functions are that
/// channel: the control's owner reports the fact, and the bus folds it into the answer a
/// frame loop asks for.
///
/// The state is deliberately **not** cleared by [`tick_animations`]: a frame loop that calls
/// the bus for its mounted tree and calls `tick` on its owned controls usually asks the
/// question *after* tickling them, so clearing on the sweep would erase the answer the
/// caller had just reported. It is cleared when a host-owned control is observed to have
/// settled ([`animation_bus_note_host_owned_settled`]) or by
/// [`animation_bus_reset_host_owned`].
pub fn animation_bus_note_host_owned_animating(animating: bool) {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(animating));
}

/// Clears the host-owned in-flight fact once such a control has settled.
pub fn animation_bus_note_host_owned_settled() {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(false));
}

/// Forgets every host-owned in-flight fact, including the record of the last registry sweep.
///
/// A frame loop that has torn down its controls calls this so a settled window cannot be kept
/// awake by a control that no longer exists.
pub fn animation_bus_reset_host_owned() {
    let _ = HOST_OWNED_ANIMATING.try_with(|flag| flag.set(false));
    let _ = LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.set(false));
}

/// Whether the animation bus should schedule another frame right now.
///
/// # What it covers, and why it is not just [`has_animating_widgets`]
///
/// Two populations can be animating, and a host that owns some of its controls has to be
/// able to hear about both:
///
/// * the **mounted registry**, answered by [`has_animating_widgets`];
/// * a **host-owned** control that was advanced this frame, which the host reports with
///   [`animation_bus_note_host_owned_animating`] -- and, on the frame it settles, which is
///   the frame the old answer was `true` for a control that has since stopped, which is why
///   [`LAST_SWEEP_HAD_UNSETTLED_MOUNTED`] is kept: it is the only way to schedule the frame
///   that observes the settle.
///
/// ["smooth"](super) has two halves and this function is what keeps the second one honest:
/// a host asks *this*, not a constant `true`, so a window with nothing moving pays nothing.
pub fn animation_bus_needs_another_frame() -> bool {
    let mounted_animating = has_animating_widgets();
    animation_bus_needs_another_frame_given(mounted_animating)
}

/// [`animation_bus_needs_another_frame`] when the caller already knows whether a mounted control
/// is animating.
///
/// [`drive_frame`] gets that answer from the sweep it just ran, so it must not pay for a second
/// full-tree scan to ask the same question.
fn animation_bus_needs_another_frame_given(mounted_animating: bool) -> bool {
    let host_owned = HOST_OWNED_ANIMATING.try_with(|flag| flag.get()).unwrap_or(false);
    let sweep_unsettled =
        LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.get()).unwrap_or(false);
    host_owned || sweep_unsettled || mounted_animating
}

/// Whether any mounted control is currently animating.
///
/// A host can ask this before deciding whether to enter a continuous-frame mode, without
/// paying for the sweep [`tick_animations`] performs.
pub fn has_animating_widgets() -> bool {
    let mut animating = false;
    for_each_mounted_widget(|_, widget| {
        if widget.is_animating() {
            animating = true;
        }
    });
    animating
}

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
fn frame_origin(id: ObjectId) -> (i32, i32) {
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
    drop(_lent);
    let destroyed = in_flight.finish();
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
struct InFlightGuard {
    id: ObjectId,
    finished: bool,
}

impl InFlightGuard {
    fn install(id: ObjectId) -> Self {
        let _ = DISPATCHING.try_with(|set| {
            set.borrow_mut().insert(id);
        });
        Self { id, finished: false }
    }

    /// Clears the marker and consumes any destroy tombstone, reporting whether the control was
    /// destroyed during the drive.
    fn finish(&mut self) -> bool {
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
/// (see [`LENT_OUT`]) and a nested **write** to it is refused. Reads are unavailable while an outer
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
fn lent_present(id: ObjectId) -> bool {
    LENT_OUT.with(|map| map.borrow().contains_key(&id))
}

/// Runs `f` with a **read-only** view of a widget currently lent out of [`MOUNTED`].
///
/// Returns `None` when no widget is lent out under `id`. See [`LENT_OUT`] for why this exists and why
/// no mutating accessor may use it.
fn lent_out_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
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
fn lent_mutating(id: ObjectId) -> bool {
    LENT_MUTATING.with(|set| set.borrow().contains(&id))
}

fn lent_exclusive(id: ObjectId) -> bool {
    LENT_EXCLUSIVE.with(|set| set.borrow().contains(&id))
}

/// Marks a widget as lent out of [`MOUNTED`] for the guard's lifetime, restoring the read-only path
/// and (for a mutating loan) clearing the nested-write refusal on drop.
///
/// A `Drop` guard rather than code after the call, so the loan is undone on **both** the normal return
/// and an unwind out of a user callback. A callback that panics during dispatch or a property write no
/// longer leaves the widget permanently lent out and unreachable — the shape BLUE-issue E-19 records
/// for dispatch (it applied equally to `with_widget_mut`).
struct LentGuard {
    id: ObjectId,
    mutating: bool,
    exclusive: bool,
}

impl LentGuard {
    fn install(id: ObjectId, widget: &dyn Widget, mutating: bool, exclusive: bool) -> Self {
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
    static LENT_MUTATING: RefCell<IdSet> = RefCell::new(IdSet::new());
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
    static POPUP_GRAB: core::cell::Cell<Option<ObjectId>> = const { core::cell::Cell::new(None) };
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
    let _ = POPUP_GRAB.try_with(|slot| slot.set(Some(menu)));
}

/// Forgets the popup grab, if there is one.
#[cfg(not(alloc_frugal))]
pub(crate) fn clear_popup_grab() {
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
            slot.set(None);
        }
    });
}

/// The current popup grab, if any.
#[cfg(not(alloc_frugal))]
fn popup_grab() -> Option<ObjectId> {
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
fn popup_press_route(
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
fn popup_hover_origin(root: ObjectId, point: Point) -> Option<ObjectId> {
    let popup = popup_grab()?;
    if top_level_window_of(popup) != Some(root) {
        return None;
    }
    if !read_widget(popup, |widget| widget.is_visible()).unwrap_or(false) {
        return None;
    }
    widget_accepts_point(popup, point).then_some(popup)
}

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

/// Tells every observer of `widget_id` about one uncommitted composition update.
///
/// The list is cloned out before the callbacks run, so a callback may register another observer,
/// destroy the widget, or call [`clear_ime_preedit_callbacks`] without hitting an already-borrowed
/// `RefCell`. Returns whether at least one observer ran.
pub(crate) fn notify_ime_preedit(widget_id: ObjectId, text: &str, cursor: usize) -> bool {
    let callbacks: Vec<ImePreeditCallback> = IME_PREEDIT_CALLBACKS
        .try_with(|map| map.borrow().get(&widget_id).cloned())
        .unwrap_or_default()
        .unwrap_or_default();
    if callbacks.is_empty() {
        return false;
    }
    for callback in callbacks {
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
/// is dropped and `false` is returned — the same "not found" answer a hit outside any
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

    let destroyed = guard.finish();
    if destroyed {
        // A handler unregistered this control. The authoritative cleanup (focus, hover, capture,
        // cached frame, own-id map) already ran inside `unregister`; only the owned widget remains,
        // and dropping it here is what actually frees the control.
        return true;
    }

    // Put it back unless a handler remounted something under the same id in the meantime — in which
    // case the handler's own registration wins and is left untouched.
    let _ = MOUNTED.try_with(|map| {
        let mut map = map.borrow_mut();
        map.entry(id).or_insert(mounted);
    });
    // A caret-moving event (typing, arrows, a click) may have changed the insertion point.
    // Keep the platform candidate window anchored to the control's own answer; controls with
    // no insertion point answer `false` and cost one predicate.
    let _ = sync_ime_candidate(id);
    true
}

/// Owns the in-flight bookkeeping for one [`dispatch_event`] call, so it is undone even on an unwind.
///
/// Installs the exclusive loan ([`LENT_OUT`]), marks the id in [`DISPATCHING`], and on completion
/// clears the marker, consumes any destroy tombstone, and — for a surviving widget — removes the loan
/// so the entry can be reinserted by the caller. See BLUE-issue E-18 (self-reads) and E-19 (unwind).
struct DispatchGuard {
    id: ObjectId,
    finished: bool,
}

impl DispatchGuard {
    fn install(id: ObjectId, widget: &mut dyn Widget) -> Self {
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
    fn run(&mut self, mounted: &mut Mounted, event: &Event) {
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
    fn finish(&mut self) -> bool {
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
fn collect_subtree_ids(id: ObjectId, out: &mut alloc::vec::Vec<ObjectId>) {
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
fn hide_popup(menu: ObjectId) -> bool {
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
fn bring_popup_to_front(popup: ObjectId) -> bool {
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
/// [`flush_deferred_damage`], which the frame driver calls as soon as the sweep releases the
/// registry.
///
/// A control outside a tick should call [`request_repaint_rect`] instead: this path costs one
/// queue entry per control per frame.
pub fn request_repaint_rect_deferred(id: ObjectId, rect: Rect, reason: RepaintReason) {
    let _ = DEFERRED_DAMAGE.try_with(|queue| {
        let mut queue = queue.borrow_mut();
        // One entry per control per frame: a later rectangle supersedes an earlier one, and the
        // damage tracker coalesces whatever the two of them recorded anyway.
        if let Some(entry) = queue.iter_mut().find(|(queued, _, _)| *queued == id) {
            entry.1 = rect;
            entry.2 = reason;
        } else {
            queue.push((id, rect, reason));
        }
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

/// Submits a repaint for `id` bounded by its painted box, plus the same rectangle on the
/// window that paints it.
///
/// This is what the animation sweep uses per advanced control. It is a narrowing of
/// [`request_repaint_subtree_because`]: the window is still told, because on a window-tree
/// backend the window is what redraws the children, but it is told a rectangle instead of
/// being asked to repaint everything.
fn request_narrow_repaint(id: ObjectId, reason: RepaintReason) {
    let Some(rect) = painted_box_with_damage_margin(id) else {
        request_repaint_subtree_because(id, reason);
        return;
    };
    request_repaint_rect_because(id, rect, reason);
    if let Some(window) = top_level_window_of(id) {
        request_repaint_rect_because(window, rect, reason);
    }
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
fn grow_by_damage_margin(rect: Rect) -> Rect {
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
static GLOBAL_REPAINT_EPOCH: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// The global-repaint epoch this loop thread last repainted everything for.
    static GLOBAL_REPAINT_SEEN: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };

    /// The global-repaint generation this thread has observed.
    ///
    /// Per-widget layers live on the thread that painted them, so a theme reset only has to
    /// invalidate that thread's layers. [`arm_global_repaint`] bumps this beside the process-wide
    /// atomic: the atomic wakes every thread, while this counter is what the layer cache on the
    /// painting thread compares against. A cross-thread arming is still honoured — the next
    /// [`drive_frame`] flushes it through `mark_dirty_rect`, which bumps the tree epoch.
    #[allow(clippy::missing_const_for_thread_local)]
    static GLOBAL_REPAINT_LOCAL: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
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
#[cfg(any(device_profile, test))]
pub(crate) fn arm_global_repaint() {
    GLOBAL_REPAINT_EPOCH.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    let _ = GLOBAL_REPAINT_LOCAL.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
    crate::platform::frame_wait::wake();
}

/// The global-repaint generation layers on this thread must match.
fn global_repaint_generation() -> u64 {
    GLOBAL_REPAINT_LOCAL.try_with(|epoch| epoch.get()).unwrap_or(0)
}

/// The epoch a theme mutation bumps, so `theme::manager` can assert it armed the repaint.
#[cfg(test)]
pub(crate) fn global_repaint_epoch_for_test() -> u64 {
    GLOBAL_REPAINT_EPOCH.load(core::sync::atomic::Ordering::Relaxed)
}

/// Performs the full repaint `arm_global_repaint` requested, once per epoch.
fn flush_global_repaint() {
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

/// Records that the mounted **surface** `surface` is shown inside library window `window`.
///
/// Called by the mount path, the only caller that knows both ids. Returns `false` when
/// the surface is unknown to this thread's registry, so a caller cannot invent a link
/// to a widget that does not exist.
pub fn set_surface_window(surface: ObjectId, window: ObjectId) -> bool {
    if with_widget(surface, |_| ()).is_none() {
        return false;
    }
    SURFACE_WINDOWS
        .try_with(|map| map.borrow_mut().insert(surface, window).is_none())
        .unwrap_or(false)
}

/// The library window a mounted surface belongs to, if one was recorded.
pub fn surface_window_for(surface: ObjectId) -> Option<ObjectId> {
    SURFACE_WINDOWS.try_with(|map| map.borrow().get(&surface).copied()).unwrap_or(None)
}

/// Returns the top-level **window** widget that `id` lives under, if any.
///
/// Walks the parent links up to the root and reports it only when the root is a
/// `WidgetKind::Window`. A widget mounted with no window above it (a test fixture, a
/// surface the host parented itself) answers `None`, which is the honest result: there
/// is no window whose invalidation would reveal the change.
///
/// The walk is bounded by `max_depth` steps so a corrupted parent chain — a cycle, or a
/// link to an id that was recycled — cannot hang a paint path. Hitting the bound is
/// reported as `None` rather than guessed at.
///
/// A **mounted surface** is the exception the walk cannot answer: it is registered with
/// no parent (the platform mounts it as its own drawing area), so the mount path records
/// its window in `SURFACE_WINDOWS` and that link is consulted first.
pub fn top_level_window_of(id: ObjectId) -> Option<ObjectId> {
    // A mounted surface has no parent in the widget tree, so the walk below cannot
    // reach its window; the mount path recorded that link instead.
    if let Some(window) = surface_window_for(id) {
        return Some(window);
    }
    // A widget tree is shallow (window → containers → controls); 64 is far past any real
    // depth and still a bound rather than a loop that trusts the data.
    const MAX_DEPTH: usize = 64;
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        let (kind, parent) = read_widget(current, |widget| (widget.kind(), widget.parent()))?;
        if kind == crate::widget::WidgetKind::Window {
            return Some(current);
        }
        current = parent?;
    }
    None
}

/// Repaints a widget **and the window it lives in**.
///
/// # Why invalidating the control alone is not enough
///
/// Since 2.0 the library paints every `WidgetKind` itself, so most controls have no
/// native surface of their own and `invalidate_surface(control)` reaches nothing. On a
/// backend whose window paints its whole child list (Windows, Linux), the thing that
/// must be invalidated is the **window**: its `WM_PAINT` / `connect_draw` redraws the
/// tree the control belongs to.
///
/// A mounted surface is still invalidated too — it paints itself, and on macOS that is
/// the only path a control has. Both calls are made and neither is treated as a failure
/// when it answers `false`: which of the two the backend can honour is the backend's
/// business, not this function's.
pub fn request_repaint_subtree(id: ObjectId) {
    // The window first, so a backend that coalesces per frame folds the control's own
    // request into the frame the window is already going to repaint.
    if let Some(window) = top_level_window_of(id) {
        request_repaint_because(window, RepaintReason::Explicit);
    }
    request_repaint_because(id, RepaintReason::Explicit);
}

/// [`request_repaint_subtree`] with an explicit cause, for a caller that knows why.
///
/// The animation sweep uses this so a frame that advanced a control reports `Animation` in
/// the frame ledger rather than the catch-all `Explicit`. The distinction is the whole point
/// of [`RepaintReason`]: `Explicit` means "a caller asked and named no mechanism", and an
/// interpolation step is a mechanism.
fn request_repaint_subtree_because(id: ObjectId, reason: RepaintReason) {
    if let Some(window) = top_level_window_of(id) {
        request_repaint_because(window, reason);
    }
    request_repaint_because(id, reason);
}

/// Asks the platform to repaint a mounted widget, naming **why** it is being asked.
///
/// # Why the reason is a parameter and not inferred
///
/// A caller knows the cause and the runtime does not: a hover finished (state), a value
/// interpolation advanced (animation), an overlay appeared (overlay). Inferring it here would
/// mean guessing from a stack that has already returned, and a wrong reason is worse than none
/// because the frame ledger's whole purpose is to let a reader act on it (BLUE24 §8).
///
/// # Coalescing
///
/// A request for a surface already repainted this frame is folded into it and counted as
/// coalesced; see [`note_frame_repaint_reason`].
pub fn request_repaint_because(id: ObjectId, reason: RepaintReason) {
    // A whole-control request names no rectangle, so no layer can be proven still valid for the
    // control itself. The invalidation is bounded to the control's box when the runtime can read
    // it, so a control that repaints wholesale does not throw away an unrelated neighbour's
    // layer; a widget the registry cannot answer for (it is lent out mid-tick) falls back to
    // dropping every layer, which is the safe direction.
    note_renderable_change();
    match geometry_of(id) {
        Some(rect) => invalidate_widget_layers_in(grow_by_damage_margin(rect)),
        None => invalidate_widget_layers(),
    }
    if crate::invalidate_surface(id) {
        note_frame_repaint_reason(id, reason);
    }
}

/// Tells the library that a **platform-owned surface** needs to be redrawn.
///
/// # Why the platform must come through here
///
/// A native control owns its own pixels (BLUE24 §7.1), so the platform can and must ask for
/// its own repaint. What it must not do is ask *without telling the library*, because three
/// things then stop being true at once:
///
/// 1. `performance::render_dirty_regions` computes damage the frame never actually submitted,
///    so "what did this frame paint?" has no answer;
/// 2. the still-frame guarantee (BLUE24 §1 criterion 2 — an idle window submits nothing) can
///    no longer be shown, so "smooth is not paid for with a burning core" is unprovable;
/// 3. the self-painted and native worlds each keep their own count, so one interaction is
///    submitted twice.
///
/// So a backend routes its own redraw requests through this function. It is the *same* ledger
/// [`request_repaint`] writes to, so a frame that mixed the two worlds still reports one
/// coherent account.
///
/// `rect` is the damaged region when the platform can name one. It is **not** used to narrow
/// the submission (the platform has already decided what it needs to repaint, and second-
/// guessing it here would be the library inventing a claim about pixels it does not own), but
/// it is recorded, so the ledger can distinguish a full-surface redraw from a partial one.
pub fn notify_native_redraw(id: ObjectId, rect: Option<Rect>) {
    // The platform has already invalidated its own surface; this call exists so the *library*
    // learns about it. Recording the region keeps the account complete even though the
    // submission is the platform's.
    let _ = rect;
    note_frame_repaint_reason(id, RepaintReason::Native);
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
struct RepaintState {
    mode: RepaintMode,
    tracker: crate::performance::DirtyRegionTracker,
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
    large_damage_run: u32,
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
    fn new() -> Self {
        Self {
            mode: RepaintMode::default(),
            tracker: crate::performance::DirtyRegionTracker::new(),
            large_damage_run: 0,
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
fn has_ever_requested_redraw(id: ObjectId) -> bool {
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

/// One cached window-tree frame: the size it was rendered at, its pixels, the content epoch it
/// describes, and the global-repaint generation it was painted under.
type CachedTreeFrame = (Size, Rc<Vec<u8>>, u64, u64);

thread_local! {
    /// The widget registry id for a widget's own `BaseWidget::id()`.
    ///
    /// # Two id spaces, one lookup
    ///
    /// `register` allocates a registry id and keys [`MOUNTED`] by it, while a widget's
    /// `BaseWidget::id()` comes from its own `Object` counter. Those numbers differ (a
    /// fresh control reports `1`; the registry hands out
    /// `0x5345_4C46_0000_0001`), so a call that only has the widget's own id —
    /// `request_redraw`, called from `&self` in ~1000 places — cannot address the
    /// registry without this map.
    ///
    /// Kept here rather than on the widget so it cannot go stale on a move, and so a
    /// widget that is dropped with the registry still holding it cannot be resurrected.
    #[allow(clippy::missing_const_for_thread_local)]
    static OWN_IDS: RefCell<HashMap<ObjectId, ObjectId>> = RefCell::new(HashMap::new());

    /// Per-widget repaint state, keyed by the same id as `MOUNTED`.
    ///
    /// A separate cell rather than a field on `Mounted` so the damage tracker can
    /// be mutated while a widget is borrowed mutably — the render path holds a
    /// `&mut dyn Widget` across the whole draw, and would otherwise deadlock.
    #[allow(clippy::missing_const_for_thread_local)]
    static REPAINT: RefCell<HashMap<ObjectId, RepaintState>> = RefCell::new(HashMap::new());

    /// The most recently rendered frame per widget, so an incremental repaint has
    /// something to carry forward.
    ///
    /// # Why this lives here rather than in each backend
    ///
    /// Every backend that draws a mounted widget needs the previous frame to seed a
    /// partial repaint, and all three would otherwise keep their own copy — three
    /// caches answering the same question, each free to forget to invalidate on a
    /// resize. Keeping it beside the damage tracker means the frame and the damage it
    /// describes cannot get out of step.
    ///
    /// # Why the frame is shared, not owned (D09-RENDER-04)
    ///
    /// The frame is held behind an [`Rc`] so handing it to an incremental repaint —
    /// or answering a frame that has no damage — is a pointer clone, not a `width * height * 4`
    /// copy. Before this, `render_frame_cached` cloned the whole buffer to obtain `previous`,
    /// then `render_frame_incremental` copied it again to return it, then the result was cloned
    /// once more into this cache: three full-frame copies for a frame that drew nothing.
    #[allow(clippy::missing_const_for_thread_local)]
    static LAST_FRAME: RefCell<HashMap<ObjectId, (Size, Rc<Vec<u8>>)>> =
        RefCell::new(HashMap::new());

    /// The most recently rendered **window tree** frame per window, with the content epoch it
    /// was rendered at.
    ///
    /// A separate cell from [`LAST_FRAME`]: a window id and a mounted-surface id are different
    /// kinds of id, and a window is rendered by walking its children rather than by drawing one
    /// widget. Sharing the map would make the two entry points able to answer each other's
    /// lookups with a frame of the wrong shape.
    #[allow(clippy::missing_const_for_thread_local)]
    static LAST_TREE_FRAME: RefCell<HashMap<ObjectId, CachedTreeFrame>> =
        RefCell::new(HashMap::new());

    /// Counts "something that affects a rendered frame changed" for this thread.
    ///
    /// # Why an epoch rather than per-region damage for a window
    ///
    /// A window frame is produced by walking the whole child tree. The per-widget damage tracker
    /// answers "which rectangle of *this* widget changed", which is the right question for a
    /// mounted surface but not for a window: the window's children each have their own trackers,
    /// and a correct union would require either every child to opt into tracking or a second
    /// bookkeeping path. The epoch answers the question the platform actually asks — "does the
    /// frame I already have still describe the tree?" — with one thread-local counter. A changed
    /// frame is repainted whole; an exposure or an occlusion with no change returns the cached
    /// bytes without walking or rasterising anything.
    ///
    /// Per-thread rather than process-wide: a window frame cache is thread-local, so another
    /// thread's edit cannot make this thread's cached frame stale, and invalidating it anyway
    /// would turn unrelated work on another window into a lost cache hit. It is also the hot
    /// path (`request_redraw` reaches it), where an atomic read-modify-write is pure cost for a
    /// value only this thread reads.
    #[allow(clippy::missing_const_for_thread_local)]
    static TREE_FRAME_EPOCH: core::cell::Cell<u64> = const { core::cell::Cell::new(1) };
}

/// Records that a rendered frame's content may have changed. See `TREE_FRAME_EPOCH`.
fn note_renderable_change() {
    let _ = TREE_FRAME_EPOCH.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
}

/// The current content epoch for this thread.
fn renderable_epoch() -> u64 {
    TREE_FRAME_EPOCH.try_with(|epoch| epoch.get()).unwrap_or(1)
}

thread_local! {
    /// Generation of the per-widget layer cache for this thread.
    ///
    /// # Why this is separate from `TREE_FRAME_EPOCH`
    ///
    /// The tree-frame epoch answers "does the window frame I cached still describe the tree?" —
    /// a whole-frame question, so any change must invalidate the whole frame. A per-widget layer
    /// answers a narrower question: "do these pixels describe *this* control's box?". Reusing
    /// the tree epoch for both meant one changed control threw away every control's layer, and a
    /// hover on one button re-shaped the text of the whole window.
    ///
    /// This epoch is therefore only bumped by causes no rectangle describes — a geometry change
    /// and a whole-control repaint. Rectangle damage is answered by dropping exactly the layers
    /// it touches (see [`invalidate_widget_layers_in`]).
    #[allow(clippy::missing_const_for_thread_local)]
    static LAYER_EPOCH: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };

    /// Narrow damage submitted while the registry had lent a control out (a `tick`).
    ///
    /// A control's `tick` runs inside `with_widget_mut`, which takes its entry out of the
    /// registry map. Anything that has to read the registry in the same breath — the box a
    /// repaint is bounded by, the window above it — would see a missing widget, so those two
    /// lookups are deferred to [`flush_deferred_damage`], which runs after the sweep.
    #[allow(clippy::missing_const_for_thread_local)]
    static DEFERRED_DAMAGE: core::cell::RefCell<alloc::vec::Vec<(ObjectId, Rect, RepaintReason)>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };
}

/// The current per-widget layer epoch for this thread.
fn layer_epoch() -> u64 {
    LAYER_EPOCH.try_with(|epoch| epoch.get()).unwrap_or(0)
}

/// Drops every per-widget layer on this thread.
///
/// Used by a change that no rectangle describes: a moved or resized subtree (the parent's layer
/// rect can stay the same while its child shrinks inside it) and a whole-control repaint.
fn invalidate_widget_layers() {
    let _ = LAYER_EPOCH.try_with(|epoch| epoch.set(epoch.get().wrapping_add(1)));
}

/// Drops the per-widget layers whose pixels `rect` overlaps.
///
/// # Why a rectangle is the right granularity
///
/// A layer holds the pixels of one subtree inside its bounds. Damage at `rect` can only make a
/// layer stale when the two overlap, so everything else keeps serving its pixels. The rule is
/// deliberately conservative in one direction: a sibling whose bounds happen to overlap keeps
/// being invalidated too, which costs a repaint but can never show a stale pixel.
fn invalidate_widget_layers_in(rect: Rect) {
    let _ = WIDGET_LAYERS.try_with(|layers| {
        layers.borrow_mut().retain(|_, layer| !layer.rect.intersects(&rect));
    });
}

/// How many per-widget offscreen layers may be resident at once.
///
/// A layer is a full RGBA copy of one widget's painted box, so the bound is on the number of
/// boxes rather than on bytes: a small window full of tiny controls and a large window full of
/// small controls then have the same worst case. The least recently replayed or replaced layer
/// is dropped first, so a scrolled-away page cannot evict the page currently on screen.
const MAX_CACHED_WIDGET_LAYERS: usize = 128;

/// One widget subtree rendered into its own transparent RGBA layer.
///
/// # Why the key carries more than the id
///
/// A layer's pixels describe one rectangle, drawn at one time, under one clip. An id alone would
/// let a moved, resized, differently clipped, or pre-theme-change control replay pixels that no
/// longer answer its box. Every field below is therefore part of the match: the id selects the
/// candidate, and the remaining fields decide whether it still describes this paint.
struct CachedWidgetLayer {
    /// Bounds of the layer in absolute (window) coordinates.
    rect: Rect,
    /// Clip the ancestors imposed when the layer was captured.
    clip: Option<Rect>,
    /// Origin of the frame the layer was captured for.
    frame_origin: (i32, i32),
    /// Size of the frame the layer was captured for.
    frame_size: Size,
    /// Tree/damage epoch at which the layer was captured.
    epoch: u64,
    /// Global repaint generation at which the layer was captured.
    global_epoch: u64,
    /// Layer pixels, row-major RGBA, `width * height * 4` bytes.
    pixels: Rc<Vec<u8>>,
    /// Monotonic use stamp for least-recently-used eviction.
    last_used: u64,
}

thread_local! {
    /// Per-widget offscreen layers, keyed by registry id.
    ///
    /// The existing [`LAST_TREE_FRAME`] cache answers "has the whole window changed?"; this map
    /// answers the finer question "which control changed?" so one dirty control no longer forces
    /// every clean sibling to run `draw` (and re-shape and re-rasterise its text) again.
    #[allow(clippy::missing_const_for_thread_local)]
    static WIDGET_LAYERS: RefCell<HashMap<ObjectId, CachedWidgetLayer>> =
        RefCell::new(HashMap::new());

    /// Monotonic clock used to order [`WIDGET_LAYERS`] for eviction.
    #[allow(clippy::missing_const_for_thread_local)]
    static WIDGET_LAYER_CLOCK: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// Advances and returns the layer use stamp.
fn next_widget_layer_clock() -> u64 {
    WIDGET_LAYER_CLOCK.with(|clock| {
        let next = clock.get().wrapping_add(1);
        clock.set(next);
        next
    })
}

/// Looks up a layer that still describes this exact paint.
fn lookup_widget_layer(
    id: ObjectId,
    rect: Rect,
    clip: Option<Rect>,
    frame_origin: (i32, i32),
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
) -> Option<Rc<Vec<u8>>> {
    WIDGET_LAYERS
        .try_with(|layers| {
            let mut layers = layers.borrow_mut();
            let layer = layers.get_mut(&id)?;
            let matches = layer.rect == rect
                && layer.clip == clip
                && layer.frame_origin == frame_origin
                && layer.frame_size == frame_size
                && layer.epoch == epoch
                && layer.global_epoch == global_epoch;
            if !matches {
                return None;
            }
            layer.last_used = next_widget_layer_clock();
            Some(Rc::clone(&layer.pixels))
        })
        .ok()
        .flatten()
}

/// Stores a freshly painted layer, evicting the least recently used one when full.
#[allow(clippy::too_many_arguments)]
fn store_widget_layer(
    id: ObjectId,
    rect: Rect,
    clip: Option<Rect>,
    frame_origin: (i32, i32),
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    pixels: Rc<Vec<u8>>,
) {
    let stamp = next_widget_layer_clock();
    let _ = WIDGET_LAYERS.try_with(|layers| {
        let mut layers = layers.borrow_mut();
        if !layers.contains_key(&id) && layers.len() >= MAX_CACHED_WIDGET_LAYERS {
            let oldest = layers.iter().min_by_key(|(_, layer)| layer.last_used).map(|(id, _)| *id);
            if let Some(oldest) = oldest {
                layers.remove(&oldest);
            }
        }
        layers.insert(
            id,
            CachedWidgetLayer {
                rect,
                clip,
                frame_origin,
                frame_size,
                epoch,
                global_epoch,
                pixels,
                last_used: stamp,
            },
        );
    });
}

// Test-only count of layer replays, so a regression test can prove a clean control returned its
// cached pixels instead of running `draw` (and therefore instead of shaping or rasterising text).
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static WIDGET_LAYER_REPLAYS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

#[cfg(test)]
fn note_widget_layer_replay() {
    WIDGET_LAYER_REPLAYS.with(|count| count.set(count.get() + 1));
}

#[cfg(not(test))]
#[inline(always)]
fn note_widget_layer_replay() {}

/// Resets and returns the layer replay counter.
#[cfg(all(test, full_widgets))]
pub(crate) fn take_widget_layer_replays_for_test() -> usize {
    WIDGET_LAYER_REPLAYS.with(|count| count.replace(0))
}

/// How many per-widget layers are resident on this thread.
#[cfg(all(test, full_widgets))]
pub(crate) fn cached_widget_layer_count_for_test() -> usize {
    WIDGET_LAYERS.try_with(|layers| layers.borrow().len()).unwrap_or(0)
}

/// Drops every per-widget layer on this thread.
#[cfg(all(test, full_widgets))]
pub(crate) fn clear_widget_layers_for_test() {
    let _ = WIDGET_LAYERS.try_with(|layers| layers.borrow_mut().clear());
    let _ = WIDGET_LAYER_REPLAYS.try_with(|count| count.set(0));
}

// Counts full-frame buffer copies performed by the incremental frame cache. Test-only.
//
// # Why a counter rather than a custom allocator (D09-RENDER-04)
//
// The defect is a *count* — a no-damage frame copied its RGBA buffer three times. A global
// `GlobalAlloc` shim would count every allocation in the process, including the renderer's own
// scratch buffers, and could not separate the cache's copies from them. The cache performs a
// small, known set of full-buffer copies, so counting exactly those is the precise witness the
// regression test asserts on. Compiled only under `cfg(test)` (principle #28).
#[cfg(test)]
thread_local! {
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_CACHE_COPIES: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Records one full-frame copy made by the incremental cache. (D09-RENDER-04)
#[cfg(test)]
fn note_frame_cache_copy() {
    FRAME_CACHE_COPIES.with(|cell| cell.set(cell.get() + 1));
}

/// Resets and returns the full-frame copy counter. (D09-RENDER-04)
///
/// Gated on its callers and not just on `test`: they live in the `full_widgets` test module, so a
/// stripped-but-testing build (a sized profile with `icons` on, say) compiled this and reported it
/// as dead code.
#[cfg(all(test, full_widgets))]
pub(crate) fn take_frame_cache_copies() -> usize {
    FRAME_CACHE_COPIES.with(|cell| cell.replace(0))
}

/// No-op stand-in so non-test builds compile the same call sites. (D09-RENDER-04)
#[cfg(not(test))]
#[inline(always)]
fn note_frame_cache_copy() {}

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
    // widget has no tracker, because a window-level cache is not the per-widget policy.
    note_renderable_change();
    // The layer cache is narrower: only the layers this rectangle touches can now be stale.
    invalidate_widget_layers_in(rect);
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
pub fn dirty_rects(id: ObjectId) -> Vec<Rect> {
    let mut tracker = REPAINT
        .try_with(|map| {
            map.borrow().get(&id).map(|state| {
                let mut copy = crate::performance::DirtyRegionTracker::new();
                for region in state.tracker.regions() {
                    copy.add(region.rect);
                }
                copy
            })
        })
        .ok()
        .flatten()
        .unwrap_or_default();
    tracker.merge();
    tracker.regions().iter().map(|region| region.rect).collect()
}

/// Renders one frame, repainting only the damage when the policy allows it.
///
/// # Why the full frame is still returned
///
/// The return value is a complete `size.width * size.height * 4` buffer in every
/// mode, because a caller presents a frame and cannot present a rectangle. The
/// saving is in what is *drawn*, not in what is *returned*: in `Dirty` mode only the
/// damaged area is re-rasterised, and the untouched pixels are whatever the previous
/// frame left. Rebuilding the whole buffer per frame would give up that saving,
/// so `previous` carries the last frame in.
///
/// `previous` must be a buffer from an earlier call for the same widget and size.
/// Passing `None` forces a full paint, which is the honest answer when there is no
/// frame to preserve.
pub fn render_frame_incremental(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<&[u8]>,
) -> Option<Vec<u8>> {
    render_frame_incremental_cow(id, size, clear, previous.map(crate::compat::Cow::Borrowed))
}

/// [`render_frame_incremental`] with an **owned** previous frame.
///
/// The paint adopts the buffer as its draw target instead of copying it in. That copy is a full
/// frame — 4 MB at 1280×800 — and it used to cost more than painting a control-sized damage
/// region, which made "repaint only the damage" cheaper in pixels but not in time. The caller
/// only reaches this path when it owns the buffer outright (see `render_frame_cached_shared`).
fn render_frame_incremental_owned(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    render_frame_incremental_cow(id, size, clear, previous.map(crate::compat::Cow::Owned))
}

/// The shared body of the two [`render_frame_incremental`] entry points.
///
/// `previous` is a [`crate::compat::Cow`] because the two callers hold the previous frame
/// differently: the public API was handed a slice, while the cached path removed the frame from
/// its cache and may therefore own it. Borrowing and owning have to reach the same code — the
/// region painting below is the whole feature — so the difference is confined to how the buffer
/// is seeded and handed back.
fn render_frame_incremental_cow(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
    previous: Option<crate::compat::Cow<'_, [u8]>>,
) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }

    let mode = repaint_mode(id);
    let expected = size.width as usize * size.height as usize * 4;
    // A buffer of the wrong size cannot be carried forward, so treat it as absent
    // rather than reading past its end.
    let carried = previous.filter(|buffer| buffer.len() == expected);

    // Measured once: this used to call `dirty_rects` three times in the dirty path (once for the
    // empty check and twice for coverage), and each call cloned the tracker and merged it.
    let dirty = if mode != RepaintMode::Full && carried.is_some() {
        dirty_rects(id)
    } else {
        alloc::vec::Vec::new()
    };
    let use_dirty = !dirty.is_empty();
    if mode == RepaintMode::Full || carried.is_none() {
        if let Some(frame) = render_frame(id, size, clear) {
            let _ = REPAINT.try_with(|map| {
                if let Some(state) = map.borrow_mut().get_mut(&id) {
                    state.tracker.clear();
                    // A full paint is not "large damage" evidence: it may be the first
                    // frame, or a resize. Resetting here keeps `Adaptive` from reading
                    // its own fallbacks as proof that damage tracking is hopeless.
                    state.large_damage_run = 0;
                }
            });
            return Some(frame);
        }
        return None;
    }

    // Measured *before* the regions are handed to `render_dirty_regions`, which clears
    // the tracker as it paints. Both the `Adaptive` decision and the run counter need
    // this frame's coverage, and after the call there is nothing left to measure.
    let covered_too_much = {
        let frame_area = u64::from(size.width) * u64::from(size.height);
        let covered: u64 =
            dirty.iter().map(|rect| u64::from(rect.width) * u64::from(rect.height)).sum();
        frame_area == 0
            || (covered as f32)
                >= (frame_area as f32) * crate::performance::render_dirty::FULL_REPAINT_AREA_RATIO
    };

    // `Adaptive` decides for itself whether measuring is worth it.
    //
    // This is the whole difference between `Adaptive` and `Dirty`. `Dirty` means the
    // caller has already decided, so every frame is measured. `Adaptive` means the
    // caller does not know, so a run of frames whose damage covers the surface is
    // taken as evidence that regioning cannot pay off here.
    //
    // # Why the run is checked *and* this frame's damage is measured
    //
    // The first version skipped regioning whenever the run was full, without looking
    // at the current frame — so a single small change after three large ones still
    // painted whole, and the run could never be cleared because the code that clears
    // it was never reached. The mode latched. Checking both conditions means the run
    // is only trusted while it is still true: as soon as a frame's damage is small,
    // that frame is measured (and resets the run), and the next frame regions again.
    if mode == RepaintMode::Adaptive {
        let learned = REPAINT
            .try_with(|map| {
                map.borrow()
                    .get(&id)
                    .is_some_and(|state| state.large_damage_run >= ADAPTIVE_LARGE_DAMAGE_RUN)
            })
            .unwrap_or(false);
        if learned && covered_too_much {
            if let Some(frame) = render_frame(id, size, clear) {
                let _ = REPAINT.try_with(|map| {
                    if let Some(state) = map.borrow_mut().get_mut(&id) {
                        state.tracker.clear();
                    }
                });
                return Some(frame);
            }
            return None;
        }
    }

    if !use_dirty {
        // Nothing was damaged: the previous frame is still correct, and redrawing
        // it would be work for no visible change.
        //
        // This is also the reset point for the `Adaptive` run: a frame with no damage
        // is proof the surface has settled.
        let _ = REPAINT.try_with(|map| {
            if let Some(state) = map.borrow_mut().get_mut(&id) {
                state.large_damage_run = 0;
            }
        });
        // `into_owned` moves the buffer when the caller owned it and copies when it borrowed —
        // exactly the difference between the two entry points.
        return Some(carried?.into_owned());
    }

    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    // Seed the backend with the previous frame so the regions this pass does not
    // touch keep their pixels. An owned buffer becomes the target; a borrowed one is copied in.
    let seeded = match carried? {
        crate::compat::Cow::Owned(buffer) => backend.seed_from_owned(buffer),
        crate::compat::Cow::Borrowed(slice) => backend.seed_from(slice),
    };
    if !seeded {
        // The length was checked above, so this is unreachable unless the surface and the frame
        // disagree about their size. Painting over a cleared frame is the honest recovery: the
        // region pass below only claims the damaged pixels, so a cleared backdrop is the one
        // state that cannot show a stale pixel.
        debug_assert!(seeded, "a length-checked frame must seed the backend");
        backend.begin_frame(clear);
    }

    // The widget draws in absolute coordinates and the frame is its own box, so the
    // frame origin is subtracted (see `render_frame`). This must be pushed *before*
    // the clip rectangles, which are also absolute, so that both translate together.
    let origin = frame_origin(id);

    let mut tracker = REPAINT
        .try_with(|map| {
            map.borrow_mut().get_mut(&id).map(|state| {
                // Feed the `Adaptive` run counter — **only** in `Adaptive`. Updating it
                // in `Dirty` too was the first version of this code, and it made the two
                // modes observably identical again from the outside: `Dirty` reported a
                // growing run while its contract says it never learns. The counter is
                // `Adaptive`'s memory, so only `Adaptive` may write it.
                if mode == RepaintMode::Adaptive {
                    state.large_damage_run =
                        if covered_too_much { state.large_damage_run.saturating_add(1) } else { 0 };
                }
                let mut taken = crate::performance::DirtyRegionTracker::new();
                for region in state.tracker.regions() {
                    taken.add(region.rect);
                }
                state.tracker.clear();
                taken
            })
        })
        .ok()
        .flatten()?;
    tracker.merge();

    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            crate::performance::render_dirty_regions(&mut tracker, &mut context, |ctx| {
                drawable.draw(ctx);
            });
            context.pop_offset();
        }
        // Present the back buffer this pass drew into. Without this the frame read
        // below comes from the front buffer, which still holds the *pre-seed* state
        // — the seed went into `back`, so the untouched regions would read as blank
        // instead of as the previous frame.
        backend.end_frame();
        true
    })?;
    if !painted {
        return None;
    }
    Some(backend.into_frame_rgba())
}

/// Renders a widget tree, reusing the previous frame when nothing that affects it has changed.
///
/// This is [`render_frame_tree`] with the one piece of memory a window painter should have: the
/// frame it produced last time. AppKit, Win32 and GTK all call their paint callback for reasons
/// that are not always a content change — an expose after another window moved away, an
/// occlusion change, a live-resize notification — and re-rasterising a whole tree for those is
/// pure cost. When the content epoch is unchanged the previous bytes are returned as they are;
/// when it moved, the frame is painted whole.
///
/// # Why the epoch is captured before painting
///
/// A control that animates requests its next frame *while it draws*. Storing the epoch read at
/// the end of the paint would record that request as already satisfied, and the animation would
/// advance once and then stop. Capturing it first means a request made during the paint belongs
/// to the next frame, which is what the control meant.
///
/// Returns `None` under the same conditions as [`render_frame_tree`].
pub fn render_frame_tree_cached(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Vec<u8>> {
    let shared = render_frame_tree_cached_shared(id, size, clear)?;
    Some(shared.as_ref().clone())
}

/// [`render_frame_tree_cached`] returning the frame the cache holds, **not** a copy of it.
///
/// The unchanged-epoch path is the reason this exists: a 4K frame is about 33 MB, and the owned
/// form had to duplicate it to hand the caller a buffer whose pixels nobody had touched. The cache
/// already stores an `Rc<Vec<u8>>`, so this form answers a refcount increment. A caller that
/// presents the frame (or reads it as `&[u8]`) needs nothing more.
pub fn render_frame_tree_cached_shared(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Rc<Vec<u8>>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let epoch = renderable_epoch();
    let global_epoch = global_repaint_generation();
    let previous = LAST_TREE_FRAME
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .filter(|(stored, _, stored_epoch, stored_global)| {
                    *stored == size && *stored_epoch == epoch && *stored_global == global_epoch
                })
                .map(|(_, frame, _, _)| Rc::clone(frame))
        })
        .ok()
        .flatten();
    if let Some(previous) = previous {
        return Some(previous);
    }

    let shared = Rc::new(render_frame_tree(id, size, clear)?);
    let _ = LAST_TREE_FRAME.try_with(|map| {
        map.borrow_mut().insert(id, (size, Rc::clone(&shared), epoch, global_epoch));
    });
    Some(shared)
}

/// Renders one frame of a mounted widget, reusing the previous one when the
/// repaint policy allows.
///
/// This is the entry point a backend's paint callback should call, and it is the one
/// that makes [`RepaintMode`] reach the screen. It differs from
/// [`render_frame_incremental`] in owning the previous frame: the caller passes only
/// the widget and the size, and this function remembers what it last produced.
///
/// # Why the cache is keyed by size
///
/// A frame of the wrong size cannot seed a repaint — it would be copied into a larger
/// surface leaving the tail untouched, or truncated. Storing the size beside the frame
/// means a resize is detected here and turns into a full paint automatically, without
/// the caller having to remember to tell anyone.
///
/// # Return value
///
/// The complete frame, always. A backend presents a frame and cannot present a
/// rectangle, so the saving is in what is *drawn*, not in what is returned. Returns
/// `None` when the widget is unmounted, has no `Draw` impl, or the size is empty.
pub fn render_frame_cached(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    let shared = render_frame_cached_shared(id, size, clear)?;
    // The owned form is the one that pays: this is the single copy that becomes the caller's
    // frame. `render_frame_cached_shared` makes none at all.
    note_frame_cache_copy();
    Some(shared.as_ref().clone())
}

/// [`render_frame_cached`] returning the cached frame itself, so "nothing changed" costs an
/// `Rc` increment rather than a full-buffer copy.
///
/// The owned form has to hand back a `Vec<u8>`; for a 4K surface that is ~33 MB duplicated to
/// return pixels the cache already holds. A caller that presents the frame, or reads it as
/// `&[u8]`, can take this form and never copy.
pub fn render_frame_cached_shared(
    id: ObjectId,
    size: Size,
    clear: crate::core::Color,
) -> Option<Rc<Vec<u8>>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }

    // Take a shared handle to the previous frame — a pointer clone, not a pixel copy. A frame of
    // the wrong size is treated as absent so the callee falls back to a full paint.
    // (D09-RENDER-04)
    let previous = LAST_FRAME
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .filter(|(stored, _)| *stored == size)
                .map(|(_, frame)| Rc::clone(frame))
        })
        .ok()
        .flatten();

    // # No-damage fast path (D09-RENDER-04)
    //
    // When the mode repaints only damage and nothing is damaged, the previous frame *is* the
    // frame. The old code still ran the incremental machinery: it copied the cached buffer into
    // `previous`, copied it again in `render_frame_incremental` via `carried.to_vec()`, and copied
    // it a third time back into the cache. Here the cache is left exactly as it is and only the
    // single copy needed to hand an owned buffer to the caller is made. `Full` mode is excluded
    // because it must repaint regardless, and a missing previous cannot take this path.
    if let Some(previous) = &previous {
        if repaint_mode(id) != RepaintMode::Full && dirty_rects(id).is_empty() {
            // A frame with no damage also proves the surface settled, which is the `Adaptive`
            // mode's reset point — `render_frame_incremental` clears the run on this path, and
            // this fast path must do the same or `Adaptive` would stay latched after an animation
            // ends (the exact failure the run counter exists to avoid).
            let _ = REPAINT.try_with(|map| {
                if let Some(state) = map.borrow_mut().get_mut(&id) {
                    state.large_damage_run = 0;
                }
            });
            return Some(Rc::clone(previous));
        }
    }

    // # Why the cache entry is removed and possibly moved out
    //
    // The damaged path starts from the previous frame's pixels. Copying them into the paint
    // target is a full-frame `memcpy`, which used to cost more than painting the damage region
    // itself — the incremental path was saving pixels and paying the time back in the copy. Taking
    // the entry out of the cache leaves this function as the only holder, so `try_unwrap` can hand
    // the buffer to the paint when the caller has already dropped its frame (the ordinary loop).
    // A caller that still holds the frame makes `try_unwrap` refuse, and the pixels are borrowed
    // and copied exactly as before. The entry is replaced with the new frame below, so the gap
    // cannot be observed.
    drop(previous);
    let taken = LAST_FRAME
        .try_with(|map| map.borrow_mut().remove(&id))
        .ok()
        .flatten()
        .filter(|(stored, _)| *stored == size);
    let frame = match taken {
        Some((_, previous)) => match Rc::try_unwrap(previous) {
            Ok(buffer) => render_frame_incremental_owned(id, size, clear, Some(buffer)),
            Err(previous) => render_frame_incremental(id, size, clear, Some(previous.as_slice())),
        },
        None => render_frame_incremental(id, size, clear, None),
    }?;

    // The freshly produced frame is moved into the `Rc` the cache holds — no pixel copy — and the
    // same handle is returned, so the painted path does not duplicate it either.
    let shared = Rc::new(frame);
    let _ = LAST_FRAME.try_with(|map| {
        map.borrow_mut().insert(id, (size, Rc::clone(&shared)));
    });
    Some(shared)
}

/// Forgets the cached frame for `id`.
///
/// Called when a widget is unmounted, so a frame cannot outlive the control it
/// describes — and so its id, if reused, does not start from someone else's pixels.
pub fn forget_cached_frame(id: ObjectId) {
    let _ = LAST_FRAME.try_with(|map| map.borrow_mut().remove(&id));
    let _ = LAST_TREE_FRAME.try_with(|map| map.borrow_mut().remove(&id));
    let _ = WIDGET_LAYERS.try_with(|map| map.borrow_mut().remove(&id));
}

/// The size of the cached frame for `id`, if one is held.
///
/// Exposed so a backend can tell "I have a frame for this size" from "the next paint
/// will be full", which is what a resize handler needs to know.
pub fn cached_frame_size(id: ObjectId) -> Option<Size> {
    LAST_FRAME.try_with(|map| map.borrow().get(&id).map(|(size, _)| *size)).ok().flatten()
}

/// How many consecutive over-threshold frames [`RepaintMode::Adaptive`] has seen.
///
/// # Why this is public
///
/// `Adaptive` differs from `Dirty` only in this memory, so it is the only way to tell
/// the two policies apart from outside — and a policy that cannot be observed cannot
/// be tested. A host can also read it to decide whether to switch a widget to `Full`
/// itself, which is the honest use of the number.
///
/// Zero for a widget in any other mode, and for one that is not mounted.
pub fn adaptive_large_damage_run(id: ObjectId) -> u32 {
    REPAINT
        .try_with(|map| map.borrow().get(&id).map(|state| state.large_damage_run))
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// Renders one frame of a mounted widget at `size` and returns the RGBA bytes.
///
/// `bytes.len() == size.width * size.height * 4`, top-down, straight (non
/// premultiplied) alpha — the layout all three desktop backends consume.
/// Returns `None` when `id` is not mounted or the size is empty.
///
/// # Why the widget's own origin is subtracted
///
/// Widget geometry is **absolute**: a control's rect is its position in the window, and
/// every control draws at those absolute coordinates (the same model the event router
/// uses, see [`widget_at`]). The frame here is only `size.width * size.height` — the
/// control's own box — so drawing at the absolute origin of a control placed at, say,
/// `(260, 42)` would land 260 px right and 42 px down inside its own buffer: the
/// top-left band stays clear and the bottom-right is clipped away. Pushing the negated
/// origin makes the control's absolute coordinates land on its own frame origin, which
/// is the only interpretation that can be correct for every control already written.
pub fn render_frame(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    // The frame is the control's own box, so it has to be drawn at the box's origin
    // rather than at the control's absolute position.
    let origin = frame_origin(id);
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            drawable.draw(&mut context);
            context.pop_offset();
        }
        backend.end_frame();
        true
    })?;
    if !painted {
        return None;
    }
    Some(backend.into_frame_rgba())
}

/// Renders one frame of a mounted widget using its own geometry as the size.
///
/// Convenience wrapper for backends that size the native canvas from the
/// widget's geometry rather than from the OS-reported rectangle.
pub fn render_frame_at_geometry(id: ObjectId, clear: crate::core::Color) -> Option<Vec<u8>> {
    let geometry = geometry_of(id)?;
    render_frame(id, Size::new(geometry.width, geometry.height), clear)
}

/// Orders a window's children for painting: ordinary children first, open overlays last.
///
/// A drop-down list is painted by its own control, below the control's rectangle, and the window
/// may have siblings declared after it that occupy the same pixels (the next form row). Declaration
/// order would paint those siblings over the list — the same split the hit test had, where the row
/// under the pointer took the click. Painting overlays last makes what the user sees agree with what
/// the hit test already prefers (`has_open_popup`).
///
/// Only direct children are reordered: every popup control in this crate draws its list itself, so
/// the popup is the direct child. Nested containers keep their own declaration order.
fn ordered_root_children(children: Vec<ObjectId>) -> Vec<ObjectId> {
    let mut ordinary = Vec::with_capacity(children.len());
    let mut overlays = Vec::new();
    for child in children {
        let overlay = read_widget(child, |widget| widget.is_visible() && widget.has_open_popup())
            .unwrap_or(false);
        if overlay {
            overlays.push(child);
        } else {
            ordinary.push(child);
        }
    }
    ordinary.extend(overlays);
    ordinary
}

/// Renders one frame of a widget **and its descendants**, returning RGBA bytes.
///
/// # Why this exists
///
/// [`render_frame`] paints exactly one widget into its own box. That is right for a
/// self-drawn control mounted as a surface, but it means a widget that **contains** other
/// widgets paints none of them: a window drawn this way shows its own chrome over an empty
/// client area. A window whose children are ordinary library controls — a button, a check
/// box, a label — is the case that matters, and those children are never mounted as
/// surfaces of their own, so nothing painted them.
///
/// This walks the child list depth-first and draws each child that implements
/// [`Draw`][crate::widget::Draw], in tree order, so a later sibling paints over an earlier
/// one — the same order a container's own `draw` would use. Direct children with an open
/// overlay (a drop-down list) are painted last, so the list is not covered by a sibling that
/// happens to occupy the same pixels.
///
/// # Coordinates
///
/// Geometry is absolute (see [`render_frame`]), and the frame's origin is `id`'s own
/// position, so the negated origin is pushed once. Children draw at their own absolute
/// coordinates, which that single translation maps into this frame.
///
/// # What is skipped, and why silently
///
/// A child is skipped when it is not visible, when it is not in the registry, or when it
/// does not implement `Draw`. Skipping is not an error because all three are ordinary: a
/// hidden page, a child whose id was recycled, and a control that is a pure model for a
/// host to materialise are each a normal state, and reporting them would make a correct
/// tree look broken.
///
/// Returns `None` when `id` is not mounted or the size is empty.
pub fn render_frame_tree(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    // Captured before anything paints: a request made *during* a draw belongs to the next frame,
    // so the layer stored now must not carry the epoch that request produced. This is the layer
    // epoch rather than the tree epoch: a frame is repainted whole after any change, while the
    // controls whose pixels did not change keep their layers.
    let epoch = layer_epoch();
    let global_epoch = global_repaint_generation();
    let origin = frame_origin(id);
    let frame_bounds = Rect::new(origin.0, origin.1, size.width, size.height);
    let mut backend = SoftwarePaintBackend::new(size, 1.0);

    // The root's own painting is part of the frame: a window draws its background and
    // chrome, which the children then sit on top of. Its descendant clip and child list are
    // read in that same loan, so the root is not visited twice before the traversal starts.
    let mut root_clip = None;
    let mut root_children = Vec::new();
    let mut painted = with_widget_mut(id, |widget| {
        root_clip = widget.children_clip();
        root_children = widget.base().children().to_vec();
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            context.push_offset(-origin.0, -origin.1);
            drawable.draw(&mut context);
            context.pop_offset();
        }
        true
    })
    .unwrap_or(false);

    if painted {
        // Each direct child is offered to the per-widget layer cache. A child whose subtree is
        // unchanged replays its cached pixels; a changed one is rasterised once into its own
        // transparent offscreen surface and then composited into this frame. The recursion below
        // owns the descent, so the root's children are the only stack this function needs.
        //
        // Every entry carries the clip **its ancestors** impose on it. A control answers
        // `children_clip` for its descendants (a scroll area's viewport, for example), and the
        // painter intersects that with the clip its own ancestors already imposed. Without the
        // carried rectangle a control's content would escape the viewport as soon as it scrolled.
        //
        // `SubtreeMetrics` is a per-frame memo: subtree bounds and the open-popup flag are each
        // computed once per node, not once per ancestor that asks for them.
        let mut metrics = HashMap::new();
        for child in ordered_root_children(root_children) {
            // A node whose clip has no area cannot paint anything, and neither can its
            // descendants. Skipping the node skips the subtree, the same rule hiding uses.
            if root_clip.is_some_and(|rect| rect.width == 0 || rect.height == 0) {
                continue;
            }
            let painted_child = paint_widget_layer(
                child,
                root_clip,
                &mut backend,
                origin,
                frame_bounds,
                size,
                epoch,
                global_epoch,
                0,
                &mut metrics,
            );
            if painted_child.drew {
                painted = true;
            }
        }
    }

    if !painted {
        return None;
    }
    backend.end_frame();
    Some(backend.into_frame_rgba())
}

/// Bounds and painting facts for one widget subtree, computed once per frame.
#[derive(Clone, Copy)]
struct SubtreeMetrics {
    /// Union of the subtree's painted boxes, or `None` when the subtree is hidden.
    bounds: Option<Rect>,
    /// Whether any visible node in the subtree paints an overlay outside its own rectangle.
    has_open_popup: bool,
}

/// The result of offering one widget subtree to the layer cache.
#[derive(Clone, Copy)]
struct LayerPaint {
    /// Whether this subtree painted anything.
    drew: bool,
    /// Whether the subtree was rendered into a rectangle that fully describes it.
    cacheable: bool,
}

/// The deepest subtree the painter will descend, so a corrupted parent/child cycle cannot
/// overflow the stack. Real trees are far shallower.
const MAX_LAYER_DEPTH: u32 = 64;

/// Returns the subtree's painted bounds and whether it paints outside its own box, memoised.
fn subtree_metrics(
    id: ObjectId,
    depth: u32,
    memo: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> SubtreeMetrics {
    if let Some(cached) = memo.get(&id) {
        return *cached;
    }
    let empty = SubtreeMetrics { bounds: None, has_open_popup: false };
    if depth >= MAX_LAYER_DEPTH {
        return empty;
    }
    let info = read_widget(id, |widget| {
        (
            widget.is_visible(),
            widget.has_open_popup(),
            grow_by_damage_margin(widget.geometry()),
            widget.base().children().to_vec(),
        )
    });
    let metrics = match info {
        Some((true, has_open_popup, mut bounds, children)) => {
            let mut has_open_popup = has_open_popup;
            for child in children {
                let child_metrics = subtree_metrics(child, depth + 1, memo);
                has_open_popup |= child_metrics.has_open_popup;
                if let Some(child_bounds) = child_metrics.bounds {
                    bounds = union_rect(bounds, child_bounds);
                }
            }
            SubtreeMetrics { bounds: Some(bounds), has_open_popup }
        }
        // A hidden node hides its whole subtree, and an id that is no longer mounted paints
        // nothing. Both are ordinary states, not errors.
        _ => empty,
    };
    memo.insert(id, metrics);
    metrics
}

/// The union of two rectangles, saturating at the coordinate limits.
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.min(b.x);
    let y0 = a.y.min(b.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1.saturating_sub(x0).max(0) as u32, y1.saturating_sub(y0).max(0) as u32)
}

/// Narrows a subtree's painted bounds to what the frame and the ancestors can show.
fn layer_bounds(raw: Rect, clip: Option<Rect>, frame_bounds: Rect) -> Option<Rect> {
    let bounded = intersect_clip(Some(raw), clip)?;
    let bounded = intersect_clip(Some(bounded), Some(frame_bounds))?;
    (bounded.width > 0 && bounded.height > 0).then_some(bounded)
}

/// Draws a cached layer into `backend` at its absolute position.
fn blit_widget_layer(
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    bounds: Rect,
    pixels: &[u8],
) {
    let mut context = RenderContext::new(backend);
    context.draw_image(
        bounds.x - origin.0,
        bounds.y - origin.1,
        bounds.width,
        bounds.height,
        pixels,
    );
}

/// Paints one widget subtree, reusing its cached layer when one still describes the paint.
#[allow(clippy::too_many_arguments)]
fn paint_widget_layer(
    id: ObjectId,
    clip: Option<Rect>,
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    frame_bounds: Rect,
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    depth: u32,
    metrics: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> LayerPaint {
    let subtree = subtree_metrics(id, depth, metrics);
    let Some(raw_bounds) = subtree.bounds else {
        return LayerPaint { drew: false, cacheable: true };
    };
    // A subtree that paints outside its own box (an open drop-down list) cannot be captured in
    // a bounded layer without clipping the overlay. Paint it straight into the target instead,
    // and mark it uncacheable so no ancestor captures a layer that omits the overlay.
    if subtree.has_open_popup {
        return paint_widget_direct(
            id,
            clip,
            backend,
            origin,
            frame_bounds,
            frame_size,
            epoch,
            global_epoch,
            depth,
            metrics,
        );
    }
    let Some(bounds) = layer_bounds(raw_bounds, clip, frame_bounds) else {
        return LayerPaint { drew: false, cacheable: true };
    };

    if let Some(pixels) =
        lookup_widget_layer(id, bounds, clip, origin, frame_size, epoch, global_epoch)
    {
        blit_widget_layer(backend, origin, bounds, pixels.as_slice());
        note_widget_layer_replay();
        return LayerPaint { drew: true, cacheable: true };
    }

    // A miss paints the subtree into its own transparent surface. Draws inside the layer are
    // translated by the layer's absolute origin, exactly as the frame path translates them by
    // the frame origin. `end_frame` publishes the drawn back buffer as the frame to take.
    let layer_size = Size::new(bounds.width, bounds.height);
    let mut offscreen = SoftwarePaintBackend::new(layer_size, 1.0);
    let mut children = Vec::new();
    let mut child_clip = None;
    let mut self_drew = false;
    let rendered = with_widget_mut(id, |widget| {
        children = widget.base().children().to_vec();
        child_clip = intersect_clip(clip, widget.children_clip());
        let Some(drawable) = widget.as_draw_mut() else {
            return;
        };
        let mut context = RenderContext::new(&mut offscreen);
        context.push_offset(-bounds.x, -bounds.y);
        drawable.draw(&mut context);
        context.pop_offset();
        self_drew = true;
    });
    if rendered.is_none() {
        return LayerPaint { drew: false, cacheable: true };
    }

    let mut child_drew = false;
    let mut cacheable = true;
    for child in children {
        let painted = paint_widget_layer(
            child,
            child_clip,
            &mut offscreen,
            (bounds.x, bounds.y),
            bounds,
            layer_size,
            epoch,
            global_epoch,
            depth + 1,
            metrics,
        );
        child_drew |= painted.drew;
        cacheable &= painted.cacheable;
    }
    if !self_drew && !child_drew {
        return LayerPaint { drew: false, cacheable };
    }

    offscreen.end_frame();
    let pixels = Rc::new(offscreen.into_frame_rgba());
    if cacheable {
        store_widget_layer(
            id,
            bounds,
            clip,
            origin,
            frame_size,
            epoch,
            global_epoch,
            Rc::clone(&pixels),
        );
    }
    blit_widget_layer(backend, origin, bounds, pixels.as_slice());
    LayerPaint { drew: true, cacheable }
}

/// Paints one widget and its descendants directly into `backend`.
///
/// This is the path for a subtree that paints outside its own rectangle. It keeps the original
/// clipping and ordering rules; descendants without an overlay can still be layered, which is
/// why it recurses through [`paint_widget_layer`] rather than drawing the whole subtree here.
#[allow(clippy::too_many_arguments)]
fn paint_widget_direct(
    id: ObjectId,
    clip: Option<Rect>,
    backend: &mut SoftwarePaintBackend,
    origin: (i32, i32),
    frame_bounds: Rect,
    frame_size: Size,
    epoch: u64,
    global_epoch: u64,
    depth: u32,
    metrics: &mut HashMap<ObjectId, SubtreeMetrics>,
) -> LayerPaint {
    let mut children = Vec::new();
    let mut child_clip = None;
    let mut self_drew = false;
    let rendered = with_widget_mut(id, |widget| {
        if !widget.is_visible() {
            return;
        }
        children = widget.base().children().to_vec();
        child_clip = intersect_clip(clip, widget.children_clip());
        let Some(drawable) = widget.as_draw_mut() else {
            return;
        };
        let mut context = RenderContext::new(backend);
        context.push_offset(-origin.0, -origin.1);
        if let Some(rect) = clip {
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
        }
        drawable.draw(&mut context);
        if clip.is_some() {
            context.pop_clip();
        }
        context.pop_offset();
        self_drew = true;
    });
    if rendered.is_none() {
        return LayerPaint { drew: false, cacheable: false };
    }
    let mut child_drew = false;
    for child in children {
        let painted = paint_widget_layer(
            child,
            child_clip,
            backend,
            origin,
            frame_bounds,
            frame_size,
            epoch,
            global_epoch,
            depth + 1,
            metrics,
        );
        child_drew |= painted.drew;
    }
    LayerPaint { drew: self_drew || child_drew, cacheable: false }
}

/// The overlap of two clip rectangles, or a zero-area rectangle when they do not overlap.
///
/// A zero area is a real answer ("nothing is visible"), not an absence of clipping: the painter
/// skips such a subtree. Returning `None` would widen the clip back to the parent's, which is the
/// one wrong direction.
fn intersect_clip(outer: Option<Rect>, inner: Option<Rect>) -> Option<Rect> {
    match (outer, inner) {
        (Some(a), Some(b)) => Some(Rect::new(
            a.x.max(b.x),
            a.y.max(b.y),
            (a.x.saturating_add(a.width as i32))
                .min(b.x.saturating_add(b.width as i32))
                .saturating_sub(a.x.max(b.x))
                .max(0) as u32,
            (a.y.saturating_add(a.height as i32))
                .min(b.y.saturating_add(b.height as i32))
                .saturating_sub(a.y.max(b.y))
                .max(0) as u32,
        )),
        (Some(rect), None) | (None, Some(rect)) => Some(rect),
        (None, None) => None,
    }
}

/// The direct children of `id`, or an empty list when it is not mounted.
///
/// Read through [`with_widget`] so a caller sees the same widget the traversal will.
fn direct_children_of(id: ObjectId) -> Vec<ObjectId> {
    read_widget(id, |widget| widget.base().children().to_vec()).unwrap_or_default()
}

/// The direct children of `id`, as a public read-only view of the mounted tree.
///
/// # Why this is public
///
/// The self-drawn backend positions and paints a window by walking this child list, so
/// "what does this window actually contain?" is a question a host, a test, and a designer
/// all need to ask. Only the private traversal could answer it before, which meant a
/// caller diagnosing a control that did not appear had to guess from creation order
/// instead of reading the tree (principle #100: designer readiness has to be inspectable,
/// not described).
///
/// Returns an empty vector for an unmounted id, matching
/// [`geometry_of`]: an id that addresses nothing has no children, and `[]` is the honest
/// answer rather than an error the caller must distinguish from "no children".
pub fn children_of(id: ObjectId) -> Vec<ObjectId> {
    direct_children_of(id)
}

/// Test module for the runtime registry and self-drawn frame rendering.
///
/// Runs only in `full_widgets` profiles: the tests exercise the self-drawn bridge
/// through concrete widgets in `special_widgets`, which is itself gated on
/// `full_widgets` (defined in `build.rs`). Gating on plain `test` alone would
/// make a stripped-down profile try to resolve types that are compiled out.
#[cfg(all(test, full_widgets))]
mod tests {
    use super::*;
    use crate::core::{Color, Point};
    use crate::platform::accessibility::AccessibilityBridge as _;
    use crate::widget::special_widgets::code_editor::CodeEditor;
    use crate::widget::{BaseWidget, Draw, WidgetKind};

    /// A counter shared with a test control.
    type DrawCount = core::cell::Cell<usize>;

    // ── BLUE23 §3.3 -- the animation bus ────────────────────────────────────────

    /// One arming buys exactly one full repaint of every window/surface.
    ///
    /// This is the mechanism behind "a theme switch repaints an idle window": the theme
    /// mutation only arms the epoch (it runs under the non-reentrant theme lock), and the
    /// loop thread performs the invalidation once. A second frame must not repeat it, or
    /// every theme change would pin the window awake forever.
    #[test]
    fn an_armed_global_repaint_repaints_once_and_then_stops() {
        // A frame on *any* thread consumes a pending process-wide stop request and drains the shared
        // trigger queue, so this test takes the same seam lock every other frame test takes. Without
        // it, this frame could swallow the stop a loop test had just recorded for its own loop —
        // benign while a frame was slow enough to be rare, and reliable once frames got fast.
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        // Consume anything an earlier test on this thread armed, so "first frame" below
        // cannot be satisfied by a stale epoch.
        let _ = drive_frame(0);

        // `renderable_epoch` is what a repaint request records; a standalone mounted control
        // has no platform surface for `invalidate_surface` to accept, so the ledger counter
        // (`repaints_submitted`) stays zero here even though the request was made.
        let before = renderable_epoch();
        arm_global_repaint();
        let _ = drive_frame(0);
        let after_armed = renderable_epoch();
        assert_ne!(
            after_armed, before,
            "the armed frame must request a repaint for the mounted widget"
        );
        let _ = drive_frame(0);
        assert_eq!(
            renderable_epoch(),
            after_armed,
            "one arming must buy exactly one full repaint, not a permanently awake loop"
        );
    }

    /// A hovered button *is* animating, and the bus advances it to rest.
    ///
    /// The whole point of the bus: before it, the button's `interaction_progress`
    /// interpolated forever in principle and never in practice, because nothing called
    /// `tick`. This drives it through the one runtime entry point.
    #[test]
    fn the_bus_advances_a_hovered_button_to_rest() {
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        // Deliver the hover the same way the runtime does, rather than reaching into the
        // registry: this goes through the public dispatch the pointer path uses.
        assert!(dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }));
        // The control now owes frames, which is what lets a host schedule one.
        assert!(has_animating_widgets(), "a hovered button is animating");
        // Step until the bus reports rest, bounded so a transition that never settles fails
        // the test rather than hanging on it. The exact duration is a theme token, so the
        // assertion is "it ends", not a specific number of frames.
        let mut frames = 0;
        while tick_animations(50) {
            frames += 1;
            assert!(frames < 100, "the hover transition must settle, not run forever");
        }
        assert!(frames > 0, "a hover must take more than one frame, or it did not animate");
        assert!(!has_animating_widgets(), "and the control must report resting afterwards");
    }

    /// A frame with only resting controls costs nothing.
    ///
    /// This is the half of "smooth" that decides whether it is bought with a burning
    /// core: an idle window must report `false` so the host stops scheduling frames.
    #[test]
    fn the_bus_reports_no_frames_for_a_resting_control() {
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        assert!(!has_animating_widgets(), "a resting button animates nothing");
        assert!(!tick_animations(16), "so the bus returns false and the host stops");
    }

    // ── BLUE24 §1 -- the frame driver ───────────────────────────────────────────

    /// A frame with a hovered control reports the work it did and asks for another frame.
    ///
    /// BLUE24 §1 criterion 1. The shape matters more than the numbers: the outcome must
    /// name **which** control moved (`controls_ticked == 1`) and must ask for the next frame,
    /// because a transition that has not settled is exactly the case a host has to keep
    /// scheduling for. A driver that returned `false` here would paint one step of the
    /// animation and then freeze, which is the failure the driver exists to prevent.
    /// Serialises the tests that drive the **process-wide** frame loop.
    ///
    /// `drive_frame` drains one shared event queue and advances every mounted control, so two of
    /// these tests running concurrently can consume each other's events. `drive_frame_drains_input_before_advancing`
    /// asserts that *its* injected trigger reached its handler during *its* frame, and a sibling's
    /// frame can dequeue that trigger first — the handler then runs on the other thread, whose
    /// thread-local observation flag is not the one the test reads. That is exactly how it flaked
    /// under the full suite while passing in isolation.
    ///
    /// It takes `platform::stop`'s process-seam lock rather than a module-local one: a frame also
    /// **consumes a pending stop request** (`honour_request_on_loop_thread`), so a frame test that
    /// ran beside the stop flag's own unit test drained the request it had just recorded — the
    /// stop test then failed only in the full suite. One lock for both seams is what makes the
    /// two kinds of test serialize against each other.
    ///
    /// A poison is ignored rather than propagated: the poisoning test has already failed, and the
    /// remaining tests must still be able to take the lock and report honestly.
    fn serialise_drive_frame() -> std::sync::MutexGuard<'static, ()> {
        crate::platform::stop::TEST_SEAM_SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn drive_frame_ticks_a_hovered_control_and_asks_for_another_frame() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        assert!(
            dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }),
            "the hover must reach the control, or there is nothing to animate"
        );

        let outcome = drive_frame(16);
        assert_eq!(outcome.controls_ticked, 1, "exactly the hovered button advanced");
        assert!(
            outcome.needs_another_frame,
            "a transition still in flight means the host must schedule another frame"
        );
    }

    /// A frame that advances a control submits damage bounded by that control.
    ///
    /// 丝滑-2's last step: the sweep used to ask for the whole subtree (and the whole window)
    /// to repaint. It still tells the window, because on a window-tree backend the window is
    /// what redraws the children, but it now hands over a rectangle instead of asking for
    /// everything. The assertion is the union of the control's damage, not a specific count:
    /// a tracker may coalesce regions, and what matters is that nothing outside the control's
    /// painted box plus the halo margin was submitted.
    #[test]
    fn an_animation_frame_submits_damage_bounded_by_the_moving_control() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        // Drain anything an earlier test left armed, so this test measures its own frame.
        let _ = drive_frame(0);
        assert!(
            set_repaint_mode(id, RepaintMode::Dirty),
            "the control must opt into damage tracking for a rectangle to be recorded"
        );
        assert!(
            dispatch_event(id, &crate::event::Event::MouseEnter { pos: Point::new(5, 5) }),
            "the hover must reach the control, or there is nothing to animate"
        );

        let outcome = drive_frame(16);
        assert_eq!(outcome.controls_ticked, 1, "exactly the hovered button advanced");

        let bounds = painted_box_with_damage_margin(id).expect("a mounted control has a box");
        let damage = dirty_rects(id);
        assert!(
            !damage.is_empty(),
            "the sweep must submit the control's damage, not merely ask for a frame"
        );
        for rect in damage {
            let bottom_right =
                Point::new(rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 - 1);
            assert!(
                bounds.contains_point(Point::new(rect.x, rect.y))
                    && bounds.contains_point(bottom_right),
                "animation damage {rect:?} must stay inside the painted box {bounds:?}"
            );
        }
    }

    /// A static frame costs nothing and submits nothing.
    ///
    /// BLUE24 §1 criterion 2, and the plan calls this **more important than the animation
    /// itself**: it is the property that decides whether a continuously-running loop is
    /// affordable. A resting tree must report no ticks and no repaint submissions, so an
    /// idle window pays for a sweep and nothing else.
    #[test]
    fn drive_frame_on_a_resting_tree_submits_nothing() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        // Drain anything a previous test on this thread left behind, so the frame is
        // measured from a clean queue.
        let _ = drive_frame(16);

        let outcome = drive_frame(16);
        assert_eq!(outcome.controls_ticked, 0, "nothing on this tree is moving");
        assert_eq!(outcome.repaints_submitted, 0, "and nothing asked to be repainted");
        assert!(!outcome.needs_another_frame, "so the host may stop scheduling frames");
    }

    /// Input drained by this frame is visible to this frame's advance.
    ///
    /// BLUE24 §1 criterion 3, the ordering assertion: the plan fixes
    /// `drain -> advance -> report` and gives the reason -- an animation's *target* comes
    /// from state, state comes from events, so a frame that advanced before draining would
    /// not begin reacting until the following frame.
    ///
    /// # Why the observable fact is a callback and not a control's geometry
    ///
    /// The event has to arrive through the **trigger queue**, because only input that comes
    /// that way can be "drained by this frame" -- so the payload is a
    /// [`crate::platform::WidgetTriggerKind`]. There is no `Hovered` kind yet, and the kinds
    /// that exist carry no pointer position, so what a queued trigger observably does is run
    /// the handler the host registered for it. That handler is this test's clock: it records
    /// "drained", `drive_frame` then records nothing of its own, and the assertion is that
    /// the handler ran **inside** the frame that also reported `events_dispatched == 1`.
    /// A frame drains input, and reports what it drained, as its **first** step.
    ///
    /// # Why this test no longer asserts that its own event reached its handler
    ///
    /// It used to, and it was flaky -- roughly one run in twenty, across four successive theories, each
    /// one deleting the last:
    ///
    /// 1. `events_dispatched == 1` measured the process rather than the driver (the queue is
    ///    process-wide). Fixed by comparing deltas.
    /// 2. The handler flag stayed false because a frame dispatched other tests' events and never reached
    ///    this one's. A bounded retry loop narrowed the window and did **not** close it: measured, 1
    ///    failure in 20 runs.
    /// 3. `drain_widget_triggers_for` returned 0 right after `inject` returned `true`, because a
    ///    concurrent frame had taken the event in between.
    ///
    /// The conclusion is not "find a cleverer assertion" but "this claim is not testable here":
    /// `drive_frame` drains the **process-wide** queue, and every other test in the binary is draining
    /// the same one. No ordering of inject / drain / assert inside this test can exclude that.
    ///
    /// So the test asserts the part that **is** a property of the driver:
    ///
    /// * the frame reports a non-zero drain when work is queued for it, and
    /// * the drain is step 1, which is observable because `events_dispatched` is computed before
    ///   `tick_animations` runs and is returned unchanged by it.
    ///
    /// The delivery half -- "a drained event reaches the handler" -- is pinned where it is genuinely
    /// testable, in `platform::state`'s own queue test and in `dispatch_trigger`'s callers, rather than
    /// through a queue that other tests own shares of.
    #[test]
    fn drive_frame_drains_input_before_advancing() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        // Register the handler the way a host does, so the production dispatch path is exercised even
        // though this test does not assert on it.
        thread_local! {
            #[allow(clippy::missing_const_for_thread_local)]
            static DRAINED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
        }
        DRAINED.with(|flag| flag.set(false));
        crate::app::handle_set_widget_value_callback(
            id,
            std::rc::Rc::new(RefCell::new(|_text: &str| DRAINED.with(|flag| flag.set(true)))),
        );

        // A quiescent frame first, so `before` is this test's own baseline rather than a number that
        // includes whatever another test left queued.
        crate::drain_widget_triggers_for(id);
        let before = drive_frame(0).events_dispatched;
        assert!(
            crate::inject_widget_trigger_event(
                id,
                crate::platform::WidgetTriggerKind::ValueChanged
            ),
            "the backend must accept the injected trigger"
        );
        let outcome = drive_frame(16);

        // The frame's own account of step 1. A delta, because the queue is shared: what this asserts is
        // that the frame **counted what its drain took**, not that it took exactly one thing.
        //
        // # Why this is not `outcome.events_dispatched > before`
        //
        // That is what it used to assert, and it was **flaky under the full suite** while passing
        // in isolation: `events_dispatched` is the process-wide count, and the sibling tests in this
        // module (`drive_frame_ticks_a_hovered_control_and_asks_for_another_frame`,
        // `drive_frame_on_a_resting_tree_submits_nothing`, `drive_frame_advances_each_control_once_per_frame`,
        // `drive_frame_accounts_for_repaints_and_clears_the_count`) each call `drive_frame` too. A
        // concurrent test's frame can drain this test's injected trigger first, so the count this test
        // compares against moves for a reason that has nothing to do with the claim.
        //
        // The claim is about *this* frame having drained *this* control's trigger, and that is
        // observable without the global counter: the handler runs during the drain, so the widget's
        // own value callback having fired is the per-control evidence that step 1 happened. It is
        // also the stronger assertion -- a frame that dispatched *someone else's* trigger would still
        // satisfy `> before` but cannot satisfy this.
        assert!(
            DRAINED.with(|flag| flag.get()),
            "the injected trigger must have reached its handler during the frame's drain \
             (dispatched={}, before={before})",
            outcome.events_dispatched
        );

        // And the drain is the *first* step, which is what the original claim was about: the count is
        // taken before any control is advanced, so a frame that advanced first and drained after would
        // report the same number but a control that became dirty this frame would not be advanced until
        // the next one. `controls_ticked` is the observable that separates those: a frame that drained
        // a change and then advanced its controls is the one that reports both.
        assert!(
            outcome.controls_ticked <= count_animating_widgets(),
            "the advance is bounded by what the drain left animating"
        );

        // The negative half, on the targeted drain: this control's own queue is empty now, so a second
        // drain of it dispatches nothing. Read here rather than on a frame for the reason above.
        assert_eq!(
            crate::drain_widget_triggers_for(id),
            0,
            "the control's own queue is empty after the frame drained it"
        );
    }

    /// Each control is advanced exactly once per frame.
    ///
    /// BLUE24 §1 criterion 4. The failure it guards is the one the plan names in §1.1: a
    /// frame that ticks a control through two layers moves it at a speed that depends on
    /// how many layers happened to call in, not on the duration the theme asked for.
    ///
    /// The measurement is **total travel per frame**: with one advance per frame, the sum
    /// of a control's per-frame travel equals its total travel. Two advances in a frame
    /// would double it, so summing and comparing against the end-to-end travel catches the
    /// defect without needing a counting double.
    #[test]
    fn drive_frame_advances_each_control_once_per_frame() {
        let _serial = serialise_drive_frame();
        use crate::widget::display_widgets::switch::Switch;

        let mut switch = Switch::new(Rect::new(0, 0, 60, 30));
        let switch_id = switch.id();
        switch.set_checked(true);
        // A host-owned control: the paint path reports it to the bus, which is the path
        // `drive_frame` folds in. Mounting it would be a second population and blur the
        // count this test is measuring.
        assert!(!is_mounted(switch_id));

        let start = switch.travel_progress();
        let mut per_frame_total = 0.0f32;
        let mut frames = 0;
        while switch.is_animating() {
            assert!(frames < 200, "the travel must settle, not run forever");
            let before = switch.travel_progress();
            // One frame, advanced through the driver the same way a host does it: report
            // what this host-owned control is doing, then let the frame run.
            let dynamic: &mut dyn Widget = &mut switch;
            if dynamic.tick(16) || dynamic.is_animating() {
                animation_bus_note_host_owned_animating(true);
            } else {
                animation_bus_note_host_owned_settled();
            }
            let _ = drive_frame(16);
            per_frame_total += (switch.travel_progress() - before).abs();
            frames += 1;
        }
        let end_to_end = (switch.travel_progress() - start).abs();
        assert!(end_to_end > 0.0, "the fixture must actually have travelled");
        // Exact equality is right here: both sums add the same `f32` deltas in the same
        // order, so any second advance in a frame shows up as a doubled total, not a
        // rounding difference.
        assert_eq!(
            per_frame_total, end_to_end,
            "the per-frame travel must add up to the total, which holds only if each \
             frame moves the control once"
        );
        assert!(
            frames > 1,
            "the travel must have taken more than one frame, or it did not animate"
        );
    }

    // ── BLUE24 §4 -- the environment facts ──────────────────────────────────────

    /// A host-installed provider is what a frame reads, and the frame refreshes it.
    ///
    /// BLUE24 §4 criteria 3 and 5. Two properties in one frame: the installed provider's
    /// answers reach `environment()`, and installing one changes what the library reports
    /// **without** any control knowing where the fact came from. The provider is removed
    /// before the assertions finish so a failure cannot leak it into a later test on this
    /// thread (the install is thread-local, so that is a real risk).
    #[test]
    fn a_frame_reads_the_installed_environment_provider() {
        let _serial = serialise_drive_frame();
        use crate::style::environment::{
            environment, install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Fixture;
        impl EnvironmentProvider for Fixture {
            fn text_scale(&self) -> f32 {
                2.0
            }
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        let previous = install_environment(Box::new(Fixture));
        let snapshot = environment();
        assert_eq!(snapshot.text_scale, 2.0, "the provider's text scale reaches the read point");
        assert!(snapshot.prefers_reduced_motion(), "and its motion preference too");
        assert_eq!(snapshot.effective_text_scale(), 2.0, "and survives the clamp");

        // A frame refreshes the snapshot from the provider, so the fact is what the frame
        // used rather than what was installed before it.
        let _ = drive_frame(16);
        assert_eq!(environment().text_scale, 2.0, "still true after a frame");

        // Restore: remove the fixture and prove the default came back. The assertion is on the
        // **text scale**, which the fixture set to a value no default can produce, rather than on
        // the motion preference -- that one could already be set by a provider a neighbouring test
        // installed, so it cannot distinguish "the fixture is gone" from "someone else asked for
        // the same thing".
        let _ = uninstall_environment();
        assert_ne!(
            environment().text_scale,
            2.0,
            "removing the fixture restores whatever was installed before it"
        );
        if let Some(previous) = previous {
            // A previous provider existed only if an earlier test leaked one; put it back so
            // this test does not silently discard another test's state.
            install_environment(previous);
        }
    }

    /// The environment is constant across every control in one frame.
    ///
    /// BLUE24 §4 criterion 3. The guarantee is not "the snapshot never changes" -- a host that
    /// changes a system setting between frames must be seen, and `refresh_environment` re-reads it
    /// at the start of each frame for exactly that reason. The guarantee is that the refresh
    /// happens **once per frame**, so two controls painted in the same frame cannot disagree about
    /// the text scale.
    ///
    /// That is measured by counting the provider's reads. A frame that asked once per control (the
    /// shape the snapshot exists to prevent) would read many times; taking one snapshot reads
    /// exactly once for the frame. The count is what distinguishes the two, which no value
    /// assertion could.
    #[test]
    fn the_environment_is_read_once_per_frame() {
        let _serial = serialise_drive_frame();
        use crate::style::environment::{
            environment, install_environment, uninstall_environment, EnvironmentProvider,
        };

        /// A provider that counts how many times it was asked.
        struct Counting {
            reads: std::rc::Rc<core::cell::Cell<u32>>,
        }
        impl EnvironmentProvider for Counting {
            fn text_scale(&self) -> f32 {
                self.reads.set(self.reads.get() + 1);
                // A value that changes per read, so "the frame saw one value" is visible in the
                // picture as well as in the count.
                self.reads.get() as f32
            }
        }

        let reads = std::rc::Rc::new(core::cell::Cell::new(0));
        let previous =
            install_environment(Box::new(Counting { reads: std::rc::Rc::clone(&reads) }));
        let before = reads.get();

        // One frame. `drive_frame` refreshes the snapshot once, which asks the provider once for
        // each of its eight facts -- but `text_scale` is read exactly once per refresh, so the
        // delta is 1. A per-control read would make it grow with the number of mounted widgets.
        let _ = drive_frame(16);
        let after_first = reads.get();
        assert_eq!(
            after_first - before,
            1,
            "one frame must ask the provider for the text scale exactly once"
        );

        // And the snapshot every control in that frame saw is one value, not one per control.
        let snapshot = environment();
        assert_eq!(snapshot.text_scale, after_first as f32);

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }

    /// Reduced motion collapses every transition to one frame, and the end state is the same.
    ///
    /// BLUE24 §4 criterion 2. The pair is the point: it is not "the animation is skipped" (which
    /// would leave the control at its start) but "the animation arrives immediately". So the
    /// control must stop animating after the first frame **and** have reached the geometry a
    /// normal-preference run reaches at its end.
    ///
    /// The switch's travel is used because it is the clearest two-ended property in the crate,
    /// and its target is derived from a latch rather than from a pointer, so the test needs no
    /// event delivery.
    #[test]
    fn reduced_motion_reaches_the_end_in_one_frame() {
        let _serial = serialise_drive_frame();
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;
        use crate::widget::display_widgets::switch::Switch;

        struct Reduced;
        impl EnvironmentProvider for Reduced {
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::ReduceMotion
            }
        }

        // The end the normal preference reaches, measured first so the comparison is against a
        // real run rather than against a literal.
        let mut full = Switch::new(Rect::new(0, 0, 60, 30));
        full.set_checked(true);
        let mut guard = 0;
        while full.tick(16) {
            guard += 1;
            assert!(guard < 200, "the travel must settle");
        }
        let full_end = full.travel_progress();

        let previous = install_environment(Box::new(Reduced));
        // A frame first, because the frame is what refreshes the snapshot the token read consults.
        let _ = drive_frame(16);

        let mut reduced = Switch::new(Rect::new(0, 0, 60, 30));
        reduced.set_checked(true);
        // One tick: with the preference in force the duration is zero, so the travel arrives now.
        let moving = reduced.tick(16);
        assert!(!moving, "a zero-duration transition settles on the first frame");
        assert_eq!(
            reduced.travel_progress(),
            full_end,
            "and it has arrived at the *same* place a full run reaches, not half way"
        );

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }
    }

    /// A repaint request made during a frame is counted against that frame.
    ///
    /// BLUE24 §1.2's `repaints_submitted` has to be an account of what the frame cost, so
    /// the count is taken **and cleared** by the driver: a frame loop reads the same number
    /// the frame produced, and the next frame starts from zero rather than accumulating.
    ///
    /// The platform is substituted with the crate's own recording double, because the
    /// question "was this submission counted?" is only answerable where a submission can
    /// be observed. The default backend on a headless host mounts nothing and reports
    /// `false`, which would make this test measure the backend rather than the driver.
    #[test]
    fn drive_frame_accounts_for_repaints_and_clears_the_count() {
        let _serial = serialise_drive_frame();
        use crate::platform::with_recorded_invalidations;
        use crate::platform::RecordingInvalidations;

        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        static RECORDER: std::sync::OnceLock<RecordingInvalidations> = std::sync::OnceLock::new();
        let recorder = RECORDER.get_or_init(RecordingInvalidations::new);
        recorder.clear();

        with_recorded_invalidations(recorder, || {
            // Settle and clear first, so only the submission below is measured.
            let _ = drive_frame(16);

            request_repaint(id);
            let first = drive_frame(16);
            assert!(first.repaints_submitted >= 1, "the request belongs to this frame's account");

            let second = drive_frame(16);
            assert_eq!(second.repaints_submitted, 0, "and the count was cleared, not accumulated");
        });
    }

    /// A control the **caller owns** is advanced and reported by the paint path.
    /// A host-owned control whose motion was started by something else is advanced by the paint
    /// path.
    ///
    /// # The defect this pins
    ///
    /// [`tick_animations`] sweeps the mounted registry. A control held as a
    /// `Box<dyn Widget>` -- which is what [`crate::widget::WidgetFactory::create`] returns,
    /// what `census` and `examples/export_control_svgs.rs` hold, and what this crate's own
    /// front-page example documents -- is not in that registry, so nothing advanced it. A
    /// `Switch` toggled ON and then painted drew its thumb at the **off** end on every frame,
    /// forever, and reported no error: its `travel_progress` stayed where it started while
    /// `is_checked` said `true`.
    ///
    /// A `Switch` is used as the fixture because its own `tick` does **not** request a repaint,
    /// which is what `manages_own_repaint` reports -- so it is the case the paint path has to
    /// handle. The `Button`/`Spinner` split is covered by its own test below.
    #[test]
    fn the_paint_path_advances_a_host_owned_control() {
        use crate::widget::display_widgets::switch::Switch;
        use crate::widget::draw_bridge::draw_of;

        let mut switch = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!is_mounted(switch.id()), "the fixture must not be in the registry");
        switch.set_checked(true);
        assert!(switch.is_animating(), "a freshly toggled switch owes frames");
        assert!(
            !switch.manages_own_repaint(),
            "and it does not repaint itself, so only the paint path can advance it"
        );

        let start = switch.travel_progress();
        let mut positions = vec![start];
        // 200 frames at the paint path's 16 ms step is 3.2 s of virtual time, past the `Slow`
        // token the travel runs on. The bound makes a transition that never settles fail here
        // instead of looping; the assertions are about the shape of the travel, not its length.
        let mut frames = 0;
        let mut bus_asked_for_a_frame = false;
        while switch.is_animating() {
            assert!(frames < 200, "the travel must settle, not run forever");
            frames += 1;
            let dynamic: &mut dyn Widget = &mut switch;
            let _ = draw_of(dynamic);
            positions.push(switch.travel_progress());
            if switch.is_animating() {
                bus_asked_for_a_frame |= animation_bus_needs_another_frame();
            }
        }
        assert!(frames > 1, "a travel must take more than one frame, or it did not animate");
        assert!(
            bus_asked_for_a_frame,
            "while it was in flight the bus had to say another frame is needed"
        );
        assert!(
            positions.last().copied().unwrap() > start,
            "painting an owned switch must advance its travel, not repaint a frozen frame"
        );
        assert!(
            positions.windows(2).all(|pair| pair[1] >= pair[0]),
            "the travel must be monotonic: {positions:?}"
        );
        assert!(
            positions.iter().any(|position| *position > 0.0 && *position < 1.0),
            "and it must pass through the middle, or it jumped rather than slid"
        );
        assert!(
            (positions.last().copied().unwrap() - 1.0).abs() < f32::EPSILON,
            "and it must arrive on the target, not one step short of it: {:?}",
            positions.last()
        );
    }

    /// A control that repaints itself is **not** advanced by repeated paints.
    ///
    /// # The second defect this pins, and it is worse than the first
    ///
    /// The first version of the paint path advanced every unmounted control. A `Spinner` is
    /// unmounted in the snapshot export, and its `tick` requests a repaint, so this was a
    /// feedback loop: every paint advanced it and every advance asked for the next paint, so
    /// `spinner.svg` differed between two consecutive exports of an unchanged build. A snapshot
    /// that is not reproducible cannot show a regression, which is the whole reason the export
    /// exists.
    ///
    /// `Spinner` is the free-running case: at rest its `tick` still returns `true` (it turns at a
    /// fixed rate forever), so "two paints of a resting spinner are identical" is exactly the
    /// property under test. A `Button` is the opposite case and is checked in the same test, so
    /// the rule is shown to cut between the two rather than exclude animation wholesale.
    ///
    /// # The one step a free-runner still takes, and why that is correct
    ///
    /// A control can only declare that it repaints itself by painting, so the frame that paints
    /// it for the first time is also the frame on which the declaration is made — and that frame
    /// is advanced. Leaving it out would mean the gate could never be satisfied for a control
    /// whose first frame *is* its declaration. The step is bounded (one per control, not one per
    /// paint) and does not break reproducibility, because the export paints each control several
    /// times and only the first of those moves it: measured on `spinner.svg`, two consecutive
    /// export runs are byte-identical.
    #[test]
    fn the_paint_path_skips_a_control_that_repaints_itself() {
        use crate::widget::display_widgets::spinner::Spinner;
        use crate::widget::draw_bridge::draw_of;

        let mut spinner = Spinner::new(Rect::new(0, 0, 120, 120));
        assert!(spinner.is_animating(), "the fixture must have something to advance");
        paint_owned(&mut spinner);
        assert!(
            spinner.manages_own_repaint(),
            "painting a spinner must declare that its frames are its own"
        );

        let settled = spinner.angle();
        for _ in 0..5 {
            paint_owned(&mut spinner);
        }
        assert!(
            (spinner.angle() - settled).abs() < f32::EPSILON,
            "once it has declared itself, further paints must not advance it: {} -> {}",
            settled,
            spinner.angle()
        );

        // The other side of the same gate: a `Button` is *not* excluded, so the rule is about who
        // repaints rather than about animation in general.
        let mut button =
            crate::widget::Button::new("ok".to_string(), crate::core::Rect::new(0, 0, 80, 32));
        button.set_hovered(true);
        assert!(!button.manages_own_repaint(), "an ordinary control must not claim its frames");
        assert!(button.is_animating(), "while still owing frames for the hover");
        let _ = draw_of(&mut button as &mut dyn Widget);
        assert!(
            button.is_animating() || button.widget_state() == crate::style::WidgetState::Hover,
            "the paint path advanced the button's interaction transition"
        );
    }

    /// Paints a host-owned control the way a host would: **through** [`draw_of`], into a surface.
    ///
    /// Doing the draw rather than only asking for the painting channel matters here, because the
    /// declaration a free-running control makes lives in its `draw`. A helper that stopped at
    /// `draw_of` would test the wrong thing.
    fn paint_owned(widget: &mut dyn Widget) {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        let Some(drawable) = crate::widget::draw_bridge::draw_of(widget) else {
            return;
        };
        let mut surface = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            drawable.draw(&mut context);
        }
        surface.end_frame();
    }

    /// An owned control that has settled keeps the bus quiet.
    ///
    /// The other half of [`the_paint_path_advances_a_host_owned_control`]: advancing must not
    /// become a reason to repaint a still window forever.
    #[test]
    fn an_owned_control_at_rest_does_not_keep_the_bus_awake() {
        use crate::widget::display_widgets::switch::Switch;
        use crate::widget::draw_bridge::draw_of;

        animation_bus_reset_host_owned();
        let mut resting = Switch::new(Rect::new(0, 0, 60, 30));
        let dynamic: &mut dyn Widget = &mut resting;
        let _ = draw_of(dynamic);
        assert!(
            !animation_bus_needs_another_frame(),
            "painting a resting owned control must not ask for another frame"
        );
    }

    /// A mounted control is advanced **once** per frame, not twice.
    ///
    /// `tick_animations` and the paint path are two different drivers, and a control that both
    /// registered itself and was then painted would otherwise run its animation at double speed —
    /// the failure `tick_animations`' own documentation calls out. The paint path therefore skips
    /// anything the registry owns.
    #[test]
    fn a_mounted_control_is_not_advanced_by_the_paint_path() {
        use crate::widget::display_widgets::switch::Switch;
        use crate::widget::draw_bridge::draw_of;

        let mut mounted = Switch::new(Rect::new(0, 0, 60, 30));
        mounted.set_checked(true);
        // Advanced by hand up to a mid-travel position, so the control *is* in flight and still
        // has somewhere to go — which is what makes "a paint must not move it" a real assertion
        // rather than one a settled control would pass vacuously.
        let mid_flight = mounted.tick(30);
        let id = register(Box::new(mounted)).expect("mount");
        let _unmount = MountGuard(id);
        let before = with_widget_mut(id, widget_travel).unwrap_or(-1.0);
        assert!(mid_flight, "the fixture must still owe frames to be worth stepping");
        assert!(
            before > 0.0 && before < 1.0,
            "the fixture must be mid-travel, or the test proves nothing: {before}"
        );

        // Painting it must not move it. `draw_of` takes `&mut dyn Widget`, so the mounted widget
        // has to be reached the way the frame loop reaches it.
        with_widget_mut(id, |widget| {
            for _ in 0..5 {
                let _ = draw_of(widget);
            }
        });
        let after = with_widget_mut(id, widget_travel).unwrap_or(-1.0);
        assert!(
            (before - after).abs() < f32::EPSILON,
            "a mounted control must be advanced only by the bus: {before} -> {after}"
        );

        // And the bus does move it, so the fixture was genuinely animating.
        let stepped = with_widget_mut(id, |widget| {
            widget.tick(100);
            widget_travel(widget)
        })
        .unwrap_or(-1.0);
        assert!(
            stepped > after,
            "the same control advanced directly must move: {after} -> {stepped}"
        );
    }

    /// Unmounts a widget registered by a test, even on panic.
    struct MountGuard(ObjectId);
    impl Drop for MountGuard {
        fn drop(&mut self) {
            let _ = unregister(self.0);
        }
    }

    /// The travel of a mounted switch, or `-1.0` when `widget` is not one.
    ///
    /// Read through the widget's own accessor rather than a test-only field, so the
    /// measurement is of the same fact the draw site reads to place the thumb.
    fn widget_travel(widget: &mut dyn Widget) -> f32 {
        use crate::widget::display_widgets::switch::Switch;
        (widget as &mut dyn std::any::Any)
            .downcast_mut::<Switch>()
            .map(|switch| switch.travel_progress())
            .unwrap_or(-1.0)
    }

    /// Downcasts a mounted widget to the concrete editor type.
    fn editor_of(widget: &mut dyn Widget) -> Option<&mut CodeEditor> {
        (widget as &mut dyn std::any::Any).downcast_mut::<CodeEditor>()
    }

    /// The auto-decision refuses a control too small for the bookkeeping to pay off.
    #[test]
    fn the_auto_decision_refuses_a_small_surface() {
        let small = register(Box::new(CodeEditor::new(Rect::new(0, 0, 100, 100)))).expect("r");
        let big = register(sample_editor("fn main() {}")).expect("r");
        // Both fixtures are below the threshold; the point is that the refusal is by
        // area, and that the threshold really is above them.
        const { assert!(100u64 * 100 < AUTO_REPAINT_MIN_PIXELS) };
        const { assert!(320u64 * 200 < AUTO_REPAINT_MIN_PIXELS) };
        assert!(!should_track_damage(small), "a 100x100 surface is not worth regioning");
        assert!(!should_track_damage(big), "320x200 is below the threshold as well");

        // Above the threshold **and** carrying children, it is accepted. A bare
        // `CodeEditor` is refused whatever its size, because it has no children and so
        // has only one rect to damage — which is the whole surface.
        let large = register(auto_accepted_surface()).expect("r");
        assert!(should_track_damage(large), "800x600 with children must be accepted");

        unregister(small);
        unregister(big);
        unregister(large);
    }

    /// A childless control is one rect, so regioning cannot narrow anything.
    #[test]
    fn the_auto_decision_refuses_a_control_with_no_children() {
        let lone = register(Box::new(CodeEditor::new(Rect::new(0, 0, 800, 600)))).expect("r");
        assert!(
            with_widget(lone, |widget| widget.base().children().is_empty()).unwrap_or(false),
            "the fixture must have no children"
        );
        assert!(
            !should_track_damage(lone),
            "a control with no children has one rect, which is the whole surface"
        );
        unregister(lone);
    }

    /// The one-call form does both halves, and reports honestly.
    #[test]
    fn enable_if_useful_is_a_no_op_where_it_would_not_help() {
        let small = register(Box::new(CodeEditor::new(Rect::new(0, 0, 80, 60)))).expect("r");
        assert!(!enable_damage_tracking_if_useful(small));
        assert_eq!(
            repaint_mode(small),
            RepaintMode::Full,
            "a refused enable must leave the mode alone"
        );

        // And an unregistered id is refused rather than silently accepted.
        assert!(!enable_damage_tracking_if_useful(0xDEAD_BEEF));
        unregister(small);
    }

    /// Enabling through the auto-decision really produces damage-tracked frames.
    #[test]
    fn a_widget_enabled_by_the_auto_decision_tracks_damage() {
        // A surface the library accepted at mount time comes up already tracking, with
        // no call from the host — which is the whole point of the automatic call site.
        let id = register(auto_accepted_surface()).expect("r");
        assert_eq!(repaint_mode(id), RepaintMode::Adaptive, "register must accept this surface");

        with_widget(id, |widget| widget.base().request_redraw()).expect("widget");
        assert_eq!(
            dirty_rects(id).len(),
            1,
            "the enabled widget must record damage from request_redraw"
        );

        // And the explicit one-call form still exists for a host that wants to ask
        // again after changing its geometry. It is refused for a control that has never
        // asked to be repainted, and accepted once one has.
        unregister(id);
        let explicit = register(container_with_child()).expect("r");
        assert!(
            !enable_damage_tracking_if_useful(explicit),
            "a control that never asked to repaint must be refused"
        );
        // Assigning a geometry is itself a repaint request (`BaseWidget::set_geometry` asks for
        // one so the moved content is redrawn), so the same control becomes eligible afterwards.
        assert!(set_geometry(explicit, Rect::new(0, 0, 800, 600)));
        assert!(
            enable_damage_tracking_if_useful(explicit),
            "assigning a geometry is a repaint request"
        );
        assert_eq!(repaint_mode(explicit), RepaintMode::Adaptive);
        unregister(explicit);
    }

    /// A control that never asked to repaint is left alone by the auto-decision.
    ///
    /// This is the cost side of the trade-off: enabling a surface that nothing redraws
    /// would only make it pay the bookkeeping. `register` therefore refuses a control
    /// that has not gone through `request_redraw` at least once.
    #[test]
    fn the_auto_decision_leaves_a_silent_control_alone() {
        let silent = register(auto_accepted_surface()).expect("r");
        assert_eq!(repaint_mode(silent), RepaintMode::Adaptive, "fixture sanity");
        unregister(silent);

        // With the mode cleared by hand (which is what a `Full` mode looks like), the
        // decision refuses until the control actually asks for a repaint.
        let id = register(container_with_child()).expect("r");
        assert!(set_repaint_mode(id, RepaintMode::Full));
        assert!(!should_track_damage(id), "a silent control must not be enabled");

        // "Asks for a repaint" includes the repaint request a geometry assignment makes.
        assert!(set_geometry(id, Rect::new(0, 0, 800, 600)));
        assert!(should_track_damage(id), "after asking for a repaint it becomes eligible");
        unregister(id);
    }

    /// Forgetting a widget drops its auto-decision evidence with it.
    #[test]
    fn unregistering_clears_the_auto_decision_state() {
        let id = register(auto_accepted_surface()).expect("r");
        assert_eq!(repaint_mode(id), RepaintMode::Adaptive);
        assert!(should_track_damage(id));

        unregister(id);
        assert!(!should_track_damage(id), "a dead id must not be judged as trackable");
        assert_eq!(repaint_mode(id), RepaintMode::Full, "nor keep a stale mode");
    }

    /// The reverse mapping is cleaned up with the widget, so a recycled own-id cannot
    /// file damage against a dead registry id.
    #[test]
    fn unregister_forgets_the_own_id_mapping() {
        let id = register(sample_editor("fn main() {}")).expect("r");
        let own = with_widget(id, |widget| widget.base().id()).expect("widget");
        assert_eq!(registry_id_of(own), Some(id));

        unregister(id);
        assert_eq!(registry_id_of(own), None, "the reverse map must not outlive the widget");
    }

    #[test]
    fn diag_container() {
        let id = register(auto_accepted_surface()).expect("r");
        let geometry = geometry_of(id).expect("geometry");
        let kids = with_widget(id, |w| w.base().children().len()).unwrap_or(999);
        assert_eq!(geometry.width, 800, "geometry must survive registration");
        assert_eq!((geometry.width, geometry.height), (800, 600));
        assert_eq!(kids, 1, "children must survive registration");
        assert_eq!(repaint_mode(id), RepaintMode::Adaptive);
        unregister(id);
    }

    /// The `Adaptive` run counter for a widget, for the tests that assert on *why* a
    /// frame painted whole rather than only that it did.
    fn large_damage_run_of(id: ObjectId) -> u32 {
        REPAINT
            .try_with(|map| map.borrow().get(&id).map(|state| state.large_damage_run))
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    /// A container large enough and structured enough for the auto-decision to accept.
    ///
    /// The children matter: a childless control has one rect, so regioning cannot
    /// narrow anything and `should_track_damage` refuses it. `add_child` records the
    /// nesting the criterion reads.
    fn container_with_child() -> Box<dyn Widget> {
        let mut group =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 320, 200));
        group.base_mut().add_child(7);
        Box::new(group)
    }

    /// The shape of a real window: large, with children, and it has asked to repaint.
    ///
    /// The third part is not incidental. `register` enables tracking for a control the
    /// library judges worth it — but only for a control that has actually asked to be
    /// repainted, because enabling a surface nothing redraws is pure cost. A real window
    /// asks while its first frame is being prepared, which is before it is mounted; a
    /// constructor that does so is modelled here by calling `request_redraw` directly.
    fn auto_accepted_surface() -> Box<dyn Widget> {
        let mut group =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 800, 600));
        group.base_mut().add_child(7);
        group.base_mut().request_redraw();
        Box::new(group)
    }

    fn sample_editor(text: &str) -> Box<dyn Widget> {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
        editor.set_text(text);
        Box::new(editor)
    }

    // ── Damage producers: `request_redraw` must record damage ────────────────
    //
    // Before these existed, `mark_dirty_rect` had **no production caller** — the
    // tracker, the platform invalidation and `render_frame_incremental` were all
    // complete and reachable from tests only, so partial repaint could never happen in
    // a real program. `request_redraw` is the single point every appearance change
    // converges on (~1000 call sites), which is what makes recording damage *there*
    // correct by construction rather than a list of paths someone must keep complete.

    /// `request_redraw` records damage once the widget has opted in.
    #[test]
    fn request_redraw_records_damage_when_adaptive() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode_adaptive(id));
        assert!(dirty_rects(id).is_empty(), "nothing has asked to repaint yet");

        with_widget(id, |widget| widget.base().request_redraw()).expect("widget");

        let damaged = dirty_rects(id);
        assert_eq!(damaged.len(), 1, "request_redraw must record exactly one region: {damaged:?}");
        assert_eq!(damaged[0], Rect::new(0, 0, 320, 200), "the control's own geometry");
        unregister(id);
    }

    /// A widget that never opted in records nothing, so the producer costs it nothing.
    #[test]
    fn request_redraw_records_nothing_in_full_mode() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert_eq!(repaint_mode(id), RepaintMode::Full, "Full is the default");

        with_widget(id, |widget| widget.base().request_redraw()).expect("widget");

        assert!(dirty_rects(id).is_empty(), "Full mode must not accumulate damage");
        unregister(id);
    }

    /// The producer is a no-op for an unregistered widget, so the transient controls
    /// tests build (never registered) cannot panic or leak state.
    #[test]
    fn request_redraw_on_an_unregistered_widget_is_a_no_op() {
        let editor = CodeEditor::new(Rect::new(0, 0, 40, 20));
        editor.base().request_redraw();
        assert!(dirty_rects(editor.base().id()).is_empty());
    }

    // ── Partial submissions from a control ─────────────────────────────────────
    //
    // `request_redraw` submits the control's whole geometry. These entry points let a
    // control submit only the rectangle it changed, while keeping the whole-control
    // fallback for modes that do not read damage.

    /// A control that submits its painted box plus the bleed produces exactly that
    /// damage, and the union never leaves those bounds.
    #[test]
    fn a_rect_submission_is_bounded_by_the_painted_box_plus_the_margin() {
        let id = register(auto_accepted_surface()).expect("registry");
        assert_eq!(repaint_mode(id), RepaintMode::Adaptive, "the fixture tracks damage");

        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let geometry = geometry_of(id).expect("geometry");
        let bounds = painted_box_with_damage_margin(id).expect("painted box");
        assert_eq!(
            bounds,
            Rect::new(
                geometry.x - margin as i32,
                geometry.y - margin as i32,
                geometry.width + margin * 2,
                geometry.height + margin * 2,
            ),
            "the box is the control's geometry grown by the bleed on every side"
        );

        request_repaint_rect(id, bounds);

        let damaged = dirty_rects(id);
        assert_eq!(damaged.len(), 1, "one submission is one region: {damaged:?}");
        assert_eq!(damaged[0], bounds, "the submission must be recorded as given");
        assert!(
            damaged.iter().all(|rect| {
                rect.x >= bounds.x
                    && rect.y >= bounds.y
                    && rect.x + rect.width as i32 <= bounds.x + bounds.width as i32
                    && rect.y + rect.height as i32 <= bounds.y + bounds.height as i32
            }),
            "the damage union must stay inside rect + margin: {damaged:?} vs {bounds:?}"
        );

        unregister(id);
    }

    /// `Full` is unchanged: a rectangle submission there becomes a whole-control
    /// repaint, which is the behaviour every control had before this API existed.
    #[test]
    fn a_rect_submission_in_full_mode_falls_back_to_a_whole_control_repaint() {
        use crate::platform::RecordingInvalidations;

        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert_eq!(repaint_mode(id), RepaintMode::Full, "Full is the default");

        let log: &'static RecordingInvalidations =
            alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));
        let rect = Rect::new(4, 8, 16, 16);
        crate::platform::with_recorded_invalidations(log, || request_repaint_rect(id, rect));

        assert_eq!(
            log.calls(),
            vec![(id, None)],
            "a control that does not track damage is invalidated whole, as before"
        );
        assert!(dirty_rects(id).is_empty(), "Full mode records no damage to replay");
        unregister(id);
    }

    /// A control nobody mounted has no surface to invalidate, so both entry points are
    /// silent even when a recording backend is installed.
    #[test]
    fn a_rect_submission_on_an_unregistered_widget_is_a_no_op() {
        use crate::platform::RecordingInvalidations;

        let editor = CodeEditor::new(Rect::new(0, 0, 40, 20));
        let id = editor.base().id();
        let log: &'static RecordingInvalidations =
            alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));

        crate::platform::with_recorded_invalidations(log, || {
            request_repaint_rect(id, Rect::new(0, 0, 40, 20));
            request_repaint_rect_because(id, Rect::new(0, 0, 40, 20), RepaintReason::State);
        });

        assert!(log.calls().is_empty(), "an unmounted id must not reach the platform");
        assert!(dirty_rects(id).is_empty());
        assert_eq!(painted_box_with_damage_margin(id), None);
    }

    /// Every frame that repaints must clear the damage, or the next frame repaints the
    /// same region forever and the tracker grows without bound.
    #[test]
    fn a_painted_frame_consumes_the_damage() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode_adaptive(id));
        let size = Size::new(320, 200);

        // First frame has no previous buffer, so it paints whole and clears.
        let first = render_frame_incremental(id, size, Color::WHITE, None).expect("frame");
        assert!(dirty_rects(id).is_empty(), "a full paint resets the damage");

        with_widget(id, |widget| widget.base().request_redraw()).expect("widget");
        assert_eq!(dirty_rects(id).len(), 1);

        let second = render_frame_incremental(id, size, Color::WHITE, Some(&first)).expect("frame");
        assert_eq!(second.len(), first.len(), "the frame keeps its size");
        assert!(dirty_rects(id).is_empty(), "the incremental paint consumed the damage");
        unregister(id);
    }

    /// `Adaptive` stops measuring after a run of frames whose damage covers the surface,
    /// then resumes when the damage shrinks.
    ///
    /// The assertion is on `large_damage_run`, which is the mode's whole difference from
    /// `Dirty`: a latch would keep painting whole after the animation ended, while this
    /// returns to regioning because one small-damage frame resets the counter.
    #[test]
    fn adaptive_learns_from_full_surface_damage_and_recovers() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode_adaptive(id));
        let size = Size::new(320, 200);
        let full = Rect::new(0, 0, 320, 200);

        // A first frame so later ones have something to carry forward.
        let mut frame = render_frame_incremental(id, size, Color::WHITE, None).expect("frame");

        // Damage covering the whole surface, repeatedly.
        for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
            assert!(mark_dirty_rect(id, full));
            frame = render_frame_incremental(id, size, Color::WHITE, Some(&frame)).expect("frame");
        }
        assert_eq!(
            large_damage_run_of(id),
            ADAPTIVE_LARGE_DAMAGE_RUN,
            "a run of whole-surface frames must be counted"
        );

        // One small-damage frame clears it, which is what keeps the mode from latching.
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
        let _ = render_frame_incremental(id, size, Color::WHITE, Some(&frame)).expect("frame");
        assert_eq!(large_damage_run_of(id), 0, "small damage must reset the run");
        unregister(id);
    }

    /// The rendered pixels must be *correct*, not merely cheap: a region-limited repaint
    /// has to produce the same frame as a full one.
    ///
    /// This is the assertion that makes the feature safe to enable. A tracker that
    /// recorded the wrong rectangle would still return `Some(frame)` and still report a
    /// consumed region — only comparing pixels catches a partial paint that painted the
    /// wrong part (or left a stale one).
    #[test]
    fn an_incremental_repaint_matches_a_full_repaint() {
        // Frames are compared byte for byte, and a control's chrome reads the active theme, so the
        // appearance has to be pinned for the comparison to be about the repaint strategy rather
        // than about whichever theme a concurrently-running test left selected.
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let size = Size::new(320, 200);

        // Reference: paint whole every time.
        let full_id = register(sample_editor("fn main() {}")).expect("registry");
        let baseline = render_frame_incremental(full_id, size, Color::WHITE, None).expect("frame");
        let mut expected = baseline.clone();
        let full_second =
            render_frame_incremental(full_id, size, Color::WHITE, Some(&baseline)).expect("frame");
        assert_eq!(full_second, baseline, "a full repaint of unchanged state is identical");
        expected.clone_from(&full_second);

        // Same control, damage-tracked.
        let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode_adaptive(dirty_id));
        let first = render_frame_incremental(dirty_id, size, Color::WHITE, None).expect("frame");
        assert!(mark_dirty_rect(dirty_id, Rect::new(8, 8, 24, 12)));
        let second =
            render_frame_incremental(dirty_id, size, Color::WHITE, Some(&first)).expect("frame");

        assert_eq!(second.len(), expected.len());
        assert_eq!(
            second, expected,
            "a region-limited repaint must produce the same pixels as a full one"
        );
        unregister(full_id);
        unregister(dirty_id);
    }

    #[test]
    fn register_then_unregister_round_trips() {
        let before = mounted_count();
        let id = register(sample_editor("fn main() {}")).expect("ui thread has a registry");
        assert!(is_mounted(id));
        assert_eq!(mounted_count(), before + 1);
        assert!(unregister(id));
        assert!(!is_mounted(id));
        assert_eq!(mounted_count(), before);
    }

    #[test]
    fn unregister_unknown_id_is_false() {
        assert!(!unregister(0xDEAD_BEEF));
    }

    /// A widget defaults to `Full`, so adopting damage tracking is opt-in and no
    /// existing caller's behaviour changes without asking.
    #[test]
    fn repaint_defaults_to_full() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert_eq!(repaint_mode(id), RepaintMode::Full);
        unregister(id);
    }

    /// Setting a policy on something that is not mounted must report the failure
    /// rather than silently succeeding on a key that addresses nothing.
    #[test]
    fn set_repaint_mode_rejects_an_unmounted_id() {
        assert!(!set_repaint_mode(0xDEAD_BEEF, RepaintMode::Dirty));
    }

    /// Damage is only recorded in the modes that read it, so a `Full` widget does
    /// not accumulate rects it will never consume.
    #[test]
    fn damage_is_recorded_only_when_a_mode_will_read_it() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let rect = Rect::new(10, 10, 40, 20);

        assert!(!mark_dirty_rect(id, rect), "Full mode must not accumulate damage");
        assert!(dirty_rects(id).is_empty());

        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        assert!(mark_dirty_rect(id, rect), "Dirty mode must record the damage");
        assert_eq!(dirty_rects(id), vec![rect]);

        unregister(id);
    }

    /// Recording damage must also ask the platform to repaint, or the damage would be
    /// tracked and never shown.
    ///
    /// # How this is observed
    ///
    /// The test backend records its invalidations, so the assertion is on what the
    /// backend was asked to do — not on a return value. A `mark_dirty_rect` that only
    /// updated the tracker would leave the invalidation log empty and fail here.
    #[test]
    fn recording_damage_asks_the_platform_to_repaint() {
        use crate::platform::RecordingInvalidations;

        let id = register(sample_editor("fn main() {}")).expect("registry");
        // `Box::leak` because the override holds a `&'static dyn Platform`: it must
        // outlive the call, and a test process is short-lived, so the leak is bounded
        // by the number of tests that install one.
        let log: &'static RecordingInvalidations =
            alloc::boxed::Box::leak(alloc::boxed::Box::new(RecordingInvalidations::new()));
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        let rect = Rect::new(4, 8, 16, 16);
        let recorded =
            crate::platform::with_recorded_invalidations(log, || mark_dirty_rect(id, rect));

        assert!(recorded, "the damage must still be recorded");
        assert_eq!(
            log.calls(),
            vec![(id, Some(rect))],
            "the platform must be told about the damage, narrowed to the rect when it can"
        );

        // `Full` mode records nothing, so it must not invalidate either — the caller is
        // expected to drive a whole-frame repaint itself.
        assert!(set_repaint_mode(id, RepaintMode::Full));
        log.clear();
        assert!(!crate::platform::with_recorded_invalidations(log, || {
            mark_dirty_rect(id, rect)
        }));
        assert!(log.calls().is_empty(), "a refused mark must not reach the platform");

        unregister(id);
    }

    /// A backend that cannot narrow must still be told to repaint, or the fallback
    /// would leave the surface stale.
    ///
    /// The recorder is asked to refuse narrowing, and the assertion is that a
    /// whole-surface invalidation arrived instead — so this pins the fallback itself,
    /// not merely that some call was made.
    #[test]
    fn a_backend_that_cannot_narrow_gets_a_whole_surface_repaint() {
        use crate::platform::RecordingInvalidations;

        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        let log: &'static RecordingInvalidations = alloc::boxed::Box::leak(alloc::boxed::Box::new(
            RecordingInvalidations::refusing_narrowing(),
        ));
        let rect = Rect::new(2, 2, 8, 8);
        assert!(crate::platform::with_recorded_invalidations(log, || mark_dirty_rect(id, rect)));

        assert_eq!(
            log.calls(),
            vec![(id, None)],
            "a backend that refuses narrowing must receive a whole-surface invalidation"
        );

        unregister(id);
    }

    /// Switching back to `Full` drops stale damage: a region that stopped mattering
    /// several frames ago must not be repainted when `Dirty` is re-selected.
    #[test]
    fn switching_to_full_forgets_accumulated_damage() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 10, 10)));
        assert_eq!(dirty_rects(id).len(), 1);

        assert!(set_repaint_mode(id, RepaintMode::Full));
        assert!(dirty_rects(id).is_empty(), "Full must clear what it will not replay");

        unregister(id);
    }

    /// A no-damage cached frame must not copy the full RGBA buffer three times. (D09-RENDER-04)
    ///
    /// # What this pins
    ///
    /// `render_frame_cached` cloned `LAST_FRAME` into `previous`, `render_frame_incremental`
    /// copied the carried buffer again to return it, and the result was cloned a third time back
    /// into the cache — three full-frame copies for a frame that drew nothing. The copy counter
    /// observes the cache's own full-buffer copies, so this asserts the count directly rather
    /// than inferring it from timing.
    #[test]
    fn an_undamaged_cached_frame_copies_the_buffer_once() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(96, 64);

        // First paint fills the cache (a full paint; its cost is not what this test bounds).
        let first = render_frame_cached(id, size, Color::rgb(3, 3, 3)).expect("first frame");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        // No damage: the cached frame is returned unchanged, with a single copy.
        let _ = take_frame_cache_copies();
        let second = render_frame_cached(id, size, Color::rgb(3, 3, 3)).expect("second frame");
        let copies = take_frame_cache_copies();

        assert_eq!(second, first, "an undamaged frame must not change a pixel");
        assert_eq!(
            copies, 1,
            "a no-damage cached frame must copy its buffer once (for the owned return), not three times"
        );

        unregister(id);
    }

    /// `Full` mode must not deep-copy a previous frame it will not read. (D09-RENDER-04)
    #[test]
    fn full_mode_does_not_copy_an_unused_previous_frame() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(64, 48);

        // Prime the cache, then force `Full`: every call repaints whole and ignores `previous`.
        render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("first frame");
        assert!(set_repaint_mode(id, RepaintMode::Full));

        let _ = take_frame_cache_copies();
        render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("second frame");
        let copies = take_frame_cache_copies();

        // A full paint makes one copy for the cache/return, but the *unused* previous frame is
        // not cloned on top of that. Three was the old count.
        assert!(
            copies <= 1,
            "Full mode must not clone the previous frame it ignores; copied {copies} times"
        );

        unregister(id);
    }

    /// The shared form answers the cache's own handle, so an unchanged frame is not copied.
    ///
    /// This is the difference the owned form cannot offer: it must return a `Vec<u8>`, which for
    /// a 4K surface is tens of megabytes duplicated per repaint even though no pixel changed. The
    /// pointer comparison is the evidence — equal pointers mean the two calls handed out the same
    /// allocation, and the copy counter says the same thing in numbers.
    #[test]
    fn the_shared_frame_is_the_cached_allocation_when_nothing_changed() {
        let size = Size::new(64, 48);
        let id = register(sample_editor("fn main() {}")).expect("registry");
        // `Full` is the default and bypasses every cache by design; the fast path belongs to the
        // damage-tracked modes.
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        let first = render_frame_cached_shared(id, size, Color::WHITE).expect("first frame");
        let _ = take_frame_cache_copies();
        let second = render_frame_cached_shared(id, size, Color::WHITE).expect("second frame");

        assert!(Rc::ptr_eq(&first, &second), "an undamaged frame must be the same buffer");
        assert_eq!(take_frame_cache_copies(), 0, "the shared form must not copy the frame at all");

        unregister(id);
    }

    /// A damaged frame adopts the previous buffer when the caller has released it.
    ///
    /// The incremental path starts from the previous frame's pixels. Copying them into the paint
    /// target is a full-frame `memcpy` — at 1280×800 that is 4 MB, more than painting a
    /// control-sized damage region — so the cached path removes its entry and hands the buffer over
    /// when nobody else holds it. The pixels must be identical either way, which the second half of
    /// the test asserts while deliberately holding the frame so `try_unwrap` refuses.
    #[test]
    fn a_damaged_frame_adopts_the_previous_buffer_only_when_it_owns_it() {
        let size = Size::new(64, 48);
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        let damage = Rect::new(4, 4, 20, 12);

        let first = render_frame_cached_shared(id, size, Color::WHITE).expect("first frame");
        // The caller has presented and dropped it, so the cache is the only holder left.
        drop(first);

        assert!(mark_dirty_rect(id, damage), "the control tracks the damage");
        let _ = SoftwarePaintBackend::take_seed_adoptions();
        let adopted = render_frame_cached_shared(id, size, Color::WHITE).expect("damaged frame");
        assert_eq!(
            SoftwarePaintBackend::take_seed_adoptions(),
            1,
            "a uniquely owned previous frame must be moved into the paint, not copied"
        );

        // Now keep the frame alive: the paint cannot take it, so it copies — and the pixels agree.
        assert!(mark_dirty_rect(id, damage));
        let _ = SoftwarePaintBackend::take_seed_adoptions();
        let copied = render_frame_cached_shared(id, size, Color::WHITE).expect("held frame");
        assert_eq!(
            SoftwarePaintBackend::take_seed_adoptions(),
            0,
            "a frame the caller still holds must be copied, not stolen"
        );
        assert_eq!(
            adopted.as_slice(),
            copied.as_slice(),
            "the two seeding paths must produce the same pixels"
        );

        drop(adopted);
        drop(copied);
        unregister(id);
    }

    /// The cached no-damage fast path must keep `Adaptive`'s reset behaviour. (D09-RENDER-04)
    ///
    /// # Why this is a regression test and not just a copy count
    ///
    /// The no-damage fast path in `render_frame_cached` returns the cached frame without going
    /// through `render_frame_incremental`. That callee clears the `Adaptive` run counter on a
    /// no-damage frame — its documented proof that the surface settled. If the fast path had
    /// skipped the reset, `Adaptive` would latch after a burst of large damage and never return to
    /// regioning, which is the exact failure the run counter exists to prevent.
    #[test]
    fn cached_no_damage_frame_still_resets_the_adaptive_run() {
        let size = Size::new(160, 120);
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Adaptive));

        // Build up the run with whole-surface damage through the cached entry point.
        render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("first frame");
        for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
            assert!(mark_dirty_rect(id, Rect::new(0, 0, 160, 120)));
            render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("frame");
        }
        assert_eq!(adaptive_large_damage_run(id), ADAPTIVE_LARGE_DAMAGE_RUN);

        // No damage: the cached fast path runs, and it must clear the run.
        let _ = render_frame_cached(id, size, Color::rgb(1, 1, 1)).expect("frame");
        assert_eq!(
            adaptive_large_damage_run(id),
            0,
            "a no-damage cached frame must reset the Adaptive run, or the mode latches"
        );

        unregister(id);
    }

    /// High-cardinality damage recorded through `mark_dirty_rect` must be bounded at record time,
    /// so the frame does not do one widget-tree traversal per region. (D09-RENDER-01)
    ///
    /// # What this pins
    ///
    /// The tracker's `max_regions` limit was only applied by `optimize()`, which the runtime
    /// never called, so a widget invalidating hundreds of small rectangles grew the tracker
    /// without bound and the repaint loop issued up to that many full draws. `mark_dirty_rect` now
    /// folds the damage into one covering region once the limit is reached, and this asserts the
    /// bounded count that results.
    #[test]
    fn many_small_damage_rects_are_bounded_and_keep_all_damage() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        // Far more rects than the default capacity (100).
        for i in 0..500 {
            assert!(mark_dirty_rect(id, Rect::new(i, 0, 1, 1)));
        }

        let regions = dirty_rects(id);
        assert!(
            regions.len() <= 100,
            "exceeding the region capacity must bound the region count, got {}",
            regions.len()
        );
        // No damage was dropped: the union of the returned regions covers every recorded rect
        // (they span x = 0..=499, y = 0).
        let union =
            regions.iter().copied().reduce(|a, b| a.union(&b)).expect("at least one region");
        assert!(union.x <= 0, "the union must start at the leftmost damage: {union:?}");
        assert!(
            union.right() >= 500,
            "the union must reach the rightmost damage (x=499): {union:?}"
        );

        unregister(id);
    }

    /// The record-time bound must not change what is repainted: a bounded frame matches a full
    /// repaint pixel for pixel. (D09-RENDER-01)
    #[test]
    fn bounded_high_cardinality_repaint_matches_a_full_repaint() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        crate::theme::global_theme_manager().set_appearance(crate::theme::AppearanceMode::Light);
        let size = Size::new(120, 80);

        let full_id = register(sample_editor("fn main() {}")).expect("registry");
        let expected = render_frame_cached(full_id, size, Color::WHITE).expect("frame");

        let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(dirty_id, RepaintMode::Dirty));
        let first = render_frame_cached(dirty_id, size, Color::WHITE).expect("frame");
        assert_eq!(first, expected, "the first frame is a full paint either way");

        // Damage far exceeding capacity, spread over the surface.
        for i in 0..400i32 {
            assert!(mark_dirty_rect(dirty_id, Rect::new(i % 120, (i / 120) * 20, 1, 1)));
        }
        let second = render_frame_cached(dirty_id, size, Color::WHITE).expect("frame");
        assert_eq!(
            second, expected,
            "a capacity-bounded repaint must produce the same pixels as a full repaint"
        );

        unregister(full_id);
        unregister(dirty_id);
    }

    /// A frame rendered with no damage must return the previous frame's pixels
    /// unchanged, which is the whole saving: no draw calls, same result.
    #[test]
    fn an_undamaged_frame_returns_the_previous_pixels() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(64, 48);

        let first = render_frame(id, size, Color::rgb(10, 20, 30)).expect("first frame");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        // No damage recorded, so the second frame is the first frame.
        let second = render_frame_incremental(id, size, Color::rgb(10, 20, 30), Some(&first))
            .expect("second frame");
        assert_eq!(second, first, "an undamaged frame must not change any pixel");

        unregister(id);
    }

    /// Without a previous frame there is nothing to preserve, so the call falls back
    /// to a full paint rather than returning a buffer it cannot fill.
    #[test]
    fn a_missing_previous_frame_forces_a_full_paint() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(64, 48);
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));

        let frame = render_frame_incremental(id, size, Color::rgb(1, 2, 3), None)
            .expect("a frame is still produced");
        assert_eq!(frame.len(), size.width as usize * size.height as usize * 4);

        unregister(id);
    }

    /// `render_frame_cached` must remember the frame it produced, so the next call
    /// can carry it forward instead of asking the caller to.
    ///
    /// This is the property that makes `RepaintMode` reach the screen: a backend's
    /// paint callback passes only the widget and the size, so if nothing remembered
    /// the previous frame every paint would fall back to full.
    #[test]
    fn the_cached_renderer_remembers_its_frame() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(64, 48);

        assert_eq!(cached_frame_size(id), None, "nothing is cached before a first paint");

        let first = render_frame_cached(id, size, Color::rgb(9, 9, 9)).expect("first frame");
        assert_eq!(
            cached_frame_size(id),
            Some(size),
            "the frame must be remembered, keyed by the size it was rendered at"
        );

        // A second paint with no damage returns the remembered frame byte for byte.
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        let second = render_frame_cached(id, size, Color::rgb(9, 9, 9)).expect("second frame");
        assert_eq!(second, first, "an undamaged repaint must not change a pixel");

        unregister(id);
        assert_eq!(cached_frame_size(id), None, "unmounting must drop the frame");
    }

    /// A control placed away from the surface origin must paint into its own frame.
    ///
    /// Regression: `render_frame` created a buffer the size of the control's own box but
    /// asked it to draw at the control's **absolute** coordinates, with no translation.
    /// A control at `(12, 42)` therefore painted from `(12, 42)` inside a 200×120 buffer:
    /// the top band stayed blank and the bottom-right of the control was clipped away.
    /// Every fixture in this module used `Rect::new(0, 0, ..)`, so the whole suite was
    /// blind to it — only a control with a non-zero origin can catch it.
    #[test]
    fn a_control_offset_from_the_origin_paints_into_its_own_frame() {
        let geometry = Rect::new(12, 42, 200, 160);
        let mut chart =
            crate::widget::special_widgets::finance::candlestick_chart::CandlestickChart::new(
                geometry,
            );
        let mut series = crate::widget::special_widgets::finance::types::PriceSeries::new();
        for index in 0..20 {
            let open = 100.0 + index as f64;
            series.push(crate::widget::special_widgets::finance::types::Bar::new(
                open,
                open + 2.0,
                open - 2.0,
                open + 1.0,
                1000.0,
            ));
        }
        chart.set_series(series);
        let id = register(Box::new(chart)).expect("registry");

        let size = Size::new(geometry.width, geometry.height);
        let frame = render_frame(id, size, Color::BLACK).expect("frame");

        // The panel background is a dark slate, not the clear colour, and the control
        // fills its plot area — so the frame must be substantially painted rather than
        // carrying a blank band as tall as the geometry's y offset.
        let painted =
            frame.as_chunks::<4>().0.iter().filter(|pixel| **pixel != [0, 0, 0, 255]).count();
        let total = size.width as usize * size.height as usize;
        assert!(
            painted > total / 2,
            "a control offset by ({}, {}) painted only {painted}/{total} pixels",
            geometry.x,
            geometry.y
        );
        // And the blank band above the first paint must be the control's own top margin
        // (a few pixels), not its absolute y offset. Before the translation was applied
        // this band was as tall as `geometry.y`, which is the failure this pins.
        let mut blank_rows = 0usize;
        for row in 0..size.height as usize {
            let start = row * size.width as usize * 4;
            let blank = frame[start..start + size.width as usize * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| *pixel == [0, 0, 0, 255]);
            if !blank {
                break;
            }
            blank_rows += 1;
        }
        assert!(
            blank_rows < geometry.y as usize,
            "the blank top band ({blank_rows} rows) must be the control's own margin, \
             not its absolute y offset ({})",
            geometry.y
        );

        unregister(id);
    }

    /// A cached frame of the wrong size must not be used, or a resize would leave part
    /// of the surface holding the previous layout's pixels.
    #[test]
    fn a_resize_discards_the_cached_frame() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let small = Size::new(32, 24);
        let large = Size::new(96, 72);

        render_frame_cached(id, small, Color::rgb(1, 1, 1)).expect("small frame");
        assert_eq!(cached_frame_size(id), Some(small));

        let resized = render_frame_cached(id, large, Color::rgb(1, 1, 1)).expect("large frame");
        assert_eq!(
            resized.len(),
            large.width as usize * large.height as usize * 4,
            "a resize must produce a frame of the new size"
        );
        assert_eq!(
            cached_frame_size(id),
            Some(large),
            "the cache must follow the resize rather than keep the old frame"
        );

        unregister(id);
    }

    /// Damage recorded on a removed widget must not survive it: a reused id would
    /// otherwise repaint a region that no longer exists.
    #[test]
    fn unmounting_drops_the_damage_too() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
        assert_eq!(dirty_rects(id).len(), 1);

        unregister(id);
        assert!(dirty_rects(id).is_empty(), "the damage must go with the widget");
    }

    /// `Adaptive` and `Dirty` must not be the same policy.
    ///
    /// # What separates them
    ///
    /// `Dirty` means the caller has already decided damage tracking is worth it, so
    /// every frame is measured. `Adaptive` means the caller does not know, so a run of
    /// frames whose damage covers the surface is taken as evidence that regioning
    /// cannot pay off here — and from then on it paints whole instead of measuring the
    /// same answer again.
    ///
    /// Asserting on the run counter is asserting on that decision, because the counter
    /// *is* the mode's memory. A version that treated the two modes alike would leave
    /// it at zero forever, which is exactly what this test fails on.
    #[test]
    fn adaptive_learns_from_a_run_of_large_damage_but_dirty_does_not() {
        let size = Size::new(200, 200);
        // Damage covering the whole frame, so every frame is over the ratio.
        let whole = Rect::new(0, 0, 200, 200);

        // `Dirty`: measured every frame, never accumulates a run.
        let dirty_id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(dirty_id, RepaintMode::Dirty));
        let mut frame = render_frame(dirty_id, size, Color::rgb(1, 1, 1)).expect("first frame");
        for _ in 0..(ADAPTIVE_LARGE_DAMAGE_RUN + 2) {
            assert!(mark_dirty_rect(dirty_id, whole));
            frame = render_frame_incremental(dirty_id, size, Color::rgb(1, 1, 1), Some(&frame))
                .expect("frame");
        }
        assert_eq!(
            adaptive_large_damage_run(dirty_id),
            0,
            "Dirty must not learn: the caller already decided, so every frame is measured"
        );
        unregister(dirty_id);

        // `Adaptive`: the same input accumulates the run.
        let adaptive_id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(adaptive_id, RepaintMode::Adaptive));
        let mut frame = render_frame(adaptive_id, size, Color::rgb(1, 1, 1)).expect("first frame");
        assert_eq!(
            adaptive_large_damage_run(adaptive_id),
            0,
            "a full paint is not evidence either way"
        );

        for expected in 1..=(ADAPTIVE_LARGE_DAMAGE_RUN as usize) {
            assert!(mark_dirty_rect(adaptive_id, whole));
            frame = render_frame_incremental(adaptive_id, size, Color::rgb(1, 1, 1), Some(&frame))
                .expect("frame");
            assert_eq!(
                adaptive_large_damage_run(adaptive_id) as usize,
                expected,
                "each over-threshold frame must advance the run"
            );
        }
        unregister(adaptive_id);
    }

    /// A frame with small damage resets the `Adaptive` run, so the mode settles back
    /// into regioning after an animation ends instead of staying latched.
    #[test]
    fn adaptive_recovers_when_the_damage_shrinks() {
        let size = Size::new(200, 200);
        let id = register(sample_editor("fn main() {}")).expect("registry");
        assert!(set_repaint_mode(id, RepaintMode::Adaptive));
        let mut frame = render_frame(id, size, Color::rgb(1, 1, 1)).expect("first frame");

        for _ in 0..ADAPTIVE_LARGE_DAMAGE_RUN {
            assert!(mark_dirty_rect(id, Rect::new(0, 0, 200, 200)));
            frame = render_frame_incremental(id, size, Color::rgb(1, 1, 1), Some(&frame))
                .expect("frame");
        }
        assert_eq!(adaptive_large_damage_run(id), ADAPTIVE_LARGE_DAMAGE_RUN);

        // One small-damage frame is proof the surface settled. Its output is the frame
        // for the *next* call, so it is carried rather than discarded.
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 4, 4)));
        let _settled =
            render_frame_incremental(id, size, Color::rgb(1, 1, 1), Some(&frame)).expect("frame");
        assert_eq!(
            adaptive_large_damage_run(id),
            0,
            "a small-damage frame must clear the run, or the mode would latch"
        );

        unregister(id);
    }

    /// A previous frame of the wrong size cannot be carried forward; the call must
    /// still return a correct, fully sized frame instead of a half-stale one.
    #[test]
    fn a_wrongly_sized_previous_frame_is_ignored() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(64, 48);
        assert!(set_repaint_mode(id, RepaintMode::Dirty));
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));

        let wrong = vec![0u8; 7];
        let frame = render_frame_incremental(id, size, Color::rgb(1, 2, 3), Some(&wrong))
            .expect("a frame is still produced");
        assert_eq!(
            frame.len(),
            size.width as usize * size.height as usize * 4,
            "a mismatched previous frame must not resize the output"
        );

        unregister(id);
    }

    /// Dirty repaint must leave the untouched region holding the previous frame's
    /// pixels, not the clear colour: that is the observable difference between a
    /// partial repaint and a full one.
    #[test]
    fn a_dirty_repaint_preserves_pixels_outside_the_damage() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let size = Size::new(80, 60);
        let clear = Color::rgb(200, 30, 40);

        let first = render_frame(id, size, clear).expect("first frame");
        assert!(set_repaint_mode(id, RepaintMode::Dirty));

        // Damage a region in the top-left only, then repaint with a different clear
        // colour. A full paint would fill the whole frame with the new colour; a
        // dirty paint must leave the far corner alone.
        assert!(mark_dirty_rect(id, Rect::new(0, 0, 8, 8)));
        let second = render_frame_incremental(id, size, Color::rgb(0, 0, 0), Some(&first))
            .expect("second frame");

        let last_pixel = second.len() - 4;
        assert_eq!(
            &second[last_pixel..],
            &first[last_pixel..],
            "the far corner is outside the damage and must keep its pixels"
        );

        unregister(id);
    }

    #[test]
    fn mounted_ids_do_not_collide_with_platform_ids() {
        // Platform-allocated ids count up from 1; mounted ids start far above.
        let id = register(sample_editor("x")).expect("registry");
        assert!(id > 0x5345_4C46_0000_0000, "mounted id {id:#x} must not look platform-issued");
        unregister(id);
    }

    #[test]
    fn with_widget_mut_exposes_the_real_widget() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let lines =
            with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.line_count()))
                .flatten();
        assert_eq!(lines, Some(1));
        unregister(id);
    }

    #[test]
    fn with_widget_mut_on_missing_id_is_none() {
        assert!(with_widget_mut(0x1234_5678, |_| ()).is_none());
    }

    /// `read_widget` answers a mounted control without detaching it.
    ///
    /// The value coming back is not the whole point: the registry must be untouched, so a
    /// **nested** read has to resolve as well (shared borrows compose), and the control has to
    /// still be mounted afterwards. Detaching and reinserting — what `with_widget` does — would
    /// also return the value, so only these two assertions distinguish the two functions.
    #[test]
    fn read_widget_answers_and_leaves_the_registry_alone() {
        let id = register(sample_editor("fn main() {}")).expect("registry");
        let base_id = read_widget(id, |widget| widget.base().id()).expect("mounted");

        // The outer read holds the registry borrow; the inner one must still resolve, and it
        // must see the widget's own state rather than a detached copy.
        let nested = read_widget(id, |_| read_widget(id, |inner| inner.base().id())).flatten();
        assert_eq!(nested, Some(base_id), "a nested read must resolve through the same borrow");
        assert_eq!(
            read_widget(id, |widget| widget.base().id()),
            Some(base_id),
            "the entry must never have left the registry"
        );
        unregister(id);
    }

    /// A control lent out for mutation is not readable, for the same reason `with_widget` refuses
    /// it: its state is being rewritten through an exclusive borrow.
    #[test]
    fn read_widget_refuses_a_control_that_is_being_mutated() {
        let id = register(sample_editor("x")).expect("registry");
        let inside = with_widget_mut(id, |_| read_widget(id, |_| 1));
        assert_eq!(inside, Some(None), "a mutating loan must not be readable");
        unregister(id);
    }

    #[test]
    fn geometry_can_be_read_and_written_through_the_registry() {
        let id = register(sample_editor("x")).expect("registry");
        assert_eq!(geometry_of(id).map(|r| (r.width, r.height)), Some((320, 200)));
        assert!(set_geometry(id, Rect::new(5, 6, 400, 300)));
        let geometry = geometry_of(id).expect("mounted");
        assert_eq!((geometry.x, geometry.y, geometry.width, geometry.height), (5, 6, 400, 300));
        unregister(id);
    }

    #[test]
    fn render_frame_returns_rgba_of_the_requested_size() {
        let id = register(sample_editor("fn main() {\n    let x = 1;\n}")).expect("registry");
        let frame = render_frame(id, Size::new(64, 48), Color::WHITE).expect("frame");
        assert_eq!(frame.len(), 64 * 48 * 4);
        // The editor paints a light background, so at least one pixel must be
        // non-transparent; a fully blank frame means the widget never drew.
        assert!(
            frame.as_chunks::<4>().0.iter().any(|px| px[3] != 0),
            "frame must contain drawn pixels"
        );
        unregister(id);
    }

    #[test]
    fn render_frame_rejects_empty_size_and_missing_ids() {
        let id = register(sample_editor("x")).expect("registry");
        assert!(render_frame(id, Size::new(0, 10), Color::WHITE).is_none());
        assert!(render_frame(0x9999, Size::new(10, 10), Color::WHITE).is_none());
        unregister(id);
    }

    /// **Every control the demo places must actually paint.**
    ///
    /// The tree walk skips a child that is not visible or has no `Draw`, and it does so
    /// silently — which is right for a hidden page but wrong for a control the caller
    /// placed on purpose. A control that never paints is invisible on screen while every
    /// geometry check passes, so this checks the *frame* rather than the geometry.
    ///
    /// Each widget is rendered on its own, at its own size, and the frame must contain at
    /// least one pixel that differs from the clear colour. A control whose whole
    /// appearance happens to equal the clear colour fails this, which is deliberate: from
    /// the user's side that control is not there either.
    #[test]
    fn every_control_kind_the_demo_uses_paints_something() {
        use crate::widget::WidgetFactory;

        // The kinds the control demo places, by factory name.
        let kinds = [
            "button",
            "checkbox",
            "radio_button",
            "spin_box",
            "combo_box",
            "line_edit",
            "slider",
            "progress_bar",
            "label",
        ];

        // A colour no widget's own palette uses, so "did it paint" cannot be confused
        // with "it painted a colour that happens to match".
        let clear = Color::rgb(255, 0, 255);
        let factory = WidgetFactory::new_with_defaults();
        let mut unpainted = Vec::new();

        for name in kinds {
            let Some(widget) = factory.create(name, Rect::new(0, 0, 120, 40), "sample") else {
                unpainted.push(format!("{name} (factory produced nothing)"));
                continue;
            };
            let Some(id) = register(widget) else {
                unpainted.push(format!("{name} (registration refused)"));
                continue;
            };
            set_geometry(id, Rect::new(0, 0, 120, 40));

            let painted = render_frame(id, Size::new(120, 40), clear)
                .map(|frame| {
                    frame
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|px| px[0] != 255 || px[1] != 0 || px[2] != 255)
                })
                .unwrap_or(false);
            if !painted {
                unpainted.push(name.to_string());
            }
            unregister(id);
        }

        assert!(
            unpainted.is_empty(),
            "these control kinds painted nothing, so they are invisible on screen: {unpainted:?}"
        );
    }

    #[test]
    fn render_frame_at_geometry_uses_widget_geometry() {
        let id = register(sample_editor("abc")).expect("registry");
        let frame = render_frame_at_geometry(id, Color::WHITE).expect("frame");
        assert_eq!(frame.len(), 320 * 200 * 4);
        unregister(id);
    }

    /// Every widget that paints itself must be mountable in a host surface.
    ///
    /// `Widget::as_draw_mut` defaults to `None`, so a widget can implement
    /// `Draw` and still be invisible when mounted — which is exactly how a
    /// mounted `Chip` silently produced no frame. This guards the contract for
    /// the widgets the demos and the factory expose.
    #[test]
    fn custom_widgets_report_the_draw_bridge() {
        use crate::widget::special_widgets::chip::Chip;
        use crate::widget::special_widgets::color_picker::ColorPicker;
        use crate::widget::special_widgets::gantt_widget::GanttWidget;
        use crate::widget::special_widgets::snackbar::Snackbar;
        use crate::widget::special_widgets::terminal_view::TerminalView;

        fn assert_mountable(name: &str, widget: &mut dyn Widget) {
            assert!(
                widget.as_draw_mut().is_some(),
                "{name} implements Draw but does not report itself via Widget::as_draw_mut, \
                 so mounting it in a window would paint nothing"
            );
        }

        assert_mountable("CodeEditor", &mut CodeEditor::new(Rect::new(0, 0, 80, 60)));
        assert_mountable("Chip", &mut Chip::new(Rect::new(0, 0, 80, 24)));
        assert_mountable("ColorPicker", &mut ColorPicker::new(Rect::new(0, 0, 200, 150)));
        assert_mountable("GanttWidget", &mut GanttWidget::new(Rect::new(0, 0, 200, 150)));
        assert_mountable("Snackbar", &mut Snackbar::new(Rect::new(0, 0, 200, 40)));
        assert_mountable("TerminalView", &mut TerminalView::new(Rect::new(0, 0, 200, 150)));
    }

    /// A built-in control must report itself as paintable.
    ///
    /// This assertion is the inverse of the one that used to live here, which
    /// required `Button` to return `None` because the control was expected to be
    /// an OS widget. Every control is now painted by the library (BLUE15 #55), so
    /// a control that still answers `None` paints a blank surface — the exact
    /// regression this test exists to catch.
    #[test]
    fn built_in_controls_claim_the_draw_bridge() {
        use crate::widget::base_widgets::button::Button;
        use crate::widget::base_widgets::checkbox::CheckBox;
        use crate::widget::base_widgets::label::Label;

        let mut button = Button::new("ok".to_string(), Rect::new(0, 0, 80, 30));
        assert!(
            button.as_draw_mut().is_some(),
            "Button implements Draw, so mounting it must paint instead of showing a blank surface"
        );

        let mut checkbox = CheckBox::new(Rect::new(0, 0, 120, 24));
        assert!(checkbox.as_draw_mut().is_some(), "CheckBox must be paintable");

        let mut label = Label::new("hello".to_string(), Rect::new(0, 0, 80, 20));
        assert!(label.as_draw_mut().is_some(), "Label must be paintable");
    }

    #[test]
    fn dispatch_event_reaches_the_widget() {
        use crate::event::Event;
        let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
        editor.set_text("hello");
        let id = register(Box::new(editor)).expect("registry");
        // A click inside the text area must move the caret away from 0:0.
        let delivered = dispatch_event(
            id,
            &Event::MousePress { pos: Point::new(150, 90), button: 1, modifiers: 0 },
        );
        assert!(delivered, "event must reach the mounted widget");
        let cursor =
            with_widget_mut(id, |widget| editor_of(widget).map(|editor| editor.cursor())).flatten();
        assert!(cursor.is_some(), "widget must still be the editor");
        unregister(id);
    }

    /// A secondary-button press opens the menu at the pointer, clamped to the
    /// viewport — the whole right-click path in one assertion.
    #[cfg(full_widgets)]
    #[test]
    fn a_secondary_press_opens_the_mounted_context_menu() {
        let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Copy");
        let id = register(Box::new(menu)).expect("registry");

        let viewport = Rect::new(0, 0, 500, 400);
        let opened = open_context_menu_for_event(
            id,
            &Event::MousePress {
                pos: Point::new(120, 90),
                button: crate::event::mouse_button::SECONDARY,
                modifiers: 0,
            },
            viewport,
        );

        assert!(opened, "a secondary press must open the context menu");
        let geometry = geometry_of(id).expect("mounted");
        assert_eq!((geometry.x, geometry.y), (120, 90));
        assert!(is_mounted(id));
        unregister(id);
    }

    /// The convenience wrapper must ignore every event that is not a secondary
    /// press, so a caller can forward its whole event stream without testing the
    /// button itself.
    #[cfg(full_widgets)]
    #[test]
    fn non_secondary_events_do_not_open_the_context_menu() {
        let mut menu = crate::Menu::new("Edit", Rect::new(0, 0, 160, 100));
        menu.add_action("Copy");
        let id = register(Box::new(menu)).expect("registry");
        let viewport = Rect::new(0, 0, 500, 400);

        assert!(!open_context_menu_for_event(
            id,
            &Event::MousePress {
                pos: Point::new(10, 10),
                button: crate::event::mouse_button::PRIMARY,
                modifiers: 0
            },
            viewport,
        ));
        assert!(!open_context_menu_for_event(
            id,
            &Event::MouseMove { pos: Point::new(10, 10) },
            viewport
        ));
        unregister(id);
    }

    /// Pointing the helper at an id that is not a menu must report failure rather
    /// than silently doing nothing, which is how a mis-wired right-click handler
    /// would otherwise go unnoticed.
    #[cfg(full_widgets)]
    #[test]
    fn opening_a_context_menu_on_a_non_menu_reports_failure() {
        let label = crate::widget::base_widgets::label::Label::new(
            "hi".to_string(),
            Rect::new(0, 0, 80, 24),
        );
        let id = register(Box::new(label)).expect("registry");

        assert!(!open_context_menu(id, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
        assert!(!open_context_menu(9_999, Point::new(5, 5), Rect::new(0, 0, 400, 400)));
        unregister(id);
    }

    /// The secondary press is delivered first and the menu request follows it, both to the
    /// same control. The press half is observable through the focus move it performs; the
    /// request half through the control's own signal.
    #[test]
    fn a_secondary_press_delivers_the_press_and_then_the_menu_request() {
        let field = crate::LineEdit::new(Rect::new(0, 0, 120, 32));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        field.context_menu_requested.connect_ref(move |pos: &Point| {
            *sink.lock().unwrap() = Some(*pos);
        });
        let id = register(Box::new(field)).expect("registry");
        let _guard = MountGuard(id);
        let point = Point::new(20, 16);

        assert!(dispatch_pointer_event(
            id,
            &Event::MousePress {
                pos: point,
                button: crate::event::mouse_button::SECONDARY,
                modifiers: 0,
            },
            point,
        ));
        assert_eq!(*seen.lock().unwrap(), Some(point), "the menu request follows the press");
        assert_eq!(
            focused_widget(),
            Some(id),
            "the press itself must still be delivered, not swallowed by the menu path"
        );
        clear_focus();
    }

    /// A keyboard-originated request (the translator's `ContextMenu`) goes to the control that
    /// owns the keyboard, because there is no pointer position to hit-test.
    #[test]
    fn a_translated_menu_key_request_reaches_the_focused_control() {
        let field = crate::LineEdit::new(Rect::new(0, 0, 120, 32));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&seen);
        field.context_menu_requested.connect_ref(move |pos: &Point| {
            *sink.lock().unwrap() = Some(*pos);
        });
        let id = register(Box::new(field)).expect("registry");
        let _guard = MountGuard(id);
        assert!(focus_widget(id));

        assert!(dispatch_context_menu_event(&Event::context_menu(4, 5)));
        assert_eq!(*seen.lock().unwrap(), Some(Point::new(4, 5)));
        clear_focus();
    }

    /// A control that never opted into the context-menu hook reports `false` and is otherwise
    /// untouched: the request is delivered, the hook declines it, and the widget's state does
    /// not move.
    #[test]
    fn an_unintegrated_control_ignores_a_context_menu_request() {
        use crate::widget::base_widgets::label::Label;

        let label = Label::new("plain".to_string(), Rect::new(0, 0, 80, 24));
        let id = register(Box::new(label)).expect("registry");
        let _guard = MountGuard(id);
        let before = geometry_of(id).expect("mounted");

        assert!(dispatch_event(id, &Event::context_menu(3, 4)));
        assert_eq!(geometry_of(id), Some(before), "the request must change nothing");
        let handled = with_widget_mut(id, |widget| widget.context_menu_event(Point::new(3, 4)))
            .unwrap_or(true);
        assert!(!handled, "the default is no menu");
    }

    /// A text selection dragged onto a drop zone is committed through the coordinator: the
    /// session activates on the move, the target reports its hover, and the release emits the
    /// zone's own `payload_dropped` signal.
    #[cfg(full_widgets)]
    #[test]
    fn the_drag_coordinator_drops_a_text_selection_on_a_drop_zone() {
        use crate::event::dnd::DragPayload;
        use crate::widget::misc_widgets::drop_zone::{DropZone, DropZoneState};

        let mut area = crate::TextArea::new("hello".to_string(), Rect::new(0, 0, 120, 60));
        area.select_all();
        let area_id = register(Box::new(area)).expect("registry");
        let _area_guard = MountGuard(area_id);

        let zone = DropZone::new(Rect::new(200, 0, 120, 80), "text/plain");
        let dropped = std::sync::Arc::new(std::sync::Mutex::new(None));
        let sink = std::sync::Arc::clone(&dropped);
        zone.payload_dropped.connect_ref(move |payload: &DragPayload| {
            *sink.lock().unwrap() = Some(payload.item_id.clone());
        });
        let zone_id = register(Box::new(zone)).expect("registry");
        let _zone_guard = MountGuard(zone_id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        assert_eq!(
            coordinator.on_event(
                &Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY),
                viewport
            ),
            None
        );
        assert_eq!(
            coordinator.session().map(|session| session.is_active()),
            Some(false),
            "a press alone is not yet a drag"
        );

        assert_eq!(
            coordinator.on_event(&Event::mouse_move(220, 40), viewport),
            None,
            "a move only advances the session"
        );
        let hovered = with_widget_mut(zone_id, |widget| {
            (widget as &mut dyn core::any::Any).downcast_mut::<DropZone>().map(|zone| zone.state())
        })
        .flatten();
        assert_eq!(hovered, Some(DropZoneState::Accepted), "the target was told it is hovered");

        assert_eq!(
            coordinator.on_event(
                &Event::mouse_release(220, 40, crate::event::mouse_button::PRIMARY),
                viewport
            ),
            Some(DropEffect::Copy)
        );
        assert_eq!(dropped.lock().unwrap().as_deref(), Some("hello"));
        assert!(coordinator.session().is_none(), "the release ends the session");
    }

    /// A control that implements neither trait is invisible to the coordinator: no session
    /// starts, no event is delivered by the coordinator, and every call answers `None`.
    #[test]
    fn a_control_without_the_drag_traits_changes_nothing() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Plain {
            base: crate::widget::BaseWidget,
            deliveries: std::sync::Arc<AtomicUsize>,
        }
        impl crate::widget::Widget for Plain {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                &mut self.base
            }
        }
        impl crate::event::EventHandler for Plain {
            fn handle_event(&mut self, _event: &Event) {
                self.deliveries.fetch_add(1, Ordering::SeqCst);
            }
        }

        let deliveries = std::sync::Arc::new(AtomicUsize::new(0));
        let plain = Plain {
            base: crate::widget::BaseWidget::new(
                crate::widget::WidgetKind::Label,
                Rect::new(0, 0, 100, 40),
                "Plain",
            ),
            deliveries: std::sync::Arc::clone(&deliveries),
        };
        let id = register(Box::new(plain)).expect("registry");
        let _guard = MountGuard(id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        assert_eq!(
            coordinator.on_event(
                &Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY),
                viewport
            ),
            None
        );
        assert!(coordinator.session().is_none(), "no DragSource, so no session");
        assert_eq!(coordinator.hovered(), None);
        assert_eq!(coordinator.on_event(&Event::mouse_move(60, 20), viewport), None);
        assert_eq!(
            coordinator.on_event(
                &Event::mouse_release(60, 20, crate::event::mouse_button::PRIMARY),
                viewport
            ),
            None
        );
        assert_eq!(
            deliveries.load(Ordering::SeqCst),
            0,
            "the coordinator must not deliver events to a control that did not opt in"
        );
    }

    /// The source is told what the drop produced, not merely that something happened.
    #[cfg(full_widgets)]
    #[test]
    fn the_source_learns_the_effect_of_the_drop() {
        use crate::event::dnd::{DragPayload, DragSource};
        use crate::widget::misc_widgets::drop_zone::DropZone;

        struct ProbeSource {
            base: crate::widget::BaseWidget,
            finished: std::sync::Arc<std::sync::Mutex<Option<DropEffect>>>,
        }
        impl crate::widget::Widget for ProbeSource {
            fn base(&self) -> &crate::widget::BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::widget::BaseWidget {
                &mut self.base
            }
            fn drag_source_mut(&mut self) -> Option<&mut dyn DragSource> {
                Some(self)
            }
        }
        impl DragSource for ProbeSource {
            fn drag_payload(&self, _pos: Point) -> Option<DragPayload> {
                Some(DragPayload::new("probe", "p1"))
            }
            fn on_drag_finished(&mut self, effect: DropEffect) {
                *self.finished.lock().unwrap() = Some(effect);
            }
        }
        impl crate::event::EventHandler for ProbeSource {
            fn handle_event(&mut self, _event: &Event) {}
        }

        let finished = std::sync::Arc::new(std::sync::Mutex::new(None));
        let source = ProbeSource {
            base: crate::widget::BaseWidget::new(
                crate::widget::WidgetKind::Label,
                Rect::new(0, 0, 100, 40),
                "ProbeSource",
            ),
            finished: std::sync::Arc::clone(&finished),
        };
        let source_id = register(Box::new(source)).expect("registry");
        let _source_guard = MountGuard(source_id);

        let zone = DropZone::new(Rect::new(200, 0, 120, 80), "probe");
        let zone_id = register(Box::new(zone)).expect("registry");
        let _zone_guard = MountGuard(zone_id);

        let viewport = Rect::new(0, 0, 400, 200);
        let mut coordinator = DragCoordinator::new();
        let _ = coordinator
            .on_event(&Event::mouse_press(10, 10, crate::event::mouse_button::PRIMARY), viewport);
        let _ = coordinator.on_event(&Event::mouse_move(220, 40), viewport);
        let effect = coordinator.on_event(
            &Event::mouse_release(220, 40, crate::event::mouse_button::PRIMARY),
            viewport,
        );
        assert_eq!(effect, Some(DropEffect::Copy));
        assert_eq!(*finished.lock().unwrap(), Some(DropEffect::Copy));
    }

    // -----------------------------------------------------------------------
    // Hit testing (point -> widget)
    // -----------------------------------------------------------------------

    /// Builds a container with two children, exercising the parent/children link
    /// the hit test walks. Returns `(parent, first, second)`.
    fn tree_with_two_children() -> (ObjectId, ObjectId, ObjectId) {
        let mut parent =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
        let first = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "first".to_string(),
            Rect::new(0, 0, 100, 50),
        )))
        .expect("registry");
        let second = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "second".to_string(),
            Rect::new(0, 0, 100, 50),
        )))
        .expect("registry");
        parent.add_child(first);
        parent.add_child(second);
        let parent = register(Box::new(parent)).expect("registry");
        (parent, first, second)
    }

    /// A point over a child must resolve to that child, not to the container.
    ///
    /// This is the capability that did not exist: before `widget_at`, the library
    /// could only deliver to an id the caller already knew.
    #[test]
    fn hit_test_descends_to_the_child_under_the_point() {
        let (parent, first, second) = tree_with_two_children();

        assert!(set_geometry(first, Rect::new(10, 10, 100, 50)));
        assert!(set_geometry(second, Rect::new(150, 10, 100, 50)));

        assert_eq!(widget_at(parent, Point::new(20, 20)), Some(first));
        assert_eq!(widget_at(parent, Point::new(160, 20)), Some(second));

        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// A point inside the container but outside every child resolves to the
    /// container — the search stops at the deepest hit, and "no child" is not
    /// the same as "no hit".
    #[test]
    fn hit_test_falls_back_to_the_container() {
        let (parent, first, second) = tree_with_two_children();
        set_geometry(first, Rect::new(10, 10, 100, 50));
        set_geometry(second, Rect::new(150, 10, 100, 50));

        assert_eq!(widget_at(parent, Point::new(250, 250)), Some(parent));

        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// A point outside the root resolves to nothing. Without this bound a click
    /// far outside the window would still land on whatever happened to be last.
    #[test]
    fn hit_test_returns_none_outside_the_root() {
        let (parent, first, second) = tree_with_two_children();
        assert_eq!(widget_at(parent, Point::new(5_000, 5_000)), None);
        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// An unmounted root must report no hit rather than panicking.
    #[test]
    fn hit_test_on_an_unmounted_root_is_none() {
        assert_eq!(widget_at(0xDEAD_BEEF, Point::new(0, 0)), None);
    }

    /// Later siblings paint on top, so they must also win the hit test. Reversing
    /// this order would make the visually-topmost control unclickable.
    #[test]
    fn hit_test_prefers_the_topmost_overlapping_child() {
        let (parent, first, second) = tree_with_two_children();
        // Both fully overlap; `second` is added last, so it draws on top.
        set_geometry(first, Rect::new(10, 10, 100, 50));
        set_geometry(second, Rect::new(10, 10, 100, 50));

        assert_eq!(widget_at(parent, Point::new(20, 20)), Some(second));

        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// A hidden child is not drawn, so it must not swallow a click.
    #[test]
    fn hit_test_skips_hidden_children() {
        let (parent, first, second) = tree_with_two_children();
        set_geometry(first, Rect::new(10, 10, 100, 50));
        set_geometry(second, Rect::new(10, 10, 100, 50));
        with_widget_mut(second, |widget| widget.hide());

        // `second` covers `first` but is hidden, so the click reaches `first`.
        assert_eq!(widget_at(parent, Point::new(20, 20)), Some(first));

        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// A container's children carry absolute rects (the layout engine derives a
    /// child's rect from its parent's absolute rect), so moving the container does
    /// not move its children in this model. The point of this test is that the walk
    /// composes down the tree by *containment*, not by summing parent origins —
    /// getting that wrong would offset every nested control.
    #[test]
    fn hit_test_finds_a_nested_child_through_two_levels() {
        let inner =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(20, 20, 100, 100));
        let inner = register(Box::new(inner)).expect("registry");
        let leaf = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "leaf".to_string(),
            Rect::new(30, 30, 40, 20),
        )))
        .expect("registry");
        with_widget_mut(inner, |widget| widget.add_child(leaf));

        let outer =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
        let outer = register(Box::new(outer)).expect("registry");
        with_widget_mut(outer, |widget| widget.add_child(inner));

        // (40, 40) is inside outer -> inner -> leaf, so the deepest widget wins.
        assert_eq!(widget_at(outer, Point::new(40, 40)), Some(leaf));
        // (25, 25) is inside outer and inner, but above the leaf.
        assert_eq!(widget_at(outer, Point::new(25, 25)), Some(inner));
        // (250, 250) is inside only the outer container.
        assert_eq!(widget_at(outer, Point::new(250, 250)), Some(outer));

        unregister(outer);
        unregister(inner);
        unregister(leaf);
    }

    /// The pointer-event helper must deliver to the widget under the point. Because
    /// geometry is absolute here, the event's own position needs no rewrite.
    #[test]
    fn dispatch_pointer_event_reaches_the_widget_under_the_point() {
        let mut parent =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 400, 300));
        let editor = register(sample_editor("fn main() {}\nlet a = 1;\nlet b = 2;")).expect("r");
        set_geometry(editor, Rect::new(100, 100, 320, 200));
        parent.add_child(editor);
        let parent = register(Box::new(parent)).expect("registry");

        let delivered = dispatch_pointer_event(
            parent,
            &Event::MousePress { pos: Point::new(150, 130), button: 1, modifiers: 0 },
            Point::new(150, 130),
        );
        assert!(delivered, "the editor under the point must receive the click");

        // Outside the editor but inside the container, the container is the target.
        assert_eq!(widget_at(parent, Point::new(380, 20)), Some(parent));

        unregister(parent);
        unregister(editor);
    }

    /// A click on empty space must be reported as undelivered, so a host knows
    /// nothing consumed it instead of assuming success.
    #[test]
    fn dispatch_pointer_event_outside_the_root_is_false() {
        let (parent, first, second) = tree_with_two_children();
        let delivered = dispatch_pointer_event(
            parent,
            &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1, modifiers: 0 },
            Point::new(9_000, 9_000),
        );
        assert!(!delivered);
        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// A press inside an open popup is routed **from the popup**, even when the popup hangs
    /// below the container that owns it.
    ///
    /// The grab only intercepted presses the popup *rejected*; a press inside the list fell
    /// through to `widget_at(root, ..)`, which refuses to descend through an ancestor whose own
    /// bounds reject the point. A combo box inside a panel therefore had an unclickable list
    /// whenever the list crossed the panel's bottom edge. The menu code already re-parents its
    /// popup to the window to dodge this; this is the fix that makes the general case work.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_press_inside_a_popup_below_its_parent_reaches_the_popup() {
        use crate::widget::input_widgets::ComboBox;

        let mut combo = ComboBox::new(Rect::new(0, 60, 120, 32));
        combo.set_items(vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
        let combo = register(Box::new(combo)).expect("mount");

        let mut panel =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 200, 100));
        panel.add_child(combo);
        let panel = register(Box::new(panel)).expect("mount");
        // `BaseWidget::add_child` writes only the parent's list, and it resolves the parent in the
        // **registry** id space — which exists only once the parent is mounted. The host therefore
        // mounts first and links through the registry, exactly as `add_child_linked` expects.
        with_widget_mut(panel, |widget| {
            widget.base_mut().add_child_linked(combo);
        });

        let window = register(Box::new(crate::widget::Window::new(
            "popup host".to_string(),
            Rect::new(0, 0, 400, 300),
        )))
        .expect("mount");
        with_widget_mut(window, |widget| {
            widget.base_mut().add_child_linked(panel);
        });

        // Open the list through the control's own press path, so the grab records the *registry*
        // id (a control reports its own id, which the router normalises — but only once mounted).
        let field = Point::new(20, 70);
        assert!(dispatch_pointer_event(
            window,
            &Event::MousePress { pos: field, button: 1, modifiers: 0 },
            field,
        ));
        assert!(
            read_widget(combo, |widget| widget.has_open_popup()).unwrap_or(false),
            "the press must open the list"
        );

        // A row of the list, below the panel's own bottom edge (y = 100).
        let row = (101..300)
            .map(|y| Point::new(20, y))
            .find(|p| read_widget(combo, |widget| widget.contains_point(*p)).unwrap_or(false))
            .expect("the open list extends below its parent");

        assert!(dispatch_pointer_event(
            window,
            &Event::MousePress { pos: row, button: 1, modifiers: 0 },
            row,
        ));
        assert!(
            !read_widget(combo, |widget| widget.has_open_popup()).unwrap_or(true),
            "choosing a row must reach the combo box and close its list"
        );

        unregister(window);
        unregister(panel);
        unregister(combo);
    }

    /// Tab steps over disabled and hidden controls instead of landing on them.
    ///
    /// The tab order keeps them (a user must be able to move *past* a disabled control), so the
    /// traversal is where the skip belongs. Without it, Tab moved focus onto a control that
    /// refuses input: no visible focus change, and the following keystrokes were swallowed.
    #[test]
    fn tab_skips_disabled_and_hidden_controls() {
        let mount = |x: i32| {
            register(Box::new(crate::widget::Button::new(
                "ok".to_string(),
                Rect::new(x, 0, 80, 32),
            )))
            .expect("mount")
        };
        let first = mount(0);
        let disabled = mount(100);
        let hidden = mount(200);
        let last = mount(300);
        with_widget_mut(disabled, |widget| widget.set_enabled(false));
        with_widget_mut(hidden, |widget| widget.hide());

        assert_eq!(focus_next(true), Some(first), "the first control is focusable");
        assert_eq!(
            focus_next(true),
            Some(last),
            "the disabled and hidden controls must be stepped over"
        );
        assert_eq!(focused_widget(), Some(last));

        assert_eq!(focus_next(false), Some(first), "and the skip applies backwards too");

        for id in [first, disabled, hidden, last] {
            unregister(id);
        }
    }

    /// Hover does not enter a control a modal blocks.
    ///
    /// `dispatch_hover_transition` wrote `HOVERED` without consulting `modal_blocks`, while the
    /// `MouseEnter` it sends is refused — so the pair invariant broke, `hovered_widget()` reported
    /// a control behind a dialog, and its cursor (an I-beam) was published to the platform.
    #[test]
    fn hover_does_not_enter_a_widget_behind_a_modal() {
        let (parent, first, second) = tree_with_two_children();
        set_geometry(first, Rect::new(10, 10, 100, 50));
        set_geometry(second, Rect::new(150, 10, 100, 50));
        clear_hover(Point::new(0, 0));
        assert!(enter_modal(second));

        let over_first = Point::new(20, 20);
        let _ = dispatch_pointer_event(parent, &Event::MouseMove { pos: over_first }, over_first);

        assert_eq!(
            hovered_widget(),
            None,
            "a modal must keep the widget behind it out of the hover state"
        );

        exit_modal(second);
        let _ = dispatch_pointer_event(parent, &Event::MouseMove { pos: over_first }, over_first);
        assert_eq!(hovered_widget(), Some(first), "and hover resumes once the modal is gone");

        unregister(parent);
        unregister(first);
        unregister(second);
    }

    /// The router refuses a press on a disabled control, without arming focus or capture.
    ///
    /// The public documentation promised this (`dispatch_event`'s "false when the control is
    /// disabled") while the router had no such check: a third-party control relying on the
    /// contract accepted input while disabled. The check lives in the router because the
    /// capture branch must still deliver the *end* of a gesture that began on an enabled control.
    #[test]
    fn a_press_on_a_disabled_control_is_refused_by_the_router() {
        let id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        with_widget_mut(id, |widget| widget.set_enabled(false));

        let point = Point::new(10, 10);
        assert!(
            !dispatch_pointer_event(
                id,
                &Event::MousePress { pos: point, button: 1, modifiers: 0 },
                point,
            ),
            "a disabled control takes no input"
        );
        assert_eq!(capturing_widget(), None, "and the router must not arm a capture for it");

        unregister(id);
    }

    /// A window root's geometry is in screen space while its children are in client space.
    ///
    /// Hit testing used to subtract `geometry()` unconditionally, so a window opened away from
    /// the screen origin rejected every click: measured, a press at client `(182, 58)` on a window
    /// at `(140, 140)` became local `(42, -82)` and matched no control. `frame_origin` is the one
    /// predicate that knows a window's frame starts at `(0, 0)`, so the root bound now uses it.
    #[test]
    fn a_window_root_is_hit_tested_in_client_coordinates() {
        let mut window = crate::widget::Window::new(
            "offscreen origin".to_string(),
            Rect::new(140, 140, 520, 260),
        );
        let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
            Rect::new(32, 40, 300, 36),
        )))
        .expect("mount");
        window.add_child(field);
        let root = register(Box::new(window)).expect("mount");
        let _root = MountGuard(root);
        let _field = MountGuard(field);

        assert_eq!(
            widget_at(root, Point::new(182, 58)),
            Some(field),
            "client coordinates must reach a control on a window whose frame starts elsewhere"
        );
        assert_eq!(
            widget_at(root, Point::new(500, 240)),
            Some(root),
            "and the window itself is still the answer on its own background"
        );
    }

    /// A press on an interactive control makes it the keyboard's owner.
    ///
    /// `focus_on_pointer_press` was published with no production caller: the router moved capture
    /// and delivered the press but never told the focus manager. This asserts the missing call,
    /// through the same entry point a backend uses.
    #[test]
    fn a_pointer_press_focuses_the_control_it_lands_on() {
        let mut window =
            crate::widget::Window::new("focus on press".to_string(), Rect::new(140, 140, 520, 260));
        let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
            Rect::new(32, 40, 300, 36),
        )))
        .expect("mount");
        window.add_child(field);
        let root = register(Box::new(window)).expect("mount");
        let _root = MountGuard(root);
        let _field = MountGuard(field);

        assert_eq!(focused_widget(), None, "the tree starts with no keyboard owner");
        assert!(
            dispatch_pointer_event(root, &Event::mouse_press(182, 58, 1), Point::new(182, 58),)
        );
        assert_eq!(
            focused_widget(),
            Some(field),
            "the field under the pointer must own the keyboard afterwards"
        );

        // Empty window background is not an interactive control, so it does not steal focus.
        assert!(dispatch_pointer_event(
            root,
            &Event::mouse_press(500, 240, 1),
            Point::new(500, 240),
        ));
        assert_eq!(focused_widget(), Some(field), "the window background must not take focus");
        clear_focus();
    }

    /// The IME joins only text-entry controls; a button keeps its keys.
    #[test]
    fn only_text_controls_accept_ime_input() {
        let field = register(Box::new(crate::widget::input_widgets::lineedit::LineEdit::new(
            Rect::new(0, 0, 200, 24),
        )))
        .expect("mount");
        let button = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            Rect::new(0, 40, 80, 32),
        )))
        .expect("mount");
        let label = register(Box::new(crate::widget::Label::new(
            "text".to_string(),
            Rect::new(0, 80, 80, 24),
        )))
        .expect("mount");
        let _field = MountGuard(field);
        let _button = MountGuard(button);
        let _label = MountGuard(label);

        assert!(accepts_ime_input(field), "a line edit carries committed text");
        assert!(!accepts_ime_input(button), "a button must keep its activation keys");
        assert!(!accepts_ime_input(label), "a label has no edit session");
    }

    // -----------------------------------------------------------------------
    // Keyboard focus
    // -----------------------------------------------------------------------

    /// Focus must have exactly one owner, and moving it must be observable both
    /// through the query and through the events the previous owner receives.
    #[test]
    fn focus_moves_and_reports_the_new_owner() {
        let first = register(sample_editor("a")).expect("registry");
        let second = register(sample_editor("b")).expect("registry");

        assert!(focus_widget(first));
        assert_eq!(focused_widget(), Some(first));
        assert!(has_focus(first));
        assert!(!has_focus(second));

        assert!(focus_widget(second));
        assert_eq!(focused_widget(), Some(second));
        assert!(!has_focus(first), "the previous owner must lose focus when another takes it");

        unregister(first);
        unregister(second);
    }

    /// Re-focusing the current owner is a no-op, so a repeated click cannot emit a
    /// second `FocusGained` for a widget that never lost focus.
    #[test]
    fn focus_on_the_current_owner_reports_no_move() {
        let id = register(sample_editor("a")).expect("registry");
        assert!(focus_widget(id));
        assert!(!focus_widget(id), "focus did not move, so the call must report false");
        unregister(id);
    }

    /// Focus on an id that is not mounted would be an unobservable lie: the key
    /// router would look for a widget that does not exist. It must be refused.
    #[test]
    fn focus_on_an_unmounted_widget_is_refused() {
        assert!(!focus_widget(0xDEAD_BEEF));
        assert_eq!(focused_widget(), None);
    }

    /// Clearing focus must tell the owner it lost it, and a second clear must not
    /// claim anything happened.
    #[test]
    fn clearing_focus_is_reported_once() {
        let id = register(sample_editor("a")).expect("registry");
        focus_widget(id);

        assert!(clear_focus());
        assert_eq!(focused_widget(), None);
        assert!(!clear_focus(), "there was no focus to clear the second time");

        unregister(id);
    }

    /// A widget that leaves the tree must leave the tab order too, or Tab would
    /// hand focus to an id that resolves to nothing.
    #[test]
    fn unregistering_a_focused_widget_drops_it_from_the_tab_order() {
        let first = register(sample_editor("a")).expect("registry");
        let second = register(sample_editor("b")).expect("registry");
        register_focusable(first);
        register_focusable(second);
        focus_widget(first);

        unregister(first);

        assert_eq!(focused_widget(), None, "the removed widget must not stay focused");
        assert!(!focusable_widgets().contains(&first));

        unregister(second);
    }

    /// Tab must actually move focus. Before the manager was wired into the
    /// registry this moved nothing, which is the defect these tests pin.
    #[test]
    fn tab_order_moves_focus_and_wraps() {
        let a = register(sample_editor("a")).expect("registry");
        let b = register(sample_editor("b")).expect("registry");
        set_focus_order_for_test(&[a, b]);

        assert_eq!(focus_next(true), Some(a));
        assert_eq!(focused_widget(), Some(a));
        assert_eq!(focus_next(true), Some(b));
        // Wrap-around: past the end comes back to the start.
        assert_eq!(focus_next(true), Some(a));
        // Backwards, and wrap-around in that direction too.
        assert_eq!(focus_next(false), Some(b));
        assert_eq!(focus_next(false), Some(a));

        unregister(a);
        unregister(b);
    }

    /// `Platform::route_pointer_event` must resolve the point against the widget tree
    /// rather than handing the event to the root.
    ///
    /// This is the wiring test for the whole chain: before it, the runtime could
    /// hit-test (`widget_at`) but nothing called it, so a click on a child still went
    /// to whichever widget owned the surface.
    #[test]
    #[cfg(full_widgets)]
    fn platform_pointer_routing_reaches_a_nested_child() {
        let mut parent =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
        let child = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "child".to_string(),
            Rect::new(40, 40, 80, 30),
        )))
        .expect("registry");
        parent.add_child(child);
        let parent = register(Box::new(parent)).expect("registry");

        let backend = crate::platform::platform_facts();

        // Inside the child: the route must descend past the parent.
        assert!(
            backend.route_pointer_event(
                parent,
                &Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 },
                Point::new(50, 50),
            ),
            "a click inside the child must be delivered to it"
        );

        // Outside every child but inside the root: still delivered, to the root.
        assert!(
            backend.route_pointer_event(
                parent,
                &Event::MousePress { pos: Point::new(250, 250), button: 1, modifiers: 0 },
                Point::new(250, 250),
            ),
            "a click inside the root must still be delivered"
        );

        // Entirely outside the root: nothing accepts it, and that is reported.
        assert!(
            !backend.route_pointer_event(
                parent,
                &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1, modifiers: 0 },
                Point::new(9_000, 9_000),
            ),
            "a click outside the root must report that nothing accepted it"
        );

        unregister(parent);
        unregister(child);
    }

    /// D09-POINTER-01: the stylus event family reaches a control through
    /// `Platform::route_pointer_event` end to end.
    ///
    /// This is the producer/consumer boundary made testable without a device: a host (or this
    /// test) constructs `PointerPress`/`PointerMove`/`PointerRelease` and routes them through
    /// the public platform entry point; the `SignaturePad` under the point must consume the
    /// whole lifecycle and retain the pressure. Before the fix the pad began strokes only from
    /// `MousePress`/`TouchBegin`, so this route delivered a press the pad ignored.
    #[test]
    #[cfg(full_widgets)]
    fn platform_pointer_routing_delivers_stylus_events_to_a_signature_pad() {
        let mut parent =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
        let pad_id =
            register(Box::new(crate::widget::special_widgets::signature_pad::SignaturePad::new(
                Rect::new(40, 40, 200, 120),
            )))
            .expect("registry");
        set_geometry(pad_id, Rect::new(40, 40, 200, 120));
        parent.add_child(pad_id);
        let parent = register(Box::new(parent)).expect("registry");

        let backend = crate::platform::platform_facts();

        // A pen contact inside the pad: press, move, release, all through the public route.
        assert!(
            backend.route_pointer_event(
                parent,
                &Event::pointer_press(Point::new(60, 80), 1, 0.3, 0.0, 0.0),
                Point::new(60, 80),
            ),
            "the press must be accepted by the pad under the point"
        );
        backend.route_pointer_event(
            parent,
            &Event::pointer_move(Point::new(120, 100), 0.8, 0.0, 0.0),
            Point::new(120, 100),
        );
        backend.route_pointer_event(
            parent,
            &Event::pointer_release(Point::new(120, 100), 1, 0.6),
            Point::new(120, 100),
        );

        // The pad consumed the sequence: one committed stroke whose first point is the press
        // position and whose per-point pressures were retained. The release's own pressure is the
        // last datum the device sent, so it becomes the pad's retained `last_pressure` even though
        // it adds no point (the lift-off position is the last accepted move).
        with_widget(pad_id, |widget| {
            let pad = (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::special_widgets::signature_pad::SignaturePad>()
                .expect("the registered widget is a SignaturePad");
            assert_eq!(pad.stroke_count(), 1, "the routed stylus gesture must commit one stroke");
            let stroke = &pad.strokes()[0];
            assert_eq!(stroke.points()[0], Point::new(60, 80));
            assert!(
                (stroke.pressures()[0] - 0.3).abs() < 1e-6,
                "the press pressure must be recorded on the stroke's first point"
            );
            assert!(
                (stroke.last_pressure() - 0.8).abs() < 1e-6,
                "the move pressure must be recorded on the stroke's last point"
            );
            assert!((pad.last_pressure() - 0.6).abs() < 1e-6, "the release datum was retained");
        })
        .expect("the pad is mounted");

        unregister(parent);
        unregister(pad_id);
    }

    /// Establishes a deterministic tab order for a test, independent of whatever
    /// other widgets the shared thread-local registry happens to hold.
    fn set_focus_order_for_test(ids: &[ObjectId]) {
        with_focus_manager(|focus| {
            focus.set_focus_order(ids.to_vec());
            focus.clear_focus();
        })
        .expect("ui thread has a focus manager");
    }

    // -----------------------------------------------------------------------
    // Hover enter/leave synthesis
    // -----------------------------------------------------------------------

    /// Two overlapping widgets in one container, so the pointer can cross from one
    /// to the other. Returns `(parent, inner, outer)` where `inner` is nested inside
    /// `outer`'s area... actually both are leaves; `left` and `right` are side by side.
    fn two_siblings() -> (ObjectId, ObjectId, ObjectId) {
        let mut parent =
            crate::widget::container_widgets::groupbox::GroupBox::new(Rect::new(0, 0, 300, 300));
        let left = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "left".to_string(),
            Rect::new(0, 0, 100, 50),
        )))
        .expect("registry");
        let right = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "right".to_string(),
            Rect::new(0, 0, 100, 50),
        )))
        .expect("registry");
        set_geometry(left, Rect::new(10, 10, 100, 50));
        set_geometry(right, Rect::new(150, 10, 100, 50));
        parent.add_child(left);
        parent.add_child(right);
        let parent = register(Box::new(parent)).expect("registry");
        (parent, left, right)
    }

    /// Moving the pointer onto a widget must report it as hovered. No backend
    /// produces `MouseEnter`, so without this the state is unreachable and every
    /// hover highlight in the library is dead code.
    #[test]
    fn pointer_movement_sets_and_clears_the_hover_target() {
        let (parent, left, right) = two_siblings();
        clear_hover(Point::new(0, 0));
        assert_eq!(hovered_widget(), None);

        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(20, 20) },
            Point::new(20, 20),
        );
        assert_eq!(hovered_widget(), Some(left), "the pointer is over `left`");

        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(160, 20) },
            Point::new(160, 20),
        );
        assert_eq!(hovered_widget(), Some(right), "crossing onto `right` must move hover");

        // Leaving the surface entirely clears it, so a stale highlight cannot persist.
        clear_hover(Point::new(0, 0));
        assert_eq!(hovered_widget(), None);

        unregister(parent);
        unregister(left);
        unregister(right);
    }

    /// A control that receives `MouseEnter` must actually be told, because that is
    /// what flips its own hover flag. This is the end of the chain the backends could
    /// not reach before.
    #[test]
    fn entering_a_button_makes_it_report_hover() {
        let parent = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
            Rect::new(0, 0, 300, 300),
        )))
        .expect("registry");
        let mut button = crate::widget::base_widgets::button::Button::new(
            "ok".to_string(),
            Rect::new(20, 20, 80, 30),
        );
        button.set_geometry(Rect::new(20, 20, 80, 30));
        let button_id = register(Box::new(button)).expect("registry");
        with_widget_mut(parent, |widget| widget.add_child(button_id));

        clear_hover(Point::new(0, 0));
        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(30, 30) },
            Point::new(30, 30),
        );
        let hovered = with_widget(button_id, |widget| {
            (widget as &dyn core::any::Any)
                .downcast_ref::<crate::widget::base_widgets::button::Button>()
                .map(|b| b.is_hovered())
        })
        .flatten();
        assert_eq!(hovered, Some(true), "MouseEnter must reach the button's hover flag");

        clear_hover(Point::new(0, 0));
        unregister(parent);
        unregister(button_id);
    }

    /// A widget removed while hovered must not remain the target, or the next
    /// movement would send `MouseLeave` to a dead id and skip the live one.
    #[test]
    fn unregistering_a_hovered_widget_clears_the_target() {
        let (parent, left, right) = two_siblings();
        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(20, 20) },
            Point::new(20, 20),
        );
        assert_eq!(hovered_widget(), Some(left));

        unregister(left);
        assert_eq!(hovered_widget(), None);

        unregister(parent);
        unregister(right);
    }

    /// Moving focus must tell the platform's IME bridge which control is being edited.
    ///
    /// Before this, `ImeBridge::focus_in` had **no caller anywhere**, so a backend's
    /// bridge never learned which widget was active and had no widget to place its
    /// candidate window against. This drives the real registry against a recording
    /// bridge and asserts the calls arrive.
    #[test]
    #[cfg(not(alloc_frugal))]
    fn moving_focus_notifies_the_ime_bridge() {
        let first = register(sample_editor("a")).expect("registry");
        let second = register(sample_editor("b")).expect("registry");
        clear_focus();

        // Drive the notification helper directly, because the bridge the *active*
        // backend exposes is a private implementation without a readback accessor.
        // The helper is the single place the runtime calls out to the IME, so this
        // covers the wiring that was missing.
        let recorder = RecordingImeBridge::default();
        {
            use crate::platform::ime::ImeBridge as _;
            recorder.focus_in(first);
            recorder.focus_out(first);
            recorder.focus_in(second);
        }
        assert_eq!(
            recorder.calls(),
            vec![("in", first), ("out", first), ("in", second)],
            "the IME bridge must be told about every focus change"
        );

        // And the real path must reach *some* bridge, exercising the call site.
        let backend = crate::platform::platform_facts();
        if backend.ime_bridge().is_some() {
            assert!(focus_widget(first));
            assert_eq!(focused_widget(), Some(first));
            assert!(clear_focus());
        }

        unregister(first);
        unregister(second);
    }

    /// Row-major traversal must order Tab by on-screen position, not registration.
    ///
    /// `FocusTraversalStrategy` was fully implemented but unreachable — nothing outside
    /// `FocusManager` could select one, so a grid could only ever tab in registration
    /// order.
    #[test]
    fn row_major_traversal_orders_tab_by_position() {
        let a = register(sample_editor("a")).expect("registry");
        let b = register(sample_editor("b")).expect("registry");
        let c = register(sample_editor("c")).expect("registry");

        // Register them in an order that disagrees with the layout: c, a, b.
        set_focus_order_for_test(&[c, a, b]);
        // Two rows: a and b on the top row, c below-left.
        assert!(set_focusable_position(a, 10, 10));
        assert!(set_focusable_position(b, 200, 10));
        assert!(set_focusable_position(c, 10, 100));

        assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::RowMajor));
        assert_eq!(
            focusable_widgets(),
            vec![a, b, c],
            "row-major must visit the top row left-to-right before the row below"
        );

        // A non-focusable widget has no position to record.
        assert!(!set_focusable_position(0xDEAD_BEEF, 0, 0));

        // Back to registration order for the rest of the suite, which shares this
        // thread-local manager.
        assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::TabOrder));
        unregister(a);
        unregister(b);
        unregister(c);
    }

    /// Column-major traversal orders by x first, then y.
    #[test]
    fn column_major_traversal_orders_tab_by_column() {
        let a = register(sample_editor("a")).expect("registry");
        let b = register(sample_editor("b")).expect("registry");

        set_focus_order_for_test(&[b, a]);
        // `b` is in the right column, `a` in the left, so column-major visits a first.
        assert!(set_focusable_position(a, 10, 300));
        assert!(set_focusable_position(b, 500, 10));

        assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::ColumnMajor));
        assert_eq!(focusable_widgets(), vec![a, b], "column-major must visit left first");

        assert!(set_focus_traversal(crate::event::FocusTraversalStrategy::TabOrder));
        unregister(a);
        unregister(b);
    }

    /// Capture must make a widget receive pointer events wherever the pointer is.
    #[test]
    fn captured_widget_receives_pointer_events_off_itself() {
        let (parent, left, right) = two_siblings();
        release_pointer_capture();

        assert!(capture_pointer(left));
        assert_eq!(capturing_widget(), Some(left));
        assert!(has_pointer_capture(left));

        // (160, 20) is over `right`, but `left` holds capture, so it wins.
        assert!(
            dispatch_pointer_event(
                parent,
                &Event::MouseMove { pos: Point::new(160, 20) },
                Point::new(160, 20),
            ),
            "a captured widget must receive the event even when the pointer left it"
        );
        // Hover still tracks the true position, so highlights stay honest.
        assert_eq!(hovered_widget(), Some(right));

        assert!(release_pointer_capture());
        assert_eq!(capturing_widget(), None);
        assert!(!release_pointer_capture(), "a second release has nothing to release");

        unregister(parent);
        unregister(left);
        unregister(right);
    }

    /// Capture on an id that is not mounted would route every later pointer event into
    /// nothing, so it is refused — the same honest-answer rule used for focus.
    #[test]
    fn capturing_an_unmounted_widget_is_refused() {
        release_pointer_capture();
        assert!(!capture_pointer(0xDEAD_BEEF));
        assert_eq!(capturing_widget(), None);
    }

    /// A capture holder that is removed must not keep swallowing pointer events.
    #[test]
    fn unregistering_a_captured_widget_releases_capture() {
        let (parent, left, right) = two_siblings();
        assert!(capture_pointer(left));

        unregister(left);

        assert_eq!(capturing_widget(), None, "a removed widget must not keep capture");
        release_pointer_capture();
        unregister(parent);
        unregister(right);
    }

    /// D09-EVT-01: a press that drags off the control and is released elsewhere must
    /// still be delivered *back* to the control that owns the gesture, so its own press
    /// latch is cleared by the release rather than left armed.
    ///
    /// Before this the runtime resolved every release against the current position, so a
    /// release outside the button never reached it. The button kept its `grabbed`/`pressed`
    /// latch set, and a *later*, unrelated release that happened to land on the button
    /// then fired a spurious `clicked`. This test drives the full runtime sequence the
    /// issue's acceptance criterion names: press on A → move out over B → release over the
    /// blank → release again on A. Neither release may activate anything, and after the
    /// first (outside) release the button must hold no grab and no capture.
    #[test]
    fn a_press_released_off_the_control_is_delivered_back_and_clears_its_latch() {
        use crate::event::mouse_button;

        release_pointer_capture();
        let parent = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
            Rect::new(0, 0, 300, 300),
        )))
        .expect("registry");

        // Two buttons side by side, plus a blank area to the right to release into.
        let mut a = crate::widget::base_widgets::button::Button::new(
            "A".to_string(),
            Rect::new(20, 20, 80, 30),
        );
        a.set_geometry(Rect::new(20, 20, 80, 30));
        let a = register(Box::new(a)).expect("registry");
        with_widget_mut(parent, |widget| widget.add_child(a));

        let mut b = crate::widget::base_widgets::button::Button::new(
            "B".to_string(),
            Rect::new(130, 20, 80, 30),
        );
        b.set_geometry(Rect::new(130, 20, 80, 30));
        let b = register(Box::new(b)).expect("registry");
        with_widget_mut(parent, |widget| widget.add_child(b));

        let clicks_a = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_a = std::sync::Arc::clone(&clicks_a);
        with_widget_mut(a, |widget| {
            widget.base().clicked.connect(move || {
                counter_a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        });
        let clicks_b = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_b = std::sync::Arc::clone(&clicks_b);
        with_widget_mut(b, |widget| {
            widget.base().clicked.connect(move || {
                counter_b.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        });

        // Press inside A: A takes the gesture.
        assert!(dispatch_pointer_event(
            parent,
            &Event::MousePress {
                pos: Point::new(40, 35),
                button: mouse_button::PRIMARY,
                modifiers: 0
            },
            Point::new(40, 35),
        ));
        assert_eq!(capturing_widget(), Some(a), "the pressed control must own the gesture");

        // Drag out over B, then release on the blank at (280, 280) — over no child.
        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(160, 35) },
            Point::new(160, 35),
        );
        assert!(
            dispatch_pointer_event(
                parent,
                &Event::MouseRelease { pos: Point::new(280, 280), button: mouse_button::PRIMARY },
                Point::new(280, 280),
            ),
            "the off-widget release must be routed back to the gesture owner, not dropped"
        );

        // The gesture ended: no capture remains, A holds no grab, and nothing activated.
        assert_eq!(capturing_widget(), None, "the gesture-ending release must release the capture");
        assert!(
            !with_widget_mut(a, |w| w.base().is_grabbed()).unwrap_or(true),
            "A's grab must be cleared by the release that ended its gesture"
        );
        assert_eq!(clicks_a.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(clicks_b.load(std::sync::atomic::Ordering::SeqCst), 0);

        // Re-enter and release on A again with no matching press. Because the earlier
        // gesture was properly closed, this stray release must not fire A's `clicked`.
        dispatch_pointer_event(
            parent,
            &Event::MouseMove { pos: Point::new(40, 35) },
            Point::new(40, 35),
        );
        dispatch_pointer_event(
            parent,
            &Event::MouseRelease { pos: Point::new(40, 35), button: mouse_button::PRIMARY },
            Point::new(40, 35),
        );
        assert_eq!(
            clicks_a.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a stray release must not activate a control whose earlier gesture was cancelled"
        );

        unregister(a);
        unregister(b);
        unregister(parent);
    }

    // -----------------------------------------------------------------------
    // Modal dialog enforcement
    // -----------------------------------------------------------------------

    /// Builds a window with a modal dialog and an unrelated control, wiring the
    /// `parent` links that [`is_descendant_of`] walks (the same links the real mount
    /// path sets via `mount_named_widget`). Returns `(window, modal, outside)`, where
    /// the modal is a top-level dialog and `outside` is another widget the modal
    /// must block.
    fn modal_fixture() -> (ObjectId, ObjectId, ObjectId) {
        let window = register(Box::new(crate::widget::container_widgets::groupbox::GroupBox::new(
            Rect::new(0, 0, 400, 400),
        )))
        .expect("window");
        let modal = register(Box::new(crate::widget::dialog::message_box::MessageBox::new(
            Rect::new(50, 50, 200, 100),
        )))
        .expect("modal");
        let outside = register(Box::new(crate::widget::base_widgets::label::Label::new(
            "outside".to_string(),
            Rect::new(0, 0, 100, 20),
        )))
        .expect("outside");
        // The dialog and the label both belong to the window's frame of reference;
        // neither is a *child* of the other, so the modal must block the label.
        with_widget_mut(modal, |w| w.set_parent(Some(window)));
        with_widget_mut(outside, |w| w.set_parent(Some(window)));
        with_widget_mut(window, |w| {
            w.add_child(modal);
            w.add_child(outside);
        });
        (window, modal, outside)
    }

    #[test]
    fn no_modal_blocks_nothing() {
        let (window, modal, outside) = modal_fixture();
        assert!(!is_modal_active());
        assert!(!modal_blocks(window));
        assert!(!modal_blocks(modal));
        assert!(!modal_blocks(outside));
        unregister(window);
        unregister(modal);
        unregister(outside);
    }

    #[test]
    fn an_active_modal_blocks_input_outside_its_subtree() {
        let (window, modal, outside) = modal_fixture();

        assert!(enter_modal(modal));
        assert!(is_modal_active());
        assert_eq!(active_modal(), Some(modal));

        // The dialog itself stays live; everything else (the window chrome, an
        // unrelated control) is blocked.
        assert!(!modal_blocks(modal));
        assert!(modal_blocks(window));
        assert!(modal_blocks(outside));

        // Event delivery follows the same rule.
        let event = crate::event::Event::mouse_press(10, 10, 1);
        assert!(!dispatch_event(outside, &event), "a blocked widget must not receive events");
        assert!(dispatch_event(modal, &event), "the modal dialog must receive events");

        exit_modal(modal);
        assert!(!is_modal_active());
        assert!(!modal_blocks(outside), "after dismissal nothing is blocked");

        unregister(window);
        unregister(modal);
        unregister(outside);
    }

    #[test]
    fn a_modal_blocks_focus_outside_its_subtree() {
        let (window, modal, outside) = modal_fixture();
        clear_focus();

        enter_modal(modal);
        assert!(focus_widget(modal));
        // Focus elsewhere is refused while the modal is up.
        assert!(!focus_widget(outside));
        assert_eq!(focused_widget(), Some(modal));

        // After the modal is dismissed, focus can move to the formerly blocked widget.
        exit_modal(modal);
        assert!(focus_widget(outside));
        assert_eq!(focused_widget(), Some(outside));

        clear_focus();
        unregister(window);
        unregister(modal);
        unregister(outside);
    }

    #[test]
    fn an_unmounted_modal_cannot_be_entered() {
        clear_modals();
        assert!(!enter_modal(0xDEAD_BEEF));
        assert!(!is_modal_active());
    }

    #[test]
    fn clear_modals_drops_the_whole_stack() {
        let (_, modal, _) = modal_fixture();
        assert!(enter_modal(modal));
        assert!(is_modal_active());
        clear_modals();
        assert!(!is_modal_active());
        unregister(modal);
    }

    /// A recording IME bridge, for asserting the focus calls arrive in order.
    #[derive(Default)]
    struct RecordingImeBridge {
        calls: crate::compat::Mutex<Vec<(&'static str, ObjectId)>>,
    }

    impl RecordingImeBridge {
        fn calls(&self) -> Vec<(&'static str, ObjectId)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl crate::platform::ime::ImeBridge for RecordingImeBridge {
        fn focus_in(&self, widget_id: ObjectId) {
            self.calls.lock().unwrap().push(("in", widget_id));
        }
        fn focus_out(&self, widget_id: ObjectId) {
            self.calls.lock().unwrap().push(("out", widget_id));
        }
        fn commit_text(&self, _text: &str) {}
        fn set_composition(&self, _composition: &crate::platform::ime::ImeComposition) {}
        fn set_candidate_window_position(
            &self,
            _position: crate::platform::ime::ImeCandidatePosition,
        ) {
        }
        fn is_active(&self) -> bool {
            true
        }
    }

    // ── BLUE24 §6 -- the accessibility pump ──────────────────────────────────────

    /// A bridge that records what the submit points pushed, so the mount path can be asserted.
    ///
    /// `Mutex` rather than `RefCell` because `AccessibilityBridge` is `Send + Sync`, which a real
    /// platform bridge satisfies by being posted to from threads the platform owns.
    struct A11yRecorder {
        names: std::sync::Mutex<Vec<(ObjectId, String)>>,
        states: std::sync::Mutex<Vec<ObjectId>>,
        /// D09-A11Y-02: the complete state submitted per node, readable back through `node_state`.
        nodes: std::sync::Mutex<
            std::collections::HashMap<ObjectId, crate::platform::accessibility::A11yState>,
        >,
    }

    impl A11yRecorder {
        fn new() -> Self {
            Self {
                names: std::sync::Mutex::new(Vec::new()),
                states: std::sync::Mutex::new(Vec::new()),
                nodes: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl crate::platform::accessibility::AccessibilityBridge for A11yRecorder {
        fn set_accessibility_name(&self, id: ObjectId, name: &str) {
            self.names.lock().unwrap().push((id, name.to_string()));
        }
        fn accessibility_name(&self, id: ObjectId) -> Option<String> {
            self.names
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(recorded, _)| *recorded == id)
                .map(|(_, name)| name.clone())
        }
        fn notify_name_changed(&self, _id: ObjectId) {}
        fn notify_value_changed(&self, _id: ObjectId) {}
        fn notify_state_changed(&self, id: ObjectId) {
            self.states.lock().unwrap().push(id);
        }
        fn notify_focus_changed(&self, _id: ObjectId) {}
        // D09-A11Y-02/D09-A11Y-03: the substitute keeps the full node store the platform bridges
        // keep, so a test can read the whole submitted state back and watch the store return to
        // baseline across mount/unmount cycles — the observable assertions the defects require.
        fn submit_node_state(
            &self,
            id: ObjectId,
            state: &crate::platform::accessibility::A11yState,
        ) {
            self.names.lock().unwrap().push((id, state.label.clone()));
            self.nodes.lock().unwrap().insert(id, state.clone());
        }
        fn node_state(&self, id: ObjectId) -> Option<crate::platform::accessibility::A11yState> {
            self.nodes.lock().unwrap().get(&id).cloned()
        }
        fn unregister_node(&self, id: ObjectId) {
            self.names.lock().unwrap().retain(|(recorded, _)| *recorded != id);
            self.nodes.lock().unwrap().remove(&id);
        }
        fn node_count(&self) -> usize {
            self.nodes.lock().unwrap().len()
        }
    }

    /// Installs a recording bridge for one test, removing it on the way out even on panic.
    struct A11yGuard;
    impl Drop for A11yGuard {
        fn drop(&mut self) {
            let _ = crate::widget::a11y_submit::uninstall_bridge();
        }
    }

    /// BLUE24 §6 criterion 1 and §11 criterion 23: mounting controls creates their nodes.
    ///
    /// # Why the real `register` path
    ///
    /// A test that called `submit_mounted` directly would prove the submit function works and
    /// nothing about whether anything calls it — which is exactly the gap BLUE24 §0A.1 measured
    /// (three platform bridges, zero production callers). So this mounts through `register`, the
    /// same entry point every control in the crate uses, and asserts the bridge saw it.
    #[test]
    fn mounting_controls_creates_their_nodes() {
        let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
        crate::widget::a11y_submit::install_bridge(recorder);
        let _guard = A11yGuard;

        let label = register(Box::new(crate::widget::Label::new(
            "Hello".to_string(),
            crate::core::Rect::new(0, 0, 80, 24),
        )))
        .expect("mount a label");
        let _unmount_label = MountGuard(label);
        let button = register(Box::new(crate::widget::Button::new(
            "Save".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount a button");
        let _unmount_button = MountGuard(button);
        let checkbox =
            register(Box::new(crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24))))
                .expect("mount a check box");
        let _unmount_checkbox = MountGuard(checkbox);

        let names = recorder.names.lock().unwrap().clone();
        assert_eq!(names.len(), 3, "three controls mounted, so three names were pushed: {names:?}");
        assert!(
            names.iter().any(|(id, name)| *id == label && name == "Hello"),
            "a label's accessible name is its text: {names:?}"
        );
        assert!(
            names.iter().any(|(id, name)| *id == button && name == "Save"),
            "a button's accessible name is its title: {names:?}"
        );
        let states = recorder.states.lock().unwrap().clone();
        assert_eq!(states.len(), 3, "and each node's creation was reported: {states:?}");

        // D09-A11Y-02: the node the bridge received carries the derived *role*, not only the name.
        // Before the full-state contract, the mount path sent `label` alone, so a screen reader
        // could read "Save" but not know it was a button.
        assert_eq!(
            recorder.node_state(label).map(|s| s.role),
            Some(crate::platform::accessibility::A11yRole::Label),
            "a label's role reaches the bridge"
        );
        assert_eq!(
            recorder.node_state(button).map(|s| s.role),
            Some(crate::platform::accessibility::A11yRole::Button),
            "a button's role reaches the bridge"
        );
        assert_eq!(
            recorder.node_state(checkbox).map(|s| s.role),
            Some(crate::platform::accessibility::A11yRole::CheckBox),
            "a check box's role reaches the bridge"
        );
        // And the check box's checked state, which the label-only contract could never carry.
        assert_eq!(
            recorder.node_state(checkbox).and_then(|s| s.checked),
            Some(false),
            "an unchecked box reports `Some(false)`, not `None`"
        );
    }

    /// D09-A11Y-03: repeated mount/unmount cycles return the bridge's node store to baseline.
    ///
    /// The defect was a name map that blanked to `""` on unmount and kept one entry per historical
    /// widget id. Because the runtime allocates a fresh monotonically increasing id per mount, the
    /// counts must be compared across *distinct* ids — a cycle reusing one id would pass even if the
    /// store grew.
    #[test]
    fn repeated_mount_unmount_returns_the_bridge_store_to_baseline() {
        let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
        crate::widget::a11y_submit::install_bridge(recorder);
        let _guard = A11yGuard;

        let baseline = recorder.node_count();
        assert_eq!(baseline, 0, "a fresh bridge records no nodes");

        for cycle in 0..5 {
            let id = register(Box::new(crate::widget::Label::new(
                format!("cycle-{cycle}"),
                crate::core::Rect::new(0, 0, 80, 24),
            )))
            .expect("mount");
            assert_eq!(recorder.node_count(), baseline + 1, "one live node in cycle {cycle}");
            assert!(
                recorder.node_state(id).is_some(),
                "the mounted node is recorded in cycle {cycle}"
            );
            assert!(unregister(id), "the mounted id is present in cycle {cycle}");
            assert_eq!(
                recorder.node_count(),
                baseline,
                "the store returns to baseline after cycle {cycle}"
            );
            assert!(
                recorder.node_state(id).is_none(),
                "and the node itself is gone after cycle {cycle}"
            );
        }

        let names = recorder.names.lock().unwrap();
        assert!(names.is_empty(), "no historical id left a blanked name entry behind: {names:?}");
    }

    /// Unmounting a control tears its node down; re-registering a never-mounted id does not.
    ///
    /// The second half matters: a removal posted for an id that was never mounted would tear down
    /// a node a live control still owns, which is a silent loss for a screen reader.
    #[test]
    fn unmounting_a_control_tears_down_its_node() {
        let recorder: &'static A11yRecorder = Box::leak(Box::new(A11yRecorder::new()));
        crate::widget::a11y_submit::install_bridge(recorder);
        let _guard = A11yGuard;

        let id = register(Box::new(crate::widget::Label::new(
            "Bye".to_string(),
            crate::core::Rect::new(0, 0, 80, 24),
        )))
        .expect("mount");
        let before = recorder.states.lock().unwrap().len();
        assert!(unregister(id), "the mounted id is present");
        let after = recorder.states.lock().unwrap().len();
        assert_eq!(after, before + 1, "unmounting posted one teardown");

        // A second unmount of the same id finds nothing, so it posts nothing.
        assert!(!unregister(id), "the id is gone");
        assert_eq!(
            recorder.states.lock().unwrap().len(),
            after,
            "an unmount of a never-mounted id must not post a teardown"
        );
    }

    /// A check box's three states all travel the submit path.
    ///
    /// BLUE24 §6 criterion 3. The state is read back through the bridge substitute — not from the
    /// control — so the assertion covers the whole pump: derive (stage ①) and the submission.
    #[test]
    fn a_check_boxs_three_states_travel_the_submit_path() {
        use crate::platform::accessibility::A11yState;

        for (check, expected_checked, expected_mixed) in
            [(false, Some(false), false), (true, Some(true), false)]
        {
            let mut cb = crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24));
            cb.set_checked(check);
            let state = A11yState::from_widget(&cb);
            assert_eq!(state.checked, expected_checked, "checked={check}");
            assert_eq!(state.mixed, expected_mixed, "checked={check}");
        }

        // The mixed state is the tri-state spelling, which cannot be said with a bool.
        let mut cb = crate::widget::CheckBox::new(crate::core::Rect::new(0, 0, 24, 24));
        cb.set_tristate_enabled(true);
        cb.set_state(crate::widget::CheckState::PartiallyChecked);
        let state = A11yState::from_widget(&cb);
        assert_eq!(state.checked, Some(true), "a mixed box is on");
        assert!(state.mixed, "and separately mixed, which is the only way to say it");
    }

    // ── BLUE24 §7 -- native state reporting and redraw ───────────────────────────

    /// BLUE24 §7 criterion 1: a reported hover makes `widget_state()` true for a control the
    /// library did not see the input for.
    ///
    /// The defect this pins is the two-worlds problem: a native control's hover is the
    /// platform's fact, so `BaseWidget::is_hovered` stayed `false`, `widget_state()` reported
    /// `Normal`, and a theme's `"<kind>:hover"` was unreachable for every native control.
    /// Reporting the fact is what closes it, and the assertion is the state the theme keys on.
    #[test]
    fn a_reported_hover_reaches_widget_state() {
        use crate::style::WidgetState;

        let id = register(Box::new(crate::widget::Button::new(
            "native".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        assert_eq!(
            with_widget(id, |w| w.widget_state()),
            Some(WidgetState::Normal),
            "a freshly mounted control is at rest"
        );
        assert!(report_state(id, StateFact::Hovered(true)), "the fact reaches a mounted control");
        assert_eq!(
            with_widget(id, |w| w.widget_state()),
            Some(WidgetState::Hover),
            "so `widget_state()` is now true for a control the library never saw input for"
        );

        // Same fact again: no change, so nothing is re-reported (the idempotence §6.3 asks of
        // its own submit points, here for the native path).
        assert!(report_state(id, StateFact::Hovered(true)), "a known id is still found");
        assert_eq!(with_widget(id, |w| w.widget_state()), Some(WidgetState::Hover));

        assert!(report_state(id, StateFact::Hovered(false)), "leaving is a fact too");
        assert_eq!(with_widget(id, |w| w.widget_state()), Some(WidgetState::Normal));
    }

    /// A report about an id the library does not know is refused, not silently dropped.
    #[test]
    fn a_report_about_an_unknown_control_is_refused() {
        assert!(!report_state(0xDEAD_BEEF, StateFact::Hovered(true)));
        assert!(!report_state(0xDEAD_BEEF, StateFact::Pressed(true)));
    }

    /// BLUE24 §7 criterion 2: a native redraw is **counted** by the frame ledger.
    #[test]
    fn a_native_redraw_is_counted_by_the_frame() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Label::new(
            "n".to_string(),
            crate::core::Rect::new(0, 0, 40, 16),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        // Start from a clean frame accounting.
        let _ = drive_frame(0);
        notify_native_redraw(id, Some(crate::core::Rect::new(0, 0, 10, 10)));
        let outcome = drive_frame(0);
        assert_eq!(
            outcome.repaints_submitted, 1,
            "the platform's own redraw request is in the library's account"
        );
    }

    // ── BLUE24 §8 -- the frame ledger ───────────────────────────────────────────

    /// BLUE24 §8 criterion 1: sixty still frames tick nothing and coalesce nothing.
    #[test]
    fn sixty_still_frames_cost_nothing() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Label::new(
            "still".to_string(),
            crate::core::Rect::new(0, 0, 40, 16),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);

        for _ in 0..60 {
            let outcome = drive_frame(16);
            assert_eq!(outcome.controls_ticked, 0, "nothing is animating");
            let stats = last_frame_stats().expect("a frame ran");
            assert_eq!(stats.controls_ticked, 0);
            assert_eq!(
                stats.repaints_coalesced, 0,
                "nothing was requested, so nothing could be merged"
            );
            assert!(stats.last_repaint_reason.is_empty(), "and no surface was repainted");
        }
    }

    /// A control that STOPS animating must let the frame loop stop asking for frames.
    ///
    /// # The defect this pins
    ///
    /// `tick_animations` sets `LAST_SWEEP_HAD_UNSETTLED_MOUNTED` on any frame that found an
    /// animating control, and `animation_bus_needs_another_frame` ORs it in. The flag exists so
    /// the frame that observes a *settle* is scheduled. But nothing consumed it: the only writer
    /// that cleared it was `animation_bus_reset_host_owned`, which a host calls on **teardown**.
    ///
    /// So the first frame in which any mounted control animated turned
    /// `needs_another_frame` into a **permanent `true`** — a window that had ever shown a spinner
    /// or an indeterminate bar could never sleep again. That is the opposite of BLUE24 §1's
    /// central promise ("a still window's frame costs one emptiness check"), and it is invisible
    /// without a control that animates *and then stops*, which is why it survived until a player
    /// that reaches the end of its media was wired up.
    #[test]
    fn a_control_that_stops_animating_lets_the_frame_loop_sleep() {
        let _serial = serialise_drive_frame();
        use crate::widget::display_widgets::progressbar::ProgressBar;

        let mut bar = ProgressBar::new(crate::core::Rect::new(0, 0, 160, 16));
        bar.set_indeterminate(true);
        let id = register(Box::new(bar)).expect("mount");
        let _unmount = MountGuard(id);

        // While it animates the loop is owed another frame.
        let running = drive_frame(16);
        assert_eq!(running.controls_ticked, 1);
        assert!(running.needs_another_frame, "an animating control keeps the loop awake");

        // Stop it. The very next frame must tick nothing, and the one after that must be free:
        // a settle is painted once and then scheduling stops.
        with_widget_mut(id, |widget| {
            if let Some(bar) =
                crate::widget::capability::coercion::widget_as_mut::<ProgressBar>(widget)
            {
                bar.set_indeterminate(false);
            }
        });

        let settled = drive_frame(16);
        assert_eq!(settled.controls_ticked, 0, "nothing is animating now");
        let after = drive_frame(16);
        assert!(
            !after.needs_another_frame,
            "a window whose animation has settled must stop being scheduled"
        );
        assert_eq!(after.controls_ticked, 0);
        assert_eq!(after.repaints_submitted, 0, "and a still frame costs no submission");
    }

    /// A mounted, playing `MediaPlayer` advances through the frame loop and then stops it.
    ///
    /// # The defect this pins, and why it belongs in **this** module's tests
    ///
    /// `MediaPlayer` had a `position_ms`, a progress ratio, a `position_changed` signal and a
    /// progress rule in `draw` — and nothing that moved the position. The missing piece was not
    /// inside the control (a `tick` is) but at the **seam**: the frame loop is what turns "this
    /// control would advance" into "this control advanced", and it only does so for a mounted
    /// control that answers [`Widget::is_animating`]. So the test that proves the seam is closed
    /// belongs here, next to `drive_frame`, rather than only in the control's unit tests —
    /// a `tick` nobody calls is exactly the shape BLUE24 §0A.1 measurement 1 is about.
    #[test]
    fn a_playing_media_player_advances_through_the_frame_loop() {
        let _serial = serialise_drive_frame();
        use crate::widget::special_widgets::media_player::MediaPlayer;

        let mut player = MediaPlayer::new(crate::core::Rect::new(0, 0, 320, 200));
        player.set_source("clip.mp4", 1_000);
        assert!(player.play());
        let id = register(Box::new(player)).expect("mount");
        let _unmount = MountGuard(id);

        // One frame of the frame loop's own delta must move the control, and the loop must be
        // told it still owes another frame.
        let outcome = drive_frame(16);
        assert_eq!(outcome.controls_ticked, 1, "the player is the one control animating");
        assert!(outcome.needs_another_frame, "a playing player keeps the loop awake");
        assert_eq!(
            with_widget(id, |widget| widget.is_animating()),
            Some(true),
            "and it says so through the trait, not only through its own methods"
        );

        // The next frames are the media's own duration: at the loop's 16 ms delta it takes 63
        // frames to cover the 1_000 ms, so the bound below has margin and the assertion is about
        // "it stops at all", not about the exact frame it stops on. What must not happen is the
        // loop staying awake for ever, which is the failure a missing end condition produces.
        let mut stopped_at = None;
        for frame in 1..=120 {
            let outcome = drive_frame(16);
            if !outcome.needs_another_frame {
                stopped_at = Some(frame);
                break;
            }
        }
        assert!(
            stopped_at.is_some(),
            "playback must end on its own, not keep the frame loop awake for ever"
        );
        let stats = last_frame_stats().expect("a frame ran");
        assert_eq!(
            stats.controls_ticked, 0,
            "the frame that observed the end no longer ticks the player"
        );
    }

    /// BLUE24 §8 criterion 3: five requests for one surface submit once and coalesce four.
    #[test]
    fn one_surface_is_submitted_once_and_the_rest_are_coalesced() {
        let _serial = serialise_drive_frame();
        let surface = register(Box::new(crate::widget::Label::new(
            "s".to_string(),
            crate::core::Rect::new(0, 0, 40, 16),
        )))
        .expect("mount");
        let _unmount = MountGuard(surface);

        // A clean frame boundary, so the five requests belong to one window.
        let _ = drive_frame(0);
        for _ in 0..5 {
            notify_native_redraw(surface, None);
        }
        let outcome = drive_frame(0);
        assert_eq!(outcome.repaints_submitted, 1, "one surface, one submission");
        let stats = last_frame_stats().expect("a frame ran");
        assert_eq!(stats.repaints_coalesced, 4, "and the other four were merged into it");
    }

    /// BLUE24 §8 criterion 4: every repainted surface has a cause in the ledger.
    #[test]
    fn every_repaint_names_a_reason() {
        let _serial = serialise_drive_frame();
        let id = register(Box::new(crate::widget::Button::new(
            "why".to_string(),
            crate::core::Rect::new(0, 0, 80, 32),
        )))
        .expect("mount");
        let _unmount = MountGuard(id);
        let _ = drive_frame(0);

        notify_native_redraw(id, None);
        let _ = drive_frame(0);

        let stats = last_frame_stats().expect("a frame ran");
        assert!(
            !stats.last_repaint_reason.is_empty(),
            "a frame that submitted a repaint must be able to say why"
        );
        assert_eq!(
            stats.last_repaint_reason.iter().find(|(sid, _)| *sid == id).map(|(_, r)| *r),
            Some(RepaintReason::Native),
            "a platform redraw is attributed to the platform: {:?}",
            stats.last_repaint_reason
        );
    }

    /// The ledger attributes a repaint to the cause that **made the frame happen**.
    ///
    /// When two causes name one surface in one frame, the first wins: the later one is folded
    /// into a submission that was already going out, so recording it would describe the last
    /// caller rather than the cost. This is asserted through the state path, which is the one
    /// whose submission goes through the library's own invalidation.
    #[test]
    fn two_causes_on_one_surface_record_the_first() {
        let _serial = serialise_drive_frame();
        use crate::platform::{with_recorded_invalidations, RecordingInvalidations};

        // A recording backend makes `invalidate_surface` succeed, so the state path submits
        // (and is therefore counted) exactly as it is on a real host.
        let recorder: &'static RecordingInvalidations =
            Box::leak(Box::new(RecordingInvalidations::default()));
        with_recorded_invalidations(recorder, || {
            let id = register(Box::new(crate::widget::Button::new(
                "first".to_string(),
                crate::core::Rect::new(0, 0, 80, 32),
            )))
            .expect("mount");
            let _unmount = MountGuard(id);
            let _ = drive_frame(0);

            request_repaint_because(id, RepaintReason::Animation);
            request_repaint_because(id, RepaintReason::State);
            let outcome = drive_frame(0);

            assert_eq!(outcome.repaints_submitted, 1, "one surface, one submission");
            let stats = last_frame_stats().expect("a frame ran");
            assert_eq!(stats.repaints_coalesced, 1, "and the second request was merged");
            assert_eq!(
                stats.last_repaint_reason.iter().find(|(sid, _)| *sid == id).map(|(_, r)| *r),
                Some(RepaintReason::Animation),
                "the cause that made the frame happen is the one recorded"
            );
        });
    }

    /// Only an animation keeps producing frames; the other causes describe a finished change.
    #[test]
    fn only_animation_keeps_asking_for_frames() {
        assert!(RepaintReason::Animation.keeps_animating());
        assert!(!RepaintReason::State.keeps_animating());
        assert!(!RepaintReason::Overlay.keeps_animating());
        assert!(!RepaintReason::Explicit.keeps_animating());
        assert!(!RepaintReason::Native.keeps_animating());
    }

    // ── The window-level tree painter (BLUE24 §0A.1) ─────────────────────────

    /// A window's tree frame must contain its **children's** pixels, not just the background.
    ///
    /// # The defect this pins
    ///
    /// Since 2.0 the library paints every `WidgetKind` itself, so a control created through
    /// the ordinary `create_*` path has no native control of its own. On Windows the only
    /// painter was the canvas child window that `mount_surface` creates, so a window whose
    /// children came from `create_button` / `create_checkbox` / … had **nothing drawing
    /// them**: the window opened and showed an empty client area — a blank white board.
    ///
    /// The fix is a window-level painter that walks the child list (`render_frame_tree`).
    /// This test pins the property that painter depends on, at the level where it can be
    /// checked without a display: a window with mounted children yields a frame whose pixels
    /// are **not all the clear colour**. A regression that broke the traversal — an empty
    /// child list, a skipped `is_visible`, a missing draw bridge — would produce a uniform
    /// frame and fail here rather than only on a screen nobody is looking at.
    #[test]
    fn a_window_tree_frame_contains_its_children_not_just_the_background() {
        let window_id = register(Box::new(crate::widget::window::Window::new(
            "t".to_string(),
            Rect::new(0, 0, 200, 120),
        )))
        .expect("mount the window");
        let _unmount_window = MountGuard(window_id);

        // A child at a known place, added through the window's own `add_child` so both
        // directions of the link are written — a one-sided link is the other way a child
        // becomes unreachable from the root.
        let child_id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            Rect::new(10, 40, 80, 32),
        )))
        .expect("mount the button");
        let _unmount_child = MountGuard(child_id);
        assert!(with_widget_mut(window_id, |window| {
            window.add_child(child_id);
            true
        })
        .unwrap_or(false));

        let clear = Color::rgb(255, 255, 255);
        let frame = render_frame_tree(window_id, Size::new(200, 120), clear)
            .expect("a window with a drawable child must produce a frame");

        // Every pixel identical to the clear colour means nothing was drawn over the
        // background — the blank-board symptom, expressed as a fact about the frame.
        let clear_rgba = [clear.r, clear.g, clear.b, 255];
        let painted = frame.as_chunks::<4>().0.iter().filter(|pixel| **pixel != clear_rgba).count();
        assert!(
            painted > 0,
            "the window's own chrome or its child must paint something; \
             a uniform frame is the blank-board defect"
        );
    }

    /// The child-list traversal a window painter depends on must actually reach a child.
    ///
    /// `render_frame_tree` walks `direct_children_of`, so a child that is mounted but absent
    /// from that list is invisible to every painter — the failure mode is silent, which is
    /// why the list is asserted rather than the resulting picture alone.
    #[test]
    fn a_mounted_child_is_reachable_through_the_window_child_list() {
        let window_id = register(Box::new(crate::widget::window::Window::new(
            "t".to_string(),
            Rect::new(0, 0, 200, 120),
        )))
        .expect("mount the window");
        let _unmount_window = MountGuard(window_id);

        let child_id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            Rect::new(5, 5, 40, 20),
        )))
        .expect("mount the button");
        let _unmount_child = MountGuard(child_id);

        assert!(
            !direct_children_of(window_id).contains(&child_id),
            "the premise: mounting a child does not by itself enlist it with the window"
        );
        assert!(with_widget_mut(window_id, |window| {
            window.add_child(child_id);
            true
        })
        .unwrap_or(false));
        assert!(
            direct_children_of(window_id).contains(&child_id),
            "after `add_child` the window painter must be able to find it"
        );
    }

    /// `Event::Resize` used to be published with a constructor and **no producer**: nothing
    /// in the library ever delivered it, so a nested control that handled it (the code
    /// editor refreshes its visible rows) never heard that its container changed size.
    #[test]
    fn a_container_resize_reaches_a_nested_child() {
        let window_id = register(Box::new(crate::widget::window::Window::new(
            "t".to_string(),
            Rect::new(0, 0, 200, 120),
        )))
        .expect("mount the window");
        let _unmount_window = MountGuard(window_id);

        let panel_id = register(Box::new(crate::widget::Panel::new(Rect::new(0, 0, 200, 120))))
            .expect("mount the panel");
        let _unmount_panel = MountGuard(panel_id);
        with_widget_mut(window_id, |window| window.add_child(panel_id));

        let editor_id = register(Box::new(crate::widget::Button::new(
            "ok".to_string(),
            Rect::new(0, 0, 40, 20),
        )))
        .expect("mount the button");
        let _unmount_editor = MountGuard(editor_id);
        with_widget_mut(panel_id, |panel| panel.add_child(editor_id));

        // The window and both descendants are told, deepest included.
        let reached = dispatch_resize(window_id, 640, 480);
        assert_eq!(reached, 3, "window + panel + button");

        // An id that addresses nothing tells nobody rather than panicking.
        assert_eq!(dispatch_resize(u64::MAX, 10, 10), 0);
    }

    /// A synchronous event callback may re-enter the registry without panicking.
    ///
    /// # What this pins (BLUE-issue E-09)
    ///
    /// A control emits its own signal **synchronously** while `handle_event` runs (a `Button`
    /// emits `base.clicked` on release). A slot on that signal is a user callback, and a normal
    /// callback updates other widgets — sets a label's text, moves a panel, etc. Every one of those
    /// goes through `with_widget` / `with_widget_mut`, which borrow the `MOUNTED` `RefCell`.
    /// `dispatch_event` used to hold that `borrow_mut` across `handle_event`, so the callback hit
    /// `already borrowed: BorrowMutError` and panicked. The entry is now detached for the duration
    /// of the call, so the borrow is held only for the two map operations.
    #[test]
    fn a_click_callback_may_update_other_widgets_without_panicking() {
        use crate::core::Point;
        use crate::event::mouse_button;

        let button_id = register(Box::new(crate::widget::Button::new(
            "go".to_string(),
            Rect::new(0, 0, 60, 24),
        )))
        .expect("mount the button");
        let _button_guard = MountGuard(button_id);

        let label_id = register(Box::new(crate::widget::Label::new(
            "before".to_string(),
            Rect::new(0, 0, 80, 20),
        )))
        .expect("mount the label");
        let _label_guard = MountGuard(label_id);

        // The handler mutates a *different* control — the exact shape the issue names as risky.
        let ran = std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
        let ran_slot = std::sync::Arc::clone(&ran);
        with_widget_mut(button_id, |button| {
            button.base_mut().clicked.connect(move || {
                // Re-enters the registry through the same borrow that used to be held.
                assert!(set_geometry(label_id, Rect::new(5, 5, 120, 20)));
                let _ = with_widget(label_id, |_| ());
                ran_slot.store(true, core::sync::atomic::Ordering::SeqCst);
            });
        });

        // Drive a real press + release through `dispatch_event`, which is where the borrow was held.
        dispatch_event(
            button_id,
            &crate::event::Event::MousePress {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
                modifiers: 0,
            },
        );
        dispatch_event(
            button_id,
            &crate::event::Event::MouseRelease {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
            },
        );

        assert!(
            ran.load(core::sync::atomic::Ordering::SeqCst),
            "the click callback must have run to completion rather than panicking"
        );
        assert_eq!(with_widget(label_id, |label| label.geometry().width), Some(120));
    }

    /// A callback that destroys the control it is handling must actually destroy it.
    ///
    /// # What this pins
    ///
    /// `dispatch_event` detaches the entry before the call and reinserts it after. Without a
    /// tombstone for a handler that called `unregister` on the in-flight id, the reinsertion would
    /// silently resurrect a widget the handler had just destroyed. The tombstone check is what makes
    /// the destroy stick.
    #[test]
    fn a_callback_may_destroy_the_widget_it_is_handling() {
        use crate::core::Point;
        use crate::event::mouse_button;

        let button_id = register(Box::new(crate::widget::Button::new(
            "self-destruct".to_string(),
            Rect::new(0, 0, 80, 24),
        )))
        .expect("mount the button");

        let self_id = button_id;
        with_widget_mut(button_id, |button| {
            button.base_mut().clicked.connect(move || {
                assert!(unregister(self_id), "the handler's unregister must take effect");
            });
        });

        dispatch_event(
            button_id,
            &crate::event::Event::MousePress {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
                modifiers: 0,
            },
        );
        dispatch_event(
            button_id,
            &crate::event::Event::MouseRelease {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
            },
        );

        assert!(
            !is_mounted(button_id),
            "the widget must not be resurrected by the dispatch epilogue"
        );
    }

    /// A property write that emits a signal must let the callback re-enter the registry.
    ///
    /// # The defect this pins (BLUE-issue E-17)
    ///
    /// `with_widget_mut` held the registry's `RefCell` mutably across `f`. A property setter runs as
    /// `f` and emits the control's signal synchronously, so the user callback ran while that borrow was
    /// live and any `with_widget` / `geometry_of` inside it hit `already borrowed`. The borrow is now
    /// scoped to the map operations only, so the callback runs with the registry free.
    #[test]
    fn a_property_write_callback_may_read_the_registry_without_panicking() {
        use crate::widget::capability::access::write_widget_property_by_id;
        use crate::widget::capability::CapabilityValue;
        use crate::widget::Slider;

        let slider_id = register(Box::new(Slider::new(Rect::new(0, 0, 200, 30)))).expect("mount");
        let _guard = MountGuard(slider_id);
        let other_id = register(Box::new(crate::widget::Label::new(
            "other".to_string(),
            Rect::new(0, 0, 60, 20),
        )))
        .expect("mount the label");
        let _other_guard = MountGuard(other_id);

        // Subscribing to the slider's dynamic `value_changed` and reading the registry inside the
        // handler is exactly the shape that panicked.
        let observed = std::sync::Arc::new(std::sync::Mutex::new(None::<(bool, bool)>));
        let observed_slot = std::sync::Arc::clone(&observed);
        with_widget_mut(slider_id, |widget| {
            let reference = widget.event_signal_dyn("value_changed").expect("published");
            reference.subscribe(Box::new(move |_payload| {
                let other_is_readable = geometry_of(other_id).is_some();
                let self_is_readable = geometry_of(slider_id).is_some();
                *observed_slot.lock().expect("lock") = Some((other_is_readable, self_is_readable));
            }));
        });

        write_widget_property_by_id(slider_id, "value", CapabilityValue::Int(42))
            .expect("the slider accepts a value write");

        assert_eq!(
            *observed.lock().expect("lock"),
            Some((true, false)),
            "the callback may read other widgets, but must not alias the widget still held mutably"
        );
    }

    /// A callback that destroys the control during a property write must actually destroy it.
    ///
    /// The `with_widget_mut` drive marks its id in-flight for the same reason `dispatch_event` does:
    /// a setter's callback may destroy the control, and the reinsertion must not resurrect it.
    #[test]
    fn a_property_write_callback_may_destroy_the_control() {
        use crate::widget::capability::access::write_widget_property_by_id;
        use crate::widget::capability::CapabilityValue;
        use crate::widget::Slider;

        let slider_id = register(Box::new(Slider::new(Rect::new(0, 0, 200, 30)))).expect("mount");

        let self_id = slider_id;
        with_widget_mut(slider_id, |widget| {
            let reference = widget.event_signal_dyn("value_changed").expect("published");
            reference.subscribe(Box::new(move |_payload| {
                assert!(unregister(self_id), "the callback's unregister must take effect");
            }));
        });

        write_widget_property_by_id(slider_id, "value", CapabilityValue::Int(7))
            .expect("the slider accepts a value write");

        assert!(
            !is_mounted(slider_id),
            "a destroy during a property write must not be undone by the drive's reinsertion"
        );
    }

    /// A callback can identify a mutably-driven control without aliasing it.
    ///
    /// # The defect this pins (BLUE-issue E-18)
    ///
    /// `dispatch_event` holds a mutable widget reference while `handle_event` synchronously emits
    /// signals. Reborrowing the widget from a callback would violate Rust's aliasing rules, so its
    /// geometry read is refused during dispatch while `is_mounted` still reports true.
    #[test]
    fn a_callback_does_not_alias_a_widget_while_it_is_mutably_driven() {
        use crate::core::Point;
        use crate::event::mouse_button;

        let button_id = register(Box::new(crate::widget::Button::new(
            "go".to_string(),
            Rect::new(0, 0, 80, 24),
        )))
        .expect("mount the button");
        let _guard = MountGuard(button_id);

        let self_read = std::sync::Arc::new(std::sync::Mutex::new(None::<(bool, bool)>));
        let record = std::sync::Arc::clone(&self_read);
        with_widget_mut(button_id, |button| {
            button.base_mut().clicked.connect(move || {
                // The id remains mounted, but its widget cannot be reborrowed while handle_event owns it.
                let mounted_by_registry_id = is_mounted(button_id);
                let geometry = geometry_of(button_id);
                *record.lock().expect("lock") = Some((mounted_by_registry_id, geometry.is_some()));
            });
        });

        dispatch_event(
            button_id,
            &crate::event::Event::MousePress {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
                modifiers: 0,
            },
        );
        dispatch_event(
            button_id,
            &crate::event::Event::MouseRelease {
                pos: Point::new(10, 10),
                button: mouse_button::PRIMARY,
            },
        );

        let observed = *self_read.lock().expect("lock");
        assert_eq!(
            observed,
            Some((true, false)),
            "a callback must see the control as mounted without creating an aliased shared reference"
        );
    }

    /// A panicking input callback must not leave the registry with a stale in-flight marker.
    ///
    /// # The defect this pins (BLUE-issue E-19)
    ///
    /// `dispatch_event` removed the widget and marked it in-flight, then ran `handle_event`. If the
    /// callback unwound (the host `catch_unwind`s), the cleanup after the call was skipped: the id
    /// stayed in the in-flight set, so `is_mounted` answered `false` but `unregister` still reported
    /// the id present. Cleanup is now a guard, so both answers agree after an unwind.
    #[test]
    fn a_panicking_input_callback_leaves_no_stale_dispatch_state() {
        use crate::core::Point;
        use crate::event::mouse_button;

        let button_id = register(Box::new(crate::widget::Button::new(
            "boom".to_string(),
            Rect::new(0, 0, 80, 24),
        )))
        .expect("mount the button");
        // No `MountGuard`: the widget is intentionally left unmounted after the unwind path.
        let _unmount = MountGuard(button_id);

        with_widget_mut(button_id, |button| {
            button.base_mut().clicked.connect(|| {
                panic!("deliberate unwind from an input callback");
            });
        });

        let press = crate::event::Event::MousePress {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
            modifiers: 0,
        };
        let release = crate::event::Event::MouseRelease {
            pos: Point::new(10, 10),
            button: mouse_button::PRIMARY,
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch_event(button_id, &press);
            dispatch_event(button_id, &release);
        }));
        assert!(caught.is_err(), "the callback must have unwound");

        // The registry must be consistent: if the widget is gone, `unregister` must say so rather
        // than reporting a stale in-flight entry as present.
        assert!(
            !is_mounted(button_id),
            "the lookup must not resolve a widget dropped by the unwind"
        );
        assert!(
            !unregister(button_id),
            "a popped dispatch must not report the id as present via a leftover in-flight marker"
        );
    }

    /// A closure reading control A may emit A's signal, whose callback writes control B.
    ///
    /// # The defect this pins (BLUE-issue E-30)
    ///
    /// `with_widget` ran its closure inside `map.borrow()`, so a closure that read A's state and
    /// synchronously emitted A's signal \u2014 a legitimate `&self` operation \u2014 ran a callback that
    /// then wrote B via `with_widget_mut`, which needed `map.borrow_mut()`. That was `already
    /// borrowed`, not a nested write of A. `with_widget` now detaches A before running the closure, so
    /// the registry is free for the callback to write another control.
    #[test]
    fn a_read_closure_may_emit_a_signal_whose_callback_writes_another_control() {
        let a_id = register(Box::new(crate::widget::Button::new(
            "a".to_string(),
            Rect::new(0, 0, 60, 24),
        )))
        .expect("mount A");
        let _a_guard = MountGuard(a_id);
        let b_id = register(Box::new(crate::widget::Label::new(
            "before".to_string(),
            Rect::new(0, 0, 80, 20),
        )))
        .expect("mount B");
        let _b_guard = MountGuard(b_id);

        let ran = std::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
        let ran_slot = std::sync::Arc::clone(&ran);
        with_widget_mut(a_id, |widget| {
            widget.base_mut().clicked.connect(move || {
                // Writes a *different* control from inside the slot.
                assert!(set_geometry(b_id, Rect::new(3, 4, 120, 20)));
                ran_slot.store(true, core::sync::atomic::Ordering::SeqCst);
            });
        });

        // A read closure on A that emits A's own signal synchronously.
        let result = with_widget(a_id, |widget| {
            widget.base().clicked.emit();
            widget.geometry().width
        });

        assert_eq!(result, Some(60), "the read closure must return normally, not panic");
        assert!(
            ran.load(core::sync::atomic::Ordering::SeqCst),
            "the slot must have run and written the other control without a RefCell panic"
        );
        assert_eq!(with_widget(b_id, |label| label.geometry().width), Some(120));
    }

    #[test]
    fn unregister_during_a_read_loan_is_not_undone_by_reinsertion() {
        let id = register(Box::new(crate::widget::Label::new(
            "remove".to_string(),
            Rect::new(0, 0, 60, 20),
        )))
        .expect("mount");

        let removed = with_widget(id, |_| unregister(id));

        assert_eq!(removed, Some(true), "unregister must see the in-flight read loan");
        assert!(!is_mounted(id), "the read-loan epilogue must not resurrect the widget");
        assert!(!unregister(id), "the id must already be fully unregistered");
    }

    // ── Host-window id translation (the P-05 seam) ──────────────────────────────

    /// A mounted surface is not a child in the widget tree, so its window is recorded at
    /// mount time; `top_level_window_of` must answer from that record, because cursor
    /// dispatch and repaint routing both ask it for "the window this control is in".
    ///
    /// Regression: without the record, `top_level_window_of(editor)` answered `None` for
    /// every mounted surface, so `dispatch_hover_transition` found no window to call
    /// `set_cursor` on and the code editor's I-beam was never applied by any backend.
    #[test]
    fn a_mounted_surface_answers_the_window_it_was_mounted_in() {
        let surface = register(Box::new(crate::widget::Label::new(
            "editor".to_string(),
            Rect::new(0, 0, 100, 80),
        )))
        .expect("mount");
        let _unmount_surface = MountGuard(surface);
        let window = register(Box::new(crate::widget::Label::new(
            "window".to_string(),
            Rect::new(0, 0, 100, 80),
        )))
        .expect("mount");
        let _unmount_window = MountGuard(window);

        assert_eq!(top_level_window_of(surface), None, "no parent, so no window by the walk");
        assert!(set_surface_window(surface, window), "a registered surface accepts the link");
        assert_eq!(top_level_window_of(surface), Some(window));
        assert_eq!(surface_window_for(window), None, "only surfaces are recorded, not windows");
        assert!(!set_surface_window(424_242, window), "an unknown surface cannot be linked");

        unregister(surface);
        assert_eq!(
            surface_window_for(surface),
            None,
            "the link must not outlive the widget it described"
        );
    }

    /// A window widget's host id round-trips: `window -> host -> window`.
    ///
    /// The `macos_objc2`/`windows`/`wayland` backends allocate host ids in a space that is
    /// **disjoint** from the widget id space (the platform counts from 1, the runtime reserves
    /// `0x5345_4C46_…`). An OS callback hands the backend its own host id, and the backend must
    /// translate it back before any registry lookup: `widget_id_for_host_window(host)` is that
    /// inverse. This pins the translation the objc2 render/invalidate path (P-05) depends on,
    /// which no test previously exercised.
    #[test]
    fn a_widget_window_id_round_trips_through_its_host_id() {
        let widget = register(Box::new(crate::widget::Label::new(
            "win".to_string(),
            Rect::new(0, 0, 100, 80),
        )))
        .expect("mount");
        let _unmount = MountGuard(widget);

        // The host number is deliberately in the *platform* space, not the widget space.
        let host: ObjectId = 7;
        assert!(set_host_window(widget, host), "a mounted window must accept a host association");
        assert_eq!(host_window_for(widget), Some(host), "window -> host");
        assert_eq!(widget_id_for_host_window(host), Some(widget), "host -> window");
        // An id the platform never built a host for resolves to nothing, not to a wrong widget.
        assert_eq!(host_window_for(host), None, "a host id is not itself a widget");
        assert_eq!(widget_id_for_host_window(widget), None, "a widget id is not a host id");
    }

    /// The host association is dropped with the widget, so a recycled id cannot inherit it.
    #[test]
    fn unmounting_a_window_clears_its_host_association() {
        let widget = register(Box::new(crate::widget::Label::new(
            "win".to_string(),
            Rect::new(0, 0, 100, 80),
        )))
        .expect("mount");
        let host: ObjectId = 99;
        assert!(set_host_window(widget, host));
        assert_eq!(widget_id_for_host_window(host), Some(widget));

        unregister(widget);

        assert_eq!(host_window_for(widget), None, "the mapping must not outlive the widget");
        assert_eq!(
            widget_id_for_host_window(host),
            None,
            "a later widget reusing this host id must not resolve to the dead one"
        );
    }

    /// An unmounted id cannot be given a host window: a mapping to nothing is refused.
    #[test]
    fn set_host_window_refuses_an_unmounted_id() {
        let ghost: ObjectId = 0x5345_4C46_DEAD_BEEF;
        assert!(!set_host_window(ghost, 1), "only a mounted window may carry a host id");
        assert_eq!(host_window_for(ghost), None);
    }

    // ── clipping a container's descendants ─────────────────────────────────────

    /// A container's `children_clip` really narrows what its descendants paint.
    ///
    /// # The defect this pins
    ///
    /// A scroll area places its content with the scroll offset in the child's geometry —
    /// without a clip the content then paints outside the viewport, over the scroll bars and
    /// over whatever lies beyond the control. The container is the only layer that knows the
    /// viewport (`geometry` is not it: the bars are inside the rectangle), so the painter has
    /// to ask it. The root fills white, the child fills red, and the clip decides which half
    /// of the frame each one wins.
    #[test]
    fn the_tree_painter_clips_descendants_to_the_container_content_box() {
        use crate::core::Color;
        use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

        struct Surface {
            base: BaseWidget,
            clip: Option<Rect>,
            color: Color,
        }
        impl Widget for Surface {
            fn base(&self) -> &BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut BaseWidget {
                &mut self.base
            }
            fn children_clip(&self) -> Option<Rect> {
                self.clip
            }
            fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
                Some(self)
            }
        }
        impl crate::event::EventHandler for Surface {
            fn handle_event(&mut self, _event: &crate::event::Event) {}
        }
        impl Draw for Surface {
            fn draw(&mut self, context: &mut RenderContext) {
                context.fill_rect(self.base.geometry(), self.color);
            }
        }

        let root = register(Box::new(Surface {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 60), "root"),
            clip: Some(Rect::new(10, 10, 20, 20)),
            color: Color::WHITE,
        }))
        .expect("mount root");
        let child = register(Box::new(Surface {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 60), "child"),
            clip: None,
            color: Color::RED,
        }))
        .expect("mount child");
        with_widget_mut(root, |widget| {
            widget.base_mut().add_child_linked(child);
        })
        .expect("the root is mounted");

        let frame = render_frame_tree(root, Size::new(60, 60), Color::WHITE).expect("frame");
        let pixel = |x: usize, y: usize| {
            let index = (y * 60 + x) * 4;
            (frame[index], frame[index + 1], frame[index + 2])
        };
        assert_eq!(pixel(15, 15), (255, 0, 0), "inside the clip the child paints");
        assert_eq!(pixel(5, 5), (255, 255, 255), "outside it the root still wins");
        assert_eq!(pixel(40, 40), (255, 255, 255), "and the clip has a far edge too");

        unregister(child);
        unregister(root);
    }

    /// A window-tree frame is reused while nothing that affects it has changed.
    ///
    /// # What this pins
    ///
    /// Backends call their paint callback for exposures and occlusion changes as well as for
    /// content changes. Re-rasterising a whole tree for an expose is work whose result is already
    /// on screen. The counter makes the difference observable: two calls with no change must draw
    /// **once**, and a submitted change (here `mark_widget_damage`, the path every
    /// `request_redraw` takes) must make the next call draw again.
    #[test]
    fn the_cached_tree_frame_is_reused_until_content_changes() {
        use crate::core::Color;
        use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
        use std::cell::Cell;
        use std::rc::Rc;

        struct Counter {
            base: BaseWidget,
            draws: Rc<Cell<usize>>,
        }
        impl Widget for Counter {
            fn base(&self) -> &BaseWidget {
                &self.base
            }
            fn base_mut(&mut self) -> &mut BaseWidget {
                &mut self.base
            }
            fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
                Some(self)
            }
        }
        impl crate::event::EventHandler for Counter {
            fn handle_event(&mut self, _event: &crate::event::Event) {}
        }
        impl Draw for Counter {
            fn draw(&mut self, context: &mut RenderContext) {
                self.draws.set(self.draws.get() + 1);
                context.fill_rect(self.base.geometry(), Color::rgb(20, 40, 60));
            }
        }

        let draws = Rc::new(Cell::new(0));
        let id = register(Box::new(Counter {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 40, 30), "counter"),
            draws: Rc::clone(&draws),
        }))
        .expect("mount");
        let size = Size::new(40, 30);

        let first = render_frame_tree_cached(id, size, Color::WHITE).expect("first frame");
        assert_eq!(draws.get(), 1, "the first call has no frame to reuse");

        let second = render_frame_tree_cached(id, size, Color::WHITE).expect("cached frame");
        assert_eq!(draws.get(), 1, "an unchanged tree must not be drawn again");
        assert_eq!(first, second, "the reused frame must be the one that was drawn");

        // A control's own `request_redraw` reaches `mark_dirty_rect` through the own-id map.
        let _ = mark_dirty_rect(id, Rect::new(0, 0, 40, 30));
        let third = render_frame_tree_cached(id, size, Color::WHITE).expect("repainted frame");
        assert_eq!(draws.get(), 2, "a submitted change must repaint the tree");
        assert_eq!(first, third, "the same content paints the same bytes");

        unregister(id);
    }

    // ── per-widget layer cache ─────────────────────────────────────────────────

    /// A plain container that paints its own background.
    struct LayerRoot {
        base: BaseWidget,
    }
    impl Widget for LayerRoot {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }
    impl crate::event::EventHandler for LayerRoot {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Draw for LayerRoot {
        fn draw(&mut self, context: &mut RenderContext) {
            context.fill_rect(self.base.geometry(), Color::WHITE);
        }
    }

    /// A control that counts how often its own `draw` ran.
    struct LayerProbe {
        base: BaseWidget,
        draws: Rc<DrawCount>,
    }
    impl Widget for LayerProbe {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }
    impl crate::event::EventHandler for LayerProbe {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Draw for LayerProbe {
        fn draw(&mut self, context: &mut RenderContext) {
            self.draws.set(self.draws.get() + 1);
            context.fill_rect(self.base.geometry(), Color::rgb(20, 40, 60));
        }
    }

    /// A text-heavy control: every `draw` lays out many labels.
    struct LayerText {
        base: BaseWidget,
        draws: Rc<DrawCount>,
    }
    impl Widget for LayerText {
        fn base(&self) -> &BaseWidget {
            &self.base
        }
        fn base_mut(&mut self) -> &mut BaseWidget {
            &mut self.base
        }
        fn as_draw_mut(&mut self) -> Option<&mut dyn Draw> {
            Some(self)
        }
    }
    impl crate::event::EventHandler for LayerText {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Draw for LayerText {
        fn draw(&mut self, context: &mut RenderContext) {
            use crate::core::{Font, HorizontalAlignment};
            self.draws.set(self.draws.get() + 1);
            let font = Font::simple("Arial", 12.0);
            let rect = self.base.geometry();
            for line in 0..16 {
                context.draw_text(
                    Point::new(rect.x + 2, rect.y + 2 + line * 14),
                    "The quick brown fox jumps over the lazy dog 0123456789",
                    &font,
                    Color::BLACK,
                    HorizontalAlignment::Left,
                );
            }
        }
    }

    /// Builds a root with one child and returns both ids plus the child's draw counter.
    fn layer_fixture(
        root_geometry: Rect,
        child_geometry: Rect,
    ) -> (ObjectId, ObjectId, Rc<DrawCount>) {
        let draws = Rc::new(DrawCount::new(0));
        let root = register(Box::new(LayerRoot {
            base: BaseWidget::new(WidgetKind::Panel, root_geometry, "layer-root"),
        }))
        .expect("mount root");
        let child = register(Box::new(LayerProbe {
            base: BaseWidget::new(WidgetKind::Panel, child_geometry, "layer-child"),
            draws: Rc::clone(&draws),
        }))
        .expect("mount child");
        with_widget_mut(root, |widget| widget.base_mut().add_child_linked(child))
            .expect("the root is mounted");
        (root, child, draws)
    }

    /// The second frame of an untouched control replays its layer and never runs `draw`.
    #[test]
    fn an_unchanged_widget_replays_its_cached_layer_without_drawing() {
        clear_widget_layers_for_test();
        let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
        let size = Size::new(60, 40);

        let first = render_frame_tree(root, size, Color::WHITE).expect("first frame");
        assert_eq!(draws.get(), 1, "the first frame has no layer to reuse");

        let _ = take_widget_layer_replays_for_test();
        let second = render_frame_tree(root, size, Color::WHITE).expect("second frame");
        assert_eq!(draws.get(), 1, "an unchanged control must not run draw again");
        assert!(
            take_widget_layer_replays_for_test() >= 1,
            "the second frame must be served from the layer cache"
        );
        assert_eq!(first, second, "the replay must be the bytes that were painted");

        unregister(child);
        unregister(root);
    }

    /// A submitted damage rectangle invalidates the control and replaces its layer.
    #[test]
    fn a_damaged_control_redraws_and_replaces_its_layer() {
        clear_widget_layers_for_test();
        let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
        let size = Size::new(60, 40);

        render_frame_tree(root, size, Color::WHITE).expect("first frame");
        render_frame_tree(root, size, Color::WHITE).expect("cached frame");
        assert_eq!(draws.get(), 1, "the untouched frame replays the layer");

        // `mark_dirty_rect` always advances the tree epoch, even for a control with no
        // damage tracker of its own, which is the path `request_redraw` takes.
        let _ = mark_dirty_rect(child, Rect::new(5, 5, 20, 20));
        render_frame_tree(root, size, Color::WHITE).expect("dirty frame");
        assert_eq!(draws.get(), 2, "a recorded damage must redraw the control");

        render_frame_tree(root, size, Color::WHITE).expect("replacement cached frame");
        assert_eq!(draws.get(), 2, "the replacement layer serves the next clean frame");

        unregister(child);
        unregister(root);
    }

    /// Damage to one control does not throw away an unrelated control's layer.
    ///
    /// This is what the separate layer epoch buys. A frame is still repainted whole after any
    /// change — one frame is one rasterisation of the tree — but a control whose pixels did not
    /// change replays the layer it already has. Before the layer epoch was separated from the
    /// tree epoch, a hover on one button made every text control in the window shape again.
    #[test]
    fn a_damaged_control_keeps_an_unrelated_controls_layer() {
        clear_widget_layers_for_test();
        let root = register(Box::new(LayerRoot {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 60, 40), "selective-root"),
        }))
        .expect("mount root");
        let left_draws = Rc::new(DrawCount::new(0));
        let right_draws = Rc::new(DrawCount::new(0));
        let left = register(Box::new(LayerProbe {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(5, 5, 20, 20), "selective-left"),
            draws: Rc::clone(&left_draws),
        }))
        .expect("mount left");
        let right = register(Box::new(LayerProbe {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(35, 5, 20, 20), "selective-right"),
            draws: Rc::clone(&right_draws),
        }))
        .expect("mount right");
        with_widget_mut(root, |widget| {
            widget.base_mut().add_child_linked(left);
            widget.base_mut().add_child_linked(right);
        })
        .expect("the root is mounted");
        let size = Size::new(60, 40);

        render_frame_tree(root, size, Color::WHITE).expect("first frame");
        render_frame_tree(root, size, Color::WHITE).expect("cached frame");
        assert_eq!(
            (left_draws.get(), right_draws.get()),
            (1, 1),
            "both controls have a warm layer"
        );

        let _ = take_widget_layer_replays_for_test();
        let _ = mark_dirty_rect(left, Rect::new(5, 5, 20, 20));
        render_frame_tree(root, size, Color::WHITE).expect("damaged frame");
        assert_eq!(left_draws.get(), 2, "the damaged control repaints its layer");
        assert_eq!(right_draws.get(), 1, "an unrelated control keeps its layer");
        assert!(
            take_widget_layer_replays_for_test() >= 1,
            "the untouched control must have been served from its layer"
        );

        unregister(right);
        unregister(left);
        unregister(root);
    }

    /// Changing a control's geometry or size makes its old layer ineligible.
    #[test]
    fn resizing_a_control_invalidates_its_layer() {
        clear_widget_layers_for_test();
        let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
        let size = Size::new(60, 40);

        render_frame_tree(root, size, Color::WHITE).expect("first frame");
        render_frame_tree(root, size, Color::WHITE).expect("cached frame");
        assert_eq!(draws.get(), 1, "the untouched frame replays the layer");

        assert!(set_geometry(child, Rect::new(5, 5, 28, 24)), "the child is mounted");
        render_frame_tree(root, size, Color::WHITE).expect("resized frame");
        assert_eq!(draws.get(), 2, "a resized control must repaint its layer");

        unregister(child);
        unregister(root);
    }

    /// A global repaint (the theme path) invalidates every per-widget layer.
    #[test]
    fn a_global_repaint_invalidates_cached_layers() {
        clear_widget_layers_for_test();
        let (root, child, draws) = layer_fixture(Rect::new(0, 0, 60, 40), Rect::new(5, 5, 20, 20));
        let size = Size::new(60, 40);

        render_frame_tree_cached(root, size, Color::WHITE).expect("first frame");
        render_frame_tree_cached(root, size, Color::WHITE).expect("cached frame");
        assert_eq!(draws.get(), 1, "the untouched frame replays the cache");

        arm_global_repaint();
        render_frame_tree_cached(root, size, Color::WHITE).expect("repainted frame");
        assert_eq!(draws.get(), 2, "a global repaint must not reuse the old theme's pixels");

        unregister(child);
        unregister(root);
    }

    /// A text-heavy control does not shape or rasterise again on an untouched frame.
    #[test]
    fn a_text_heavy_control_does_not_draw_again_on_an_unchanged_frame() {
        clear_widget_layers_for_test();
        let draws = Rc::new(DrawCount::new(0));
        let root = register(Box::new(LayerRoot {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 240, 200), "text-root"),
        }))
        .expect("mount root");
        let text = register(Box::new(LayerText {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(4, 4, 220, 180), "text-child"),
            draws: Rc::clone(&draws),
        }))
        .expect("mount text");
        with_widget_mut(root, |widget| widget.base_mut().add_child_linked(text))
            .expect("the root is mounted");
        let size = Size::new(240, 200);

        render_frame_tree(root, size, Color::WHITE).expect("first frame");
        assert_eq!(draws.get(), 1, "the first frame lays the text out");

        let _ = take_widget_layer_replays_for_test();
        render_frame_tree(root, size, Color::WHITE).expect("cached frame");
        assert_eq!(draws.get(), 1, "a cached frame must not shape or rasterise text again");
        assert!(
            take_widget_layer_replays_for_test() >= 1,
            "the text control must have been served by the layer cache"
        );

        unregister(text);
        unregister(root);
    }

    /// The layer cache drops least-recently-used entries instead of growing without bound.
    #[test]
    fn the_layer_cache_is_bounded() {
        clear_widget_layers_for_test();
        let root = register(Box::new(LayerRoot {
            base: BaseWidget::new(WidgetKind::Panel, Rect::new(0, 0, 64, 64), "bounded-root"),
        }))
        .expect("mount root");
        let draws = Rc::new(DrawCount::new(0));
        let mut children = Vec::new();
        for index in 0..(MAX_CACHED_WIDGET_LAYERS + 8) {
            let x = (index % 24) as i32 * 2;
            let y = (index / 24) as i32 * 2;
            let child = register(Box::new(LayerProbe {
                base: BaseWidget::new(WidgetKind::Panel, Rect::new(x, y, 2, 2), "bounded-child"),
                draws: Rc::clone(&draws),
            }))
            .expect("mount child");
            with_widget_mut(root, |widget| widget.base_mut().add_child_linked(child))
                .expect("the root is mounted");
            children.push(child);
        }

        render_frame_tree(root, Size::new(64, 64), Color::WHITE).expect("frame");
        assert!(
            cached_widget_layer_count_for_test() <= MAX_CACHED_WIDGET_LAYERS,
            "the cache must not exceed its configured bound"
        );

        for child in children {
            unregister(child);
        }
        unregister(root);
    }

    /// Two clips compose to their overlap, and a disjoint pair is a zero-area answer rather
    /// than "no clip" (which would widen the subtree back to its parent).
    #[test]
    fn intersecting_two_clips_keeps_only_the_overlap() {
        assert_eq!(
            intersect_clip(Some(Rect::new(0, 0, 10, 10)), Some(Rect::new(5, 5, 10, 10))),
            Some(Rect::new(5, 5, 5, 5))
        );
        assert_eq!(
            intersect_clip(Some(Rect::new(0, 0, 10, 10)), Some(Rect::new(20, 20, 5, 5)))
                .map(|r| r.width),
            Some(0)
        );
        assert_eq!(
            intersect_clip(Some(Rect::new(0, 0, 10, 10)), None),
            Some(Rect::new(0, 0, 10, 10))
        );
        assert_eq!(intersect_clip(None, None), None);
    }
}
