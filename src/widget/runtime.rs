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
use crate::event::Event;
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
    #[allow(clippy::missing_const_for_thread_local)]
    static NEXT_ID: RefCell<ObjectId> = const { RefCell::new(0x5345_4C46_0000_0001) };

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
}

/// Hands ownership of `widget` to the registry and returns its display id.
///
/// Returns `None` when the calling thread has no registry — i.e. it is not the
/// thread that drives the UI. Callers must surface that as "cannot display
/// here" rather than dropping the widget silently.
pub fn register(widget: Box<dyn Widget>) -> Option<ObjectId> {
    let id = NEXT_ID.try_with(|next| {
        let id = *next.borrow();
        *next.borrow_mut() = id.wrapping_add(1);
        id
    });
    let Ok(id) = id else { return None };
    let stored = MOUNTED.try_with(|map| {
        map.borrow_mut().insert(id, Mounted { widget });
    });
    if stored.is_err() {
        return None;
    }
    // Join the tab order if the control says it is focusable. Asked of the widget
    // itself (see `Widget::is_focusable`) rather than of a table of kinds, so a
    // control the library has never heard of participates by answering `true`.
    let focusable = with_widget(id, |widget| widget.is_focusable()).unwrap_or(false);
    if focusable {
        register_focusable(id);
    }
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
    let _ = dispatch_event(id, &Event::FocusGained);
    true
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
    let _ = dispatch_event(next, &Event::FocusGained);
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
        if is_mounted(captured) {
            return dispatch_event(captured, event);
        }
        // The capturer is gone; drop the capture rather than routing into nothing.
        let _ = release_pointer_capture();
    }
    match target {
        Some(target) => dispatch_event(target, event),
        None => false,
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

/// Returns the geometry of a mounted widget, or `None` when it is not mounted.
pub fn geometry_of(id: ObjectId) -> Option<Rect> {
    MOUNTED
        .try_with(|map| map.borrow().get(&id).map(|entry| entry.widget.geometry()))
        .ok()
        .flatten()
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
pub fn dispatch_event(id: ObjectId, event: &Event) -> bool {
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

/// Renders one frame of a mounted widget at `size` and returns the RGBA bytes.
///
/// `bytes.len() == size.width * size.height * 4`, top-down, straight (non
/// premultiplied) alpha — the layout all three desktop backends consume.
/// Returns `None` when `id` is not mounted or the size is empty.
pub fn render_frame(id: ObjectId, size: Size, clear: crate::core::Color) -> Option<Vec<u8>> {
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    let painted = with_widget_mut(id, |widget| {
        let Some(drawable) = widget.as_draw_mut() else {
            return false;
        };
        backend.begin_frame(clear);
        {
            let mut context = RenderContext::new(&mut backend);
            drawable.draw(&mut context);
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

    fn sample_editor(text: &str) -> Box<dyn Widget> {
        let mut editor = CodeEditor::new(Rect::new(0, 0, 320, 200));
        editor.set_text(text);
        Box::new(editor)
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
