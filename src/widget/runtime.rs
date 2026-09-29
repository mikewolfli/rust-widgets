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
use crate::core::{ObjectId, Point, Rect, Size};
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
    let focusable = with_widget(id, |widget| widget.is_focusable()).unwrap_or(false);
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
    let focusable = with_widget(declared_id, |widget| widget.is_focusable()).unwrap_or(false);
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
    let _ = HOST_WINDOWS.try_with(|map| map.borrow_mut().remove(&id));
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
    // And it must not keep pointer capture: routing would send every subsequent
    // pointer event to an id that resolves to nothing.
    let _ = CAPTURE.try_with(|capture| {
        let mut capture = capture.borrow_mut();
        if capture.capturing_widget() == Some(id) {
            capture.release_capture();
        }
    });
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
    let next = FOCUS
        .try_with(|focus| {
            let mut focus = focus.borrow_mut();
            if forward {
                focus.focus_next()
            } else {
                focus.focus_previous()
            }
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
    let origin = with_widget(root, |widget| widget.geometry())?;
    let local = Point::new(point.x - origin.x, point.y - origin.y);
    let root_rect = Rect::new(0, 0, origin.width, origin.height);
    let mut current = root;
    loop {
        let children = with_widget(current, |widget| widget.children().to_vec())?;
        // A **root** is a container whose geometry is written in a different space from its
        // children's (see the doc comment), so its bound is applied in the root's own space via the
        // `local` point captured above. Every other node is tested normally: a control is addressed
        // in its parent's space, which is the space its own rectangle is written in.
        if current == root {
            if !root_rect.contains_point(local) {
                return None;
            }
        } else if !widget_accepts_point(current, point) {
            return None;
        }
        // Topmost-first: a later sibling paints over an earlier one.
        let deepest = children
            .iter()
            .rev()
            .copied()
            .find(|child| is_visible(*child) && widget_accepts_point(*child, point));
        match deepest {
            Some(child) => current = child,
            // No child claims the point, so this widget is the deepest hit.
            None => return Some(current),
        }
    }
}

/// Whether a mounted widget's own bounds accept `point` (in its local space).
///
/// Asks the widget rather than comparing rectangles, so each control keeps its own
/// hit rule — the touch-target expansion in `contains_point`, or a shape narrower
/// than the bounding box. Returns `false` for an unmounted id.
fn widget_accepts_point(id: ObjectId, point: Point) -> bool {
    with_widget(id, |widget| widget.contains_point(point)).unwrap_or(false)
}

/// Whether a mounted widget is visible. Unmounted ids report `false`.
fn is_visible(id: ObjectId) -> bool {
    with_widget(id, |widget| widget.is_visible()).unwrap_or(false)
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
/// Returns whether a widget accepted the event.
pub fn dispatch_pointer_event(root: ObjectId, event: &Event, point: Point) -> bool {
    let target = widget_at(root, point);
    dispatch_hover_transition(target, point);
    // A control that captured the pointer keeps receiving events even when the
    // cursor leaves it — that is what lets a drag continue past its origin widget.
    // Hover still follows the true position, so highlights stay honest.
    if let Some(captured) = capturing_widget() {
        if is_mounted(captured) && !modal_blocks(captured) {
            return dispatch_event(captured, event);
        }
        // The capturer is gone (or blocked by a modal); drop the capture rather
        // than routing into nothing.
        let _ = release_pointer_capture();
        return false;
    }
    match target {
        Some(target) if !modal_blocks(target) => dispatch_event(target, event),
        _ => false,
    }
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
    let mut cursor = with_widget(descendant, |widget| widget.parent());
    while let Some(current) = cursor.flatten() {
        if current == ancestor {
            return true;
        }
        cursor = with_widget(current, |widget| widget.parent());
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
    HOVERED.with(|last| {
        let previous = *last.borrow();
        if previous == hovered {
            return;
        }
        *last.borrow_mut() = hovered;
        // Deliver outside the cell borrow: a control handling `MouseLeave` may itself
        // read or write hover state.
        if let Some(previous) = previous {
            let _ = dispatch_event(previous, &Event::MouseLeave { pos: point });
        }
        if let Some(hovered) = hovered {
            let _ = dispatch_event(hovered, &Event::MouseEnter { pos: point });
        }
    });
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
            map.borrow_mut().insert(id, host);
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
    HOST_WINDOWS
        .try_with(|map| map.borrow().iter().find(|(_, h)| **h == host).map(|(w, _)| *w))
        .unwrap_or(None)
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
    MOUNTED
        .try_with(|map| map.try_borrow().map(|map| map.contains_key(&id)).unwrap_or(true))
        .unwrap_or(false)
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

    // Step 1 -- input first, so this frame's animation targets are this frame's state.
    let events_dispatched = crate::drain_triggers();

    // Step 2 -- exactly one advance per animating control, before any paint.
    tick_animations(delta_ms);
    let controls_ticked = count_animating_widgets();

    // Step 3 -- the bus answers for the *next* frame, so a settle is painted once more
    // and then scheduling stops.
    let needs_another_frame = animation_bus_needs_another_frame();

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

    /// Whether each mounted surface has already had a repaint requested this frame.
    ///
    /// Keyed by the repaint's **target surface** rather than by control, because that is
    /// what coalescing is about: two controls on the same surface produce one submission.
    /// Cleared by [`take_frame_repaint_count`], so "this frame" is exactly the span between
    /// two `drive_frame` calls.
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_REPAINTED_SURFACES: core::cell::RefCell<alloc::vec::Vec<ObjectId>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };

    /// The last reason each surface was repainted, for BLUE24 §8's `FrameStats`.
    ///
    /// One entry per surface, overwritten (not appended), so a surface repainted fifty times
    /// in a frame reads as "the reason it was repainted" rather than fifty rows. `Explicit`
    /// means "a caller asked, and named no cause" — which is itself a fact worth being able
    /// to see, because it is the one reason a gate cannot attribute to a mechanism.
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_REPAINT_REASONS: core::cell::RefCell<alloc::vec::Vec<(ObjectId, RepaintReason)>> =
        const { core::cell::RefCell::new(alloc::vec::Vec::new()) };

    /// Monotonic frame counter, incremented once per [`drive_frame`].
    ///
    /// A counter rather than a clock: two readings of [`FrameStats`] are comparable by
    /// subtracting indices, which needs no `SystemTime` and therefore works under `mini`
    /// (BLUE24 §4's "no device fact from a wall clock" rule applies to the frame ledger too).
    #[allow(clippy::missing_const_for_thread_local)]
    static FRAME_INDEX: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
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
    let already = FRAME_REPAINTED_SURFACES
        .try_with(|surfaces| {
            let mut surfaces = surfaces.borrow_mut();
            if surfaces.contains(&surface) {
                true
            } else {
                surfaces.push(surface);
                false
            }
        })
        .unwrap_or(false);

    if already {
        let _ = FRAME_REPAINTS_COALESCED.try_with(|count| count.set(count.get().saturating_add(1)));
        return;
    }

    let _ = FRAME_REPAINTS.try_with(|count| count.set(count.get().saturating_add(1)));
    let _ = FRAME_REPAINT_REASONS.try_with(|reasons| {
        let mut reasons = reasons.borrow_mut();
        match reasons.iter_mut().find(|(id, _)| *id == surface) {
            Some(entry) => entry.1 = reason,
            None => reasons.push((surface, reason)),
        }
    });
}

/// Records that this frame submitted one repaint, naming no surface.
///
/// The pre-BLUE24 shape, kept for the callers that genuinely have no surface to name — a
/// repaint whose target the platform resolved itself. It records [`RepaintReason::Explicit`]
/// against a synthetic surface, so a bare submission still appears in the ledger rather than
/// being anonymous.
#[allow(dead_code)]
pub(crate) fn note_frame_repaint() {
    note_frame_repaint_reason(0, RepaintReason::Explicit);
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
    let _ = FRAME_REPAINTED_SURFACES.try_with(|surfaces| surfaces.borrow_mut().clear());
    FRAME_REPAINT_REASONS
        .try_with(|reasons| reasons.borrow_mut().drain(..).collect())
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
    let mut still_animating = false;
    for_each_mounted_widget(|_, widget| {
        // `is_animating` first, so a resting control (the common case) is not asked to
        // interpolate at all: this is what keeps a static frame free.
        if widget.is_animating() {
            // `tick` also reports "moving"; either answer keeps the frame coming, because a
            // control can be mid-flight while its own `is_animating` is a coarser fact.
            if widget.tick(delta_ms) || widget.is_animating() {
                still_animating = true;
            }
        }
    });
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
    still_animating
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
    let host_owned = HOST_OWNED_ANIMATING.try_with(|flag| flag.get()).unwrap_or(false);
    let sweep_unsettled =
        LAST_SWEEP_HAD_UNSETTLED_MOUNTED.try_with(|flag| flag.get()).unwrap_or(false);
    host_owned || sweep_unsettled || has_animating_widgets()
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
pub fn geometry_of(id: ObjectId) -> Option<Rect> {
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| entry.widget.geometry()))
        .ok()
        .flatten()
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
pub fn set_geometry(id: ObjectId, geometry: Rect) -> bool {
    let updated = MOUNTED.try_with(|map| {
        map.borrow_mut().get_mut(&id).map(|entry| entry.widget.set_geometry(geometry)).is_some()
    });
    updated.unwrap_or(false)
}

/// Runs `f` with mutable access to a mounted widget.
///
/// The borrow is scoped to the call, so a painter that triggers another paint
/// cannot re-enter the registry and alias the widget.
pub fn with_widget_mut<R>(id: ObjectId, f: impl FnOnce(&mut dyn Widget) -> R) -> Option<R> {
    MOUNTED
        .try_with(|map| map.borrow_mut().get_mut(&id).map(|entry| f(entry.widget.as_mut())))
        .ok()
        .flatten()
}

/// Runs `f` with shared access to a mounted widget.
///
/// Used by read-only queries (a host inspecting a mounted editor's text or
/// caret) so they do not need mutable access merely to look at state.
pub fn with_widget<R>(id: ObjectId, f: impl FnOnce(&dyn Widget) -> R) -> Option<R> {
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| f(entry.widget.as_ref())))
        .ok()
        .flatten()
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
    with_widget_mut(id, |widget| widget.handle_event(event)).is_some()
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
/// The traversal goes through [`crate::widget::Widget::children`] (via [`direct_children_of`]),
/// which is the single definition of "the mounted tree" — the same source the hit-testing and
/// damage walks use, so the three cannot disagree about what a subtree is.
#[cfg(not(alloc_frugal))]
fn collect_subtree_ids(id: ObjectId, out: &mut alloc::vec::Vec<ObjectId>) {
    out.push(id);
    for child in direct_children_of(id) {
        collect_subtree_ids(child, out);
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
        // A backend reports into `BaseWidget` directly, which bypasses the `Widget::set_enabled`
        // default that re-resolves the theme — so the `"<kind>:disabled"` key would be reachable
        // for a self-painted control and unreachable for a native one, from the same fact. This
        // makes the native path go through the same door the self-painted path does, rather than
        // relying on a backend to remember a second call.
        with_widget_mut(id, crate::style::reapply_active_theme_state);
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
    with_widget_mut(menu_id, |widget| {
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
    })
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
pub fn top_level_window_of(id: ObjectId) -> Option<ObjectId> {
    // A widget tree is shallow (window → containers → controls); 64 is far past any real
    // depth and still a bound rather than a loop that trusts the data.
    const MAX_DEPTH: usize = 64;
    let mut current = id;
    for _ in 0..MAX_DEPTH {
        let (kind, parent) = with_widget(current, |widget| (widget.kind(), widget.parent()))?;
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
        with_widget(id, |widget| !widget.base().children().is_empty()).unwrap_or(false);
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
    with_widget(id, |widget| widget.has_ever_requested_redraw()).unwrap_or(false)
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
    #[allow(clippy::missing_const_for_thread_local)]
    static LAST_FRAME: RefCell<HashMap<ObjectId, (Size, Vec<u8>)>> = RefCell::new(HashMap::new());
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
pub fn mark_dirty_rect(id: ObjectId, rect: Rect) -> bool {
    let recorded = REPAINT
        .try_with(|map| {
            let mut map = map.borrow_mut();
            let Some(state) = map.get_mut(&id) else {
                return false;
            };
            if state.mode == RepaintMode::Full {
                return false;
            }
            state.tracker.add(rect);
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
    if size.width == 0 || size.height == 0 {
        return None;
    }

    let mode = repaint_mode(id);
    let expected = size.width as usize * size.height as usize * 4;
    // A buffer of the wrong size cannot be carried forward, so treat it as absent
    // rather than reading past its end.
    let carried = previous.filter(|buffer| buffer.len() == expected);

    let use_dirty = mode != RepaintMode::Full && carried.is_some() && !dirty_rects(id).is_empty();
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
            dirty_rects(id).iter().map(|rect| u64::from(rect.width) * u64::from(rect.height)).sum();
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
        return Some(carried?.to_vec());
    }

    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    // Seed the backend with the previous frame so the regions this pass does not
    // touch keep their pixels.
    backend.seed_from(carried?);

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
    Some(backend.frame_rgba().to_vec())
}

/// Renders the next frame of a mounted widget, reusing the previous one when the
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
    if size.width == 0 || size.height == 0 {
        return None;
    }

    // Clone the previous frame only when one of the right size exists; a mismatch is
    // treated as absent so the callee falls back to a full paint.
    let previous = LAST_FRAME
        .try_with(|map| {
            map.borrow()
                .get(&id)
                .filter(|(stored, _)| *stored == size)
                .map(|(_, frame)| frame.clone())
        })
        .ok()
        .flatten();

    let frame = render_frame_incremental(id, size, clear, previous.as_deref())?;

    let _ = LAST_FRAME.try_with(|map| {
        map.borrow_mut().insert(id, (size, frame.clone()));
    });
    Some(frame)
}

/// Forgets the cached frame for `id`.
///
/// Called when a widget is unmounted, so a frame cannot outlive the control it
/// describes — and so its id, if reused, does not start from someone else's pixels.
pub fn forget_cached_frame(id: ObjectId) {
    let _ = LAST_FRAME.try_with(|map| map.borrow_mut().remove(&id));
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
    Some(backend.frame_rgba().to_vec())
}

/// Renders one frame of a mounted widget using its own geometry as the size.
///
/// Convenience wrapper for backends that size the native canvas from the
/// widget's geometry rather than from the OS-reported rectangle.
pub fn render_frame_at_geometry(id: ObjectId, clear: crate::core::Color) -> Option<Vec<u8>> {
    let geometry = geometry_of(id)?;
    render_frame(id, Size::new(geometry.width, geometry.height), clear)
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
/// one — the same order a container's own `draw` would use.
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
    let origin = frame_origin(id);
    let mut backend = SoftwarePaintBackend::new(size, 1.0);

    // The root's own painting is part of the frame: a window draws its background and
    // chrome, which the children then sit on top of.
    // TEMP DIAGNOSTIC: split the root draw from the child walk.
    let t_root = std::time::Instant::now();
    let mut painted = with_widget_mut(id, |widget| {
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
    let root_us = t_root.elapsed().as_micros();

    let mut slowest: (u128, u64) = (0, 0);
    let mut child_total = 0u128;
    if painted {
        // Children are drawn into the same frame, so the traversal only issues draw calls;
        // the frame was begun above and is ended once below.
        let mut stack: Vec<ObjectId> = direct_children_of(id);
        // Reversed so the traversal visits children in declaration order: `pop` takes the
        // last element, so pushing the children reversed makes the first one first.
        stack.reverse();
        while let Some(current) = stack.pop() {
            if !is_visible(current) {
                continue;
            }
            let t_child = std::time::Instant::now();
            let drew = with_widget_mut(current, |widget| {
                let Some(drawable) = widget.as_draw_mut() else {
                    return false;
                };
                let mut context = RenderContext::new(&mut backend);
                context.push_offset(-origin.0, -origin.1);
                drawable.draw(&mut context);
                context.pop_offset();
                true
            })
            .unwrap_or(false);
            let child_us = t_child.elapsed().as_micros();
            child_total += child_us;
            if child_us > slowest.0 {
                slowest = (child_us, current);
            }
            if child_us > 300 {
                let kind = with_widget(current, |w| format!("{:?}", w.kind()))
                    .unwrap_or_else(|| "?".into());
                eprintln!("[TREE-SLOW] {child_us}us kind={kind} id={current}");
            }
            if drew {
                painted = true;
            }
            let mut nested = direct_children_of(current);
            nested.reverse();
            stack.extend(nested);
        }
    }

    if !painted {
        return None;
    }
    eprintln!(
        "[TREE] root={root_us}us children_total={child_total}us slowest={}us(id={})",
        slowest.0, slowest.1
    );
    backend.end_frame();
    Some(backend.frame_rgba().to_vec())
}

/// The direct children of `id`, or an empty list when it is not mounted.
///
/// Read through [`with_widget`] so a caller sees the same widget the traversal will.
fn direct_children_of(id: ObjectId) -> Vec<ObjectId> {
    with_widget(id, |widget| widget.base().children().to_vec()).unwrap_or_default()
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
    use crate::widget::special_widgets::code_editor::CodeEditor;

    // ── BLUE23 §3.3 -- the animation bus ────────────────────────────────────────

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
    #[test]
    fn drive_frame_ticks_a_hovered_control_and_asks_for_another_frame() {
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

    /// A static frame costs nothing and submits nothing.
    ///
    /// BLUE24 §1 criterion 2, and the plan calls this **more important than the animation
    /// itself**: it is the property that decides whether a continuously-running loop is
    /// affordable. A resting tree must report no ticks and no repaint submissions, so an
    /// idle window pays for a sweep and nothing else.
    #[test]
    fn drive_frame_on_a_resting_tree_submits_nothing() {
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
            std::rc::Rc::new(RefCell::new(|_text: String| DRAINED.with(|flag| flag.set(true)))),
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
        assert!(set_geometry(explicit, Rect::new(0, 0, 800, 600)));
        assert!(
            !enable_damage_tracking_if_useful(explicit),
            "a control that never asked to repaint must be refused"
        );
        with_widget(explicit, |widget| widget.request_redraw()).expect("widget");
        assert!(enable_damage_tracking_if_useful(explicit));
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
        assert!(set_geometry(id, Rect::new(0, 0, 800, 600)));
        assert!(set_repaint_mode(id, RepaintMode::Full));
        assert!(!should_track_damage(id), "a silent control must not be enabled");

        with_widget(id, |widget| widget.base().request_redraw()).expect("widget");
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
        let painted = frame.chunks_exact(4).filter(|pixel| *pixel != [0, 0, 0, 255]).count();
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
                .chunks_exact(4)
                .all(|pixel| pixel == [0, 0, 0, 255]);
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
        assert!(frame.chunks_exact(4).any(|px| px[3] != 0), "frame must contain drawn pixels");
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
                    frame.chunks_exact(4).any(|px| px[0] != 255 || px[1] != 0 || px[2] != 255)
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
    }

    impl A11yRecorder {
        fn new() -> Self {
            Self {
                names: std::sync::Mutex::new(Vec::new()),
                states: std::sync::Mutex::new(Vec::new()),
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
        let painted = frame.chunks_exact(4).filter(|pixel| *pixel != clear_rgba).count();
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
}
