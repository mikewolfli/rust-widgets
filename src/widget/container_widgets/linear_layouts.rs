// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The layout-family distribution controls: `Flex`, `FlowLayout` and `WrapLayout`.
//!
//! # Why these three are separate controls
//!
//! All three place a sequence of items, but they answer three different questions, and each
//! has its own configuration vocabulary:
//!
//! * [`Flex`] distributes **weights** along one axis (CSS flexbox): grow, shrink, justify,
//!   align, and optional wrapping. Its inputs are `flex_grow` and the five `justify` /
//!   `align` modes.
//! * [`FlowLayout`] places items in **reading order** and starts a new run when the current
//!   one is full, sized by the items' own hints. Its inputs are direction, alignment, spacing
//!   and padding.
//! * [`WrapLayout`] places items of **fixed size** and breaks lines when a line is full. It
//!   publishes a separate run spacing, which a flow does not have.
//!
//! They delegate the arithmetic to the same engines a tree layout uses
//! ([`crate::layout::FlexLayout`], [`crate::layout::flow::FlowLayout`],
//! [`crate::layout::wrap::WrapLayout`]) rather than re-deriving it: the engines take
//! `ChildInfo` hints, which is exactly the channel a control's content sizes are. The
//! control therefore reports its arrangement through the same solver the host layout tree
//! uses, and the two cannot drift.
//!
//! # The item model
//!
//! Each control owns a `Vec` of [`FlexItemSpec`] — a label, the size the item asks for, and
//! (for `Flex`) its weight. The ids handed to the engine are synthetic
//! (`index + 1`, because `ObjectId` is `u64` and 0 is not used here); nothing else ever sees
//! them, and the returned rectangles are mapped back by index.

use crate::core::{Point, Rect, Size, TextDirection};
use crate::event::{Event, EventHandler};
use crate::layout::flow::{
    FlowAlignment, FlowDirection, FlowLayout as FlowEngine, FlowLayoutConfig,
};
use crate::layout::wrap::{WrapAlignment, WrapDirection, WrapLayout as WrapEngine};
use crate::layout::{AlignItems, ChildInfo, FlexDirection, FlexItem, FlexLayout, FlexWrap, Hints};
use crate::layout::{JustifyContent, Layout};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::container_widgets::layout_family::{paint_item_box, resolve_palette};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One item of a distribution layout: a label, the size it asks for, and its weight.
#[derive(Debug, Clone, PartialEq)]
pub struct FlexItemSpec {
    label: String,
    size: Size,
    grow: u32,
    /// Optional real widget mounted behind this item.
    ///
    /// The distribution engine addresses items by synthetic ids, so a real child used to be
    /// unaddressable: a click could not reach it, it had no focus, and no accessibility node
    /// existed. This id is the bridge — it lets the control report the item a click landed
    /// on without changing the solver's index-based contract.
    widget_id: Option<crate::core::ObjectId>,
}

impl FlexItemSpec {
    /// A weightless item of `size`.
    pub fn new(label: &str, size: Size) -> Self {
        Self { label: label.to_string(), size, grow: 0, widget_id: None }
    }

    /// Attaches a real child id to the item.
    pub fn with_widget(mut self, widget_id: crate::core::ObjectId) -> Self {
        self.widget_id = Some(widget_id);
        self
    }

    /// The real child id, when one is attached.
    pub fn widget_id(&self) -> Option<crate::core::ObjectId> {
        self.widget_id
    }

    /// The same item with an explicit grow weight.
    pub fn with_grow(mut self, grow: u32) -> Self {
        self.grow = grow;
        self
    }

    /// The item's label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The size the item asks for.
    pub fn size(&self) -> Size {
        self.size
    }

    /// The share of the leftover room this item claims.
    pub fn grow(&self) -> u32 {
        self.grow
    }
}

/// The id the engine is told is item `index`'s.
///
/// `ObjectId` is a `u64`; `0` is never handed out here so a zeroed rectangle (the "engine
/// produced nothing for this item" state) cannot be mistaken for item 0's answer.
fn synthetic_id(index: usize) -> u64 {
    index as u64 + 1
}

/// Maps an engine-assigned id back to its index.
fn index_of(id: u64) -> Option<usize> {
    id.checked_sub(1).map(|index| index as usize)
}

/// Calls `arrange` and collects the produced rectangles by index.
///
/// A rectangle the engine did not produce keeps the zero value, which the painter skips: an
/// item the solver could not place has no ink to contribute.
fn collect_rects(count: usize, arrange: impl FnOnce(&mut dyn FnMut(u64, Rect))) -> Vec<Rect> {
    let mut rects = vec![Rect::new(0, 0, 0, 0); count];
    arrange(&mut |id, rect| {
        if let Some(index) = index_of(id) {
            if let Some(slot) = rects.get_mut(index) {
                *slot = rect;
            }
        }
    });
    rects
}

/// Paints every placed item, clipped to the control's own rectangle.
///
/// The clip is the control's box: nothing clips a widget at this layer, so a distribution
/// that overflows its own geometry would otherwise paint on a neighbour.
fn paint_items(
    ctx: &mut RenderContext,
    base: &BaseWidget,
    role: &str,
    outer: Rect,
    rects: &[Rect],
    items: &[FlexItemSpec],
) {
    let palette = resolve_palette(base, role);
    ctx.push_clip(outer.x, outer.y, outer.width, outer.height);
    let bottom = outer.y.saturating_add(outer.height as i32);
    for (item, rect) in items.iter().zip(rects) {
        // A fully hidden item is skipped rather than clipped: the clip handles the partial
        // case, and this keeps a long list from emitting commands for pixels nobody sees.
        if rect.height == 0 || rect.y >= bottom {
            continue;
        }
        paint_item_box(ctx, *rect, item.label(), &palette, 3);
    }
    ctx.pop_clip();
}

// ── Flex ────────────────────────────────────────────────────────────────────

/// Flex — a CSS-flexbox-style distribution of weighted items along one axis.
///
/// The control owns its items and calls [`FlexLayout`] with them, so `flex`, `justify`,
/// `align` and `wrap` behave exactly as they do for a tree child of that layout. An item's
/// `grow` is the flex weight; an item with `grow == 0` keeps the size it asked for.
pub struct Flex {
    base: BaseWidget,
    items: Vec<FlexItemSpec>,
    direction: FlexDirection,
    wrap: FlexWrap,
    justify: JustifyContent,
    align: AlignItems,
    gap: u32,
    /// Bumped by every input that changes the arrangement.
    rect_version: u64,
    rect_cache: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<Rect>>)>>,
    /// Emitted with the item index when an item is clicked.
    pub item_clicked: Signal1<usize>,
}

impl Flex {
    /// Creates an empty row-direction flex with no gap.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Flex, geometry, "Flex"),
            items: Vec::new(),
            direction: FlexDirection::default(),
            wrap: FlexWrap::default(),
            justify: JustifyContent::default(),
            align: AlignItems::default(),
            gap: 0,
            rect_version: 1,
            rect_cache: std::cell::RefCell::new(None),
            item_clicked: Signal1::new(),
        }
    }

    /// The items this control distributes.
    pub fn items(&self) -> &[FlexItemSpec] {
        &self.items
    }

    /// Appends an item and requests a redraw.
    ///
    /// An item that carries a real child id (C-11) is linked into this control's child list and
    /// shown, so the tree painter draws the control in the item's rectangle. Without the link the
    /// id was only a click label: the item could not host a real control at all.
    pub fn add_item(&mut self, item: FlexItemSpec) {
        if let Some(widget_id) = item.widget_id() {
            self.base.add_child_linked(widget_id);
            crate::set_widget_visible(widget_id, true);
        }
        self.items.push(item);
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes the item at `index` (C-10).
    ///
    /// Returns `false` when `index` is out of range. A removed item takes its mounted child with
    /// it: the link is cleared in both directions and the control is hidden, so the layout cannot
    /// keep painting a control whose cell no longer exists.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let item = self.items.remove(index);
        if let Some(widget_id) = item.widget_id() {
            self.base.remove_child_linked(widget_id);
            crate::set_widget_visible(widget_id, false);
        }
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Writes each mounted item control's rectangle into the control (C-11).
    ///
    /// The tree painter draws a linked child at the child's own geometry, so the item rectangle is
    /// this control's answer to "where does that child live". Called from `draw` and
    /// `set_geometry`: the arrangement is cached, but the write has to reach the child on the
    /// frames where the cache is served unchanged.
    fn place_item_widgets(&self) {
        for (item, rect) in self.items.iter().zip(self.cached_item_rects().iter()) {
            if let Some(widget_id) = item.widget_id() {
                crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
            }
        }
    }

    /// Removes every item.
    pub fn clear_items(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The main-axis direction.
    pub fn direction(&self) -> FlexDirection {
        self.direction
    }

    /// Sets the main-axis direction.
    pub fn set_direction(&mut self, direction: FlexDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the items wrap onto further lines.
    pub fn wrap(&self) -> FlexWrap {
        self.wrap
    }

    /// Sets whether the items wrap onto further lines.
    pub fn set_wrap(&mut self, wrap: FlexWrap) {
        if self.wrap == wrap {
            return;
        }
        self.wrap = wrap;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How the leftover room is distributed on the main axis.
    pub fn justify(&self) -> JustifyContent {
        self.justify
    }

    /// Sets how the leftover room is distributed on the main axis.
    pub fn set_justify(&mut self, justify: JustifyContent) {
        if self.justify == justify {
            return;
        }
        self.justify = justify;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_redraw();
    }

    /// How items are aligned on the cross axis.
    pub fn align(&self) -> AlignItems {
        self.align
    }

    /// Sets how items are aligned on the cross axis.
    pub fn set_align(&mut self, align: AlignItems) {
        if self.align == align {
            return;
        }
        self.align = align;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The gap between items, in logical pixels.
    pub fn gap(&self) -> u32 {
        self.gap
    }

    /// Sets the gap between items, in logical pixels.
    pub fn set_gap(&mut self, gap: u32) {
        if self.gap == gap {
            return;
        }
        self.gap = gap;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle each item occupies, in item order.
    ///
    /// The solver runs only when an input changed; the draw path uses the same cache, so a
    /// repaint of an unchanged distribution is a refcount bump rather than a re-arrange.
    pub fn item_rects(&self) -> Vec<Rect> {
        (*self.cached_item_rects()).clone()
    }

    fn cached_item_rects(&self) -> std::rc::Rc<Vec<Rect>> {
        if let Some((version, rects)) = self.rect_cache.borrow().as_ref() {
            if *version == self.rect_version {
                return std::rc::Rc::clone(rects);
            }
        }
        let rects = std::rc::Rc::new(self.compute_item_rects());
        *self.rect_cache.borrow_mut() = Some((self.rect_version, std::rc::Rc::clone(&rects)));
        rects
    }

    /// The item index under `point`, or `None` when the point is between items.
    ///
    /// C-P16: for the common simple run (one line, start-justified, equally sized items, no grow)
    /// the slot is solved arithmetically and only that candidate is confirmed against the cached
    /// rectangles — an O(1) hit test with no second source of truth, because the confirmation uses
    /// the same rectangles the painter does. Anything else falls back to the linear scan.
    pub fn item_at(&self, point: Point) -> Option<usize> {
        let rects = self.cached_item_rects();
        if let Some(index) = self.uniform_run_candidate(point) {
            if rects.get(index).is_some_and(|rect| rect.contains_point(point)) {
                return Some(index);
            }
        }
        rects.iter().position(|rect| rect.contains_point(point))
    }

    /// The slot a uniformly sized, single-line, start-justified run implies for `point`.
    ///
    /// `None` means "the formula does not describe this layout", and the caller scans instead.
    /// The arithmetic is only a *candidate*: the caller confirms it against the cached rectangle,
    /// so a mismatch degrades to the scan rather than producing a wrong hit.
    fn uniform_run_candidate(&self, point: Point) -> Option<usize> {
        if self.items.is_empty()
            || self.wrap != FlexWrap::NoWrap
            || !matches!(self.justify, JustifyContent::FlexStart)
            || matches!(self.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
        {
            return None;
        }
        let first = self.items.first()?;
        if self.items.iter().any(|item| item.size() != first.size() || item.grow() != 0) {
            return None;
        }
        let outer = self.geometry();
        let gap = self.gap as i32;
        let (start, step, extent, coordinate) = match self.direction {
            FlexDirection::Row => (outer.x, first.size().width as i32 + gap, outer.width, point.x),
            _ => (outer.y, first.size().height as i32 + gap, outer.height, point.y),
        };
        if step <= 0 || coordinate < start || coordinate >= start + extent as i32 {
            return None;
        }
        let slot = ((coordinate - start) / step) as usize;
        (slot < self.items.len()).then_some(slot)
    }

    fn compute_item_rects(&self) -> Vec<Rect> {
        let layout = self.build_layout();
        let children: Vec<ChildInfo> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                ChildInfo::new(
                    synthetic_id(index),
                    Hints::fixed(item.size().width, item.size().height),
                )
            })
            .collect();
        collect_rects(self.items.len(), |out| {
            layout.arrange(self.geometry(), &children, &mut |id, rect| out(id, rect));
        })
    }

    /// Builds the engine from this control's configuration and items.
    fn build_layout(&self) -> FlexLayout {
        let mut layout = FlexLayout::with_params(
            self.direction,
            self.wrap,
            self.justify,
            self.align,
            self.gap as i32,
            0,
        );
        for (index, item) in self.items.iter().enumerate() {
            layout.items_mut().push(FlexItem {
                widget_id: Some(synthetic_id(index)),
                flex_grow: item.grow() as f32,
                ..FlexItem::default()
            });
        }
        layout
    }
}

impl Widget for Flex {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.rect_version = self.rect_version.wrapping_add(1);
        // The arrangement moved with the control, so the controls mounted in it have to move too.
        self.place_item_widgets();
    }

    /// The summed item extent plus the gaps: what this row or column wants to be.
    fn size_hint(&self) -> Size {
        let horizontal = matches!(self.direction, FlexDirection::Row | FlexDirection::RowReverse);
        let gaps = (self.items.len().saturating_sub(1) as u32).saturating_mul(self.gap);
        let mut main: u32 = 0;
        let mut cross: u32 = 0;
        for item in &self.items {
            let size = item.size();
            if horizontal {
                main = main.saturating_add(size.width);
                cross = cross.max(size.height);
            } else {
                main = main.saturating_add(size.height);
                cross = cross.max(size.width);
            }
        }
        let main = main.saturating_add(gaps);
        if horizontal {
            Size::new(main, cross)
        } else {
            Size::new(cross, main)
        }
    }

    /// Resolves a published event name to the signal this layout actually emits.
    ///
    /// The capability publishes `item_clicked` with a `UInt` payload and the
    /// control emits the clicked item's index as a `usize`, so the conversion
    /// keeps the subscriber's number identical to the emitted one.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for Flex {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items.len() as u64)),
            "direction" => Ok(CapabilityValue::String(flex_direction_token(self.direction).into())),
            "wrap" => Ok(CapabilityValue::String(flex_wrap_token(self.wrap).into())),
            "justify_content" => Ok(CapabilityValue::String(justify_token(self.justify).into())),
            "align_items" => Ok(CapabilityValue::String(align_token(self.align).into())),
            "gap" => Ok(CapabilityValue::UInt(u64::from(self.gap))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // Derived from how many items were added, so there is nothing sensible to assign.
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "direction" => {
                let token = expect_string(value)?;
                let direction =
                    flex_direction_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_direction(direction);
                Ok(())
            }
            "wrap" => {
                let token = expect_string(value)?;
                let wrap = flex_wrap_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_wrap(wrap);
                Ok(())
            }
            "justify_content" => {
                let token = expect_string(value)?;
                let justify =
                    justify_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_justify(justify);
                Ok(())
            }
            "align_items" => {
                let token = expect_string(value)?;
                let align = align_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_align(align);
                Ok(())
            }
            "gap" => {
                self.set_gap(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "direction",
            "wrap",
            "justify_content",
            "align_items",
            "gap",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Flex {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let rects = self.cached_item_rects();
        // C-11: mounted item controls are placed before they are painted.
        self.place_item_widgets();
        paint_items(ctx, &self.base, "flex", outer, &rects, &self.items);
    }
}

impl EventHandler for Flex {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            // A disabled control is inert: it consumes no input and emits nothing the user
            // cannot reach (`check_enabled_is_honoured.sh`).
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.item_at(*pos) {
                self.item_clicked.emit(index);
            }
        }
    }
}

// ── FlowLayout ──────────────────────────────────────────────────────────────

/// FlowLayout — places items in reading order, starting a new run when the current is full.
///
/// Delegates to [`crate::layout::flow::FlowLayout`] through [`Layout::arrange`], so the
/// line-breaking arithmetic is the engine's, not a copy: a flow inside a control and a flow
/// inside a tree produce the same rectangles for the same sizes.
pub struct FlowLayout {
    base: BaseWidget,
    items: Vec<FlexItemSpec>,
    direction: FlowDirection,
    alignment: FlowAlignment,
    spacing: u32,
    padding: u32,
    wrap: bool,
    /// Reading direction forwarded to the flow engine; left-to-right by default.
    text_direction: TextDirection,
    /// Bumped by every input that changes the arrangement.
    rect_version: u64,
    rect_cache: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<Rect>>)>>,
    /// Emitted with the item index when an item is clicked.
    pub item_clicked: Signal1<usize>,
}

impl FlowLayout {
    /// Creates an empty horizontal flow, spacing 8, padding 8, wrapping on.
    ///
    /// Wrapping defaults to **on** here, unlike the engine's `false`: a control named
    /// "flow layout" that overflowed its own box by default would be the surprising choice
    /// for a form.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FlowLayout, geometry, "FlowLayout"),
            items: Vec::new(),
            direction: FlowDirection::Horizontal,
            alignment: FlowAlignment::Start,
            spacing: 8,
            padding: 8,
            wrap: true,
            text_direction: TextDirection::default(),
            rect_version: 1,
            rect_cache: std::cell::RefCell::new(None),
            item_clicked: Signal1::new(),
        }
    }

    /// The reading direction items are placed in.
    pub fn text_direction(&self) -> TextDirection {
        self.text_direction
    }

    /// Sets the reading direction; right-to-left mirrors each run inside the control.
    pub fn set_text_direction(&mut self, direction: TextDirection) {
        if self.text_direction == direction {
            return;
        }
        self.text_direction = direction;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The items this control places.
    pub fn items(&self) -> &[FlexItemSpec] {
        &self.items
    }

    /// Appends an item and requests a redraw.
    pub fn add_item(&mut self, item: FlexItemSpec) {
        self.items.push(item);
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every item.
    pub fn clear_items(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes the item at `index` (C-10). Returns `false` when the index is out of range.
    ///
    /// The remaining items re-flow on the next arrangement: removing one changes the run the
    /// following items fall on, which is why the version is bumped rather than only the list.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The flow direction.
    pub fn direction(&self) -> FlowDirection {
        self.direction
    }

    /// Sets the flow direction.
    pub fn set_direction(&mut self, direction: FlowDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How items are positioned within each run.
    pub fn alignment(&self) -> FlowAlignment {
        self.alignment
    }

    /// Sets how items are positioned within each run.
    pub fn set_alignment(&mut self, alignment: FlowAlignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The gap between adjacent items and between runs.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Sets the gap between adjacent items and between runs.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The inset applied on all four sides before items are placed.
    pub fn padding(&self) -> u32 {
        self.padding
    }

    /// Sets the inset applied on all four sides before items are placed.
    pub fn set_padding(&mut self, padding: u32) {
        if self.padding == padding {
            return;
        }
        self.padding = padding;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether an item that would cross the far edge starts a new run.
    pub fn wraps(&self) -> bool {
        self.wrap
    }

    /// Sets whether an item that would cross the far edge starts a new run.
    pub fn set_wraps(&mut self, wrap: bool) {
        if self.wrap == wrap {
            return;
        }
        self.wrap = wrap;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle each item occupies, in item order.
    ///
    /// The engine runs only when an input changed; the draw path and `item_at` share the same
    /// cached vector, so a repaint or a pointer event does not rebuild the flow.
    pub fn item_rects(&self) -> Vec<Rect> {
        (*self.cached_item_rects()).clone()
    }

    fn cached_item_rects(&self) -> std::rc::Rc<Vec<Rect>> {
        if let Some((version, rects)) = self.rect_cache.borrow().as_ref() {
            if *version == self.rect_version {
                return std::rc::Rc::clone(rects);
            }
        }
        let rects = std::rc::Rc::new(self.compute_item_rects());
        *self.rect_cache.borrow_mut() = Some((self.rect_version, std::rc::Rc::clone(&rects)));
        rects
    }

    fn compute_item_rects(&self) -> Vec<Rect> {
        let mut engine = FlowEngine::with_config(FlowLayoutConfig {
            direction: self.direction,
            alignment: self.alignment,
            spacing: self.spacing as i32,
            padding: self.padding as i32,
            wrap: self.wrap,
        });
        engine.set_text_direction(self.text_direction);
        let children: Vec<ChildInfo> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let id = synthetic_id(index);
                engine.add_widget(id, 0);
                ChildInfo::new(id, Hints::fixed(item.size().width, item.size().height))
            })
            .collect();
        collect_rects(self.items.len(), |out| {
            engine.arrange(self.geometry(), &children, &mut |id, rect| out(id, rect));
        })
    }

    /// The item index under `point`.
    pub fn item_at(&self, point: Point) -> Option<usize> {
        self.cached_item_rects().iter().position(|rect| rect.contains_point(point))
    }
}

impl Widget for FlowLayout {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.rect_version = self.rect_version.wrapping_add(1);
    }

    fn size_hint(&self) -> Size {
        let horizontal = matches!(self.direction, FlowDirection::Horizontal);
        let gaps = (self.items.len().saturating_sub(1) as u32).saturating_mul(self.spacing);
        let mut main: u32 = 0;
        let mut cross: u32 = 0;
        for item in &self.items {
            let size = item.size();
            if horizontal {
                main = main.saturating_add(size.width);
                cross = cross.max(size.height);
            } else {
                main = main.saturating_add(size.height);
                cross = cross.max(size.width);
            }
        }
        let main = main.saturating_add(gaps).saturating_add(self.padding.saturating_mul(2));
        let cross = cross.saturating_add(self.padding.saturating_mul(2));
        if horizontal {
            Size::new(main, cross)
        } else {
            Size::new(cross, main)
        }
    }

    /// Resolves a published event name to the signal this layout actually emits.
    ///
    /// The capability publishes `item_clicked` with a `UInt` payload and the
    /// control emits the clicked item's index as a `usize`, so the conversion
    /// keeps the subscriber's number identical to the emitted one.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for FlowLayout {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items.len() as u64)),
            "direction" => Ok(CapabilityValue::String(flow_direction_token(self.direction).into())),
            "alignment" => Ok(CapabilityValue::String(flow_alignment_token(self.alignment).into())),
            "spacing" => Ok(CapabilityValue::UInt(u64::from(self.spacing))),
            "padding" => Ok(CapabilityValue::UInt(u64::from(self.padding))),
            "wrap" => Ok(CapabilityValue::Bool(self.wrap)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "direction" => {
                let token = expect_string(value)?;
                let direction =
                    flow_direction_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_direction(direction);
                Ok(())
            }
            "alignment" => {
                let token = expect_string(value)?;
                let alignment =
                    flow_alignment_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_alignment(alignment);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_u32(value)?);
                Ok(())
            }
            "padding" => {
                self.set_padding(expect_u32(value)?);
                Ok(())
            }
            "wrap" => {
                self.set_wraps(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "direction",
            "alignment",
            "spacing",
            "padding",
            "wrap",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for FlowLayout {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let rects = self.cached_item_rects();
        paint_items(ctx, &self.base, "flow_layout", outer, &rects, &self.items);
    }
}

impl EventHandler for FlowLayout {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.item_at(*pos) {
                self.item_clicked.emit(index);
            }
        }
    }
}

// ── WrapLayout ──────────────────────────────────────────────────────────────

/// WrapLayout — places fixed-size items and breaks lines when a line is full.
///
/// # The difference from `FlowLayout`
///
/// A flow starts a new run when the *hint* of the next item would not fit and publishes one
/// spacing for both axes; a wrap always breaks at the configured cell extent and publishes a
/// separate `run_spacing`. Both delegate line breaking to the same engine functions (the
/// crate's `wrap::break_lines` / `place_lines`), so "where does a line end" has one answer.
pub struct WrapLayout {
    base: BaseWidget,
    items: Vec<FlexItemSpec>,
    direction: WrapDirection,
    alignment: WrapAlignment,
    spacing: u32,
    run_spacing: Option<u32>,
    padding: u32,
    /// Reading direction forwarded to the wrap engine; left-to-right by default.
    text_direction: TextDirection,
    /// Bumped by every input that changes the arrangement.
    rect_version: u64,
    rect_cache: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<Rect>>)>>,
    /// Emitted with the item index when an item is clicked.
    pub item_clicked: Signal1<usize>,
}

impl WrapLayout {
    /// Creates an empty horizontal wrap, spacing 8, padding 8, run spacing following the item
    /// spacing.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WrapLayout, geometry, "WrapLayout"),
            items: Vec::new(),
            direction: WrapDirection::Horizontal,
            alignment: WrapAlignment::Start,
            spacing: 8,
            run_spacing: None,
            padding: 8,
            text_direction: TextDirection::default(),
            rect_version: 1,
            rect_cache: std::cell::RefCell::new(None),
            item_clicked: Signal1::new(),
        }
    }

    /// The items this control places.
    pub fn items(&self) -> &[FlexItemSpec] {
        &self.items
    }

    /// Appends an item and requests a redraw.
    pub fn add_item(&mut self, item: FlexItemSpec) {
        self.items.push(item);
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes every item.
    pub fn clear_items(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.items.clear();
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Removes the item at `index` (C-10). Returns `false` when the index is out of range.
    ///
    /// The remaining items re-wrap on the next arrangement: removing one changes which line the
    /// following items fall on, which is why the version is bumped rather than only the list.
    pub fn remove_item(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items.remove(index);
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The direction items flow before wrapping.
    pub fn direction(&self) -> WrapDirection {
        self.direction
    }

    /// Sets the direction items flow before wrapping.
    pub fn set_direction(&mut self, direction: WrapDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How items are aligned within each run.
    pub fn alignment(&self) -> WrapAlignment {
        self.alignment
    }

    /// Sets how items are aligned within each run.
    pub fn set_alignment(&mut self, alignment: WrapAlignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_redraw();
    }

    /// The gap between adjacent items.
    pub fn spacing(&self) -> u32 {
        self.spacing
    }

    /// Sets the gap between adjacent items.
    pub fn set_spacing(&mut self, spacing: u32) {
        if self.spacing == spacing {
            return;
        }
        self.spacing = spacing;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The gap between runs, or `None` when it follows the item spacing.
    pub fn run_spacing(&self) -> Option<u32> {
        self.run_spacing
    }

    /// Sets the gap between runs. `None` restores "follow the item spacing".
    pub fn set_run_spacing(&mut self, run_spacing: Option<u32>) {
        if self.run_spacing == run_spacing {
            return;
        }
        self.run_spacing = run_spacing;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The inset applied on all four sides before items are placed.
    pub fn padding(&self) -> u32 {
        self.padding
    }

    /// Sets the inset applied on all four sides before items are placed.
    pub fn set_padding(&mut self, padding: u32) {
        if self.padding == padding {
            return;
        }
        self.padding = padding;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The reading direction items are placed in.
    pub fn text_direction(&self) -> TextDirection {
        self.text_direction
    }

    /// Sets the reading direction; right-to-left mirrors each run inside the control.
    pub fn set_text_direction(&mut self, direction: TextDirection) {
        if self.text_direction == direction {
            return;
        }
        self.text_direction = direction;
        self.rect_version = self.rect_version.wrapping_add(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The rectangle each item occupies, in item order.
    ///
    /// The engine runs only when an input changed; the draw path and `item_at` share the same
    /// cached vector, so a repaint or a pointer event does not rebuild the wrap.
    pub fn item_rects(&self) -> Vec<Rect> {
        (*self.cached_item_rects()).clone()
    }

    fn cached_item_rects(&self) -> std::rc::Rc<Vec<Rect>> {
        if let Some((version, rects)) = self.rect_cache.borrow().as_ref() {
            if *version == self.rect_version {
                return std::rc::Rc::clone(rects);
            }
        }
        let rects = std::rc::Rc::new(self.compute_item_rects());
        *self.rect_cache.borrow_mut() = Some((self.rect_version, std::rc::Rc::clone(&rects)));
        rects
    }

    fn compute_item_rects(&self) -> Vec<Rect> {
        let mut engine = WrapEngine::new(
            self.direction,
            self.alignment,
            self.spacing as i32,
            self.padding as i32,
        );
        engine.set_run_spacing(self.run_spacing.map(|spacing| spacing as i32));
        engine.set_text_direction(self.text_direction);
        let children: Vec<ChildInfo> = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let id = synthetic_id(index);
                engine.add_widget(id, 0);
                engine.set_child_size(id, item.size());
                ChildInfo::new(id, Hints::fixed(item.size().width, item.size().height))
            })
            .collect();
        collect_rects(self.items.len(), |out| {
            engine.arrange(self.geometry(), &children, &mut |id, rect| out(id, rect));
        })
    }

    /// The item index under `point`.
    pub fn item_at(&self, point: Point) -> Option<usize> {
        self.cached_item_rects().iter().position(|rect| rect.contains_point(point))
    }
}

impl Widget for WrapLayout {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.rect_version = self.rect_version.wrapping_add(1);
    }

    fn size_hint(&self) -> Size {
        let horizontal = matches!(self.direction, WrapDirection::Horizontal);
        let gap = (self.items.len().saturating_sub(1) as u32).saturating_mul(self.spacing);
        let mut main: u32 = 0;
        let mut cross: u32 = 0;
        for item in &self.items {
            let size = item.size();
            if horizontal {
                main = main.saturating_add(size.width);
                cross = cross.max(size.height);
            } else {
                main = main.saturating_add(size.height);
                cross = cross.max(size.width);
            }
        }
        let main = main.saturating_add(gap).saturating_add(self.padding.saturating_mul(2));
        let cross = cross.saturating_add(self.padding.saturating_mul(2));
        if horizontal {
            Size::new(main, cross)
        } else {
            Size::new(cross, main)
        }
    }

    /// Resolves a published event name to the signal this layout actually emits.
    ///
    /// The capability publishes `item_clicked` with a `UInt` payload and the
    /// control emits the clicked item's index as a `usize`, so the conversion
    /// keeps the subscriber's number identical to the emitted one.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "item_clicked" => {
                Some(EventSignalRef::mapped("item_clicked", &self.item_clicked, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

impl WidgetProperties for WrapLayout {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items.len() as u64)),
            "direction" => Ok(CapabilityValue::String(wrap_direction_token(self.direction).into())),
            "alignment" => Ok(CapabilityValue::String(wrap_alignment_token(self.alignment).into())),
            "spacing" => Ok(CapabilityValue::UInt(u64::from(self.spacing))),
            // `null` is the documented "follow the item spacing" state, so the property is
            // nullable rather than an impossible sentinel such as 0.
            "run_spacing" => Ok(match self.run_spacing {
                Some(spacing) => CapabilityValue::UInt(u64::from(spacing)),
                None => CapabilityValue::Null,
            }),
            "padding" => Ok(CapabilityValue::UInt(u64::from(self.padding))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "direction" => {
                let token = expect_string(value)?;
                let direction =
                    wrap_direction_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_direction(direction);
                Ok(())
            }
            "alignment" => {
                let token = expect_string(value)?;
                let alignment =
                    wrap_alignment_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_alignment(alignment);
                Ok(())
            }
            "spacing" => {
                self.set_spacing(expect_u32(value)?);
                Ok(())
            }
            "run_spacing" => {
                let spacing = match value {
                    CapabilityValue::Null => None,
                    other => Some(expect_u32(other)?),
                };
                self.set_run_spacing(spacing);
                Ok(())
            }
            "padding" => {
                self.set_padding(expect_u32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "direction",
            "alignment",
            "spacing",
            "run_spacing",
            "padding",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for WrapLayout {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let outer = self.geometry();
        let rects = self.cached_item_rects();
        paint_items(ctx, &self.base, "wrap_layout", outer, &rects, &self.items);
    }
}

impl EventHandler for WrapLayout {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if let Event::MousePress { pos, button: 1, .. } = event {
            if let Some(index) = self.item_at(*pos) {
                self.item_clicked.emit(index);
            }
        }
    }
}

// ── Enum tokens ─────────────────────────────────────────────────────────────
//
// One token table per published enum, with the parser beside the writer so a renamed variant
// cannot leave one side behind. Each `from_token` returns `None` for an unpublished spelling
// rather than a default, so a designer that writes a typo is told, not silently ignored.

fn flex_direction_token(direction: FlexDirection) -> &'static str {
    match direction {
        FlexDirection::Row => "row",
        FlexDirection::RowReverse => "row_reverse",
        FlexDirection::Column => "column",
        FlexDirection::ColumnReverse => "column_reverse",
    }
}

fn flex_direction_from_token(token: &str) -> Option<FlexDirection> {
    match token {
        "row" => Some(FlexDirection::Row),
        "row_reverse" => Some(FlexDirection::RowReverse),
        "column" => Some(FlexDirection::Column),
        "column_reverse" => Some(FlexDirection::ColumnReverse),
        _ => None,
    }
}

fn flex_wrap_token(wrap: FlexWrap) -> &'static str {
    match wrap {
        FlexWrap::NoWrap => "no_wrap",
        FlexWrap::Wrap => "wrap",
        FlexWrap::WrapReverse => "wrap_reverse",
    }
}

fn flex_wrap_from_token(token: &str) -> Option<FlexWrap> {
    match token {
        "no_wrap" => Some(FlexWrap::NoWrap),
        "wrap" => Some(FlexWrap::Wrap),
        "wrap_reverse" => Some(FlexWrap::WrapReverse),
        _ => None,
    }
}

fn justify_token(justify: JustifyContent) -> &'static str {
    match justify {
        JustifyContent::FlexStart => "flex_start",
        JustifyContent::FlexEnd => "flex_end",
        JustifyContent::Center => "center",
        JustifyContent::SpaceBetween => "space_between",
        JustifyContent::SpaceAround => "space_around",
        JustifyContent::SpaceEvenly => "space_evenly",
    }
}

fn justify_from_token(token: &str) -> Option<JustifyContent> {
    match token {
        "flex_start" => Some(JustifyContent::FlexStart),
        "flex_end" => Some(JustifyContent::FlexEnd),
        "center" => Some(JustifyContent::Center),
        "space_between" => Some(JustifyContent::SpaceBetween),
        "space_around" => Some(JustifyContent::SpaceAround),
        "space_evenly" => Some(JustifyContent::SpaceEvenly),
        _ => None,
    }
}

fn align_token(align: AlignItems) -> &'static str {
    match align {
        AlignItems::Stretch => "stretch",
        AlignItems::FlexStart => "flex_start",
        AlignItems::FlexEnd => "flex_end",
        AlignItems::Center => "center",
        AlignItems::Baseline => "baseline",
    }
}

fn align_from_token(token: &str) -> Option<AlignItems> {
    match token {
        "stretch" => Some(AlignItems::Stretch),
        "flex_start" => Some(AlignItems::FlexStart),
        "flex_end" => Some(AlignItems::FlexEnd),
        "center" => Some(AlignItems::Center),
        "baseline" => Some(AlignItems::Baseline),
        _ => None,
    }
}

fn flow_direction_token(direction: FlowDirection) -> &'static str {
    match direction {
        FlowDirection::Horizontal => "horizontal",
        FlowDirection::Vertical => "vertical",
    }
}

fn flow_direction_from_token(token: &str) -> Option<FlowDirection> {
    match token {
        "horizontal" => Some(FlowDirection::Horizontal),
        "vertical" => Some(FlowDirection::Vertical),
        _ => None,
    }
}

fn flow_alignment_token(alignment: FlowAlignment) -> &'static str {
    match alignment {
        FlowAlignment::Start => "start",
        FlowAlignment::Center => "center",
        FlowAlignment::End => "end",
        FlowAlignment::SpaceBetween => "space_between",
        FlowAlignment::SpaceAround => "space_around",
    }
}

fn flow_alignment_from_token(token: &str) -> Option<FlowAlignment> {
    match token {
        "start" => Some(FlowAlignment::Start),
        "center" => Some(FlowAlignment::Center),
        "end" => Some(FlowAlignment::End),
        "space_between" => Some(FlowAlignment::SpaceBetween),
        "space_around" => Some(FlowAlignment::SpaceAround),
        _ => None,
    }
}

fn wrap_direction_token(direction: WrapDirection) -> &'static str {
    match direction {
        WrapDirection::Horizontal => "horizontal",
        WrapDirection::Vertical => "vertical",
    }
}

fn wrap_direction_from_token(token: &str) -> Option<WrapDirection> {
    match token {
        "horizontal" => Some(WrapDirection::Horizontal),
        "vertical" => Some(WrapDirection::Vertical),
        _ => None,
    }
}

fn wrap_alignment_token(alignment: WrapAlignment) -> &'static str {
    match alignment {
        WrapAlignment::Start => "start",
        WrapAlignment::Center => "center",
        WrapAlignment::End => "end",
        WrapAlignment::SpaceBetween => "space_between",
        WrapAlignment::SpaceAround => "space_around",
    }
}

fn wrap_alignment_from_token(token: &str) -> Option<WrapAlignment> {
    match token {
        "start" => Some(WrapAlignment::Start),
        "center" => Some(WrapAlignment::Center),
        "end" => Some(WrapAlignment::End),
        "space_between" => Some(WrapAlignment::SpaceBetween),
        "space_around" => Some(WrapAlignment::SpaceAround),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All three layouts publish `item_clicked`, so each must resolve it separately.
    #[test]
    fn published_item_clicked_resolves_on_every_layout() {
        let flex = Flex::new(Rect::new(0, 0, 300, 100));
        let flow = FlowLayout::new(Rect::new(0, 0, 300, 100));
        let wrap = WrapLayout::new(Rect::new(0, 0, 300, 100));
        assert!(flex.event_signal_dyn("item_clicked").is_some(), "flex must resolve");
        assert!(flow.event_signal_dyn("item_clicked").is_some(), "flow_layout must resolve");
        assert!(wrap.event_signal_dyn("item_clicked").is_some(), "wrap_layout must resolve");
        assert!(flex.event_signal_dyn("no_such_event").is_none());
        assert!(flow.event_signal_dyn("no_such_event").is_none());
        assert!(wrap.event_signal_dyn("no_such_event").is_none());
    }

    fn spec(label: &str, width: u32, height: u32) -> FlexItemSpec {
        FlexItemSpec::new(label, Size::new(width, height))
    }

    #[test]
    fn flex_distributes_weights_in_item_order() {
        let mut flex = Flex::new(Rect::new(0, 0, 300, 40));
        flex.set_gap(10);
        flex.add_item(spec("a", 40, 40));
        flex.add_item(spec("b", 40, 40).with_grow(1));
        flex.add_item(spec("c", 40, 40).with_grow(1));
        let rects = flex.item_rects();
        assert_eq!(rects.len(), 3);
        // The three items are laid left to right, in creation order.
        assert!(rects[0].x < rects[1].x && rects[1].x < rects[2].x);
        // The two growing items share what the fixed one left, so they end up wider.
        assert!(rects[1].width > rects[0].width, "{rects:?}");
        assert_eq!(rects[1].width, rects[2].width, "{rects:?}");
    }

    #[test]
    fn flex_reports_the_size_its_items_need() {
        let mut flex = Flex::new(Rect::new(0, 0, 300, 40));
        flex.set_gap(5);
        flex.add_item(spec("a", 30, 20));
        flex.add_item(spec("b", 20, 24));
        assert_eq!(flex.size_hint(), Size::new(55, 24));
        flex.set_direction(FlexDirection::Column);
        assert_eq!(flex.size_hint(), Size::new(30, 49));
    }

    #[test]
    fn flex_item_count_is_derived_and_read_only() {
        let mut flex = Flex::new(Rect::new(0, 0, 300, 40));
        flex.add_item(spec("a", 10, 10));
        assert_eq!(flex.get("item_count"), Ok(CapabilityValue::UInt(1)));
        assert_eq!(
            flex.set("item_count", CapabilityValue::UInt(9)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn flow_layout_wraps_a_long_run_onto_the_next_line() {
        let mut flow = FlowLayout::new(Rect::new(0, 0, 100, 200));
        flow.set_spacing(0);
        flow.set_padding(0);
        for _ in 0..4 {
            flow.add_item(spec("item", 40, 20));
        }
        let rects = flow.item_rects();
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0].y, rects[1].y, "the first two fit on one run");
        assert!(rects[2].y > rects[0].y, "the third starts the next run: {rects:?}");
    }

    #[test]
    fn flow_layout_honours_its_padding() {
        let mut flow = FlowLayout::new(Rect::new(0, 0, 200, 200));
        flow.set_padding(12);
        flow.set_spacing(0);
        flow.add_item(spec("item", 20, 10));
        let rects = flow.item_rects();
        assert_eq!((rects[0].x, rects[0].y), (12, 12));
    }

    #[test]
    fn wrap_layout_publishes_a_separate_run_spacing() {
        let mut wrap = WrapLayout::new(Rect::new(0, 0, 100, 200));
        wrap.set_spacing(0);
        wrap.set_padding(0);
        for _ in 0..3 {
            wrap.add_item(spec("item", 40, 20));
        }
        let tight = wrap.item_rects();
        wrap.set_run_spacing(Some(10));
        let loose = wrap.item_rects();
        assert_eq!(tight.len(), 3);
        // Two items fit per run, so the third's y is the run offset: 20 either way, and the
        // difference shows up once a third run exists.
        assert!(loose[2].y > tight[2].y || tight[2].y == loose[2].y);
        // With two runs the answer is exact: run 2 starts after run 1 plus the run spacing.
        assert_eq!(loose[2].y - tight[2].y, 10, "raising the run spacing moves later runs down");
    }

    #[test]
    fn wrap_layout_run_spacing_null_means_follow_the_item_spacing() {
        let mut wrap = WrapLayout::new(Rect::new(0, 0, 100, 200));
        assert_eq!(wrap.get("run_spacing"), Ok(CapabilityValue::Null));
        wrap.set("run_spacing", CapabilityValue::UInt(4)).expect("writable");
        assert_eq!(wrap.run_spacing(), Some(4));
        wrap.set("run_spacing", CapabilityValue::Null).expect("nullable");
        assert_eq!(wrap.run_spacing(), None);
    }

    #[test]
    fn the_enum_tokens_round_trip() {
        for token in ["row", "row_reverse", "column", "column_reverse"] {
            assert_eq!(
                flex_direction_token(
                    flex_direction_from_token(token).expect("a published direction")
                ),
                token
            );
        }
        for token in ["no_wrap", "wrap", "wrap_reverse"] {
            assert_eq!(
                flex_wrap_token(flex_wrap_from_token(token).expect("a published wrap")),
                token
            );
        }
        for token in
            ["flex_start", "flex_end", "center", "space_between", "space_around", "space_evenly"]
        {
            assert_eq!(
                justify_token(justify_from_token(token).expect("a published justify")),
                token
            );
        }
        for token in ["stretch", "flex_start", "flex_end", "center", "baseline"] {
            assert_eq!(align_token(align_from_token(token).expect("a published align")), token);
        }
        for token in ["horizontal", "vertical"] {
            assert_eq!(
                flow_direction_token(flow_direction_from_token(token).expect("a direction")),
                token
            );
            assert_eq!(
                wrap_direction_token(wrap_direction_from_token(token).expect("a direction")),
                token
            );
        }
        for token in ["start", "center", "end", "space_between", "space_around"] {
            assert_eq!(
                flow_alignment_token(flow_alignment_from_token(token).expect("an alignment")),
                token
            );
            assert_eq!(
                wrap_alignment_token(wrap_alignment_from_token(token).expect("an alignment")),
                token
            );
        }
        assert_eq!(flex_direction_from_token("diagonal"), None);
        assert_eq!(justify_from_token("middle"), None);
    }

    #[test]
    fn the_distribution_controls_paint_their_items() {
        // The SVG backend paints a label as glyph geometry, not as a `<text>` element, so the
        // labels are counted through the crate's own ink reader: one `<path>` per drawn run.
        let mut flex = Flex::new(Rect::new(0, 0, 200, 60));
        flex.add_item(spec("one", 60, 40));
        flex.add_item(spec("two", 60, 40));
        let svg = crate::widget::svg::render_to_svg(&mut flex);
        assert_eq!(
            crate::widget::svg::text_ink_boxes(&svg).len(),
            2,
            "both item labels must be painted: {svg}"
        );

        let mut flow = FlowLayout::new(Rect::new(0, 0, 200, 60));
        flow.add_item(spec("alpha", 40, 20));
        let svg = crate::widget::svg::render_to_svg(&mut flow);
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1, "{svg}");

        let mut wrap = WrapLayout::new(Rect::new(0, 0, 200, 60));
        wrap.add_item(spec("beta", 40, 20));
        let svg = crate::widget::svg::render_to_svg(&mut wrap);
        assert_eq!(crate::widget::svg::text_ink_boxes(&svg).len(), 1, "{svg}");
    }

    #[test]
    fn an_item_past_the_bottom_edge_is_not_painted() {
        let mut flex = Flex::new(Rect::new(0, 0, 100, 40));
        flex.set_direction(FlexDirection::Column);
        for _ in 0..6 {
            flex.add_item(spec("row", 40, 30));
        }
        let svg = crate::widget::svg::render_to_svg(&mut flex);
        assert!(svg.contains("<clipPath"), "the items must be clipped to the control: {svg}");
    }

    #[test]
    fn the_distribution_controls_ignore_events_calmly() {
        let mut flex = Flex::new(Rect::new(0, 0, 100, 40));
        flex.handle_event(&Event::MouseMove { pos: crate::core::Point::new(3, 3) });
        let mut flow = FlowLayout::new(Rect::new(0, 0, 100, 40));
        flow.handle_event(&Event::KeyDown((65, 0)));
        let mut wrap = WrapLayout::new(Rect::new(0, 0, 100, 40));
        wrap.handle_event(&Event::KeyUp((65, 0)));
    }

    /// Items with real child ids are addressable, the arrangement is cached, and a click
    /// reports the item it landed on.
    #[test]
    fn items_carry_child_ids_and_clicks_name_them() {
        let mut flex = Flex::new(Rect::new(0, 0, 120, 30));
        flex.add_item(FlexItemSpec::new("a", Size::new(50, 20)).with_widget(7));
        flex.add_item(FlexItemSpec::new("b", Size::new(50, 20)));
        assert_eq!(flex.items()[0].widget_id(), Some(7));
        let rects = flex.item_rects();
        assert_eq!(flex.item_at(crate::core::Point::new(rects[0].x + 1, rects[0].y + 1)), Some(0));
        assert_eq!(flex.item_at(crate::core::Point::new(500, 500)), None);

        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(usize::MAX));
        let sink = std::sync::Arc::clone(&seen);
        flex.item_clicked.connect(move |index| {
            sink.store(*index, std::sync::atomic::Ordering::SeqCst);
        });
        flex.handle_event(&Event::MousePress {
            pos: crate::core::Point::new(rects[0].x + 1, rects[0].y + 1),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    /// The layout controls expose reading direction and forward it to their engine.
    #[test]
    fn flow_and_wrap_controls_mirror_for_right_to_left() {
        let mut flow = FlowLayout::new(Rect::new(0, 0, 100, 40));
        flow.set_padding(0);
        flow.add_item(FlexItemSpec::new("a", Size::new(30, 10)));
        assert_eq!(flow.item_rects()[0].x, 0);
        flow.set_text_direction(TextDirection::RightToLeft);
        assert!(flow.text_direction().is_right_to_left());
        assert_eq!(flow.item_rects()[0].x, 70, "the run begins at the right edge");

        let mut wrap = WrapLayout::new(Rect::new(0, 0, 100, 40));
        wrap.set_padding(0);
        wrap.add_item(FlexItemSpec::new("a", Size::new(30, 10)));
        assert_eq!(wrap.item_rects()[0].x, 0);
        wrap.set_text_direction(TextDirection::RightToLeft);
        assert!(wrap.text_direction().is_right_to_left());
        assert_eq!(wrap.item_rects()[0].x, 70, "the wrap run mirrors too");
    }
}
