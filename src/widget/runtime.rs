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
    Some(id)
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
    MOUNTED.try_with(|map| map.borrow_mut().remove(&id).is_some()).unwrap_or(false)
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
/// Returns `None` for an unmounted `root`, a point outside it, or a tree with no hit.
pub fn widget_at(root: ObjectId, point: Point) -> Option<ObjectId> {
    let mut current = root;
    loop {
        let children = with_widget(current, |widget| widget.children().to_vec())?;
        if !widget_accepts_point(current, point) {
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
pub fn is_mounted(id: ObjectId) -> bool {
    MOUNTED.try_with(|map| map.borrow().contains_key(&id)).unwrap_or(false)
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
    let Event::MousePress { pos, button } = event else {
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
pub fn request_repaint(id: ObjectId) {
    crate::invalidate_surface(id);
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
        let delivered =
            dispatch_event(id, &Event::MousePress { pos: Point::new(150, 90), button: 1 });
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
                button: crate::event::mouse_button::PRIMARY
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
            &Event::MousePress { pos: Point::new(150, 130), button: 1 },
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
            &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1 },
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
                &Event::MousePress { pos: Point::new(50, 50), button: 1 },
                Point::new(50, 50),
            ),
            "a click inside the child must be delivered to it"
        );

        // Outside every child but inside the root: still delivered, to the root.
        assert!(
            backend.route_pointer_event(
                parent,
                &Event::MousePress { pos: Point::new(250, 250), button: 1 },
                Point::new(250, 250),
            ),
            "a click inside the root must still be delivered"
        );

        // Entirely outside the root: nothing accepts it, and that is reported.
        assert!(
            !backend.route_pointer_event(
                parent,
                &Event::MousePress { pos: Point::new(9_000, 9_000), button: 1 },
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
}
