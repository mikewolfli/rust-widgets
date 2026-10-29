// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::*;

// ---------------------------------------------------------------------------
// Hit testing: point -> widget
// ---------------------------------------------------------------------------

// A reusable child-id buffer for the tree walks (see `with_child_ids`). One buffer per thread,
// because every walk in this module is iterative and consumes a node's children before asking the
// next node for its own.
thread_local! {
    pub(crate) static CHILD_IDS: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
    // The root's direct children, reused across frames. `render_frame_tree` has to copy them out
    // of the registry before it starts recursing, and a fresh `Vec` per frame was pure allocator
    // traffic on an otherwise static frame (G-36).
    pub(crate) static ROOT_CHILD_IDS: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
    // One child list per recursion depth, reused across frames. A node's children must be copied
    // out of the registry before the painter recurses: `read_widget` holds the registry borrow
    // while its callback runs, so recursing inside it would make the child's `with_widget_mut`
    // panic. Depth-first painting means one buffer per live level is enough (G-36).
    pub(crate) static PAINT_CHILD_BUFFERS: RefCell<Vec<Vec<ObjectId>>> = const { RefCell::new(Vec::new()) };
    // The overlay children moved to the end of the root child list, reused across frames so
    // `ordered_root_children` allocates nothing on an otherwise static frame (G-109). Taken by
    // value for the duration of the reorder: a re-entrant render (user code inside `read_widget`)
    // then takes a fresh buffer instead of failing a borrow.
    pub(crate) static OVERLAY_CHILD_IDS: RefCell<Vec<ObjectId>> = const { RefCell::new(Vec::new()) };
    // Cached `is_visible() && has_open_popup()` per root child (G-316), so the per-frame reorder
    // reads a flag instead of making two virtual calls per child. An entry is cleared when its
    // child asks for a repaint, when a popup grab is set or released (the open/close lifecycle
    // every popup goes through), and when the child is unregistered — see `note_overlay_maybe_changed`.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static OVERLAY_CHILD_FLAGS: RefCell<HashMap<ObjectId, bool>> = RefCell::new(HashMap::new());
}

/// Takes an empty child buffer from the per-depth pool, allocating only on a cold first use.
pub(crate) fn take_paint_children() -> Vec<ObjectId> {
    PAINT_CHILD_BUFFERS.with(|pool| pool.borrow_mut().pop()).unwrap_or_default()
}

/// Returns a child buffer to the pool for the next frame's paint.
pub(crate) fn return_paint_children(mut buffer: Vec<ObjectId>) {
    buffer.clear();
    let _ = PAINT_CHILD_BUFFERS.try_with(|pool| {
        if let Ok(mut pool) = pool.try_borrow_mut() {
            if pool.len() < MAX_PAINT_BUFFER_POOL {
                pool.push(buffer);
            }
        }
    });
}

/// The deepest recursion the buffer pool retains for; beyond it a buffer is dropped rather than
/// retained forever.
pub(crate) const MAX_PAINT_BUFFER_POOL: usize = 64;

/// Runs `f` with `widget_id`'s child ids, reusing a per-thread buffer instead of allocating.
///
/// [`widget_at`] asks every node on the pointer's path for its child list, and the answer used to be
/// a fresh `Vec` each time (`children().to_vec()`). One buffer is enough because the list is
/// consumed inside one iteration. A widget's `contains_point` is user code, though, so a control
/// that re-entered a tree walk would find the buffer busy — that case reads the widget's own slice
/// instead of panicking, which is why this cannot assume the borrow succeeds.
pub(crate) fn with_child_ids<R>(
    widget_id: ObjectId,
    f: impl FnOnce(&[ObjectId]) -> R,
) -> Option<R> {
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
pub(crate) fn widget_at_inner(root: ObjectId, point: Point, trust_root: bool) -> Option<ObjectId> {
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
pub(crate) fn widget_accepts_point(id: ObjectId, point: Point) -> bool {
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
    pub(crate) session: Option<DragSession>,
    pub(crate) hovered: Option<ObjectId>,
    /// The control that supplied the payload. Kept here because [`DragSession`] describes
    /// the gesture, not who owns it, and the completion has to be delivered back to the
    /// source even when the pointer ends over a different control.
    pub(crate) source: Option<ObjectId>,
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
    pub(crate) fn begin(&mut self, pos: Point, viewport: Rect) {
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
    pub(crate) fn update(&mut self, pos: Point, viewport: Rect) {
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
    pub(crate) fn finish(&mut self, pos: Point, viewport: Rect) -> Option<DropEffect> {
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

thread_local! {
    /// Single-slot memo for the last [`widget_at_point`] answer (G-103).
    ///
    /// The key is the *exact* query plus the content epoch. With all four equal the tree,
    /// the geometry and every control's hit rule are unchanged, so the previous answer still
    /// holds. An id-only cache was rejected deliberately: the contract is "deepest hit", and
    /// after the pointer moves onto a child of the cached control an id-only answer would
    /// return the ancestor the user is no longer pointing at.
    pub(crate) static LAST_POINT_HIT: RefCell<Option<(Point, Rect, u64, Option<ObjectId>)>> =
        const { RefCell::new(None) };

    /// Per-window containment cache for [`scan_widget_at_point`].
    ///
    /// The scan runs on every pointer move of a drag, and a fresh `Vec` per call was one
    /// allocation per move (G-318). A nested scan (a control's hit rule querying the tree) gets
    /// its own local buffer instead of re-borrowing this one.
    pub(crate) static POINT_BUCKETS: RefCell<alloc::vec::Vec<(ObjectId, bool)>> =
        const { RefCell::new(alloc::vec::Vec::new()) };
}

/// Returns the deepest mounted control whose own hit rule accepts `point`.
///
/// The drag coordinator has no root id to descend from — a drag may travel between
/// surfaces, and the caller hands it a viewport rather than a tree — so it asks the
/// registry directly. The deepest hit wins, expressed as the smallest rectangle: a child
/// lies inside its parent, and the pointer belongs to the most specific control under it.
/// Equal areas are broken by the later id, matching the "later sibling paints on top"
/// rule the renderer uses.
///
/// # Why the scan is bucketed (G-103)
///
/// The previous form walked every mounted control, paying four dynamic calls per control on
/// every pointer move of a drag. Each mount now records its top-level window (G-106), so the
/// scan can skip a control whose window's client area does not contain the pointer. Controls
/// with no window, or whose cached window is stale after an adoption, stay in the scan — the
/// filter can only *exclude* a control it has positively located in another window.
pub(crate) fn widget_at_point(point: Point, viewport: Rect) -> Option<ObjectId> {
    if !viewport.contains_point(point) {
        return None;
    }
    let epoch = renderable_epoch();
    if let Some((cached_point, cached_viewport, cached_epoch, hit)) =
        LAST_POINT_HIT.with(|slot| *slot.borrow())
    {
        if cached_point == point && cached_viewport == viewport && cached_epoch == epoch {
            return hit;
        }
    }
    let hit = scan_widget_at_point(point);
    LAST_POINT_HIT.with(|slot| *slot.borrow_mut() = Some((point, viewport, epoch, hit)));
    hit
}

/// The bucketed registry scan behind [`widget_at_point`].
pub(crate) fn scan_widget_at_point(point: Point) -> Option<ObjectId> {
    // Reuse the thread's bucket list. `try_borrow_mut` keeps a nested scan from panicking on a
    // borrow conflict; that nested case is rare and pays for a local list (G-318).
    match POINT_BUCKETS.try_with(|cell| match cell.try_borrow_mut() {
        Ok(mut buckets) => {
            buckets.clear();
            Ok(scan_widget_at_point_buffered(point, &mut buckets))
        }
        Err(_) => Err(()),
    }) {
        Ok(Ok(hit)) => hit,
        _ => {
            let mut buckets = alloc::vec::Vec::new();
            scan_widget_at_point_buffered(point, &mut buckets)
        }
    }
}

pub(crate) fn scan_widget_at_point_buffered(
    point: Point,
    buckets: &mut alloc::vec::Vec<(ObjectId, bool)>,
) -> Option<ObjectId> {
    let mut best: Option<(ObjectId, u32)> = None;
    let _ = MOUNTED.try_with(|map_cell| {
        let map = map_cell.borrow();
        for (id, entry) in map.iter() {
            let widget = entry.widget.as_ref();
            // A cached window is only usable while the direct parent it was resolved from is
            // still this widget's parent (G-106); otherwise fall back to an unbucketed scan.
            let bucket = match entry.top_window {
                Some((resolved_parent, window)) if resolved_parent == widget.parent() => window,
                _ => None,
            };
            if let Some(window) = bucket {
                if !bucket_contains_point(&map, window, point, buckets) {
                    continue;
                }
            }
            if !widget.is_visible() || !widget.is_enabled() || !widget.contains_point(point) {
                continue;
            }
            let rect = widget.geometry();
            let area = rect.width.saturating_mul(rect.height);
            let better = match best {
                None => true,
                Some((best_id, best_area)) => {
                    area < best_area || (area == best_area && *id >= best_id)
                }
            };
            if better {
                best = Some((*id, area));
            }
        }
    });
    best.map(|(id, _)| id)
}

/// Whether `window`'s client area contains `point`.
///
/// A window's children are written in **client** coordinates starting at its top left (see
/// [`frame_origin`]), so containment is the window's size rather than its screen position.
/// A window that cannot be resolved is treated as containing the point: excluding a live
/// control because its bucket is unreadable would silently change the hit result.
pub(crate) fn bucket_contains_point(
    map: &HashMap<ObjectId, Mounted>,
    window: ObjectId,
    point: Point,
    cache: &mut alloc::vec::Vec<(ObjectId, bool)>,
) -> bool {
    if let Some((_, contains)) = cache.iter().find(|(candidate, _)| *candidate == window) {
        return *contains;
    }
    let contains = match map.get(&window) {
        Some(entry) => {
            let geometry = entry.widget.geometry();
            Rect::new(0, 0, geometry.width, geometry.height).contains_point(point)
        }
        None => true,
    };
    cache.push((window, contains));
    contains
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
pub(crate) fn is_descendant_of(ancestor: ObjectId, descendant: ObjectId) -> bool {
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
pub(crate) fn dispatch_hover_transition(hovered: Option<ObjectId>, point: Point) {
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
        // The manager is the library's one tooltip presenter, so the pointer path — not the
        // host and not the control — is what feeds it (COMP-09).
        #[cfg(full_widgets)]
        forward_hover_to_tooltip_managers(hovered, point);
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

#[cfg(full_widgets)]
thread_local! {
    /// Mounted [`TooltipManager`](crate::widget::companions::TooltipManager) ids.
    ///
    /// A hover transition used to filter the whole [`MOUNTED`] map for this kind, which is
    /// O(every mounted control) per pointer boundary crossing while the number of managers is
    /// almost always zero or one. The set is maintained beside the registry (written by
    /// [`finish_registration`], removed by [`unregister`]) so the transition iterates only the
    /// managers that exist and can return before reading any tooltip text when there are none.
    #[allow(clippy::missing_const_for_thread_local)]
    pub(crate) static TOOLTIP_MANAGERS: RefCell<IdSet> = RefCell::new(IdSet::new());
}

/// Adds `id` to, or removes it from, the mounted tooltip-manager set.
#[cfg(full_widgets)]
pub(crate) fn note_tooltip_manager(id: ObjectId, is_manager: bool) {
    let _ = TOOLTIP_MANAGERS.try_with(|set| {
        if is_manager {
            set.borrow_mut().insert(id);
        } else {
            set.borrow_mut().remove(&id);
        }
    });
}

/// Registers the hovered control's `tooltip` property with every mounted [`TooltipManager`].
///
/// [`TooltipManager`]: crate::widget::companions::TooltipManager
///
/// # Why the runtime owns this bridge
///
/// Until this existed a tooltip was reachable only two ways: a host called
/// `TooltipManager::register` for each control it cared about (the wiring had to be repeated
/// per control and could silently fall out of step with the tree), or the control painted its
/// own bubble. The runtime is the one place that already knows which widget the pointer is
/// over, so it is the natural producer for "the hovered control's tooltip". The manager stays
/// the only **presenter**: this function moves a registration and nothing else, so the delay,
/// duration, follow-pointer placement and drawing all remain in one implementation. The pointer
/// position is forwarded with the registration so a follow-pointer bubble anchors where the
/// hover actually began, instead of at the manager's own origin.
///
/// # The two passes, and why
///
/// The manager ids are copied out of [`TOOLTIP_MANAGERS`] first, then the tooltip text is read,
/// then each manager is mutated. Mutating while the registry's map is borrowed would alias the
/// live entries — and a manager that requests a repaint reads the registry. The set is the
/// registry's own record of which mounted controls are managers, so the function neither scans
/// `MOUNTED` nor re-observes every widget's animating bit (which a pointer transition has no
/// business rewriting). When the set is empty the function returns before reading the tooltip
/// property at all.
#[cfg(full_widgets)]
pub(crate) fn forward_hover_to_tooltip_managers(hovered: Option<ObjectId>, point: Point) {
    use crate::widget::capability::coercion::widget_as_mut;
    use crate::widget::companions::TooltipManager;

    // No mounted manager means the hover path owns no work at all: return before the tooltip
    // property is even read, so a pointer crossing costs one set probe and no allocation.
    let managers: Vec<ObjectId> = TOOLTIP_MANAGERS
        .try_with(|set| {
            let set = set.borrow();
            if set.0.is_empty() {
                None
            } else {
                Some(set.0.clone())
            }
        })
        .ok()
        .flatten()
        .unwrap_or_default();
    if managers.is_empty() {
        return;
    }

    // The published spelling is the control's `tooltip` **property**: `base_property_get`
    // answers that name from the same field `Widget::tooltip` reads, so a control with a
    // contract and one without cannot disagree about what its tooltip says.
    let text = hovered.and_then(|id| {
        read_widget(id, |widget| {
            let from_property =
                widget.properties_dyn().and_then(|properties| match properties.get("tooltip") {
                    Ok(crate::widget::CapabilityValue::String(text)) => Some(text),
                    _ => None,
                });
            from_property.unwrap_or_else(|| crate::compat::String::from(widget.tooltip()))
        })
    });

    for id in managers {
        let _ = with_widget_mut(id, |widget| {
            if let Some(manager) = widget_as_mut::<TooltipManager>(widget) {
                manager.note_runtime_hover(hovered, text.as_deref());
                manager.note_pointer(point);
            }
        });
    }
}

/// Returns the widget the pointer is currently over, if any.
pub fn hovered_widget() -> Option<ObjectId> {
    HOVERED.with(|last| *last.borrow())
}
