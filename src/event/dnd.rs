// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Drag-and-drop: a payload, a placement, and a target that decides whether to
//! accept one.
//!
//! # Why this module exists
//!
//! Twenty-eight files in this crate hand-roll the same gesture: a `MousePress`
//! records an origin, `MouseMove` updates an offset, and `MouseRelease` commits.
//! `dockwidget` drags a window, `splitter` drags a divider, `slider` and
//! `scrollbar` drag their thumbs, `bezier_curve_editor` drags control points,
//! `freeform_shape` drags vertices, `modal_bottom_sheet` drags itself closed, and
//! so on. Each implementation answers the same three questions on its own:
//! *what* is being moved, *where* it can land, and *what happens* when it does.
//!
//! This module answers them once. It is deliberately **not** a widget and produces
//! no `WidgetKind`: it is the shared vocabulary a control uses when it wants to be
//! a source or a destination.
//!
//! # What it does not do
//!
//! It does not deliver events. Which backend events constitute a drag is a control
//! decision (`Event::Drag` under `feature = "touch"`, or the mouse trio), so
//! [`DragSession`] is driven by whoever owns the gesture. That keeps the module
//! free of any assumption about input devices, which is what lets a mouse drag and
//! a touch drag share one state machine.
//!
//! # Example
//!
//! ```ignore
//! // The source: serialise what is being moved and begin a session.
//! let payload = DragPayload::new("card", "task-42").with_label("Fix the parser");
//! let mut session = DragSession::begin(payload, Point::new(x, y));
//!
//! // The target: declare what it accepts.
//! fn can_accept(&self, payload: &DragPayload) -> bool {
//!     payload.type_id == "card"
//! }
//!
//! // On release, the engine asks each target in turn and commits the first that
//! // accepts.
//! if let Some(effect) = session.drop_on(&mut target, Point::new(x, y)) {
//!     // `effect` says whether the source should delete its copy.
//! }
//! ```

use crate::compat::String;
use crate::core::Point;

/// What a drag carries.
///
/// # Why a type id and an opaque value rather than a generic parameter
///
/// A payload travels from a source the engine does not know to a target it also
/// does not know, through a single runtime list. A generic `DragPayload<T>` would
/// make the list heterogeneous — the same reason the event queue is not generic
/// over its contents. The `type_id` is the discriminator: a target compares it
/// against the id it expects, which is exactly the check a typed payload would
/// perform, only written down.
///
/// The derive is `PartialEq` without `Eq` because the payload carries a [`Point`],
/// and the geometry types are floating-point.
#[derive(Debug, Clone, PartialEq)]
pub struct DragPayload {
    /// The kind of thing being dragged, e.g. `"card"`, `"tab"`, `"file"`.
    ///
    /// This is the field a target filters on. It is a `String` rather than an enum
    /// because a library cannot enumerate its callers' domain vocabulary, and a
    /// target only ever compares it for equality.
    pub type_id: String,
    /// A stable identifier for the particular item, unique within `type_id`.
    ///
    /// Kept separate from `type_id` because a target usually accepts *some* items
    /// of a type and refuses others (a column with a WIP limit accepts a card but
    /// not when it is full) — and that decision needs to name the item.
    pub item_id: String,
    /// A human-readable label, for a drag preview or an accessibility announcement.
    pub label: String,
    /// The source's own coordinates of the drag origin, in the source's space.
    ///
    /// Carried so a target can describe a drop in the source's terms (\"after item
    /// 7 of this list\") without having to know which control started the drag.
    pub origin: Point,
}

impl DragPayload {
    /// Creates a payload with a type and an item id, and no label.
    pub fn new(type_id: impl Into<String>, item_id: impl Into<String>) -> Self {
        Self {
            type_id: type_id.into(),
            item_id: item_id.into(),
            label: String::new(),
            origin: Point::new(0, 0),
        }
    }

    /// Sets the human-readable label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Sets the drag origin.
    pub fn with_origin(mut self, origin: Point) -> Self {
        self.origin = origin;
        self
    }

    /// Returns whether this payload is of `type_id`.
    ///
    /// The comparison a target makes, named so both sides spell it the same way.
    pub fn is_type(&self, type_id: &str) -> bool {
        self.type_id == type_id
    }
}

/// What a completed drop means for the source.
///
/// # Why `None` is a variant rather than an `Option` at the call site
///
/// A drop that was refused part-way through a gesture is a *result*, not the
/// absence of one: the caller has to restore the dragged item to where it was.
/// Modelling it as a variant keeps that branch in the same `match` as the others,
/// where the compiler can point out a forgotten arm — an `Option` around the enum
/// would make the restore path the `None` arm of a different type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropEffect {
    /// The target agreed to nothing; the source must restore its state.
    #[default]
    None,
    /// The item was copied; the source keeps its original.
    Copy,
    /// The item was moved; the source should remove its original.
    Move,
    /// A link or reference was created; the source keeps its original.
    Link,
}

impl DropEffect {
    /// Returns whether the source should remove its original after this drop.
    ///
    /// Exposed because it is the one decision every source has to make from the
    /// effect, and encoding it as a match at each call site is how one of them ends
    /// up deleting a copy it should have kept.
    pub fn consumes_source(self) -> bool {
        matches!(self, DropEffect::Move)
    }

    /// Returns whether the drop was accepted at all.
    pub fn is_accepted(self) -> bool {
        !matches!(self, DropEffect::None)
    }
}

/// A control that can receive a dropped payload.
///
/// # Why three methods and not one
///
/// `can_accept` and `on_drop` are separate because the answer to \"may I take
/// this\" must be checkable **during** a drag, before any state changes — a drop
/// preview has to be drawn on every pointer move. Merging them would force the
/// target to either commit speculatively or keep the answer in a field beside the
/// question, which is the duplication this trait removes.
///
/// `preview_rect` is separate for the same reason: where a drop would land is not
/// where the target's own rectangle is (a list drops *between* rows), and a preview
/// drawn from the target's bounds would point at the wrong place.
pub trait DropTarget {
    /// Returns whether this target will accept `payload`.
    ///
    /// Must not mutate state: it is called on every pointer move to drive the
    /// preview. A target that refuses must return `false` rather than accept and
    /// then reject in [`Self::on_drop`], because the preview would have promised
    /// something the drop does not deliver.
    fn can_accept(&self, payload: &DragPayload) -> bool;

    /// Commits a drop of `payload` at `pos`, returning what the source should do.
    ///
    /// Called only after [`Self::can_accept`] returned `true` for the same payload,
    /// so an implementation may rely on that check having passed. Returning
    /// [`DropEffect::None`] is still meaningful: a target can discover during the
    /// commit that it cannot proceed (a pending async validation) and the source
    /// then restores.
    fn on_drop(&mut self, payload: &DragPayload, pos: Point) -> DropEffect;

    /// Returns the rectangle a drop of `payload` at `pos` would occupy, when the
    /// target can show a preview.
    ///
    /// `None` means \"no preview\", which is the honest default for a target that
    /// accepts a drop but has no meaningful landing zone to highlight.
    fn preview_rect(&self, payload: &DragPayload, pos: Point) -> Option<crate::core::Rect> {
        let _ = (payload, pos);
        None
    }
}

/// The state of one in-progress drag.
///
/// # What this holds, and what it deliberately does not
///
/// It holds the payload, where the pointer is now, where it started, and which
/// target is currently under it. It does **not** hold the list of targets: a drag
/// travels across controls that do not know about each other, so resolving the
/// target is the owner's job (it is the only party that can see them all).
///
/// That split is what lets a `KanbanBoard` drag a card onto a list that lives in a
/// different branch of the widget tree without either control importing the other.
#[derive(Debug, Clone)]
pub struct DragSession {
    payload: DragPayload,
    /// Where the gesture began, in the owning control's coordinates.
    start: Point,
    /// Where the pointer is now.
    current: Point,
    /// Whether the pointer has travelled far enough to count as a drag rather than
    /// a click.
    active: bool,
}

impl DragSession {
    /// Begins a session with `payload` at `start`.
    ///
    /// The session starts **inactive**: a press is not yet a drag. Until the
    /// pointer travels past `threshold`, the gesture is still a candidate click, and
    /// treating it as a drag would cancel the click that a user expected.
    pub fn begin(payload: DragPayload, start: Point) -> Self {
        Self { payload, start, current: start, active: false }
    }

    /// The payload being dragged.
    pub fn payload(&self) -> &DragPayload {
        &self.payload
    }

    /// Where the gesture began.
    pub fn start(&self) -> Point {
        self.start
    }

    /// Where the pointer is now.
    pub fn current(&self) -> Point {
        self.current
    }

    /// Whether the pointer has travelled far enough to be a drag.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The offset the pointer has travelled since the gesture began.
    pub fn delta(&self) -> Point {
        Point::new(self.current.x - self.start.x, self.current.y - self.start.y)
    }

    /// Moves the pointer to `pos`, activating the drag once `threshold` is
    /// exceeded in either axis.
    ///
    /// Returns whether the session became active on this move, so a caller can
    /// start drawing a preview exactly once rather than on every move.
    pub fn update(&mut self, pos: Point, threshold: i32) -> bool {
        self.current = pos;
        if self.active {
            return false;
        }
        let dx = (pos.x - self.start.x).abs();
        let dy = (pos.y - self.start.y).abs();
        if dx >= threshold || dy >= threshold {
            self.active = true;
            return true;
        }
        false
    }

    /// Asks `target` whether it would accept this payload at `pos`.
    ///
    /// A convenience over `target.can_accept(session.payload())` that also refuses
    /// while the gesture is still a click — so a caller cannot forget that a
    /// not-yet-active drag must not highlight anything.
    pub fn target_accepts(&self, target: &dyn DropTarget) -> bool {
        self.active && target.can_accept(&self.payload)
    }

    /// Commits the drop on `target` at `pos`.
    ///
    /// Returns [`DropEffect::None`] when the gesture never became a drag, or when
    /// the target refuses — in both cases the source restores, which is the same
    /// outcome and needs no separate branch at the call site.
    pub fn drop_on(&mut self, target: &mut dyn DropTarget, pos: Point) -> DropEffect {
        if !self.active {
            return DropEffect::None;
        }
        if !target.can_accept(&self.payload) {
            return DropEffect::None;
        }
        target.on_drop(&self.payload, pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::{MiniToString, String, Vec};
    use crate::core::Rect;

    /// A target that accepts one payload type and records what it was given.
    #[derive(Default)]
    struct Recorder {
        dropped: Vec<String>,
        effect: DropEffect,
        preview: Option<Rect>,
    }

    impl DropTarget for Recorder {
        fn can_accept(&self, payload: &DragPayload) -> bool {
            payload.is_type("card")
        }

        fn on_drop(&mut self, payload: &DragPayload, _pos: Point) -> DropEffect {
            self.dropped.push(payload.item_id.clone());
            self.effect
        }

        fn preview_rect(&self, _payload: &DragPayload, _pos: Point) -> Option<Rect> {
            self.preview
        }
    }

    /// A target that refuses everything.
    #[derive(Default)]
    struct Refuser {
        drop_calls: usize,
    }

    impl DropTarget for Refuser {
        fn can_accept(&self, _payload: &DragPayload) -> bool {
            false
        }

        fn on_drop(&mut self, _payload: &DragPayload, _pos: Point) -> DropEffect {
            self.drop_calls += 1;
            DropEffect::Move
        }
    }

    fn card() -> DragPayload {
        DragPayload::new("card", "task-1").with_label("Fix the parser")
    }

    #[test]
    fn payload_type_filter_matches_only_its_own_id() {
        let payload = card();
        assert!(payload.is_type("card"));
        assert!(!payload.is_type("tab"));
        assert_eq!(payload.label, "Fix the parser");
    }

    #[test]
    fn payload_builder_sets_label_and_origin() {
        let payload = DragPayload::new("file", "/tmp/a").with_origin(Point::new(4, 5));
        assert_eq!(payload.origin, Point::new(4, 5));
        assert_eq!(payload.label, "");
    }

    #[test]
    fn a_press_below_threshold_is_not_yet_a_drag() {
        let mut session = DragSession::begin(card(), Point::new(10, 10));
        assert!(!session.is_active());
        // Four pixels of travel is a click with an unsteady hand.
        assert!(!session.update(Point::new(14, 12), 8));
        assert!(!session.is_active());
    }

    #[test]
    fn a_move_past_threshold_activates_the_drag_exactly_once() {
        let mut session = DragSession::begin(card(), Point::new(10, 10));
        assert!(session.update(Point::new(30, 10), 8), "the first crossing reports the change");
        assert!(session.is_active());
        // Later moves do not re-report, so a caller starts one preview.
        assert!(!session.update(Point::new(60, 10), 8));
        assert!(session.is_active());
    }

    #[test]
    fn a_threshold_refuser_is_never_asked_to_drop() {
        let mut session = DragSession::begin(card(), Point::new(0, 0));
        session.update(Point::new(50, 0), 8);
        let mut target = Refuser::default();

        // `target_accepts` gates the preview.
        assert!(!session.target_accepts(&target));

        // And the drop is refused without reaching the target's commit.
        assert_eq!(session.drop_on(&mut target, Point::new(50, 0)), DropEffect::None);
        assert_eq!(target.drop_calls, 0, "a refused payload must not reach on_drop");
    }

    #[test]
    fn an_inactive_session_never_drops() {
        let mut session = DragSession::begin(card(), Point::new(0, 0));
        let mut target = Recorder { effect: DropEffect::Move, ..Recorder::default() };
        // Never moved: still a click, so no drop may be committed even though the
        // target would have accepted.
        assert_eq!(session.drop_on(&mut target, Point::new(0, 0)), DropEffect::None);
        assert!(target.dropped.is_empty());
    }

    #[test]
    fn an_accepted_payload_reaches_the_commit() {
        let mut session = DragSession::begin(card(), Point::new(0, 0));
        session.update(Point::new(40, 0), 8);
        let mut target = Recorder { effect: DropEffect::Move, ..Recorder::default() };

        assert!(session.target_accepts(&target));
        assert_eq!(session.drop_on(&mut target, Point::new(40, 0)), DropEffect::Move);
        assert_eq!(target.dropped, vec!["task-1".to_string()]);
    }

    #[test]
    fn drop_effect_describes_what_the_source_should_do() {
        assert!(DropEffect::Move.consumes_source());
        assert!(!DropEffect::Copy.consumes_source());
        assert!(!DropEffect::Link.consumes_source());
        assert!(!DropEffect::None.consumes_source());

        assert!(DropEffect::Copy.is_accepted());
        assert!(DropEffect::Move.is_accepted());
        assert!(DropEffect::Link.is_accepted());
        assert!(!DropEffect::None.is_accepted());
    }

    #[test]
    fn default_effect_is_none_so_a_forgotten_field_restores() {
        // The default has to be the safe one: a target that forgets to set its
        // effect must not make the source delete its data.
        assert_eq!(DropEffect::default(), DropEffect::None);
        assert!(!DropEffect::default().consumes_source());
    }

    #[test]
    fn delta_tracks_the_pointer_offset() {
        let mut session = DragSession::begin(card(), Point::new(10, 20));
        session.update(Point::new(35, 5), 8);
        assert_eq!(session.delta(), Point::new(25, -15));
        assert_eq!(session.start(), Point::new(10, 20));
        assert_eq!(session.current(), Point::new(35, 5));
    }

    #[test]
    fn preview_rect_defaults_to_none() {
        // A target that accepts but offers no landing zone must say so, rather
        // than have the trait invent one from its own bounds.
        struct NoPreview;
        impl DropTarget for NoPreview {
            fn can_accept(&self, _payload: &DragPayload) -> bool {
                true
            }
            fn on_drop(&mut self, _payload: &DragPayload, _pos: Point) -> DropEffect {
                DropEffect::Copy
            }
        }
        let target = NoPreview;
        assert_eq!(target.preview_rect(&card(), Point::new(1, 1)), None);
    }

    #[test]
    fn preview_rect_is_reported_when_a_target_offers_one() {
        let target = Recorder { preview: Some(Rect::new(0, 10, 100, 4)), ..Recorder::default() };
        assert_eq!(target.preview_rect(&card(), Point::new(5, 5)), Some(Rect::new(0, 10, 100, 4)));
    }

    #[test]
    fn a_target_may_refuse_at_commit_time() {
        // A target can discover during the commit that it cannot proceed, and the
        // source then restores. This is a real case (async validation), not a
        // contradiction of `can_accept`.
        let mut session = DragSession::begin(card(), Point::new(0, 0));
        session.update(Point::new(40, 0), 8);
        let mut target = Recorder { effect: DropEffect::None, ..Recorder::default() };
        assert_eq!(session.drop_on(&mut target, Point::new(40, 0)), DropEffect::None);
        assert_eq!(target.dropped.len(), 1, "the target still saw the attempt");
    }

    #[test]
    fn vertical_travel_alone_activates_the_drag() {
        let mut session = DragSession::begin(card(), Point::new(0, 0));
        // A drag that never moves horizontally must still activate, which is what
        // makes column-to-column board dragging work.
        assert!(session.update(Point::new(2, 40), 8));
        assert!(session.is_active());
    }
}
