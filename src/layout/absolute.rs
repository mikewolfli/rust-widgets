// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::Layout;
use crate::compat::{Any, Box, Vec};
use crate::core::{ObjectId, Rect, Size};
use crate::widget::Widget;
/// A positioned child in an absolute layout, with optional size and anchor.
///
/// # What `anchor` means here
///
/// The anchor picks which part of **the child** is placed at `(x, y)`. `Anchor::TopRight`
/// therefore puts the child's top-right corner at `(x, y)`, which is *not* the same as
/// "pin the child to the parent's right edge" — that needs the parent's width, and is what
/// [`AbsolutePosition::to_rect_in_parent`] provides. Keeping both spellings distinct matters:
/// the self-relative one composes with a parent-relative offset (`x = parent.width - inset`),
/// while the parent-relative one is what a caller actually means by "top-right corner of the
/// window".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsolutePosition {
    /// Horizontal offset of the anchored edge, in pixels, increasing to the
    /// right. May be negative to place a child off the left edge of the parent.
    pub x: i32,
    /// Vertical offset of the anchored edge, in pixels, increasing downwards. May
    /// be negative to place a child above the top edge of the parent.
    pub y: i32,
    /// Explicit width in pixels, or `None` to fall back to the child's own width
    /// hint. `Some(0)` is honoured as a zero-width rect and is not treated as
    /// "unset".
    pub width: Option<u32>,
    /// Explicit height in pixels, or `None` to fall back to the child's own height
    /// hint. `Some(0)` is honoured as a zero-height rect.
    pub height: Option<u32>,
    /// Which part of the child is pinned to `(x, y)`, or — via
    /// [`AbsolutePosition::to_rect_in_parent`] — which edge of the parent it is measured
    /// from.
    pub anchor: Anchor,
}
impl AbsolutePosition {
    /// Creates a position at pixel offset `(x, y)` with the child's own size and
    /// [`Anchor::TopLeft`] (so `(x, y)` is the child's top-left corner).
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y, width: None, height: None, anchor: Anchor::TopLeft }
    }
    /// Sets an explicit size in pixels, overriding the child's size hint.
    ///
    /// Zero is a valid size and yields an empty rect.
    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
    /// Sets the anchor and *replaces* `(x, y)` with the given pixel offsets.
    ///
    /// Note this overwrites any earlier `x`/`y`, including the ones passed to
    /// [`Self::new`]; to keep the existing offsets use [`Self::with_anchor_only`].
    pub fn with_anchor(mut self, anchor: Anchor, offset_x: i32, offset_y: i32) -> Self {
        self.anchor = anchor;
        self.x = offset_x;
        self.y = offset_y;
        self
    }
    /// Sets the anchor, keeping any previously set `(x, y)` offsets.
    pub fn with_anchor_only(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }
    /// Resolves this position into a concrete pixel rect, measuring the anchor against
    /// **the child's own box**.
    ///
    /// `child_size` supplies the width/height for axes where `width`/`height` is
    /// `None`. `parent_size` is unused here: this spelling places the child's own anchored
    /// corner at `(x, y)`, so `Anchor::TopRight` puts the child's top-right corner at the
    /// caller's point rather than pinning it to the parent's right edge. Offsets are
    /// unclamped, so the returned rect may fall partly or wholly outside the parent, and
    /// negative resulting `x`/`y` values are returned as-is rather than clamped to zero.
    ///
    /// For "pin to the parent's edge", use [`Self::to_rect_in_parent`].
    pub fn to_rect(&self, _parent_size: Size, child_size: Size) -> Rect {
        let width = self.width.unwrap_or(child_size.width);
        let height = self.height.unwrap_or(child_size.height);
        let (x, y) = match self.anchor {
            Anchor::TopLeft => (self.x, self.y),
            Anchor::TopCenter => {
                let x = self.x - (width as i32) / 2;
                (x, self.y)
            }
            Anchor::TopRight => {
                let x = self.x - width as i32;
                (x, self.y)
            }
            Anchor::CenterLeft => {
                let y = self.y - (height as i32) / 2;
                (self.x, y)
            }
            Anchor::Center => {
                let x = self.x - (width as i32) / 2;
                let y = self.y - (height as i32) / 2;
                (x, y)
            }
            Anchor::CenterRight => {
                let x = self.x - width as i32;
                let y = self.y - (height as i32) / 2;
                (x, y)
            }
            Anchor::BottomLeft => {
                let y = self.y - height as i32;
                (self.x, y)
            }
            Anchor::BottomCenter => {
                let x = self.x - (width as i32) / 2;
                let y = self.y - height as i32;
                (x, y)
            }
            Anchor::BottomRight => {
                let x = self.x - width as i32;
                let y = self.y - height as i32;
                (x, y)
            }
        };
        Rect::new(x, y, width, height)
    }

    /// Resolves this position into a rect **anchored to the parent's box**.
    ///
    /// Here `(x, y)` is an inset measured from the anchored edge of `parent_size`, which is
    /// what a caller means by "the top-right corner of the window": `BottomRight` with
    /// `(8, 8)` puts the child 8px in from the bottom-right, whatever the window's size.
    /// A centred anchor splits the remaining space, and the child may still overflow a
    /// parent smaller than it.
    ///
    /// This is the operation [`Self::to_rect`]'s `parent_size` parameter was reserved for;
    /// it is a separate method rather than a change to `to_rect` so that the existing
    /// self-relative behaviour — which snapshots and callers already depend on — is
    /// untouched.
    pub fn to_rect_in_parent(&self, parent_size: Size, child_size: Size) -> Rect {
        let width = self.width.unwrap_or(child_size.width);
        let height = self.height.unwrap_or(child_size.height);
        let pw = parent_size.width as i32;
        let ph = parent_size.height as i32;
        let x = match self.anchor.horizontal() {
            HorizontalAnchor::Start => self.x,
            HorizontalAnchor::Center => (pw - width as i32) / 2 + self.x,
            HorizontalAnchor::End => pw - width as i32 - self.x,
        };
        let y = match self.anchor.vertical() {
            VerticalAnchor::Start => self.y,
            VerticalAnchor::Center => (ph - height as i32) / 2 + self.y,
            VerticalAnchor::End => ph - height as i32 - self.y,
        };
        Rect::new(x, y, width, height)
    }
}
impl Default for AbsolutePosition {
    fn default() -> Self {
        Self::new(0, 0)
    }
}
/// Anchor point for absolute positioning.
///
/// Determines which corner or edge of a child widget is pinned to the
/// specified (x, y) coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    /// Top-left corner of the child is at `(x, y)`. This is the default and is
    /// the only anchor that places the child without an offset.
    #[default]
    TopLeft,
    /// Top edge centred on `x`: the child is shifted left by half its width.
    TopCenter,
    /// Top-right corner is at `(x, y)`: the child is shifted left by its width.
    TopRight,
    /// Left edge centred on `y`: the child is shifted up by half its height.
    CenterLeft,
    /// Centre of the child is at `(x, y)`.
    Center,
    /// Right edge centred on `y`: shifted left by its width and up by half its
    /// height.
    CenterRight,
    /// Bottom-left corner is at `(x, y)`: shifted up by its height.
    BottomLeft,
    /// Bottom edge centred on `x`: shifted left by half its width and up by its
    /// height.
    BottomCenter,
    /// Bottom-right corner is at `(x, y)`.
    BottomRight,
}
/// Which edge of a parent box an anchor measures from, on one axis.
///
/// The nine anchors are the product of these two axes, so a caller that only needs "is it
/// centred horizontally?" asks one question instead of matching nine variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAnchor {
    /// Measured from the left edge (`x` is an inset from the left).
    Start,
    /// Centred in the parent, with `x` as a signed nudge.
    Center,
    /// Measured from the right edge (`x` is an inset from the right).
    End,
}

/// Which edge of a parent box an anchor measures from, on the vertical axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAnchor {
    /// Measured from the top edge.
    Start,
    /// Centred in the parent, with `y` as a signed nudge.
    Center,
    /// Measured from the bottom edge.
    End,
}

impl Anchor {
    /// The horizontal half of this anchor.
    pub fn horizontal(self) -> HorizontalAnchor {
        match self {
            Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => HorizontalAnchor::Start,
            Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => HorizontalAnchor::Center,
            Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => HorizontalAnchor::End,
        }
    }

    /// The vertical half of this anchor.
    pub fn vertical(self) -> VerticalAnchor {
        match self {
            Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => VerticalAnchor::Start,
            Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight => VerticalAnchor::Center,
            Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => VerticalAnchor::End,
        }
    }
}

/// Alias for Anchor to match test expectations
pub use Anchor as AbsoluteAnchor;
/// A size constraint with optional min/max bounds for each axis.
#[derive(Debug, Clone, Copy)]
pub struct Constraint {
    /// Lower bound in pixels applied before `max_width`. `None` means no minimum.
    pub min_width: Option<u32>,
    /// Upper bound in pixels applied after `min_width`. `None` means no maximum.
    /// If it is smaller than `min_width` the minimum wins, because the minimum is
    /// applied first.
    pub max_width: Option<u32>,
    /// Lower bound in pixels applied before `max_height`. `None` means no minimum.
    pub min_height: Option<u32>,
    /// Upper bound in pixels applied after `min_height`. `None` means no maximum;
    /// `min_height` wins when the two conflict.
    pub max_height: Option<u32>,
    /// Desired width/height ratio, applied *after* all min/max clamps and possibly
    /// undoing them. `None` disables ratio correction.
    pub aspect_ratio: Option<f32>,
}
impl Constraint {
    /// Creates an unconstrained size (every bound `None`), leaving any size it is
    /// applied to unchanged.
    pub fn new() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            aspect_ratio: None,
        }
    }
    /// Sets the minimum width in pixels.
    pub fn with_min_width(mut self, width: u32) -> Self {
        self.min_width = Some(width);
        self
    }
    /// Sets the maximum width in pixels.
    pub fn with_max_width(mut self, width: u32) -> Self {
        self.max_width = Some(width);
        self
    }
    /// Sets the minimum height in pixels.
    pub fn with_min_height(mut self, height: u32) -> Self {
        self.min_height = Some(height);
        self
    }
    /// Sets the maximum height in pixels.
    pub fn with_max_height(mut self, height: u32) -> Self {
        self.max_height = Some(height);
        self
    }
    /// Sets the target width/height ratio (e.g. `2.0` for twice as wide as tall).
    pub fn with_aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }
    /// Clamps `size` to the configured pixel bounds and then corrects it to the
    /// aspect ratio.
    ///
    /// Minima are applied before maxima, so a minimum larger than the matching
    /// maximum wins. Ratio correction shrinks the wider axis relative to the other
    /// and runs last, so it can produce a size outside `min_*`/`max_*`.
    ///
    /// A zero or negative `aspect_ratio` is *not* rejected: a ratio of `0.0`
    /// collapses the width, and a negative ratio cannot be represented in `u32`
    /// so the cast saturates instead of panicking.
    pub fn apply(&self, size: Size) -> Size {
        let mut width = size.width;
        let mut height = size.height;
        if let Some(min) = self.min_width {
            width = width.max(min);
        }
        if let Some(max) = self.max_width {
            width = width.min(max);
        }
        if let Some(min) = self.min_height {
            height = height.max(min);
        }
        if let Some(max) = self.max_height {
            height = height.min(max);
        }
        if let Some(ratio) = self.aspect_ratio {
            // A zero or non-finite ratio cannot describe an aspect, so it changes
            // nothing (principle #50: degenerate input is ignored, not propagated).
            if !(ratio.is_finite() && ratio > 0.0) {
                return Size::new(width, height);
            }
            // `height` is clamped to >= 1 so the division below cannot hit zero.
            let height_f = height.max(1) as f32;
            let current_ratio = width as f32 / height_f;
            if current_ratio > ratio {
                width = (height_f * ratio).round() as u32;
            } else {
                height = (width as f32 / ratio).round() as u32;
            }
        }
        Size::new(width, height)
    }
}
crate::impl_default_via_new!(Constraint);
/// An absolute-position layout manager.
///
/// Children are placed at explicit (x, y) coordinates with optional
/// constraints (min/max size, aspect ratio).  No automatic arrangement
/// or reflow is performed.
pub struct AbsoluteLayout {
    children: Vec<AbsoluteEntry>,
}

/// A single child of an absolute layout: its id, optional owned widget, position, and
/// size constraint.
///
/// # Why one entry instead of parallel `widget_ids`/`children` lists
///
/// The two lists had no identity link, so a remove that only touched the ids left a stale
/// `None` slot, and a later `update` sent a surviving widget the geometry of a deleted
/// sibling (S-56). Holding id, widget, position, and constraint in one place means
/// `add`/`remove`/`clear` can never drift apart.
struct AbsoluteEntry {
    widget_id: ObjectId,
    widget: Option<Box<dyn Widget>>,
    position: AbsolutePosition,
    constraint: Option<Constraint>,
}

impl AbsoluteLayout {
    /// Creates a layout with no children.
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }
    /// Adds a child at an absolute position, with no size constraint.
    ///
    /// Children are laid out in insertion order; the new child is appended last.
    pub fn add_child(&mut self, child: Box<dyn Widget>, position: AbsolutePosition) {
        let widget_id = child.id();
        self.children.push(AbsoluteEntry {
            widget_id,
            widget: Some(child),
            position,
            constraint: None,
        });
    }
    /// Adds a child at an absolute position, clamping its size to `constraint`
    /// before the rect is computed.
    pub fn add_child_with_constraint(
        &mut self,
        child: Box<dyn Widget>,
        position: AbsolutePosition,
        constraint: Constraint,
    ) {
        let widget_id = child.id();
        self.children.push(AbsoluteEntry {
            widget_id,
            widget: Some(child),
            position,
            constraint: Some(constraint),
        });
    }
    /// Removes the child at insertion index `index` and returns it so the caller
    /// can drop it or reuse it.
    ///
    /// Returns `None` if `index` is out of range; nothing is removed in that case.
    /// Indices of children after `index` shift down by one.
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Widget>> {
        if index < self.children.len() {
            self.children.remove(index).widget
        } else {
            None
        }
    }
    /// Removes every child, dropping them, and forgets all recorded widget ids.
    ///
    /// Position and constraint state goes with them; the layout is left empty, as
    /// if freshly constructed.
    pub fn clear_children(&mut self) {
        self.children.clear();
    }
    /// Number of children currently registered, including children added by widget
    /// id via [`Layout::add_widget`] that have no `Box<dyn Widget>` handle.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }
    /// Computes the pixel rect for every child inside `parent_rect`.
    ///
    /// Each child's size hint is passed through its constraint, then through
    /// [`AbsolutePosition::to_rect`]. The returned vector is index-aligned with
    /// the children, and `parent_rect`'s origin is ignored — results are relative
    /// to the parent, not absolute screen coordinates. This is a pure computation:
    /// nothing on the layout is mutated.
    pub fn layout(&self, parent_rect: Rect) -> Vec<Rect> {
        let parent_size = parent_rect.size();
        let mut positions = Vec::new();
        for entry in &self.children {
            let child_size =
                entry.widget.as_ref().map(|w| w.size_hint()).unwrap_or(Size::new(0, 0));
            let constrained_size = entry
                .constraint
                .as_ref()
                .map(|constraint| constraint.apply(child_size))
                .unwrap_or(child_size);
            let rect = entry.position.to_rect(parent_size, constrained_size);
            positions.push(rect);
        }
        positions
    }
    /// Resolves `positions` against `container` using the given child sizes,
    /// without consulting any registered children.
    ///
    /// Intended for tests and previews. Sizes are *not* constrained and only the
    /// pairs present in both slices are used, so the result is at most
    /// `min(positions.len(), sizes.len())` rects long.
    pub fn calculate_positions(
        &self,
        container: &Rect,
        positions: &[AbsolutePosition],
        sizes: &[Size],
    ) -> Vec<Rect> {
        let mut rects = Vec::new();
        let parent_size = container.size();
        for (position, size) in positions.iter().zip(sizes.iter()) {
            let rect = position.to_rect(parent_size, *size);
            rects.push(rect);
        }
        rects
    }
    /// Replaces the position of the child at insertion index `index`.
    ///
    /// Returns `true` if a child was updated and `false` if `index` is out of
    /// range; any existing size constraint is preserved.
    pub fn set_position(&mut self, index: usize, position: AbsolutePosition) -> bool {
        if let Some(entry) = self.children.get_mut(index) {
            entry.position = position;
            true
        } else {
            false
        }
    }
    /// Attaches or replaces the size constraint of the child at insertion index
    /// `index`.
    ///
    /// Returns `true` if a child was updated and `false` if `index` is out of
    /// range. The child's position is left untouched. There is no way to remove a
    /// constraint once set short of replacing the child.
    pub fn set_constraint(&mut self, index: usize, constraint: Constraint) -> bool {
        if let Some(entry) = self.children.get_mut(index) {
            entry.constraint = Some(constraint);
            true
        } else {
            false
        }
    }
    /// Borrows the position of the child at insertion index `index`, or `None` if
    /// `index` is out of range.
    pub fn get_position(&self, index: usize) -> Option<&AbsolutePosition> {
        self.children.get(index).map(|entry| &entry.position)
    }
    /// Borrows the constraint of the child at insertion index `index`.
    ///
    /// Returns `None` both when `index` is out of range and when that child has no
    /// constraint, so callers cannot distinguish the two cases from the return
    /// value alone.
    pub fn get_constraint(&self, index: usize) -> Option<&Constraint> {
        self.children.get(index).and_then(|entry| entry.constraint.as_ref())
    }
}
crate::impl_default_via_new!(AbsoluteLayout);

impl Layout for AbsoluteLayout {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn add_widget(&mut self, widget_id: ObjectId, _stretch: u32) {
        // Dedupe against the unified entry list, not a parallel id list (S-56).
        if !self.children.iter().any(|entry| entry.widget_id == widget_id) {
            self.children.push(AbsoluteEntry {
                widget_id,
                widget: None,
                position: AbsolutePosition::new(0, 0),
                constraint: None,
            });
        }
    }

    fn remove_widget(&mut self, widget_id: ObjectId) {
        self.children.retain(|entry| entry.widget_id != widget_id);
    }

    fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect)) {
        let positions = self.layout(rect);
        for (i, child_rect) in positions.iter().enumerate() {
            if let Some(entry) = self.children.get(i) {
                widgets(entry.widget_id, *child_rect);
            }
        }
    }

    fn child_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = Vec::new();
        for entry in &self.children {
            if !ids.contains(&entry.widget_id) {
                ids.push(entry.widget_id);
            }
        }
        ids
    }

    fn has_child(&self, id: ObjectId) -> bool {
        self.children.iter().any(|entry| entry.widget_id == id)
    }

    fn clear(&mut self) {
        self.clear_children();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::HashMap;

    /// Minimal owned widget used to exercise the layout's `add_child` path.
    struct TestWidget {
        id: ObjectId,
        size: Size,
    }
    impl TestWidget {
        fn new(id: ObjectId, width: u32, height: u32) -> Self {
            Self { id, size: Size::new(width, height) }
        }
    }
    impl crate::event::EventHandler for TestWidget {
        fn handle_event(&mut self, _event: &crate::event::Event) {}
    }
    impl Widget for TestWidget {
        fn id(&self) -> ObjectId {
            self.id
        }
        fn size_hint(&self) -> Size {
            self.size
        }
    }

    // ── S-56: identity and geometry stay aligned across add/remove ──────────────────

    #[test]
    fn remove_keeps_identity_and_geometry_aligned() {
        let mut layout = AbsoluteLayout::new();
        layout.add_widget(1, 0);
        layout.set_position(0, AbsolutePosition::new(10, 10));
        layout.add_child(Box::new(TestWidget::new(2, 20, 20)), AbsolutePosition::new(50, 50));

        assert_eq!(layout.child_count(), 2);
        layout.remove_widget(1);
        assert_eq!(layout.child_count(), 1, "removing an id-only child must drop its slot");

        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });

        assert_eq!(rects.len(), 1, "the deleted sibling is gone from the update");
        assert_eq!(
            rects.get(&2),
            Some(&Rect::new(50, 50, 20, 20)),
            "the survivor keeps its own geometry, not the deleted sibling's"
        );
    }

    #[test]
    fn removing_any_position_keeps_survivors_aligned() {
        let mut layout = AbsoluteLayout::new();
        for (id, x) in [(1u64, 10i32), (2, 20), (3, 30)] {
            layout.add_widget(id, 0);
            let index = layout.child_count() - 1;
            layout.set_position(index, AbsolutePosition::new(x, 0));
        }
        layout.remove_widget(2); // middle

        assert_eq!(layout.child_ids(), vec![1, 3]);
        let mut rects = HashMap::new();
        layout.update(Rect::new(0, 0, 100, 100), &mut |id, rect| {
            rects.insert(id, rect);
        });
        assert_eq!(rects.get(&1).map(|r| r.x), Some(10));
        assert_eq!(rects.get(&3).map(|r| r.x), Some(30));
    }

    #[test]
    fn duplicate_register_does_not_add_a_second_entry() {
        let mut layout = AbsoluteLayout::new();
        layout.add_widget(1, 0);
        layout.add_widget(1, 0);
        assert_eq!(layout.child_count(), 1);
        assert_eq!(layout.child_ids(), vec![1]);
    }

    // ── S-55: `dyn Layout::clear` must reach the concrete clear_children ─────────────

    #[test]
    fn dyn_clear_removes_absolute_children() {
        let mut layout: Box<dyn Layout> = Box::new(AbsoluteLayout::new());
        layout.add_widget(1, 0);
        layout.add_widget(2, 0);
        assert!(layout.has_child(1));

        layout.clear();
        assert!(!layout.has_child(1));
        assert!(!layout.has_child(2));
        assert!(layout.child_ids().is_empty());

        // A repeat clear is a no-op, not a panic or a re-emission.
        layout.clear();
        assert!(layout.child_ids().is_empty());
    }

    #[test]
    fn test_absolute_position() {
        let pos =
            AbsolutePosition::new(100, 100).with_size(50, 50).with_anchor_only(Anchor::Center);
        let rect = pos.to_rect(Size::new(200, 200), Size::new(50, 50));
        assert_eq!(rect.x, 75);
        assert_eq!(rect.y, 75);
        assert_eq!(rect.width, 50);
        assert_eq!(rect.height, 50);
    }
    #[test]
    fn test_constraint() {
        let constraint = Constraint::new()
            .with_min_width(50)
            .with_max_width(100)
            .with_min_height(50)
            .with_max_height(100);
        let size = constraint.apply(Size::new(200, 200));
        assert_eq!(size.width, 100);
        assert_eq!(size.height, 100);
        let size = constraint.apply(Size::new(30, 30));
        assert_eq!(size.width, 50);
        assert_eq!(size.height, 50);
    }
    #[test]
    fn test_aspect_ratio() {
        let constraint = Constraint::new().with_aspect_ratio(2.0);
        let size = constraint.apply(Size::new(200, 100));
        assert_eq!(size.width, 200);
        assert_eq!(size.height, 100);
        let size = constraint.apply(Size::new(200, 200));
        assert_eq!(size.width, 200);
        assert_eq!(size.height, 100);
    }

    /// `to_rect_in_parent` measures the anchor from the **parent's** edge, which is what
    /// "the bottom-right corner of the window" actually means. `to_rect` measures it against
    /// the child's own box and is unchanged, so both spellings now exist and differ.
    #[test]
    fn anchored_in_parent_pins_to_the_parents_edges() {
        let parent = Size::new(400, 300);
        let child = Size::new(60, 40);

        // 8px in from the bottom-right, whatever the parent's size.
        let corner = AbsolutePosition::new(8, 8)
            .with_anchor_only(Anchor::BottomRight)
            .to_rect_in_parent(parent, child);
        assert_eq!(corner, Rect::new(400 - 60 - 8, 300 - 40 - 8, 60, 40));

        // Centred, with the offsets acting as a nudge.
        let centred = AbsolutePosition::new(0, 0)
            .with_anchor_only(Anchor::Center)
            .to_rect_in_parent(parent, child);
        assert_eq!(centred, Rect::new(170, 130, 60, 40));

        // Top-left is the identity: x/y stay absolute.
        let top_left = AbsolutePosition::new(5, 6)
            .with_anchor_only(Anchor::TopLeft)
            .to_rect_in_parent(parent, child);
        assert_eq!(top_left, Rect::new(5, 6, 60, 40));

        // And it genuinely differs from the self-relative spelling for the same anchor.
        let position = AbsolutePosition::new(400, 300).with_anchor_only(Anchor::BottomRight);
        assert_eq!(
            position.to_rect(parent, child),
            Rect::new(400 - 60, 300 - 40, 60, 40),
            "the old spelling places the child's own corner at the given point"
        );
        assert_eq!(
            position.to_rect_in_parent(parent, child),
            Rect::new(400 - 60 - 400, 300 - 40 - 300, 60, 40),
            "the new spelling treats the point as an inset from the parent's edge"
        );
    }

    /// The anchor's axis decomposition must be a total function that agrees with the nine
    /// explicit variants — the property that makes it safe to use as the single source.
    #[test]
    fn every_anchor_decomposes_onto_its_axes() {
        for (anchor, horizontal, vertical) in [
            (Anchor::TopLeft, HorizontalAnchor::Start, VerticalAnchor::Start),
            (Anchor::TopCenter, HorizontalAnchor::Center, VerticalAnchor::Start),
            (Anchor::TopRight, HorizontalAnchor::End, VerticalAnchor::Start),
            (Anchor::CenterLeft, HorizontalAnchor::Start, VerticalAnchor::Center),
            (Anchor::Center, HorizontalAnchor::Center, VerticalAnchor::Center),
            (Anchor::CenterRight, HorizontalAnchor::End, VerticalAnchor::Center),
            (Anchor::BottomLeft, HorizontalAnchor::Start, VerticalAnchor::End),
            (Anchor::BottomCenter, HorizontalAnchor::Center, VerticalAnchor::End),
            (Anchor::BottomRight, HorizontalAnchor::End, VerticalAnchor::End),
        ] {
            assert_eq!(anchor.horizontal(), horizontal, "{anchor:?}");
            assert_eq!(anchor.vertical(), vertical, "{anchor:?}");
        }
    }
}
