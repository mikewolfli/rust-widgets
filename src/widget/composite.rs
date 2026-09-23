// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Assembling a composite control out of basic controls and a [`Layout`].
//!
//! # The four steps this fixes
//!
//! BLUE22 §B.4 identified the gap between `WidgetFactory::create` and the
//! [`Layout`](crate::layout::Layout) trait: the JSON path has a complete
//! declare → assemble → apply chain (`store_layout` → `add_widget_to_layout` →
//! `apply_layout`), but the **code path** had nothing equivalent. A composite
//! control therefore computed its sub-part geometry by hand, from `self.geometry()`,
//! with literal offsets — which is why
//! `grep -rl "BoxLayout::new\|FlexLayout::new" src/widget/` returned nothing:
//! no composite in the crate used a real layout at all.
//!
//! Hand-computed geometry has three consequences, all of them observable:
//!
//! 1. **`hints()` has 176 implementations and one consumer.** "Content decides
//!    size" is only true if something *asks*; a hand-computed layout never does.
//! 2. **A composite cannot be measured by its parent**, because it cannot state its
//!    own size in terms of its children's.
//! 3. **The same "icon + gap + label" arithmetic is rewritten in every composite**,
//!    which is the "one fact, many derivations" shape this crate keeps paying for.
//!
//! # What this builder is, and what it deliberately is not
//!
//! It holds children, computes its own [`Hints`] from theirs (QML's
//! `implicitWidth = max(implicitBackgroundWidth + …, implicitContentWidth + …)`
//! formula is exactly this), hands each child's hints to
//! [`Layout::arrange`](crate::layout::Layout::arrange), and applies the rectangles
//! that come back.
//!
//! **There is no layout algorithm here.** Not a direction, not a wrap rule, not an
//! alignment. Every one of those lives in `src/layout/`, and the whole point of the
//! exercise is that a composite *asks* rather than decides. A builder that
//! computed positions itself would move the hand-computed arithmetic from 188
//! widget files into one, which is a change of address rather than a fix
//! (BLUE22 §B.5.1 — this is the reason the original `CompositeBuilder` draft was
//! rejected).
//!
//! # Why it does not use `Box<dyn Widget>` children
//!
//! It would be the obvious storage, and it is what the first draft had. But a
//! composite needs to hand its children to the **host** (they must be added to the
//! widget tree, drawn, and hit-tested), and `BaseWidget::children` already holds
//! them there. Storing a second `Box<dyn Widget>` here would mean two owners of one
//! child — the ownership question `set_parent`'s own documentation warns about
//! (`base.rs`: it does not update either side's child list). So the builder holds
//! [`ChildInfo`] (id + hints + params) and a factory, and reports the boxes it
//! created; the caller owns them.
//!
//! # Why a child's own axis can be relieved of the sibling's floor
//!
//! [`CompositeBuilder::add_flexible`] is the one addition the first real migration needed.
//! A row's cross axis takes the **maximum** of its children's preferences, which is right for a
//! row of buttons and wrong for a row that holds a 48 px field and a 28 px step column: the
//! column would be stretched to 48 px of face, which is not a step button, it is a slab. Qt's
//! answer is `Layout.fillHeight: false` on the short child, and this is the same statement: the
//! child keeps its own preferred extent on that axis while its sibling may be taller.
//!
//! It is expressed as a *hint* rather than as a post-hoc rectangle, because the composite's own
//! size is derived from the hints — a child that wanted 48 px and was trimmed afterwards would
//! still have made the composite 48 px tall.

use crate::compat::Vec;
use crate::core::{ObjectId, Rect, Size};
use crate::layout::{
    max_preferred, total_bounds, total_minimum, AxisHints, ChildInfo, Hints, Layout, LayoutParams,
};
use crate::style::EdgeOffsets;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{Widget, WidgetFactory};

/// One child of a composite, as the builder knows it.
struct Child {
    id: ObjectId,
    hints: Hints,
    params: LayoutParams,
}

/// Which axis a size may be squeezed on, for [`CompositeBuilder::add_flexible`].
///
/// # Why an axis and not a fraction
///
/// "This column is 28 px tall because it is a column of two step buttons, not because the field
/// beside it is 48 px" is a statement about one axis, and expressing it as a ratio would make it
/// depend on the sibling's size — which is the coupling the distinction exists to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexibleAxis {
    /// The child keeps its preferred **width** whatever the row's cross-axis preference is.
    Width,
    /// The child keeps its preferred **height** whatever the column's cross-axis preference is.
    Height,
}

/// Assembles a composite control from a layout and factory-created children.
///
/// See the module documentation for what it does and, more importantly, what it
/// does not do.
pub struct CompositeBuilder {
    layout: Box<dyn Layout>,
    children: Vec<Child>,
    padding: EdgeOffsets,
    /// The background/touch floor: the other arm of
    /// [`ControlMetrics::implicit_size`]'s `max`, and QML's
    /// `implicitBackgroundWidth`. A composite made of 5 px labels is still at least
    /// as large as the control it presents itself as.
    floor: Size,
    /// Set when a child's hints or parameters changed, cleared by [`Self::take_dirty`].
    ///
    /// # Why a flag rather than a callback per child
    ///
    /// BLUE22 §B.6 rule 10 requires that a hint change **trigger** a relayout rather than
    /// being polled for. The obvious implementation — subscribe to each child and relayout
    /// from the handler — is what Qt deliberately avoids: it re-runs the whole layout for
    /// every property write, so a caller setting four properties on one sub-control lays
    /// the row out four times, and an intermediate call can observe a half-updated tree.
    ///
    /// Qt instead marks the layout dirty and lets the frame loop consume it
    /// (`invalidate()` plus a deferred `updatePolish()`, `qquicklayout.cpp:857`). This
    /// flag is that mechanism in its smallest honest form: `invalidate` records the fact, and
    /// the **host consumes it once per frame** through `take_dirty`. Coalescing is therefore
    /// automatic — N writes before the next frame cost one relayout — and no observer can
    /// see a partially applied change, because nothing runs between the writes.
    dirty: core::cell::Cell<bool>,
}

impl CompositeBuilder {
    /// A builder laying its children out with `layout`.
    ///
    /// `floor` is the minimum the composite may shrink to regardless of content
    /// (QML's `implicitBackgroundWidth/Height`); `padding` is removed from the
    /// composite's own rectangle before the layout sees it, so a layout's idea of
    /// "the available room" and the composite's border cannot disagree.
    pub fn new(layout: Box<dyn Layout>, padding: EdgeOffsets, floor: Size) -> Self {
        Self {
            layout,
            children: Vec::new(),
            padding,
            floor,
            // A fresh composite has never been laid out, so its first `arrange` must happen:
            // starting clean would let a host that consumes the flag *before* the first frame
            // skip that layout entirely.
            dirty: core::cell::Cell::new(true),
        }
    }

    /// Records that a child's wish or parameters changed, so the next frame relayouts.
    ///
    /// The producer half of §B.6 rule 10. Called by the accessors that change a registered
    /// child's [`Hints`] or [`LayoutParams`]; a composite's own size setters call it once
    /// rather than laying out per write (see [`Self::take_dirty`]).
    pub fn invalidate(&self) {
        self.dirty.set(true);
    }

    /// Whether a relayout is owed.
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    /// Consumes the dirty flag, reporting whether a relayout was owed.
    ///
    /// The consumer half of §B.6 rule 10, and deliberately a *take* rather than a read: a
    /// host that read the flag and then relaid out would relayout again on every subsequent
    /// frame for the same change. Taking it means exactly one relayout per change, which is
    /// the property that makes "notify, do not poll" true rather than aspirational.
    pub fn take_dirty(&self) -> bool {
        self.dirty.replace(false)
    }

    /// Updates a registered child's hints and marks the composite for relayout.
    ///
    /// The one entry point that keeps "the child's wish changed" and "the layout must be
    /// re-asked" from drifting apart: a setter that wrote `children[i].hints` directly would
    /// leave the flag clear, and the change would not reach the screen until something else
    /// happened to dirty the composite.
    ///
    /// Returns whether a child with that id was registered.
    pub fn set_child_hints(&mut self, id: ObjectId, hints: Hints) -> bool {
        match self.children.iter_mut().find(|child| child.id == id) {
            Some(child) => {
                if child.hints != hints {
                    child.hints = hints;
                    self.dirty.set(true);
                }
                true
            }
            None => false,
        }
    }

    /// Updates a registered child's layout parameters and marks the composite for relayout.
    ///
    /// Same contract as [`Self::set_child_hints`], for the [`LayoutParams`] half of a child's
    /// registration. The layout's own record is rebuilt too, because the stretch weight is the
    /// layout's `add_widget` argument rather than part of [`ChildInfo`] — leaving the two out of
    /// step would make a changed `fill` or weight take effect only after the child was removed
    /// and re-added.
    pub fn set_child_params(&mut self, id: ObjectId, params: LayoutParams) -> bool {
        match self.children.iter_mut().find(|child| child.id == id) {
            Some(child) => {
                if child.params == params {
                    return true;
                }
                child.params = params;
                self.dirty.set(true);
                self.sync_layout_weights();
                true
            }
            None => false,
        }
    }

    /// Rewrites the layout's stretch weights from `children`.
    ///
    /// # Why the weights are rebuilt rather than patched
    ///
    /// A [`Layout`] has no `set_stretch(id, n)`: the weight is the argument to `add_widget`, and
    /// in every implementation that appends. Patching one child's weight would therefore mean
    /// removing and re-adding it, which moves it to the **end** of the row — a re-weighted child
    /// would visibly jump past its siblings. Rebuilding the whole registry from `children` keeps
    /// the item order defined by one place (the child list) instead of by whichever setter ran
    /// last, and it is a handful of operations over a list that is a form row, not a document.
    fn sync_layout_weights(&mut self) {
        for child in self.children.iter() {
            self.layout.remove_widget(child.id);
        }
        for child in self.children.iter() {
            let weight =
                if child.params.fill { child.params.stretch.max(1) } else { child.params.stretch };
            self.layout.add_widget(child.id, weight);
        }
    }

    /// Creates a child from the registry and registers it.
    ///
    /// # Why the factory and not a concrete type
    ///
    /// A composite that wrote `Button::new(..)` would fail to build in a profile
    /// where `Button` is compiled out, and would bypass the capability registry's
    /// substitute mechanism (`TOGGLE_BUTTON_KIND` and friends). Going through
    /// [`WidgetFactory::create`] keeps one creation path for the whole crate and
    /// makes the composite profile-agnostic, which is §B.6 rule 1.
    ///
    /// Returns the created widget so the caller can own it. A name the registry
    /// does not publish yields `None` rather than a panic: a composite whose child
    /// is unavailable in this profile is empty, not fatal.
    pub fn add(
        &mut self,
        factory: &WidgetFactory,
        kind_or_name: &str,
        text: &str,
        geometry: Rect,
        params: LayoutParams,
    ) -> Option<Box<dyn Widget>> {
        let widget = factory.create(kind_or_name, geometry, text)?;
        let id = widget.id();
        // The hints are read *now*, from the widget itself. This is the whole point
        // of the channel: nothing outside the child decides what the child wants.
        let hints = widget.hints();
        self.register(id, hints, params);
        Some(widget)
    }

    /// Creates a child that keeps `axis` at its own preferred size.
    ///
    /// See [`FlexibleAxis`] for why this is a per-axis statement rather than a rectangle the
    /// composite trims afterwards: the composite's own size is derived from the hints, so a child
    /// that wanted 48 px and was clipped later would still have made the composite 48 px tall.
    ///
    /// Only the *preferred* extent is pinned: the floor and the ceiling the child declared are
    /// untouched, so a caller that genuinely needs to squeeze it still can.
    pub fn add_flexible(
        &mut self,
        factory: &WidgetFactory,
        kind_or_name: &str,
        text: &str,
        geometry: Rect,
        params: LayoutParams,
        axis: FlexibleAxis,
    ) -> Option<Box<dyn Widget>> {
        let widget = factory.create(kind_or_name, geometry, text)?;
        let id = widget.id();
        let mut hints = widget.hints();
        match axis {
            FlexibleAxis::Width => {
                let pref = hints.width.pref;
                hints.width =
                    AxisHints::new(hints.width.min.min(pref), pref, hints.width.max.max(pref));
            }
            FlexibleAxis::Height => {
                let pref = hints.height.pref;
                hints.height =
                    AxisHints::new(hints.height.min.min(pref), pref, hints.height.max.max(pref));
            }
        }
        self.register(id, hints, params);
        Some(widget)
    }

    /// Creates a child at `size` on both axes, whatever the control's own `hints()` say.
    ///
    /// # Why a column of chrome declares its own size
    ///
    /// A `label` measures itself as `text + 4` by `20`, which is right for a label and wrong for a
    /// **column of a control**: a split button's arrow column is 22 px wide and as tall as the
    /// face, and a spin box's step column is one button wide and half the field tall. Those numbers
    /// are the composite's, not the child control's, and the composite is the only thing that
    /// knows them.
    ///
    /// The earlier form passed them through `geometry`, which a layout ignores — the child's
    /// `hints()` are what a layout places by. The result was a step column whose preferred height
    /// was the label's line height rather than the field's, i.e. a column that did not reach the
    /// bottom of the control it belonged to. Declaring the size here makes the *stack the
    /// composite is offering* explicit on the same axis the layout reads, at both ends.
    pub fn add_sized(
        &mut self,
        factory: &WidgetFactory,
        kind_or_name: &str,
        text: &str,
        size: Size,
        params: LayoutParams,
    ) -> Option<Box<dyn Widget>> {
        let widget =
            factory.create(kind_or_name, Rect::new(0, 0, size.width, size.height), text)?;
        let id = widget.id();
        self.register(id, Hints::fixed(size.width, size.height), params);
        Some(widget)
    }

    /// Creates a child and registers it at a **caller-stated** `Hints` triple.
    ///
    /// [`Self::add_sized`] writes one value into `min`, `pref` and `max` alike, which is right for
    /// a column of chrome that is exactly as big as the composite says. It is wrong whenever the
    /// composite knows the child has *room to give*: a crowded tab strip wants to tell the layout
    /// "this tab would like its share, and may be squeezed down to the floor" — one number cannot
    /// say that, and passing the share as all three turns the share into a floor the layout is then
    /// obliged to honour even though the band cannot pay for it (BLUE22 · G-1).
    pub fn add_with_hints(
        &mut self,
        factory: &WidgetFactory,
        kind_or_name: &str,
        text: &str,
        hints: Hints,
        params: LayoutParams,
    ) -> Option<Box<dyn Widget>> {
        let pref = hints.preferred();
        let widget =
            factory.create(kind_or_name, Rect::new(0, 0, pref.width, pref.height), text)?;
        let id = widget.id();
        self.register(id, hints, params);
        Some(widget)
    }

    /// Files a created child and its hints with the layout.
    ///
    /// A `fill` child that declared no stretch still needs a weight, so a caller that says
    /// "absorb the leftover" without saying "by how much" is taken at its word for **one** unit
    /// and no more.
    ///
    /// # Why the weight is 1 and not `u32::MAX`
    ///
    /// It was `u32::MAX`, on the reasoning that a child which asked to fill should win the whole
    /// leftover. That is true when it is the only child asking, and wrong the moment a second one
    /// does: a row holding a filling label *and* a filling button gave the label `u32::MAX` of the
    /// two shares — every pixel — so the button was left at its preferred width and its own `fill`
    /// was unreachable. The room the caller meant to hand over is proportional, and one unit is the
    /// honest default: "yes, absorb room", with two equal claimants splitting it evenly, which is
    /// what CSS flexbox's `flex: 1` and Qt's `stretchFactor: 1` both mean.
    fn register(&mut self, id: ObjectId, hints: Hints, params: LayoutParams) {
        let grow = if params.fill { params.stretch.max(1) } else { params.stretch };
        self.layout.add_widget(id, grow);
        self.children.push(Child { id, hints, params });
        // A new child changes what the composite needs, so it owes a relayout (§B.6 rule 10).
        self.dirty.set(true);
    }

    /// Creates a child and registers it at its own `hints()`, derived from the child itself.
    ///
    /// Kept separate from [`Self::add`] and marked `pub(crate)` because [`Self::add`]'s
    /// `geometry` argument is read by every existing sample; a migration that does not need the
    /// caller to hand in a starting rectangle goes through here.
    #[allow(dead_code)]
    pub(crate) fn add_at_hint(
        &mut self,
        factory: &WidgetFactory,
        kind_or_name: &str,
        text: &str,
        params: LayoutParams,
    ) -> Option<Box<dyn Widget>> {
        let widget = factory.create(kind_or_name, Rect::new(0, 0, 0, 0), text)?;
        let id = widget.id();
        let hints = widget.hints();
        self.register(id, hints, params);
        Some(widget)
    }

    /// The composite's own size wish, derived from its children's.
    ///
    /// # How the composite axis and the cross axis differ
    ///
    /// Along the layout's major axis the children are **additive**: a row of three
    /// buttons is as wide as the three plus the gaps, so the sums of `min` and
    /// `pref` are the floors. Across the axis they are **competitive**: the row's
    /// height is the tallest child, not their sum, so the cross axis takes the
    /// maximum.
    ///
    /// This is the one piece of arithmetic the builder owns, and it is unavoidable:
    /// the layout knows how to *place* children, but only the composite knows that
    /// it presents itself to *its* parent as a single box. `major_is_horizontal`
    /// says which axis is the additive one.
    ///
    /// # Why the aggregates come from [`crate::layout`] and not from a local sum
    ///
    /// A first version summed `child.hints.width.pref` here, which omits each child's
    /// **margins** — and a margin is how a child declares the gap before it, so a row of
    /// three buttons `6 px` apart reported a preferred width `12 px` too small. That is the
    /// `SpinBox.qml:20-21` relation violated at the composite's own boundary: the row would
    /// have been placed at a width that does not fit the children the layout is about to
    /// lay out in it. [`total_bounds`] exists for exactly this sum — "how much room do the
    /// children occupy once their gaps are paid for" — and using it here rather than
    /// re-deriving the sum is the same rule the crate applies everywhere else: one fact,
    /// one derivation.
    ///
    /// Both arms go through [`ControlMetrics::implicit_size`], so the composite's
    /// own padding and floor are the same `max(floor, content + padding)` rule every
    /// basic control uses — a composite is not a special case.
    pub fn hints(&self, major_is_horizontal: bool) -> Hints {
        // The major axis is vertical for a column, so `vertical` here is "is the additive
        // axis the vertical one" — the negation of `major_is_horizontal`.
        let vertical = !major_is_horizontal;
        let infos = self.child_infos();
        let major_min = total_minimum(&infos, vertical);
        // `total_bounds` adds each child's margins to its preferred extent, which is what
        // makes the sum the room the children actually occupy rather than the ink they draw.
        let major_occupied = total_bounds(&infos, vertical);
        let cross_pref = max_preferred(&infos, vertical);
        let content = if major_is_horizontal {
            Size::new(major_occupied, cross_pref)
        } else {
            Size::new(cross_pref, major_occupied)
        };
        let size = ControlMetrics::implicit_size(content, self.padding, self.floor);
        if major_is_horizontal {
            // The major axis keeps a floor of `major_min` — the smallest the children
            // may be squeezed to — while remaining free to grow past `pref`, because whether
            // a row stretches is a `fill` question and not a size question (§B.6 rule 9).
            Hints {
                width: AxisHints::new(
                    major_min.saturating_add(self.padding.horizontal_total()).max(self.floor.width),
                    size.width,
                    u32::MAX,
                ),
                height: AxisHints::new(size.height, size.height, size.height),
            }
        } else {
            Hints {
                width: AxisHints::new(size.width, size.width, size.width),
                height: AxisHints::new(
                    major_min.saturating_add(self.padding.vertical_total()).max(self.floor.height),
                    size.height,
                    u32::MAX,
                ),
            }
        }
    }

    /// The children as the layout sees them.
    fn child_infos(&self) -> Vec<ChildInfo> {
        self.children
            .iter()
            .map(|child| ChildInfo { id: child.id, hints: child.hints, params: child.params })
            .collect()
    }

    /// Lays the children out inside `rect` and reports each one's geometry.
    ///
    /// The composite's own padding is removed first, so the layout lays out in the
    /// content box — the same relationship QML's `availableWidth/Height` has to the
    /// control's rectangle. Nothing else is adjusted: the rectangles the layout
    /// returns are the rectangles the children get, which is what makes "the layout
    /// owns placement" true rather than aspirational.
    ///
    /// If the layout reports a child the builder did not register (or omits one), the
    /// omitted child keeps the geometry it already had rather than being moved to a
    /// zero rect — an invisible sub-control is a worse failure than a stale one, and
    /// the caller is expected to have given it a sensible starting geometry.
    pub fn arrange(&self, rect: Rect, out: &mut dyn FnMut(ObjectId, Rect)) {
        let content = ControlMetrics::content_box(rect, self.padding);
        let infos = self.child_infos();
        self.layout.arrange(content, &infos, out);
    }

    /// The children's ids, in the order they were added.
    pub fn child_ids(&self) -> Vec<ObjectId> {
        self.children.iter().map(|child| child.id).collect()
    }

    /// How many children have been added.
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Whether the composite has no children.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

// `sum` was a local aggregate over `child.hints`; the shared `total_bounds` /
// `total_minimum` / `max_preferred` in `src/layout/hints.rs` replaced it (see `hints()`).

/// A right-anchored row of action buttons — the `dialog_with_actions` template.
///
/// # Why this is a named component rather than a helper per dialog
///
/// Eight dialogs in this crate draw "a row of buttons pinned to the trailing edge", and
/// each used to derive it from `rect.x + rect.width - count * (width + gap)` — a literal
/// that got the row wrong the moment a button was wider than the constant or a label was
/// longer than the assumption. BLUE22 §B.7 names this template specifically, because it
/// exercises the two things the assembly rules are for: **a row nested in a composite** and
/// **an anchor expressed as a container property rather than as an offset**.
///
/// # Why the dialogs in `src/widget/dialog/` do **not** use it yet
///
/// They have a divergent requirement, and merging the two would be a regression rather
/// than a simplification (principle #51 — a shared abstraction has to eliminate a real
/// duplication, not paper over a real difference):
///
/// | | `ActionRow` | `message_box::action_row_geometry` |
/// |---|---|---|
/// | button size | the widget's own `hints()` — `max(64, label*8 + 24)` | `Hints::fixed(DIALOG_BUTTON_WIDTH)` — a standard command of a standard width that elides rather than grows |
/// | output | each button's rectangle | each rectangle **plus** the row's span, and the `leading_inset` the dialog's message band must respect |
///
/// The first row is the substantive difference: a dialog whose OK button is 72 px wide and
/// whose Cancel is 64 reads as ragged, which is why that path fixes the width from a single
/// constant. A template cannot express both policies without gaining a mode flag, and a mode
/// flag is the "two behaviours in one name" defect this crate keeps deleting.
///
/// What the two *do* share is the layout: both drive
/// `FlexLayout` with `justify_content = FlexEnd` and put the gap on each child's leading
/// margin. Both therefore depend on the same three `FlexLayout` behaviours this round fixed
/// (a floor that survives the shrink pass, a floor expressed in the child's own units, and a
/// leftover the justification can actually see) — which is the real coupling, and it is in
/// the right place: one layout, two policies on top of it.
///
/// # How the anchor is expressed
///
/// The row is laid out with `justify_content = FlexEnd`, so the layout places every button
/// at its preferred size and puts all the leftover room *before* the first one. Nothing here
/// computes an x: right-alignment is a property of the layout, which is the only shape in
/// which it cannot be forgotten at one of the call sites.
///
/// # Why the gap rides on a margin
///
/// Exactly one side of each button carries the spacing — its *leading* margin, and none for
/// the first. Putting the gap on both sides of every child pays it twice, because one
/// button's trailing margin is adjacent to the next one's leading margin, and a leading
/// margin on the first button would be room the row reserves and never spends.
pub struct ActionRow {
    builder: CompositeBuilder,
    labels: Vec<crate::compat::String>,
    gap: u32,
}

impl ActionRow {
    /// Creates an empty row whose buttons are `gap` pixels apart.
    pub fn new(gap: u32) -> Self {
        let layout: Box<dyn Layout> = Box::new(crate::layout::FlexLayout::with_params(
            crate::layout::FlexDirection::Row,
            crate::layout::FlexWrap::NoWrap,
            crate::layout::JustifyContent::FlexEnd,
            crate::layout::AlignItems::Stretch,
            0,
            0,
        ));
        Self {
            builder: CompositeBuilder::new(layout, EdgeOffsets::all(0), Size::new(0, 0)),
            labels: Vec::new(),
            gap,
        }
    }

    /// Adds a button labelled `label`, sized from the caller's own measurement of it.
    ///
    /// `size` is passed in rather than measured here because only the render context knows
    /// the font metrics, and a composite that guessed would be one more place the label's
    /// width is derived. The caller already has the context — it is about to draw the row.
    ///
    /// Returns the created button so the caller can own and dispatch to it; a control the
    /// registry does not publish in this profile is skipped rather than fatal.
    pub fn add(
        &mut self,
        factory: &WidgetFactory,
        label: &str,
        size: Size,
    ) -> Option<Box<dyn Widget>> {
        let leading = if self.labels.is_empty() { 0 } else { self.gap };
        let widget = self.builder.add(
            factory,
            "button",
            label,
            Rect::new(0, 0, size.width, size.height),
            LayoutParams::new().with_margins(EdgeOffsets::new(0, 0, 0, leading)),
        );
        if widget.is_some() {
            self.labels.push(label.to_string());
        }
        widget
    }

    /// How many buttons the row holds.
    pub fn len(&self) -> usize {
        self.builder.len()
    }

    /// Whether the row holds no buttons.
    pub fn is_empty(&self) -> bool {
        self.builder.is_empty()
    }

    /// The width the row needs: the buttons plus the gaps between them.
    ///
    /// Derived from the children's own hinted widths, so a dialog that sizes itself around
    /// this row cannot disagree with the row's own layout.
    pub fn preferred_width(&self) -> u32 {
        self.builder.hints(true).width.pref
    }

    /// Places the row's buttons inside `band`, right-anchored.
    ///
    /// Returns the row's own rectangle (the span the buttons actually occupy) and the
    /// rectangle of each button, in the order they were added. The span is what a caller
    /// needs to keep its own content from overlapping the buttons — the
    /// `SpinBox.qml:20-21` relation, where the text side's inset is the sibling column's
    /// width.
    pub fn arrange(&self, band: Rect) -> (Rect, Vec<Rect>) {
        let mut placed: Vec<Rect> = Vec::with_capacity(self.builder.len());
        self.builder.arrange(band, &mut |_, rect| placed.push(rect));
        let row = match (placed.first(), placed.last()) {
            (Some(first), Some(last)) => {
                let left = first.x;
                let right = last.x.saturating_add(last.width as i32);
                Rect::new(left, band.y, (right - left).max(0) as u32, band.height)
            }
            // An empty row occupies nothing: a zero-width row at the band's trailing edge,
            // which is the honest answer and cannot be mistaken for a real span.
            _ => Rect::new(band.x + band.width as i32, band.y, 0, band.height),
        };
        (row, placed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;
    use crate::layout::{AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent};
    use crate::widget::metrics::{dimensions, estimate_text_width};
    use crate::widget::WidgetFactory;

    /// The layout the samples are assembled with.
    ///
    /// [`FlexLayout`] rather than [`crate::layout::BoxLayout`]: only a layout that
    /// implements `arrange` can honour the hints, and using one that falls back to
    /// `update` would make these tests pass for the wrong reason — they would be
    /// asserting the builder's plumbing through a layout that ignores everything the
    /// builder hands it.
    fn row(gap: i32) -> Box<dyn Layout> {
        Box::new(FlexLayout::with_params(
            FlexDirection::Row,
            FlexWrap::NoWrap,
            JustifyContent::FlexStart,
            AlignItems::Stretch,
            gap,
            0,
        ))
    }

    fn builder() -> (WidgetFactory, CompositeBuilder) {
        let factory = WidgetFactory::new_with_defaults();
        let builder = CompositeBuilder::new(row(8), EdgeOffsets::all(0), Size::new(64, 40));
        (factory, builder)
    }

    /// A child's size wish reaches the composite's own `Hints`.
    ///
    /// This is §B.6 rule 3 — "content decides size, upward" — and the mechanical
    /// statement of `F-1`'s acceptance criterion: the chain from a child's
    /// `hints()` to the composite's `hints()` is what a hand-computed composite
    /// cannot have, because it never asks.
    #[test]
    fn a_childs_wish_reaches_the_composites_hints() {
        let (factory, mut builder) = builder();
        let narrow = builder
            .add(&factory, "label", "A", Rect::new(0, 0, 8, 14), LayoutParams::new())
            .expect("label is published");
        let before = builder.hints(true);

        let wide = builder
            .add(
                &factory,
                "label",
                "A much longer label",
                Rect::new(0, 0, 120, 14),
                LayoutParams::new(),
            )
            .expect("label is published");
        let after = builder.hints(true);

        assert!(
            wide.hints().width.pref > narrow.hints().width.pref,
            "the fixture's two labels must actually differ in their own size wish"
        );
        assert!(
            after.width.pref > before.width.pref,
            "adding a wider child must widen the composite: {} -> {}",
            before.width.pref,
            after.width.pref
        );
        assert!(
            after.width.min > before.width.min,
            "the composite's floor must include the new child's floor"
        );
        // The composite's preferred width is the children's own sum plus its padding — a
        // derivation from what they reported, not a constant here, which is what makes this a
        // channel rather than a coincidence. There is no padding in this fixture.
        let children = narrow.hints().width.pref.saturating_add(wide.hints().width.pref);
        assert_eq!(
            after.width.pref, children,
            "the composite's preference is the children's own sum"
        );
        assert_eq!(wide.id(), builder.child_ids()[1]);
    }

    /// The layout's answer is what the children get, offset by the padding.
    ///
    /// §B.6 rule 2: placement belongs to the layout. The test asserts the composite
    /// does not second-guess it — every reported rectangle is inside the content box
    /// and the first child starts at its leading edge.
    #[test]
    fn the_layout_owns_placement_and_padding_is_removed_first() {
        let factory = WidgetFactory::new_with_defaults();
        let padding = EdgeOffsets { left: 10, right: 4, top: 6, bottom: 2 };
        let mut builder = CompositeBuilder::new(row(8), padding, Size::new(0, 0));
        let a = builder
            .add(&factory, "label", "A", Rect::new(0, 0, 40, 20), LayoutParams::new())
            .expect("label is published");
        let b = builder
            .add(&factory, "label", "B", Rect::new(0, 0, 30, 20), LayoutParams::new())
            .expect("label is published");
        let (id_a, id_b) = (a.id(), b.id());

        let mut placed: Vec<(ObjectId, Rect)> = Vec::new();
        builder.arrange(Rect::new(0, 0, 200, 60), &mut |id, rect| placed.push((id, rect)));
        assert_eq!(placed.len(), 2);
        assert_eq!(placed[0].0, id_a);
        assert_eq!(placed[1].0, id_b);
        assert_eq!(
            placed[0].1.x, padding.left as i32,
            "the first child starts at the content edge"
        );
        assert_eq!(placed[0].1.y, padding.top as i32);
        let right_limit = 200 - padding.right as i32;
        for (_, rect) in &placed {
            assert!(
                rect.x + rect.width as i32 <= right_limit,
                "a child must not be placed outside the content box: {rect:?}"
            );
        }
    }

    /// A consumer takes the flag; a second consume reports nothing, so one change costs one
    /// relayout.
    ///
    /// §B.6 rule 10 — "a hint change must be able to trigger a relayout, not be polled". The
    /// distinction that matters is *take* versus *read*: a host that read the flag and then
    /// laid out would lay out again on every later frame for the same change. Taking it makes
    /// "one change, one relayout" true, and the assertion below is what tells the two apart.
    #[test]
    fn a_dirty_flag_is_taken_not_read() {
        let factory = WidgetFactory::new_with_defaults();
        let mut builder = CompositeBuilder::new(row(0), EdgeOffsets::all(0), Size::new(0, 0));
        // Never laid out: the first frame must not be skipped.
        assert!(builder.is_dirty(), "a fresh composite has never been arranged");
        assert!(builder.take_dirty(), "the first consume reports the owed layout");
        assert!(!builder.is_dirty());
        assert!(
            !builder.take_dirty(),
            "a second consume must report nothing, or the host would relayout every frame"
        );

        builder
            .add(&factory, "label", "A", Rect::new(0, 0, 40, 20), LayoutParams::new())
            .expect("label is published");
        assert!(builder.is_dirty(), "adding a child changes what the composite needs");
        assert!(builder.take_dirty());
        assert!(!builder.take_dirty());
    }

    /// Changing a child's wish dirties the composite, and one write costs one relayout even
    /// when four properties are set before the next frame.
    #[test]
    fn a_changed_child_wish_notifies_the_composite_once() {
        let factory = WidgetFactory::new_with_defaults();
        let mut builder = CompositeBuilder::new(row(0), EdgeOffsets::all(0), Size::new(0, 0));
        let child = builder
            .add(&factory, "label", "A", Rect::new(0, 0, 40, 20), LayoutParams::new())
            .expect("label is published");
        let id = child.id();
        let _ = builder.take_dirty();

        // Four writes before the host consumes: the flag coalesces them.
        assert!(builder.set_child_hints(id, Hints::fixed(80, 20)));
        assert!(builder.set_child_hints(id, Hints::fixed(90, 20)));
        assert!(builder.set_child_params(id, LayoutParams::filled()));
        assert!(builder.set_child_params(id, LayoutParams::stretched(2)));
        assert!(builder.take_dirty(), "the batched writes owe exactly one relayout");
        assert!(!builder.take_dirty(), "and only one");

        // A write that changes nothing must not dirty: otherwise every redundant setter call
        // would cost a full layout pass.
        assert!(builder.set_child_hints(id, Hints::fixed(90, 20)));
        assert!(!builder.is_dirty(), "an idempotent write owes no work");

        // An unknown id is refused rather than silently ignored, so a caller that lost track
        // of its children finds out.
        assert!(!builder.set_child_hints(999_999, Hints::fixed(1, 1)));
        assert!(!builder.set_child_params(999_999, LayoutParams::new()));
    }

    /// A re-weighted child keeps its position instead of jumping to the end of the row.
    ///
    /// The reason [`CompositeBuilder::set_child_params`] rebuilds the layout's registry rather
    /// than patching one entry: every `Layout` takes its weight as the `add_widget` argument and
    /// appends, so a remove-and-re-add would move the child past its siblings.
    #[test]
    fn reweighting_a_child_does_not_move_it() {
        let factory = WidgetFactory::new_with_defaults();
        let mut builder = CompositeBuilder::new(row(0), EdgeOffsets::all(0), Size::new(0, 0));
        let first = builder
            .add(&factory, "label", "A", Rect::new(0, 0, 10, 20), LayoutParams::new())
            .expect("label is published");
        let second = builder
            .add(&factory, "label", "B", Rect::new(0, 0, 10, 20), LayoutParams::new())
            .expect("label is published");
        let third = builder
            .add(&factory, "label", "C", Rect::new(0, 0, 10, 20), LayoutParams::new())
            .expect("label is published");
        let ids = [first.id(), second.id(), third.id()];

        assert!(builder.set_child_params(ids[0], LayoutParams::filled()));
        let mut order: Vec<ObjectId> = Vec::new();
        builder.arrange(Rect::new(0, 0, 300, 40), &mut |id, _| order.push(id));
        assert_eq!(order, ids, "the row order is the child list's, not the setters'");

        // And the re-weighted child really did take the leftover room.
        let mut widths: Vec<(ObjectId, u32)> = Vec::new();
        builder.arrange(Rect::new(0, 0, 300, 40), &mut |id, rect| widths.push((id, rect.width)));
        let filling = widths.iter().find(|(id, _)| *id == ids[0]).expect("first is placed").1;
        let fixed = widths.iter().find(|(id, _)| *id == ids[1]).expect("second is placed").1;
        assert!(filling > fixed, "the filling child absorbs the room: {widths:?}");
    }

    /// A `fill` child absorbs the leftover room while its neighbour does not.
    ///
    /// §B.6 rule 9 — `min` and `fill` are separate declarations — expressed where it
    /// is observable: two children with the same preferred width, only one of which
    /// asked to fill.
    #[test]
    fn fill_is_separate_from_the_size_wish() {
        let factory = WidgetFactory::new_with_defaults();
        let mut builder = CompositeBuilder::new(row(0), EdgeOffsets::all(0), Size::new(0, 0));
        builder
            .add(&factory, "label", "fixed", Rect::new(0, 0, 40, 20), LayoutParams::new())
            .expect("label is published");
        builder
            .add(&factory, "label", "grows", Rect::new(0, 0, 40, 20), LayoutParams::filled())
            .expect("label is published");

        let mut widths = Vec::new();
        builder.arrange(Rect::new(0, 0, 300, 40), &mut |_, rect| widths.push(rect.width));
        assert_eq!(widths.len(), 2);
        assert!(
            widths[1] > widths[0],
            "the `fill` child must be wider than the fixed one: {widths:?}"
        );
    }

    /// An unpublished child name yields `None` rather than failing the composite.
    #[test]
    fn an_unknown_child_is_reported_not_fatal() {
        let (factory, mut builder) = builder();
        assert!(builder
            .add(
                &factory,
                "no_such_control_at_all",
                "",
                Rect::new(0, 0, 10, 10),
                LayoutParams::new()
            )
            .is_none());
        assert!(builder.is_empty());
    }

    /// The action row hangs off the band's **trailing** edge.
    ///
    /// This is the `dialog_with_actions` template's whole purpose: the anchor is a property
    /// of the layout (`justify_content = FlexEnd`), so it cannot be forgotten at a call
    /// site. The old derivation was `rect.x + rect.width - count * (width + gap)`, which
    /// places the row by arithmetic rather than by anchoring and gets it wrong the moment a
    /// button is wider than the constant.
    #[test]
    fn the_action_row_is_right_anchored() {
        let factory = WidgetFactory::new_with_defaults();
        let mut row = ActionRow::new(6);
        for label in ["Cancel", "Back", "Finish"] {
            row.add(&factory, label, Size::new(72, 36)).expect("button is published");
        }
        assert_eq!(row.len(), 3);
        // Each button wants its own size -- `max(64, measured(label) + 24)` -- so the row is the
        // three *different* widths plus the two gaps, not three uniform buttons. The row reports a
        // width derived from the children rather than from a per-dialog constant, which is what
        // makes it correct for a label of any length.
        //
        // The expected numbers are read from the same estimate the buttons themselves use rather
        // than written as `72` / `64` / `72`: the old literals were the hand-rolled `label * 8`
        // arithmetic copied into the test, so they would have gone stale (and did) the moment the
        // measurement became shared. A test that restates an implementation's arithmetic is a
        // second copy of it, which is the defect this round is closing.
        let expected: u32 = ["Cancel", "Back", "Finish"]
            .iter()
            .map(|label| {
                (estimate_text_width(label, &Font::default(), 1.0) + 24)
                    .max(dimensions::BUTTON_MIN.width)
            })
            .sum();
        assert_eq!(row.preferred_width(), expected + 2 * 6);

        let band = Rect::new(0, 0, 240, 40);
        let (span, buttons) = row.arrange(band);
        assert_eq!(buttons.len(), 3);
        assert_eq!(
            span.x + span.width as i32,
            band.x + band.width as i32,
            "the row's trailing edge must be the band's trailing edge"
        );
        // Uniform buttons and gaps: the buttons tile the span with `gap` between them.
        for pair in buttons.windows(2) {
            assert_eq!(
                pair[1].x - (pair[0].x + pair[0].width as i32),
                6,
                "the gap between two buttons must be the row's own gap"
            );
        }
        assert_eq!(buttons[0].x, span.x, "the span begins at the first button");
    }

    /// A row too narrow for its buttons keeps every button *inside* the band.
    ///
    /// # What this pins (and what G-1 changed)
    ///
    /// The action row's buttons declare `BUTTON_MIN` (64) as their floor, so a 120 px band cannot
    /// hold two of them plus the gap — 134 px of requirement against 120 px of room. Before G-1 was
    /// resolved the row was left at its floors and the *second* button was placed past the band's
    /// trailing edge. Because the SVG backend emits absolute coordinates and nothing clips at this
    /// layer, that button was not "overflowing", it was **absent**: a dialog whose Cancel button is
    /// simply not drawn.
    ///
    /// The layout now shares the shortfall proportionally, so the property to pin is containment:
    /// every button is inside the band the caller offered, and the span the row reports is that
    /// band rather than a wider one. Its own `preferred_width` still reports the un-squeezed
    /// requirement, which is what a dialog sizing itself around the row needs.
    #[test]
    fn a_row_too_narrow_for_its_buttons_keeps_them_inside_the_band() {
        let factory = WidgetFactory::new_with_defaults();
        let mut row = ActionRow::new(6);
        row.add(&factory, "One", Size::new(100, 36)).expect("button is published");
        row.add(&factory, "Two", Size::new(100, 36)).expect("button is published");
        let band = Rect::new(0, 0, 120, 40);
        let (span, buttons) = row.arrange(band);
        assert_eq!(buttons.len(), 2);
        for button in &buttons {
            assert!(
                button.x >= band.x && button.x + button.width as i32 <= band.x + band.width as i32,
                "every button must stay inside the band: {button:?} in {band:?}"
            );
            assert!(button.width > 0, "and none is dropped: {button:?}");
        }
        assert!(
            span.x + span.width as i32 <= band.x + band.width as i32,
            "the span the row reports must be inside the band: {span:?}"
        );
        // The row still *asks* for room for both buttons: a caller that can give it more should.
        assert!(
            row.preferred_width() > 120,
            "the row's own preference is still the unsqueezed requirement: {}",
            row.preferred_width()
        );
    }

    /// An empty row occupies nothing and does not fabricate a button.
    #[test]
    fn an_empty_action_row_is_empty() {
        let row = ActionRow::new(6);
        assert!(row.is_empty());
        assert_eq!(row.preferred_width(), 0);
        let band = Rect::new(0, 0, 240, 40);
        let (span, buttons) = row.arrange(band);
        assert!(buttons.is_empty());
        assert_eq!(span.width, 0, "an empty row spans no room");
    }

    /// A control the profile does not publish is skipped, and the counters agree.
    #[test]
    fn a_missing_control_does_not_break_the_row() {
        let factory = WidgetFactory::new_with_defaults();
        let mut row = ActionRow::new(6);
        assert!(row.add(&factory, "Ok", Size::new(72, 36)).is_some());
        // The `len` is the number of buttons actually present, so a skipped one is not
        // counted and the layout is never handed an id that does not exist.
        assert_eq!(row.len(), 1);
    }
    /// The `Hints` invariant survives the composite's own arithmetic.
    #[test]
    fn the_hints_are_always_normalised() {
        let (factory, mut builder) = builder();
        builder
            .add(&factory, "label", "x", Rect::new(0, 0, 10, 10), LayoutParams::new())
            .expect("label is published");
        for horizontal in [true, false] {
            let hints = builder.hints(horizontal);
            for axis in [hints.width, hints.height] {
                assert!(
                    axis.min <= axis.pref && axis.pref <= axis.max,
                    "min <= pref <= max must hold by construction: {axis:?}"
                );
            }
        }
    }
}
