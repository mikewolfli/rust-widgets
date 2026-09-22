// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Size hints: what a widget *wants*, expressed so a layout can act on it.
//!
//! # The channel this module opens
//!
//! Before this module, a layout could only **write** child geometry:
//!
//! ```text
//! fn update(&self, rect: Rect, widgets: &mut dyn FnMut(ObjectId, Rect));
//! ```
//!
//! It held `ObjectId`s, not widgets, so it had no way to ask "how big does this
//! child want to be?". The crate's workaround made that concrete:
//! [`FlexLayout::set_child_sizes`](crate::layout::FlexLayout::set_child_sizes)
//! documents "call **before** update for proper sizing" — the caller had to work
//! out every child's size and hand it to the layout *before* asking the layout to
//! lay them out. `Wrap` and `Absolute` carried their own variants of the same
//! workaround, and `Flow` bypassed the protocol entirely by storing
//! `Box<dyn Widget>` itself.
//!
//! Qt Quick, Flutter and Qt Widgets all answer this the other way round: the
//! **parent asks the child**. QML computes
//! `implicitWidth = max(implicitBackgroundWidth + inset, implicitContentWidth + padding)`
//! on the control itself; Flutter's `RenderBox.layout` reads `child.size` after
//! laying the child out; Qt's layouts call `item->implicitWidth()`. Only this crate
//! had the flow reversed.
//!
//! [`Hints`] and [`ChildInfo`] are that channel: a widget states its own wish, and
//! [`Layout::arrange`](crate::layout::Layout::arrange) receives it.
//!
//! # Why three values per axis, and not Flutter's four functions
//!
//! Flutter exposes `getMinIntrinsicWidth(height)` and friends — parameterised on
//! the *opposite* axis, because it supports width-for-height coupling. That is
//! powerful and expensive: Flutter's own documentation describes `IntrinsicWidth`
//! as "a speculative layout pass" that is "O(N²) in the depth of the tree".
//!
//! Qt's model is three values per axis — `min`, `pref`, `max` — that a widget
//! computes once, from its own content. A layout reads them directly: no
//! round-trip, no opposite-axis parameter, no quadratic blow-up. It cannot express
//! "my width depends on my height", which this crate's controls never need
//! (buttons, switches and fields are all fixed-height) and for which the text layer
//! has its own measurement entry point.
//!
//! The three values answer the two questions a composite asks constantly:
//!
//! - **"How small may I squeeze you?"** — `min`. Buttons, switches and progress
//!   bars all have a floor;
//! - **"How large may I stretch you?"** — `max`. An icon or a badge should not grow
//!   without bound.
//!
//! Qt's own truth table puts the three together because they have to be decided
//! together:
//!
//! ```text
//!             | minimum                | preferred               | maximum
//! USER        | Layout.minimumWidth    | Layout.preferredWidth   | Layout.maximumWidth
//! HINT        | implicit minimum       | implicitWidth           | implicit maximum
//! FALLBACK    | 0                      | width                   | +infinity
//! ```

use crate::compat::{vec, Vec};
use crate::core::{ObjectId, Size};
use crate::style::EdgeOffsets;

/// A widget's size wish along one axis.
///
/// The invariant `min <= pref <= max` holds **by construction**: the constructors
/// normalise, so an unnormalised value is not representable. Qt instead permits any
/// values and re-normalises on every read (`normalizeHints`, `expandSize`,
/// `boundSize` in `qquicklayout.cpp`), which is a rule every consumer has to
/// remember; making the illegal state unrepresentable is cheaper than correcting it
/// afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisHints {
    /// The smallest this axis may become without breaking the widget.
    pub min: u32,
    /// The size the widget would like, all else being equal.
    pub pref: u32,
    /// The largest this axis may become before the widget looks wrong.
    pub max: u32,
}

impl AxisHints {
    /// A hint normalised so that `min <= pref <= max`.
    ///
    /// The normalisation is documented per case because the corrections are not
    /// symmetric — a caller that passes `(pref=10, min=30)` means "at least 30", while
    /// one that passes `(pref=10, max=5)` means "no more than 5", and in both cases
    /// `pref` has to move to keep the three consistent.
    pub fn new(min: u32, pref: u32, max: u32) -> Self {
        // Resolve in dependency order, because each correction changes what the next
        // one may be.
        //
        // 1. `pref` is the anchor: it is the value the widget computed from its own
        //    content, so it is kept unless it is unreachable.
        // 2. `max` may not fall below `pref`, or the preferred size would be
        //    unrepresentable; it is widened to `pref` when the caller asked for less.
        // 3. `min` may not exceed `max`, or the range would be empty and `clamp` would
        //    be undefined; it is narrowed to `max` when the caller asked for more.
        //
        // The order matters and is not arbitrary: doing (3) before (2) would let
        // `new(50, 10, 30)` narrow `min` to 30 and then widen `max` to 30 — a fixed 30
        // where the caller's preferred size of 10 was silently discarded. Resolving
        // `max` against `pref` first keeps the caller's preferred size whenever any
        // value in the range can represent it.
        let max = max.max(pref);
        let min = min.min(max);
        let pref = pref.clamp(min, max);
        Self { min, pref, max }
    }

    /// A fixed size: `min == pref == max`.
    pub fn fixed(size: u32) -> Self {
        Self { min: size, pref: size, max: size }
    }

    /// Only a floor: the widget may grow without bound but never shrink past `min`.
    pub fn at_least(min: u32) -> Self {
        Self { min, pref: min, max: u32::MAX }
    }

    /// Only a ceiling: the widget may shrink to nothing but never grow past `max`.
    pub fn at_most(max: u32) -> Self {
        Self { min: 0, pref: max, max }
    }

    /// No wish at all: free to shrink to nothing and grow without bound.
    pub fn unconstrained() -> Self {
        Self { min: 0, pref: 0, max: u32::MAX }
    }

    /// Clamps `value` into this hint's range.
    ///
    /// The single place a layout should use to honour a hint, so no consumer has to
    /// re-implement the clamp (and none can forget half of it).
    pub fn clamp(&self, value: u32) -> u32 {
        value.clamp(self.min, self.max)
    }

    /// Whether the axis may change size at all.
    pub fn is_fixed(&self) -> bool {
        self.min == self.max
    }
}

impl Default for AxisHints {
    /// Unconstrained — the same answer the old `size_hint() -> Size::ZERO` gave, so a
    /// widget that has not migrated to [`Hints`] keeps its previous behaviour in a
    /// layout rather than suddenly acquiring a floor. This is what makes the
    /// migration incremental instead of all-at-once.
    fn default() -> Self {
        Self::unconstrained()
    }
}

/// A widget's size wish on both axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hints {
    /// The horizontal wish.
    pub width: AxisHints,
    /// The vertical wish.
    pub height: AxisHints,
}

impl Hints {
    /// Fixed on both axes.
    pub fn fixed(width: u32, height: u32) -> Self {
        Self { width: AxisHints::fixed(width), height: AxisHints::fixed(height) }
    }

    /// A size that is a floor on both axes.
    pub fn at_least(width: u32, height: u32) -> Self {
        Self { width: AxisHints::at_least(width), height: AxisHints::at_least(height) }
    }

    /// The preferred size, clamped into the permitted range.
    ///
    /// This is the size a layout uses when nothing forces it either way, which is
    /// what the old `size_hint()` reported unconditionally.
    pub fn preferred(&self) -> Size {
        Size::new(self.width.pref, self.height.pref)
    }

    /// Both axes clamped into `bounds`.
    pub fn clamp_to(&self, bounds: Size) -> Size {
        Size::new(self.width.clamp(bounds.width), self.height.clamp(bounds.height))
    }
}

/// How a child wants its parent's layout to treat it.
///
/// # Why "stretch" cannot be derived from a hint
///
/// "I want to be big" and "the layout should stretch me" are different statements: a
/// slider's `pref` is a fixed number, yet it *should* be stretched across a form; a
/// button's `pref` is also a fixed number, and it should *not* be. The two are
/// indistinguishable by size alone, so the flag has to be separate — which is what
/// Qt's `Layout.fillWidth` is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutParams {
    /// Take all remaining room on the major axis.
    pub fill: bool,
    /// Share of the *leftover* room on the major axis, for proportional splitting.
    ///
    /// Same meaning as Qt's `Layout.stretchFactor` and as this crate's existing
    /// `add_widget(id, stretch)` argument.
    pub stretch: u32,
    /// Space removed from the available area before this child is placed.
    pub margins: EdgeOffsets,
}

impl LayoutParams {
    /// A child that takes its preferred size and nothing more.
    pub fn new() -> Self {
        Self::default()
    }

    /// A child that should absorb the leftover room on the major axis.
    pub fn filled() -> Self {
        Self { fill: true, ..Self::default() }
    }

    /// A child that should absorb leftover room, weighted.
    pub fn stretched(stretch: u32) -> Self {
        Self { fill: true, stretch, ..Self::default() }
    }

    /// The same parameters with explicit margins.
    pub fn with_margins(mut self, margins: EdgeOffsets) -> Self {
        self.margins = margins;
        self
    }
}

/// What a layout needs to know about one child.
///
/// # Why this parameter exists
///
/// The previous signature,
/// `update(&self, rect, &mut dyn FnMut(ObjectId, Rect))`, could only *write* a child's
/// geometry. It could not read what the child wanted, which is why every layout that
/// needed sizes invented its own way to be told in advance — see the module docs.
/// Passing `&[ChildInfo]` is what turns "the caller tells the layout" into "the layout
/// asks the child".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildInfo {
    /// The child this entry describes.
    pub id: ObjectId,
    /// What the child wants, in its own terms.
    pub hints: Hints,
    /// How the child wants to be treated.
    pub params: LayoutParams,
}

impl ChildInfo {
    /// A child that wants `hints` and asks for nothing special.
    pub fn new(id: ObjectId, hints: Hints) -> Self {
        Self { id, hints, params: LayoutParams::new() }
    }

    /// The same entry with explicit layout parameters.
    pub fn with_params(mut self, params: LayoutParams) -> Self {
        self.params = params;
        self
    }

    /// The id of `child` in `children`, if it is there.
    pub fn find(children: &[ChildInfo], id: ObjectId) -> Option<&ChildInfo> {
        children.iter().find(|child| child.id == id)
    }

    /// The room this child is asking for on both axes: its preferred extent plus its own
    /// margins.
    ///
    /// # Why this belongs on [`ChildInfo`]
    ///
    /// A composite that lays a small row out and then needs the row's own total width has to
    /// add the children's sizes *and* their gaps back up. Doing that by hand means the caller
    /// re-derives a rule the layout already applied, and the two spellings drift — the failure
    /// mode rule #101 names. `FlexLayout::arrange` performs exactly this addition for each
    /// child before handing the sizes to its solver; exposing it here lets a caller ask the
    /// same question of the same entry instead of restating it.
    ///
    /// `pref` is the right term rather than `min`: a row that has room lays each child out at
    /// its preferred size, and a caller sizing a *container* around such a row must not be told
    /// the sum of the minima (which would under-report and clip the row it is meant to hold).
    pub fn bounds(&self) -> Size {
        let preferred = self.hints.preferred();
        Size::new(
            preferred.width.saturating_add(self.params.margins.horizontal_total()),
            preferred.height.saturating_add(self.params.margins.vertical_total()),
        )
    }
}

/// The total room `children` ask for along an axis, including each child's own margins.
///
/// The split from [`total_preferred`] is the `padding`/`spacing` split rule 4 of the assembly
/// spec draws: `total_preferred` answers "how much content is in here", this answers "how much
/// room do the children occupy once their gaps are paid for". A composite that wants to size
/// itself around its children wants the second.
pub fn total_bounds(children: &[ChildInfo], vertical: bool) -> u32 {
    children
        .iter()
        .map(|child| if vertical { child.bounds().height } else { child.bounds().width })
        .sum()
}

/// The children's collective minimum and preferred extent along an axis.
///
/// Because a list of hints is consulted constantly, the two aggregates a layout asks
/// for most are computed here rather than at each call site.
pub fn total_preferred(children: &[ChildInfo], vertical: bool) -> u32 {
    children.iter().map(|child| axis(&child.hints, vertical).pref).sum()
}

/// The sum of the children's minima along an axis.
pub fn total_minimum(children: &[ChildInfo], vertical: bool) -> u32 {
    children.iter().map(|child| axis(&child.hints, vertical).min).sum()
}

/// The largest preferred extent among the children, or 0 for an empty list.
pub fn max_preferred(children: &[ChildInfo], vertical: bool) -> u32 {
    children.iter().map(|child| axis(&child.hints, vertical).pref).max().unwrap_or(0)
}

/// The hints for the axis a layout is laying out along.
///
/// Every generic layout needs this selection, and writing `if vertical { .. }` at each
/// site is how the horizontal and vertical paths of one layout come to drift apart.
pub fn axis(hints: &Hints, vertical: bool) -> AxisHints {
    if vertical {
        hints.height
    } else {
        hints.width
    }
}

/// The children of `children` in the order a layout should place them.
///
/// Currently the input order, exposed as a function so a future ordering rule (RTL
/// mirroring, `order:` parameters) has one place to live rather than being applied
/// inconsistently at each layout.
pub fn placement_order(children: &[ChildInfo]) -> Vec<ObjectId> {
    children.iter().map(|child| child.id).collect::<Vec<_>>()
}

/// The ids whose `fill` flag is set, for a layout distributing leftover room.
pub fn filling(children: &[ChildInfo]) -> Vec<ObjectId> {
    let mut ids = vec![];
    for child in children {
        if child.params.fill {
            ids.push(child.id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_hint_is_always_ordered() {
        // The point of constructing through `new` rather than a struct literal: an
        // unnormalised value must not be representable, so no consumer has to
        // re-normalise before reading (which is what Qt has to do).
        //
        // `pref` is the anchor: it is what the widget computed from its own content, so
        // a value that the range *can* represent is never discarded.
        let hint = AxisHints::new(30, 10, 50);
        // A minimum above the preferred size moves the preferred size up to the floor
        // — the request was "at least 30", so 30 is the honest preferred value.
        assert_eq!(hint, AxisHints { min: 30, pref: 30, max: 50 });

        // A maximum below the preferred size raises the maximum, because the caller's
        // preferred size is the one value they certainly meant.
        let hint = AxisHints::new(10, 60, 50);
        assert_eq!(hint, AxisHints { min: 10, pref: 60, max: 60 });

        // Every result satisfies the invariant, whatever the input.
        for (min, pref, max) in [(0, 0, 0), (5, 5, 5), (100, 1, 2), (1, 100, 2), (50, 40, 30)] {
            let hint = AxisHints::new(min, pref, max);
            assert!(
                hint.min <= hint.pref && hint.pref <= hint.max,
                "{min}/{pref}/{max} normalised to {hint:?}"
            );
        }
    }

    #[test]
    fn a_contradictory_range_stays_representable() {
        // `min=50, max=30` cannot both hold. The result must still be a valid range
        // (or `clamp` would be undefined) and must not silently invert the caller's
        // intent — the preferred size of 40 survives as the anchor.
        let hint = AxisHints::new(50, 40, 30);
        assert!(hint.min <= hint.pref && hint.pref <= hint.max);
        assert_eq!(hint.pref, 40, "the caller's preferred size is the anchor");
        assert!(hint.clamp(1) >= hint.min);
        assert!(hint.clamp(999) <= hint.max);
    }

    #[test]
    fn the_three_constructors_express_the_three_shapes() {
        let fixed = AxisHints::fixed(40);
        assert!(fixed.is_fixed());
        assert_eq!(fixed.clamp(0), 40);
        assert_eq!(fixed.clamp(100), 40);

        let floor = AxisHints::at_least(30);
        assert_eq!(floor.clamp(5), 30, "a floor cannot be squeezed past");
        assert_eq!(floor.clamp(500), 500, "and imposes no ceiling");

        let ceiling = AxisHints::at_most(30);
        assert_eq!(ceiling.clamp(5), 5, "a ceiling does not force growth");
        assert_eq!(ceiling.clamp(500), 30, "but it does stop growth");
    }

    #[test]
    fn the_default_hint_preserves_the_old_size_hint_behaviour() {
        // A widget that has not migrated must not suddenly acquire a floor, or the
        // migration could not be done in pieces — which is the whole reason the
        // default is unconstrained rather than "the widget's size".
        let none = Hints::default();
        assert_eq!(none.preferred(), Size::new(0, 0));
        assert_eq!(none.width.clamp(1234), 1234);
        assert_eq!(none.height.clamp(0), 0);
    }

    #[test]
    fn both_axes_are_kept_separate() {
        // A slider: free to stretch horizontally, fixed vertically. Collapsing the two
        // axes into one value is exactly what makes a layout stretch a control that
        // must not stretch.
        let hints = Hints { width: AxisHints::at_least(120), height: AxisHints::fixed(24) };
        assert_eq!(hints.height.clamp(999), 24);
        assert_eq!(hints.width.clamp(999), 999);
        assert_eq!(hints.preferred(), Size::new(120, 24));
    }

    #[test]
    fn clamping_to_bounds_respects_each_axis_independently() {
        let hints = Hints::at_least(100, 50);
        let clamped = hints.clamp_to(Size::new(60, 200));
        assert_eq!(clamped, Size::new(100, 200));
    }

    #[test]
    fn fill_is_declared_separately_from_size() {
        // Two children with identical hints must be distinguishable by `fill`, or a
        // layout could not tell "stretch me" from "leave me alone" — the exact reason
        // Qt has `Layout.fillWidth` alongside `implicitWidth`.
        let button = ChildInfo::new(1, Hints::fixed(64, 40));
        let slider = ChildInfo::new(2, Hints::fixed(64, 40)).with_params(LayoutParams::filled());
        assert_eq!(button.hints, slider.hints);
        assert!(!button.params.fill);
        assert!(slider.params.fill);
        assert_eq!(filling(&[button, slider]), vec![2]);
    }

    #[test]
    fn the_axis_selector_reads_the_requested_axis() {
        let hints = Hints { width: AxisHints::fixed(7), height: AxisHints::fixed(9) };
        assert_eq!(axis(&hints, false).pref, 7);
        assert_eq!(axis(&hints, true).pref, 9);
    }

    #[test]
    fn the_aggregates_sum_and_maximise_over_the_right_axis() {
        let children = vec![
            ChildInfo::new(1, Hints::at_least(10, 4)),
            ChildInfo::new(2, Hints::at_least(20, 6)),
        ];
        assert_eq!(total_preferred(&children, false), 30);
        assert_eq!(total_minimum(&children, false), 30);
        assert_eq!(total_preferred(&children, true), 10);
        assert_eq!(max_preferred(&children, true), 6);
        assert_eq!(max_preferred(&[], false), 0, "an empty list has no largest child");
    }

    #[test]
    fn a_child_can_be_looked_up_by_id() {
        let children =
            vec![ChildInfo::new(1, Hints::fixed(1, 1)), ChildInfo::new(2, Hints::fixed(2, 2))];
        assert_eq!(ChildInfo::find(&children, 2).map(|c| c.hints.width.pref), Some(2));
        assert!(ChildInfo::find(&children, 9).is_none());
        assert_eq!(placement_order(&children), vec![1, 2]);
    }

    #[test]
    fn stretched_params_carry_their_weight_and_margins() {
        let params = LayoutParams::stretched(3).with_margins(EdgeOffsets::symmetric(4, 8));
        assert!(params.fill);
        assert_eq!(params.stretch, 3);
        assert_eq!(params.margins.horizontal_total(), 16);
    }
}
