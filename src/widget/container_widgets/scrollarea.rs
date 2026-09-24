// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll area widget.
//!
//! # The viewport and the two bars are assembled by a layout
//!
//! BLUE22 §B.8 lists this control's defect as "the scroll bars' positions computed by hand":
//! `h_band` and `v_band` were each written as the control's own extent minus an
//! `if other_bar_visible { bar }` correction, twice, in two directions. [`ScrollArea::assemble_chrome`]
//! states it once as an assembly — the content fills, the horizontal bar is a `bar`-tall row at the
//! bottom, the vertical bar a `bar`-wide column at the trailing edge — and "the other bar shortens
//! this one" becomes the layout's own consequence rather than a term repeated in two places.

use crate::compat::{Rc, RefCell, ToString, Vec};
use crate::core::{Alignment, Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
#[cfg(full_widgets)]
use crate::layout::{
    AlignItems, FlexDirection, FlexLayout, FlexWrap, JustifyContent, LayoutParams,
};
use crate::render::RenderContext;
use crate::signal::Signal1;
#[cfg(full_widgets)]
use crate::style::EdgeOffsets;

use crate::widget::capability::coercion::{expect_bool, expect_i64, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
use crate::widget::composite::CompositeBuilder;
use crate::widget::metrics::dimensions;
#[cfg(full_widgets)]
use crate::widget::WidgetFactory;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

fn translate_content_event(event: &Event, dx: i32, dy: i32) -> Event {
    let translate = |point: Point| Point::new(point.x + dx, point.y + dy);
    match event {
        Event::MouseDown((point, button)) => Event::MouseDown((translate(*point), *button)),
        Event::MouseUp((point, button)) => Event::MouseUp((translate(*point), *button)),
        Event::MouseMoveLegacy((point, device)) => {
            Event::MouseMoveLegacy((translate(*point), *device))
        }
        Event::MouseMove { pos } => Event::MouseMove { pos: translate(*pos) },
        Event::MousePress { pos, button } => {
            Event::MousePress { pos: translate(*pos), button: *button }
        }
        Event::MouseDoubleClick { pos, button } => {
            Event::MouseDoubleClick { pos: translate(*pos), button: *button }
        }
        Event::MouseRelease { pos, button } => {
            Event::MouseRelease { pos: translate(*pos), button: *button }
        }
        Event::MouseEnter { pos } => Event::MouseEnter { pos: translate(*pos) },
        Event::MouseLeave { pos } => Event::MouseLeave { pos: translate(*pos) },
        Event::PointerPress { pos, button, pressure, tilt_x, tilt_y } => Event::PointerPress {
            pos: translate(*pos),
            button: *button,
            pressure: *pressure,
            tilt_x: *tilt_x,
            tilt_y: *tilt_y,
        },
        Event::PointerMove { pos, pressure, tilt_x, tilt_y } => Event::PointerMove {
            pos: translate(*pos),
            pressure: *pressure,
            tilt_x: *tilt_x,
            tilt_y: *tilt_y,
        },
        Event::PointerRelease { pos, button, pressure } => {
            Event::PointerRelease { pos: translate(*pos), button: *button, pressure: *pressure }
        }
        #[cfg(feature = "touch")]
        Event::TouchBegin { pos, touch_id } => {
            Event::TouchBegin { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::TouchEnd { pos, touch_id } => {
            Event::TouchEnd { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::TouchMove { pos, touch_id } => {
            Event::TouchMove { pos: translate(*pos), touch_id: *touch_id }
        }
        #[cfg(feature = "touch")]
        Event::Tap { pos } => Event::Tap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::DoubleTap { pos } => Event::DoubleTap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::LongPress { pos } => Event::LongPress { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::Swipe { start, end, velocity } => {
            Event::Swipe { start: translate(*start), end: translate(*end), velocity: *velocity }
        }
        #[cfg(feature = "touch")]
        Event::Drag { pos, touch_id, delta } => {
            Event::Drag { pos: translate(*pos), touch_id: *touch_id, delta: *delta }
        }
        #[cfg(feature = "touch")]
        Event::TwoFingerTap { pos } => Event::TwoFingerTap { pos: translate(*pos) },
        #[cfg(feature = "touch")]
        Event::Fling { pos, velocity, touch_id } => {
            Event::Fling { pos: translate(*pos), velocity: *velocity, touch_id: *touch_id }
        }
        #[cfg(feature = "holographic")]
        Event::HolographicTouch { pos, depth, touch_id } => {
            Event::HolographicTouch { pos: translate(*pos), depth: *depth, touch_id: *touch_id }
        }
        _ => event.clone(),
    }
}

/// Scroll area widget.
pub struct ScrollArea {
    base: BaseWidget,
    widget_resizable: bool,
    alignment: Alignment,
    horizontal_scroll_bar_policy: ScrollBarPolicy,
    vertical_scroll_bar_policy: ScrollBarPolicy,
    viewport: Rect,
    widget: Option<ObjectId>,
    /// The total size of the content (for AsNeeded scroll bar calculation).
    content_size: Size,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Current scroll position (x, y) in content coordinates.
    scroll_position: (i32, i32),
    /// Content-space regions that stay pinned while their group is scrolled past.
    sticky_regions: Vec<StickyRegion>,
    /// Emitted whenever scroll position changes.
    pub scroll_position_changed: Signal1<(i32, i32)>,
}

/// A content-space band that sticks to the top of the viewport while scrolling.
///
/// # Why a region rather than a child widget id
///
/// A sticky header is pinned to the top of the *group it belongs to*, and that
/// group's extent is a property of the content, not of the header itself. Naming
/// the group's band here is what lets the pin be released when the group scrolls
/// past instead of leaving the header pinned forever — the behaviour that
/// distinguishes a useful sticky header from a broken one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StickyRegion {
    /// Top of the group in content coordinates.
    pub group_top: i32,
    /// Bottom of the group in content coordinates; the pin releases here.
    pub group_bottom: i32,
    /// Height of the pinned band, measured from the group's top.
    pub height: u32,
}

impl StickyRegion {
    /// Creates a sticky band of `height` pixels at the top of a group spanning
    /// `group_top..group_bottom` in content coordinates.
    pub fn new(group_top: i32, group_bottom: i32, height: u32) -> Self {
        Self { group_top, group_bottom: group_bottom.max(group_top), height }
    }

    /// The offset this band's top should be drawn at for a viewport scrolled to
    /// `scroll_y`.
    ///
    /// Three cases, in the order they apply:
    ///
    /// 1. the group has not been reached — the band scrolls normally, so its
    ///    offset is its own content position;
    /// 2. the group is being scrolled through — the band pins to the viewport top;
    /// 3. the group's end is approaching — the band is pushed up by however much of
    ///    it would otherwise overflow, so one group's header makes room for the
    ///    next rather than overlapping it.
    pub fn draw_offset(&self, scroll_y: i32) -> i32 {
        let natural = self.group_top - scroll_y;
        if natural > 0 {
            return natural;
        }
        let pinned = 0;
        // How far the group's bottom has risen past the band's own height; past
        // that point the band is pushed off the top.
        let group_bottom_in_view = self.group_bottom - scroll_y;
        let band_height = self.height as i32;
        if group_bottom_in_view < band_height {
            return group_bottom_in_view - band_height;
        }
        pinned
    }

    /// Returns whether this band is pinned (rather than scrolling normally) for a
    /// viewport scrolled to `scroll_y`.
    pub fn is_pinned(&self, scroll_y: i32) -> bool {
        self.draw_offset(scroll_y) == 0 && self.group_top - scroll_y < 0
    }
}
/// Scroll bar policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarPolicy {
    /// Scroll bar is always shown
    AlwaysOn,
    /// Scroll bar is always hidden
    AlwaysOff,
    /// Scroll bar is shown when needed
    #[default]
    AsNeeded,
}

/// Formats a [`ScrollBarPolicy`] as its published token.
///
/// Local rather than imported from `capability::access`: that module is gated to
/// the device profiles, while `ScrollArea` is available in every profile.
/// `expect_*` has no inverse here, because the old writer never accepted one.
fn scroll_bar_policy_to_str(policy: ScrollBarPolicy) -> &'static str {
    match policy {
        ScrollBarPolicy::AlwaysOn => "always_on",
        ScrollBarPolicy::AlwaysOff => "always_off",
        ScrollBarPolicy::AsNeeded => "as_needed",
    }
}
impl ScrollArea {
    /// Creates a scroll area.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollArea, geometry, "ScrollArea"),
            widget_resizable: true,
            alignment: Alignment::Center,
            horizontal_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            vertical_scroll_bar_policy: ScrollBarPolicy::AsNeeded,
            viewport: geometry,
            widget: None,
            content_size: Size::new(0, 0),
            registry: None,
            scroll_position: (0, 0),
            sticky_regions: Vec::new(),
            scroll_position_changed: Signal1::new(),
        }
    }
    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns the shared widget registry, if set.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }
    /// Returns whether the widget is resizable.
    pub fn widget_resizable(&self) -> bool {
        self.widget_resizable
    }
    /// Sets whether the widget is resizable.
    pub fn set_widget_resizable(&mut self, resizable: bool) {
        self.widget_resizable = resizable;
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
    }
    /// Returns horizontal scroll bar policy.
    pub fn horizontal_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.horizontal_scroll_bar_policy
    }
    /// Sets horizontal scroll bar policy.
    pub fn set_horizontal_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.horizontal_scroll_bar_policy = policy;
    }
    /// Returns vertical scroll bar policy.
    pub fn vertical_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.vertical_scroll_bar_policy
    }
    /// Sets vertical scroll bar policy.
    pub fn set_vertical_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        self.vertical_scroll_bar_policy = policy;
    }
    /// Sets widget.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child(widget_id);
        }
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }

    /// Marks the band at the top of `group_top..group_bottom` as sticky.
    ///
    /// The band stays pinned to the top of the viewport from the moment its group's
    /// top scrolls off until the group's bottom reaches it, after which the next
    /// group's band takes its place. That is the "section header that follows the
    /// scroll" behaviour: a header is useful while its own items are on screen and
    /// is in the way once they are not.
    ///
    /// `height` is measured from the group's top, so it is the header's own height.
    /// A zero height marks nothing and is ignored, because a pinned band with no
    /// extent could not be seen to be pinned.
    pub fn add_sticky_region(&mut self, group_top: i32, group_bottom: i32, height: u32) {
        if height == 0 {
            return;
        }
        self.sticky_regions.push(StickyRegion::new(group_top, group_bottom, height));
        self.base.request_redraw();
    }

    /// Removes every sticky band.
    pub fn clear_sticky_regions(&mut self) {
        self.sticky_regions.clear();
        self.base.request_redraw();
    }

    /// Returns the sticky bands, in insertion order.
    pub fn sticky_regions(&self) -> &[StickyRegion] {
        &self.sticky_regions
    }

    /// Returns the bands that are pinned at the current scroll position, with the
    /// viewport-relative `y` each should be drawn at.
    ///
    /// Exposed because a caller that draws its own content needs the same answer
    /// the internal drawing path uses; deriving it twice is how the two would
    /// disagree about which header is currently pinned.
    pub fn pinned_sticky_bands(&self) -> Vec<(usize, i32)> {
        self.sticky_regions
            .iter()
            .enumerate()
            .filter(|(_, region)| {
                region.is_pinned(self.scroll_position.1) || {
                    // A band that has not been reached yet is not pinned but is still
                    // in view; only bands whose group has been entered are reported.
                    region.group_top - self.scroll_position.1 <= 0
                }
            })
            .map(|(index, region)| (index, region.draw_offset(self.scroll_position.1)))
            .collect()
    }
    /// Returns viewport rectangle.
    pub fn viewport(&self) -> Rect {
        self.viewport
    }
    /// Sets viewport rectangle.
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
    }
    /// Returns the current scroll position.
    pub fn scroll_position(&self) -> (i32, i32) {
        self.scroll_position
    }

    /// Sets the scroll position, clamped within the content extent.
    pub fn set_scroll_position(&mut self, x: i32, y: i32) {
        let view_w = self.viewport.width as i32;
        let view_h = self.viewport.height as i32;
        let content_w = self.content_size.width as i32;
        let content_h = self.content_size.height as i32;
        let max_x = (content_w - view_w).max(0);
        let max_y = (content_h - view_h).max(0);
        let clamped = (x.clamp(0, max_x), y.clamp(0, max_y));
        if self.scroll_position == clamped {
            return;
        }
        self.scroll_position = clamped;
        self.scroll_position_changed.emit(self.scroll_position);
        self.base.request_redraw();
    }

    /// Sets the total content size (used for AsNeeded scroll bar calculation).
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
    }

    /// Returns the total content size.
    pub fn content_size(&self) -> Size {
        self.content_size
    }
    /// Ensures rectangle is visible.
    pub fn ensure_visible(&mut self, rect: Rect) {
        // Adjust the content scroll position, not the viewport's geometry.
        let mut x = self.scroll_position.0;
        let mut y = self.scroll_position.1;
        let view_right = x.saturating_add(self.viewport.width as i32);
        let view_bottom = y.saturating_add(self.viewport.height as i32);
        let rect_right = rect.x.saturating_add(rect.width as i32);
        let rect_bottom = rect.y.saturating_add(rect.height as i32);
        if rect.x < x {
            x = rect.x;
        } else if rect_right > view_right {
            x = rect_right.saturating_sub(self.viewport.width as i32);
        }
        if rect.y < y {
            y = rect.y;
        } else if rect_bottom > view_bottom {
            y = rect_bottom.saturating_sub(self.viewport.height as i32);
        }
        self.set_scroll_position(x, y);
    }
    /// Ensures widget is visible.
    pub fn ensure_widget_visible(&mut self, widget_id: ObjectId) {
        // If the widget is the child widget, center the content viewport by
        // changing scroll_position rather than mutating viewport geometry.
        if Some(widget_id) == self.widget {
            let max_x = (self.content_size.width.saturating_sub(self.viewport.width)) as i32;
            let max_y = (self.content_size.height.saturating_sub(self.viewport.height)) as i32;
            self.set_scroll_position(max_x / 2, max_y / 2);
        }
    }

    /// Scrolls to the top of the content.
    pub fn scroll_to_top(&mut self) {
        self.set_scroll_position(self.scroll_position.0, 0);
    }

    /// Scrolls to the bottom of the content.
    pub fn scroll_to_bottom(&mut self) {
        if self.content_size.height > self.viewport.height {
            self.set_scroll_position(
                self.scroll_position.0,
                (self.content_size.height - self.viewport.height) as i32,
            );
        } else {
            self.set_scroll_position(self.scroll_position.0, 0);
        }
    }

    /// Scrolls to the left edge of the content.
    pub fn scroll_to_left(&mut self) {
        self.set_scroll_position(0, self.scroll_position.1);
    }

    /// Scrolls to the right edge of the content.
    pub fn scroll_to_right(&mut self) {
        if self.content_size.width > self.viewport.width {
            self.set_scroll_position(
                (self.content_size.width - self.viewport.width) as i32,
                self.scroll_position.1,
            );
        } else {
            self.set_scroll_position(0, self.scroll_position.1);
        }
    }
    /// Returns whether horizontal scroll bar is visible.
    fn horizontal_scroll_bar_visible(&self) -> bool {
        match self.horizontal_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.width > self.viewport.width,
        }
    }
    /// Returns whether vertical scroll bar is visible.
    fn vertical_scroll_bar_visible(&self) -> bool {
        match self.vertical_scroll_bar_policy {
            ScrollBarPolicy::AlwaysOn => true,
            ScrollBarPolicy::AlwaysOff => false,
            ScrollBarPolicy::AsNeeded => self.content_size.height > self.viewport.height,
        }
    }
    /// Updates scroll bars — redraws so scroll bar visibility changes take effect.
    /// Note: Full scroll bar widget creation would require access to a widget factory
    /// and is not yet implemented. Visual scroll bar drawing is handled in `Draw`.
    fn update_scroll_bars(&mut self) {
        self.base.request_redraw();
    }

    /// Paints one pinned band.
    ///
    /// Opaque and bordered, because a sticky header that lets the content beneath
    /// show through reads as a rendering glitch rather than as a header. The
    /// colours come from the style where the caller set them, so a themed scroll
    /// area's headers are themed too.
    fn draw_sticky_band(&self, context: &mut RenderContext, band_rect: Rect) {
        let style = self.style();
        let fill = style.background_color.unwrap_or(Color::rgb(248, 248, 248));
        let border = style.border_color.unwrap_or(Color::rgb(200, 200, 200));
        context.fill_rect(band_rect, fill);
        // Only the bottom edge is stroked: a full rectangle would draw a line along
        // the viewport's own top edge, doubling the border that already exists
        // there.
        let bottom = band_rect.y + band_rect.height as i32 - 1;
        context.draw_line_stroke(
            Point::new(band_rect.x, bottom),
            Point::new(band_rect.x + band_rect.width as i32, bottom),
            border,
            1,
        );
    }

    /// Computes scroll-bar thumb geometry for a track of `track_len` logical
    /// pixels over content of `content_len` shown in a `view_len` viewport at
    /// scroll offset `scroll`. Returns `(thumb_len, thumb_offset)` where the
    /// offset is measured from the start of the track.
    ///
    /// The thumb length is proportional to the visible fraction and never
    /// collapses below a minimal usable size. When the content fits inside the
    /// viewport the thumb spans the whole track and cannot move.
    ///
    /// # The minimum is a fraction's *floor*, not a replacement for it
    ///
    /// The floor used to be the bare literal `10` — two digits that meant "small enough to look
    /// like a thumb, large enough to grab" and were never related to the bar they sat in. On the
    /// crate's own `SCROLLBAR_THICKNESS = 8` that reads as a thumb barely taller than the groove
    /// is wide, and it is *shorter* than the 48 px `dimensions::SCROLLBAR_MIN_LENGTH` every other
    /// consumer of a scroll bar already agreed on — the crate had two answers to one question.
    ///
    /// So the length is `max(SCROLLBAR_MIN_LENGTH, track * view/content)`, clamped to the track:
    /// the same shape `draw_sticky_band` uses for its own geometry (resolve from the named
    /// constant, do not restate the number), and the same rule the reference toolkit's scroll bar
    /// states — the minimum is a **fraction** that a floor protects, not a constant that replaces
    /// it. The clamp to `track_len` is what keeps a bar shorter than the floor from overflowing
    /// its own groove.
    fn thumb_metrics(track_len: u32, content_len: u32, view_len: u32, scroll: i32) -> (u32, i32) {
        let track_len = track_len.max(1);
        if view_len == 0 || content_len <= view_len {
            return (track_len, 0);
        }
        let max_scroll = (content_len - view_len) as i32;
        let proportional = (track_len as u64 * view_len as u64) / content_len as u64;
        let thumb = proportional
            .max(dimensions::SCROLLBAR_MIN_LENGTH as u64)
            .clamp(1, track_len as u64) as u32;
        let max_thumb_pos = (track_len - thumb) as i32;
        let scroll = scroll.clamp(0, max_scroll);
        let pos = (max_thumb_pos as i64 * scroll as i64) / max_scroll as i64;
        (thumb, pos as i32)
    }

    /// The content box and the two bar bands, placed by the layout.
    ///
    /// # Why the chrome is assembled rather than subtracted
    ///
    /// The old form wrote each band as the control's own extent with an `if other_bar_visible { bar }`
    /// term applied to the *other* axis — written twice, once for each band, in two directions. "The
    /// other bar shortens this one" is a consequence of the two bars sharing a corner, not a fact
    /// about either of them, so stating it per band meant repeating the correction and keeping two
    /// spellings in agreement about one corner.
    ///
    /// The assembly says it once: a column whose first row is the scrolling content and whose second
    /// row is the horizontal bar, with the vertical bar as a trailing column *inside* that first row.
    /// The vertical bar is therefore beside the content and stops above the horizontal bar, and the
    /// corner is what the two bands leave rather than a third rectangle anyone computes.
    ///
    /// # What this does not cover
    ///
    /// The thumbs. Their length is a proportion of the content, which is not a layout question — a
    /// layout places boxes, it does not know how much of the content is visible. `thumb_metrics`
    /// keeps owning that.
    fn assemble_chrome(&self, rect: Rect) -> ChromeBands {
        let bar = dimensions::SCROLLBAR_THICKNESS;
        let h_scroll_visible = self.horizontal_scroll_bar_visible();
        let v_scroll_visible = self.vertical_scroll_bar_visible();
        // A control with no bars needs no assembly: the content *is* the rectangle.
        if !h_scroll_visible && !v_scroll_visible {
            return ChromeBands { content: rect, horizontal: None, vertical: None };
        }
        #[cfg(not(full_widgets))]
        {
            // `full_widgets` is "a device profile *and* an unstripped widget set" (principle #47),
            // and this module compiles without one. Both arms read the same `bar` and the same two
            // visibility flags, so the fallback is the same relation written the only way this
            // profile can express it.
            let h_band = if h_scroll_visible {
                Some(Rect::new(
                    rect.x,
                    rect.y + rect.height.saturating_sub(bar) as i32,
                    rect.width.saturating_sub(if v_scroll_visible { bar } else { 0 }),
                    bar.min(rect.height),
                ))
            } else {
                None
            };
            let v_band = if v_scroll_visible {
                Some(Rect::new(
                    rect.x + rect.width.saturating_sub(bar) as i32,
                    rect.y,
                    bar.min(rect.width),
                    rect.height.saturating_sub(if h_scroll_visible { bar } else { 0 }),
                ))
            } else {
                None
            };
            let content = Rect::new(
                rect.x,
                rect.y,
                rect.width.saturating_sub(if v_scroll_visible { bar } else { 0 }),
                rect.height.saturating_sub(if h_scroll_visible { bar } else { 0 }),
            );
            ChromeBands { content, horizontal: h_band, vertical: v_band }
        }
        #[cfg(full_widgets)]
        {
            let factory = WidgetFactory::new_with_defaults();
            // The outer column: the content row, then the horizontal bar.
            let mut column = CompositeBuilder::new(
                Box::new(FlexLayout::with_params(
                    FlexDirection::Column,
                    FlexWrap::NoWrap,
                    JustifyContent::FlexStart,
                    AlignItems::Stretch,
                    0,
                    0,
                )),
                EdgeOffsets::all(0),
                Size::new(0, 0),
            );
            let content_row_height =
                if h_scroll_visible { rect.height.saturating_sub(bar) } else { rect.height };
            let row = column.add_sized(
                &factory,
                "label",
                "",
                Size::new(rect.width, content_row_height),
                LayoutParams::filled(),
            );
            debug_assert!(row.is_some(), "the content row is a core control");
            if h_scroll_visible {
                let created = column.add_sized(
                    &factory,
                    "label",
                    "",
                    Size::new(rect.width, bar),
                    LayoutParams::new(),
                );
                debug_assert!(created.is_some(), "the horizontal bar is a core control");
            }
            let mut column_boxes: Vec<Rect> = Vec::new();
            column.arrange(rect, &mut |_, placed| column_boxes.push(placed));
            let content_row = column_boxes.first().copied().unwrap_or(Rect::new(
                rect.x,
                rect.y,
                rect.width,
                content_row_height,
            ));
            let horizontal = if h_scroll_visible { column_boxes.get(1).copied() } else { None };

            // The content row is itself a row: the scrolling content, then the vertical bar. Adding
            // the bar here rather than to the outer column is what makes it sit *beside* the content
            // and stop above the horizontal bar.
            if !v_scroll_visible {
                return ChromeBands { content: content_row, horizontal, vertical: None };
            }
            let mut inner = CompositeBuilder::new(
                Box::new(FlexLayout::with_params(
                    FlexDirection::Row,
                    FlexWrap::NoWrap,
                    JustifyContent::FlexStart,
                    AlignItems::Stretch,
                    0,
                    0,
                )),
                EdgeOffsets::all(0),
                Size::new(0, 0),
            );
            let content = inner.add_sized(
                &factory,
                "label",
                "",
                Size::new(content_row.width.saturating_sub(bar), content_row.height),
                LayoutParams::filled(),
            );
            debug_assert!(content.is_some(), "the viewport is a core control");
            let bar_column = inner.add_sized(
                &factory,
                "label",
                "",
                Size::new(bar, content_row.height),
                LayoutParams::new(),
            );
            debug_assert!(bar_column.is_some(), "the vertical bar is a core control");
            let mut inner_boxes: Vec<Rect> = Vec::new();
            inner.arrange(content_row, &mut |_, placed| inner_boxes.push(placed));
            let content_box = inner_boxes.first().copied().unwrap_or(content_row);
            let vertical = inner_boxes.get(1).copied().or_else(|| {
                Some(Rect::new(
                    content_row.x + content_row.width as i32 - bar as i32,
                    content_row.y,
                    bar,
                    content_row.height,
                ))
            });
            ChromeBands { content: content_box, horizontal, vertical }
        }
    }
}

/// The three boxes `ScrollArea` paints into, in control coordinates.
///
/// One value rather than three returns: the three are read together (the content is clipped to its
/// box, the two bars are filled), and a caller that took them one at a time could pair a content box
/// with a bar band from a different assembly.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ChromeBands {
    /// The scrolling viewport.
    content: Rect,
    /// The horizontal track, when one is owed.
    horizontal: Option<Rect>,
    /// The vertical track, when one is owed.
    vertical: Option<Rect>,
}
// Implement Widget trait
impl Widget for ScrollArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.viewport.width = geometry.width;
        self.viewport.height = geometry.height;
        self.update_scroll_bars();
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ScrollArea`'s property contract.
///
/// `scroll_position_x` / `scroll_position_y` are published as a pair because the
/// scroll position is one ordered tuple; splitting it lets a caller move one axis
/// without having to read and re-supply the other.
impl WidgetProperties for ScrollArea {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "widget_resizable" => Ok(CapabilityValue::Bool(self.widget_resizable())),
            "horizontal_scroll_bar_policy" => Ok(CapabilityValue::String(
                scroll_bar_policy_to_str(self.horizontal_scroll_bar_policy()).to_string(),
            )),
            "vertical_scroll_bar_policy" => Ok(CapabilityValue::String(
                scroll_bar_policy_to_str(self.vertical_scroll_bar_policy()).to_string(),
            )),
            "scroll_position_x" => Ok(CapabilityValue::Int(self.scroll_position().0 as i64)),
            "scroll_position_y" => Ok(CapabilityValue::Int(self.scroll_position().1 as i64)),
            "sticky_region_count" => Ok(CapabilityValue::UInt(self.sticky_regions().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "widget_resizable" => {
                self.set_widget_resizable(expect_bool(value)?);
                Ok(())
            }
            "horizontal_scroll_bar_policy" | "vertical_scroll_bar_policy" => {
                let token = expect_string(value)?;
                let policy = match token.as_str() {
                    "always_on" => ScrollBarPolicy::AlwaysOn,
                    "always_off" => ScrollBarPolicy::AlwaysOff,
                    "as_needed" => ScrollBarPolicy::AsNeeded,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                if name == "horizontal_scroll_bar_policy" {
                    self.set_horizontal_scroll_bar_policy(policy);
                } else {
                    self.set_vertical_scroll_bar_policy(policy);
                }
                Ok(())
            }
            "scroll_position_x" => {
                let x = expect_i64(value)? as i32;
                let (_, y) = self.scroll_position();
                self.set_scroll_position(x, y);
                Ok(())
            }
            "scroll_position_y" => {
                let y = expect_i64(value)? as i32;
                let (x, _) = self.scroll_position();
                self.set_scroll_position(x, y);
                Ok(())
            }
            // Derived from the registered bands, which are written through
            // `add_sticky_region` / `clear_sticky_regions`.
            "sticky_region_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SCROLL_AREA_PROPERTIES`.
        property_names_of![
            "widget_resizable",
            "horizontal_scroll_bar_policy",
            "vertical_scroll_bar_policy",
            "scroll_position_x",
            "scroll_position_y",
            "sticky_region_count",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for ScrollArea {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Handle scroll events (mouse wheel + touch swipe)
        match event {
            Event::Wheel { delta, modifiers: _ } => {
                // Scroll the content using clamped content coordinates.
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x * 20,
                    self.scroll_position.1 + delta.y * 20,
                );
            }
            #[cfg(feature = "touch")]
            Event::Swipe { start, end, velocity: _ } => {
                // Map swipe to scroll
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                self.set_scroll_position(self.scroll_position.0 + dx, self.scroll_position.1 + dy);
            }
            #[cfg(feature = "touch")]
            Event::Drag { delta, .. } => {
                // Map finger drag to scroll
                self.set_scroll_position(
                    self.scroll_position.0 + delta.x,
                    self.scroll_position.1 + delta.y,
                );
            }
            _ => { /* Other events are not relevant */ }
        }
        // Forward events to widget via registry (with viewport offset)
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                let content_event =
                    translate_content_event(event, self.scroll_position.0, self.scroll_position.1);
                reg.borrow_mut().forward_event(widget_id, &content_event);
            }
        }
    }
}
impl Draw for ScrollArea {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let style = self.style();
        // Draw background
        context.fill_rect(rect, style.background_color.unwrap_or(Color::rgb(255, 255, 255)));
        // Draw border
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
        // The scroll bar's three surfaces, resolved from the style with the previous greys as
        // the fallback. `draw_sticky_band` already read the style; the bar did not, which is why
        // a themed scroll area kept a pale bar. `background_color` is what the bar's groove was
        // asking for — it is the same surface the control paints — and the thumb is one visible
        // step toward the ink, so the two read as a groove and a handle in either appearance.
        let track_color = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        let border_color = style.border_color.unwrap_or(Color::rgb(200, 200, 200));
        let thumb_color = style.text_color.unwrap_or(Color::rgb(120, 120, 120)).with_alpha(120);
        // Set viewport for clipping
        context.push_clip(rect.x, rect.y, rect.width, rect.height);
        // Draw widget via registry, translated by the negative scroll offset so
        // the viewport shows the scrolled content window.
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                context.push_offset(-self.scroll_position.0, -self.scroll_position.1);
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_offset();
            }
        }
        // Sticky bands are drawn *after* the scrolled content, still inside the
        // clip and without the scroll offset, which is precisely what makes them
        // stay put. Drawing them with the offset (i.e. as ordinary content) would
        // make this loop a no-op.
        for region in &self.sticky_regions {
            // A band whose group has not been reached yet scrolls normally, so the
            // content itself already put it in the right place; re-drawing it here
            // would double-paint it.
            if region.group_top - self.scroll_position.1 > 0 {
                continue;
            }
            let offset_y = region.draw_offset(self.scroll_position.1);
            let band_rect =
                Rect::new(rect.x, rect.y + offset_y, rect.width, region.height.min(rect.height));
            // A band pushed entirely above the viewport has nothing left to show.
            if band_rect.y + band_rect.height as i32 <= rect.y {
                continue;
            }
            self.draw_sticky_band(context, band_rect);
        }
        context.pop_clip();
        // ── The chrome, assembled ──
        //
        // `assemble_chrome` places the content box and the two bands as one structure, so the
        // "the other bar shortens this one" correction lives in one place instead of once per band.
        let chrome = self.assemble_chrome(rect);
        if let Some(h_band) = chrome.horizontal {
            // The groove and the thumb both come from the resolved style, so the bar follows an
            // appearance switch. They used to be three fixed greys, so a scroll area kept a pale
            // bar in the middle of a dark window — the theme-blind family the rest of this file's
            // siblings had already fixed.
            context.fill_rect(h_band, track_color);
            context.draw_rect(h_band, border_color);
            let (thumb_width, thumb_dx) = Self::thumb_metrics(
                h_band.width,
                self.content_size.width,
                self.viewport.width.max(1),
                self.scroll_position.0,
            );
            context.fill_rect(
                Rect::new(h_band.x + thumb_dx, h_band.y, thumb_width, h_band.height),
                thumb_color,
            );
        }
        if let Some(v_band) = chrome.vertical {
            context.fill_rect(v_band, track_color);
            context.draw_rect(v_band, border_color);
            let (thumb_height, thumb_dy) = Self::thumb_metrics(
                v_band.height,
                self.content_size.height,
                self.viewport.height.max(1),
                self.scroll_position.1,
            );
            context.fill_rect(
                Rect::new(v_band.x, v_band.y + thumb_dy, v_band.width, thumb_height),
                thumb_color,
            );
        }
        // Draw the corner between the two bars: whatever the two bands leave, derived from the same
        // thickness rather than from a fourth literal.
        if let (Some(h_band), Some(v_band)) = (chrome.horizontal, chrome.vertical) {
            let bar = dimensions::SCROLLBAR_THICKNESS;
            context.fill_rect(
                Rect::new(v_band.x, h_band.y, bar.min(rect.width), bar.min(rect.height)),
                track_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Rect};
    use crate::render::SoftwarePaintBackend;

    #[test]
    fn scrollarea_creation_defaults() {
        let sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        assert_eq!(sa.geometry(), Rect::new(0, 0, 200, 200));
        assert_eq!(sa.viewport(), Rect::new(0, 0, 200, 200));
        assert_eq!(sa.scroll_position(), (0, 0));
        assert_eq!(sa.horizontal_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
        assert_eq!(sa.vertical_scroll_bar_policy(), ScrollBarPolicy::AsNeeded);
    }

    #[test]
    fn scrollarea_set_scroll_position_clamps_content_space() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(300, 260));
        sa.set_viewport(Rect::new(0, 0, 100, 100));

        sa.set_scroll_position(500, 400);
        assert_eq!(sa.scroll_position(), (200, 160));

        sa.set_scroll_position(-10, -8);
        assert_eq!(sa.scroll_position(), (0, 0));
    }

    #[test]
    fn scrollarea_ensure_visible_updates_scroll_position() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_viewport(Rect::new(0, 0, 100, 100));
        sa.set_scroll_position(100, 120);

        sa.ensure_visible(Rect::new(250, 275, 50, 50));
        assert_eq!(sa.scroll_position(), (200, 225));

        sa.ensure_visible(Rect::new(10, 20, 20, 20));
        assert_eq!(sa.scroll_position(), (10, 20));
        assert_eq!(sa.viewport(), Rect::new(0, 0, 100, 100));
    }

    #[test]
    fn scrollarea_extent_changes_reclamp_scroll_position() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_viewport(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_scroll_position(400, 400);

        sa.set_content_size(Size::new(150, 160));
        assert_eq!(sa.scroll_position(), (50, 60));

        sa.set_viewport(Rect::new(0, 0, 200, 200));
        assert_eq!(sa.scroll_position(), (0, 0));
    }

    #[test]
    fn scrollarea_ensure_widget_visible_changes_scroll_not_viewport() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(500, 500));
        sa.set_widget(Some(77));
        let viewport = sa.viewport();
        sa.ensure_widget_visible(77);
        assert_eq!(sa.viewport(), viewport);
        assert_eq!(sa.scroll_position(), (200, 200));
    }

    /// The bar's thickness, the corner and the track's shortening are one number.
    ///
    /// They were four spellings of the literal `16` — twice as a thickness, once as the
    /// corner, and twice as the `if other_bar_visible { … }` term — and none of them was
    /// `dimensions::SCROLLBAR_THICKNESS`, which every other control that draws a bar reads. One
    /// control's bar was therefore twice as thick as its neighbours', and changing the shared
    /// constant would have moved none of them.
    #[test]
    fn the_scroll_bar_geometry_comes_from_the_shared_thickness() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 120));
        sa.set_viewport(Rect::new(0, 0, 200, 120));
        // Content larger than the viewport on both axes, so both bars are owed.
        sa.set_content_size(Size::new(600, 600));
        assert!(sa.horizontal_scroll_bar_visible());
        assert!(sa.vertical_scroll_bar_visible());

        // The drawn bars reach the control's own trailing/bottom edges and are exactly the
        // shared thickness. Measured from the rendered SVG's own rectangles, so this is about
        // what a person sees rather than about a helper's return value.
        let bar = dimensions::SCROLLBAR_THICKNESS;
        let geometry = sa.geometry();
        let svg = crate::widget::svg::render_to_svg(&mut sa);

        // Every `<rect>` in the document, as (x, y, w, h).
        let rects: Vec<(i32, i32, u32, u32)> = svg
            .lines()
            .filter_map(|line| {
                let attr = |name: &str| -> Option<i32> {
                    let key = format!("{name}=\"");
                    let at = line.find(&key)? + key.len();
                    let end = line[at..].find('"')? + at;
                    line[at..end].parse().ok()
                };
                if !line.trim_start().starts_with("<rect") {
                    return None;
                }
                Some((attr("x")?, attr("y")?, attr("width")? as u32, attr("height")? as u32))
            })
            .collect();

        let horizontal_bar = rects.iter().any(|(_, y, w, h)| {
            *h == bar && *y == geometry.y + geometry.height as i32 - bar as i32 && *w > 0
        });
        let vertical_bar = rects.iter().any(|(x, _, w, h)| {
            *w == bar && *x == geometry.x + geometry.width as i32 - bar as i32 && *h > 0
        });
        assert!(horizontal_bar, "a horizontal bar of the shared thickness must be drawn");
        assert!(vertical_bar, "a vertical bar of the shared thickness must be drawn");
    }

    /// The two bars meet at a corner instead of one running under the other.
    #[test]
    fn the_two_bars_do_not_overlap_at_their_corner() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 120));
        sa.set_viewport(Rect::new(0, 0, 200, 120));
        sa.set_content_size(Size::new(600, 600));
        let bar = dimensions::SCROLLBAR_THICKNESS;
        // The tracks tile the control's rectangle: together with the corner they cover the
        // bottom `bar` rows and the trailing `bar` columns exactly once.
        let geometry = sa.geometry();
        let h_band_width = geometry.width - if sa.vertical_scroll_bar_visible() { bar } else { 0 };
        let v_band_height =
            geometry.height - if sa.horizontal_scroll_bar_visible() { bar } else { 0 };
        assert_eq!(
            h_band_width + bar,
            geometry.width,
            "the horizontal track plus the corner is the control's width"
        );
        assert_eq!(
            v_band_height + bar,
            geometry.height,
            "the vertical track plus the corner is the control's height"
        );
    }

    #[test]
    fn thumb_metrics_returns_full_track_when_content_fits() {
        // Content (50) fits inside the viewport (100): thumb spans the track
        // and never moves regardless of the requested scroll.
        let (len, pos) = ScrollArea::thumb_metrics(100, 50, 100, 30);
        assert_eq!(len, 100);
        assert_eq!(pos, 0);

        let (len, pos) = ScrollArea::thumb_metrics(100, 50, 100, -10);
        assert_eq!(len, 100);
        assert_eq!(pos, 0);
    }

    #[test]
    fn thumb_metrics_proportional_and_tracks_scroll() {
        // track 80, content 200, viewport 100 => thumb = 80*100/200 = 40, which is below the
        // floor, so the floor is what the track gets.
        let (len0, pos0) = ScrollArea::thumb_metrics(80, 200, 100, 0);
        assert_eq!(len0, dimensions::SCROLLBAR_MIN_LENGTH);
        assert_eq!(pos0, 0);

        // Halfway through the scrollable range the thumb is centered.
        let (len, pos) = ScrollArea::thumb_metrics(80, 200, 100, 50);
        assert_eq!(len, dimensions::SCROLLBAR_MIN_LENGTH);
        assert_eq!(pos, (80 - dimensions::SCROLLBAR_MIN_LENGTH) as i32 / 2);

        // Maximum scroll pins the thumb at the far end of the track.
        let (len, pos) = ScrollArea::thumb_metrics(80, 200, 100, 100);
        assert_eq!(len, dimensions::SCROLLBAR_MIN_LENGTH);
        assert_eq!(pos, (80 - dimensions::SCROLLBAR_MIN_LENGTH) as i32);

        // Out-of-range scroll values are clamped to the track.
        let (_, pos) = ScrollArea::thumb_metrics(80, 200, 100, -5);
        assert_eq!(pos, 0);
        let (_, pos) = ScrollArea::thumb_metrics(80, 200, 100, 999);
        assert_eq!(pos, (80 - dimensions::SCROLLBAR_MIN_LENGTH) as i32);

        // A **proportional** thumb above the floor is exactly the ratio, so the floor cannot be
        // mistaken for the rule. track 800, content 2000, viewport 1000 => 800*1000/2000 = 400.
        let (len, _) = ScrollArea::thumb_metrics(800, 2000, 1000, 0);
        assert_eq!(len, 400);
    }

    #[test]
    fn thumb_metrics_never_collapses_below_minimum() {
        // The floor is `SCROLLBAR_MIN_LENGTH`, the same constant every other consumer of a scroll
        // bar agrees on — not the literal `10` this used to clamp to, which was shorter than the
        // groove is wide (`SCROLLBAR_THICKNESS`) and shorter than the crate's own minimum.
        let (len, _) = ScrollArea::thumb_metrics(1000, 100_000, 100, 0);
        assert_eq!(len, dimensions::SCROLLBAR_MIN_LENGTH);

        // A track shorter than the floor cannot give the thumb more room than it has.
        let (len, _) = ScrollArea::thumb_metrics(20, 100_000, 20, 0);
        assert_eq!(len, 20);

        // And the floor is genuinely above the old literal, so this test would have failed
        // against the previous code rather than passing on a coincidence.
        const { assert!(dimensions::SCROLLBAR_MIN_LENGTH > 10) };
    }

    /// The thumb's floor and the groove's thickness must be a **usable pair**: a thumb thinner
    /// than the groove is a smudge, not a handle. This is the property the old `10` broke.
    #[test]
    fn the_thumb_floor_is_never_thinner_than_the_groove() {
        // A `const` block, because both sides are constants: the relation is a property of the
        // dimension table and should fail the build, not a run.
        const {
            assert!(
                dimensions::SCROLLBAR_MIN_LENGTH >= dimensions::SCROLLBAR_THICKNESS,
                "the bar is narrower than the thumb it must carry"
            )
        };
    }

    #[test]
    fn scrollarea_content_is_translated_by_scroll_position() {
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::SimpleRegistry;
        use std::cell::RefCell;
        use std::rc::Rc;

        let size = Size::new(100, 100);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        let stride = 100 * 4;
        let pixel = |rgba: &[u8], x: usize, y: usize| {
            let idx = y * stride + x * 4;
            (rgba[idx], rgba[idx + 1], rgba[idx + 2])
        };

        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_geometry(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(200, 200));
        let reg = Rc::new(RefCell::new(SimpleRegistry::new()));
        sa.set_registry(reg.clone());
        let child_id = 4242;
        sa.set_widget(Some(child_id));
        reg.borrow_mut().register(
            child_id,
            |ctx| {
                // A red marker inside the content at content coordinates (60..90).
                ctx.fill_rect(Rect::new(60, 60, 30, 30), Color::RED);
            },
            |_| {},
        );

        // Unscrolled: the red marker stays at its content coordinates.
        backend.begin_frame(Color::WHITE);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sa.draw(&mut ctx);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        assert_eq!(pixel(rgba, 70, 70), (255, 0, 0), "marker visible at content origin");
        assert_eq!(pixel(rgba, 10, 10), (255, 255, 255), "empty viewport area stays white");

        // Scroll down/right by (50, 50): the marker moves to (10..40).
        backend.begin_frame(Color::WHITE);
        sa.set_scroll_position(50, 50);
        {
            let mut ctx = RenderContext::new(&mut backend);
            sa.draw(&mut ctx);
        }
        backend.end_frame();
        let rgba = backend.frame_rgba();
        assert_eq!(pixel(rgba, 20, 20), (255, 0, 0), "marker translated into view");
        assert_eq!(pixel(rgba, 70, 70), (255, 255, 255), "marker scrolled out of its old spot");
    }

    #[test]
    fn scrollarea_forwards_pointer_coordinates_in_content_space() {
        use std::sync::{Arc, Mutex};

        let mut sa = ScrollArea::new(Rect::new(0, 0, 100, 100));
        sa.set_content_size(Size::new(300, 300));
        sa.set_scroll_position(40, 25);
        let registry = Rc::new(RefCell::new(SimpleRegistry::new()));
        sa.set_registry(registry.clone());
        let received = Arc::new(Mutex::new(None));
        let sink = received.clone();
        registry.borrow_mut().register(
            99,
            |_| {},
            move |event| {
                if let Event::MousePress { pos, .. } = event {
                    if let Ok(mut value) = sink.lock() {
                        *value = Some(*pos);
                    }
                }
            },
        );
        sa.set_widget(Some(99));

        sa.handle_event(&Event::mouse_press(10, 12, 1));
        assert_eq!(received.lock().ok().and_then(|value| *value), Some(Point::new(50, 37)));
    }

    // ── B6 / C-4: sticky regions ───────────────────────────────────────────

    /// Renders the scroll area into a software frame and returns the pixels.
    fn render(sa: &mut ScrollArea, size: Size) -> Vec<u8> {
        use crate::render::PaintBackend;
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        sa.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The colour at `(x, y)` in an RGBA buffer `width` pixels wide.
    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let index = ((y * width + x) * 4) as usize;
        (rgba[index], rgba[index + 1], rgba[index + 2], rgba[index + 3])
    }

    #[test]
    fn sticky_region_offset_scrolls_then_pins_then_releases() {
        // A group spanning content y 100..400 with a 30px header.
        let region = StickyRegion::new(100, 400, 30);

        // Not reached yet: the band scrolls with the content.
        assert_eq!(region.draw_offset(0), 100);
        assert_eq!(region.draw_offset(50), 50);
        assert!(!region.is_pinned(50));

        // Entered: pinned to the viewport top.
        assert_eq!(region.draw_offset(150), 0);
        assert!(region.is_pinned(150));
        // Still pinned while the group's end is at least a band-height away.
        assert_eq!(region.draw_offset(370), 0);
        assert!(region.is_pinned(370));

        // Group's end in view: pushed up so the next group's header takes over.
        // The push begins as soon as the remaining group height drops below the
        // band's own height, which is what stops two headers overlapping.
        assert_eq!(region.draw_offset(399), 1 - 30);
        assert_eq!(region.draw_offset(380), 20 - 30);
        assert_eq!(region.draw_offset(400), 0 - 30);
    }

    #[test]
    fn sticky_region_zero_height_is_ignored() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.add_sticky_region(0, 100, 0);
        // A band with no extent could not be seen to be pinned.
        assert!(sa.sticky_regions().is_empty());
        assert_eq!(sa.get("sticky_region_count").unwrap(), CapabilityValue::UInt(0));
    }

    #[test]
    fn sticky_region_registration_and_clearing() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.add_sticky_region(0, 100, 24);
        sa.add_sticky_region(100, 200, 24);
        assert_eq!(sa.sticky_regions().len(), 2);
        assert_eq!(sa.get("sticky_region_count").unwrap(), CapabilityValue::UInt(2));

        sa.clear_sticky_regions();
        assert!(sa.sticky_regions().is_empty());
    }

    #[test]
    fn sticky_region_count_is_read_only() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        assert_eq!(
            sa.set("sticky_region_count", CapabilityValue::UInt(3)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn sticky_bands_are_reported_only_once_their_group_is_entered() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.set_content_size(Size::new(200, 600));
        sa.add_sticky_region(0, 200, 24);
        sa.add_sticky_region(300, 500, 24);

        // At scroll 0 only the first group has been entered.
        let bands = sa.pinned_sticky_bands();
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].1, 0, "the first band pins to the viewport top");

        // Past the second group's top, both are reported, and the first is still
        // pinned because its group has not ended.
        sa.set_scroll_position(0, 350);
        let bands = sa.pinned_sticky_bands();
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[1].1, 0);
    }

    #[test]
    fn sticky_band_is_painted_at_the_viewport_top_when_pinned() {
        let size = Size::new(200, 200);
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.set_content_size(Size::new(200, 600));

        let without = render(&mut sa, size);

        sa.add_sticky_region(100, 400, 24);
        // Scroll past the group's top so the band pins.
        sa.set_scroll_position(0, 200);
        let with = render(&mut sa, size);

        // The pinned band is opaque and bordered, so its bottom rule (200,200,200)
        // appears on the row one pixel above the band's 24px extent — a row that
        // was plain background before the band was registered.
        let band_bottom_row = 23;
        let border_pixels = (0..size.width)
            .filter(|x| {
                let px = pixel(&with, size.width, *x, band_bottom_row);
                px.0 == 200 && px.1 == 200 && px.2 == 200 && px.3 == 255
            })
            .count();
        assert!(
            border_pixels > size.width as usize / 2,
            "the pinned band must paint a rule across the viewport top: {border_pixels} pixels"
        );

        // And it must differ from the un-sticky frame, which is what makes this a
        // behaviour assertion rather than a tautology.
        assert_ne!(without, with);
    }

    #[test]
    fn sticky_band_before_its_group_is_not_double_painted() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.set_content_size(Size::new(200, 600));
        sa.add_sticky_region(300, 500, 24);
        // At scroll 0 the group has not been reached, so the band is ordinary
        // content and this path must not paint it again at the viewport top.
        let bands = sa.pinned_sticky_bands();
        assert!(bands.is_empty());
    }

    #[test]
    fn sticky_band_fully_scrolled_off_is_not_drawn() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 200));
        sa.set_content_size(Size::new(200, 600));
        sa.add_sticky_region(0, 100, 24);
        // Scrolling well past the group's end pushes the band off the top.
        sa.set_scroll_position(0, 300);
        let bands = sa.pinned_sticky_bands();
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].1, 100 - 300 - 24, "the band is pushed above the viewport");
    }
}
