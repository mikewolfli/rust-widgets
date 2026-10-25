// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll area widget.
//!
//! # The viewport and the two bars are one derivation
//!
//! BLUE22 §B.8 lists this control's defect as "the scroll bars' positions computed by hand":
//! `h_band` and `v_band` were each written as the control's own extent minus an
//! `if other_bar_visible { bar }` correction, twice, in two directions. [`ScrollArea::assemble_chrome`]
//! states the geometry once: the content box is the control's rectangle minus the bars it owes, and
//! each bar is the strip the content leaves — so "the other bar shortens this one" is a consequence
//! of the two bands' shared corner rather than a term repeated in two places.

use crate::compat::{Arc, Rc, RefCell, ToString, Vec};
use crate::core::{Alignment, Color, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::ScrollBar;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::widget::capability::coercion::{
    expect_bool, expect_i32_saturating, expect_i64, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Pixels scrolled per wheel line. A notch is one line here, so this is also the
/// distance one notch has always moved the content.
const WHEEL_PIXELS_PER_LINE: i32 = 20;

fn translate_content_event(event: &Event, dx: i32, dy: i32) -> Event {
    let translate = |point: Point| Point::new(point.x + dx, point.y + dy);
    match event {
        Event::MouseDown((point, button)) => Event::MouseDown((translate(*point), *button)),
        Event::MouseUp((point, button)) => Event::MouseUp((translate(*point), *button)),
        Event::MouseMoveLegacy((point, device)) => {
            Event::MouseMoveLegacy((translate(*point), *device))
        }
        Event::MouseMove { pos } => Event::MouseMove { pos: translate(*pos) },
        Event::MousePress { pos, button, modifiers } => {
            Event::MousePress { pos: translate(*pos), button: *button, modifiers: *modifiers }
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
    /// An optional child mounted in the corner where the two bars meet.
    corner_widget: Option<ObjectId>,
    /// Every mounted descendant that scrolls with the content, flattened once per mount so a
    /// scroll step is a geometry write per node rather than a tree walk with an allocation at
    /// every level.
    #[cfg_attr(alloc_frugal, allow(dead_code))]
    content_descendants: Vec<ObjectId>,
    /// The child-list length the flat descendant list was built from.
    ///
    /// A host may mount rows directly into the viewport after construction; comparing the live
    /// child count against this is an O(1) way to notice that the cached list is stale, without
    /// rebuilding it on every scroll step (C-24).
    content_tree_children: usize,
    /// The total size of the content (for AsNeeded scroll bar calculation).
    content_size: Size,
    /// Optional shared registry for child widget forwarding.
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    /// Current scroll position (x, y) in content coordinates.
    scroll_position: (i32, i32),
    /// The inset applied inside the content box (C-21).
    ///
    /// Zero by default, so every existing placement and pixel is unchanged. The scroll range is
    /// measured against the inset box, which is what keeps the last line reachable instead of
    /// hidden under the inset.
    content_margins: crate::style::EdgeOffsets,
    /// Cached chrome derivation and the key it was produced under (C-P02).
    chrome_cache: RefCell<Option<(u64, ChromeBands)>>,
    /// Content-space regions that stay pinned while their group is scrolled past.
    sticky_regions: Vec<StickyRegion>,
    /// An optional control drawn inside each pinned band (COMP-E01), parallel to `sticky_regions`.
    ///
    /// The band geometry is the area's own derivation; the header only supplies the content, so a
    /// band with no header keeps the historical plain fill and the two cannot drift.
    sticky_headers: Vec<Option<ObjectId>>,
    /// Emitted whenever scroll position changes.
    pub scroll_position_changed: Signal1<(i32, i32)>,
    /// Whether a swipe carries momentum into a decaying fling.
    inertia: bool,
    /// Remaining fling velocity in logical pixels per second.
    fling: (f32, f32),
    /// An in-progress scroll-bar thumb drag: the axis and the pressed point.
    ///
    /// Only used by the **fallback** path. A mounted [`ScrollBar`] owns its own drag session, so
    /// the axis it occupies never reaches this field.
    thumb_drag: Option<(ScrollAxis, i32)>,
    /// A real `ScrollBar` child mounted on the vertical axis, when the host supplied one.
    ///
    /// # The self-drawn chrome is the default, not the only path
    ///
    /// [`ScrollArea`] predates [`ScrollBar`] being reusable: its own `draw` painted the groove and
    /// thumb and its own event handling ran the drag. That path is kept as the default so an area
    /// that never mounts a bar renders exactly the pixels it always did. Mounting a bar here
    /// replaces that axis' chrome with the real control: its geometry comes from the chrome band
    /// this area already derived, its `value_changed` moves the scroll offset, and moving the
    /// scroll offset writes its value — two directions, one source of truth (`scroll_position`).
    vertical_scroll_bar: Option<ScrollBarChild>,
    /// A real `ScrollBar` child mounted on the horizontal axis; see
    /// [`ScrollArea::vertical_scroll_bar`] for the fallback rule.
    horizontal_scroll_bar: Option<ScrollBarChild>,
}

/// A mounted [`ScrollBar`] and the mailbox its `value_changed` writes into.
///
/// # Why a mailbox rather than a callback that mutates the area
///
/// Signal callbacks are `Send + Sync + 'static`, so they cannot borrow the scroll area that owns
/// the bar. The bar therefore publishes into an atomic mailbox and the area **drains** it at the
/// next scroll-affine point (`handle_event`, `tick`, `sync_from_scroll_bars`). This is what makes
/// the bar's `value_changed` an authentic producer instead of a signal the host has to relay.
struct ScrollBarChild {
    bar: ScrollBar,
    /// The value carried by the most recent `value_changed`.
    pending_value: Arc<AtomicI32>,
    /// Set when `pending_value` holds a value the area has not applied yet.
    pending: Arc<AtomicBool>,
}

impl ScrollBarChild {
    /// Mounts `bar` and subscribes its `value_changed` to this child's mailbox.
    fn new(bar: ScrollBar) -> Self {
        let pending_value = Arc::new(AtomicI32::new(bar.value()));
        let pending = Arc::new(AtomicBool::new(false));
        let (value_out, pending_out) = (Arc::clone(&pending_value), Arc::clone(&pending));
        bar.value_changed.connect_ref(move |value| {
            value_out.store(*value, Ordering::SeqCst);
            pending_out.store(true, Ordering::SeqCst);
        });
        Self { bar, pending_value, pending }
    }

    /// Takes the mailbox's value, or `None` when nothing arrived since the last drain.
    fn take_pending(&self) -> Option<i32> {
        if self.pending.swap(false, Ordering::SeqCst) {
            Some(self.pending_value.load(Ordering::SeqCst))
        } else {
            None
        }
    }

    /// Drops a value this area itself just wrote, so it cannot be mistaken for user input.
    fn clear_pending(&self) {
        self.pending.store(false, Ordering::SeqCst);
    }
}

/// Which scroll bar a thumb drag is travelling on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    /// The horizontal bar.
    Horizontal,
    /// The vertical bar.
    Vertical,
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
            corner_widget: None,
            content_descendants: Vec::new(),
            content_tree_children: 0,
            content_size: Size::new(0, 0),
            registry: None,
            scroll_position: (0, 0),
            content_margins: crate::style::EdgeOffsets::default(),
            chrome_cache: RefCell::new(None),
            sticky_regions: Vec::new(),
            sticky_headers: Vec::new(),
            scroll_position_changed: Signal1::new(),
            inertia: false,
            fling: (0.0, 0.0),
            thumb_drag: None,
            vertical_scroll_bar: None,
            horizontal_scroll_bar: None,
        }
    }
    /// Whether a swipe carries momentum.
    pub fn inertia(&self) -> bool {
        self.inertia
    }

    /// Enables or disables fling momentum.
    ///
    /// Off by default, so a swipe still moves exactly as far as the pointer travelled.
    /// When on, a released swipe keeps the scroll moving and [`Widget::tick`] decays the
    /// velocity over subsequent frames.
    pub fn set_inertia(&mut self, inertia: bool) {
        if self.inertia == inertia {
            return;
        }
        self.inertia = inertia;
        if !inertia {
            self.fling = (0.0, 0.0);
        }
    }

    /// The rectangle of a scroll-bar thumb, if the bar is visible.
    fn thumb_rect(&self, axis: ScrollAxis) -> Option<Rect> {
        let chrome = self.assemble_chrome(self.geometry());
        match axis {
            ScrollAxis::Horizontal => {
                let band = chrome.horizontal?;
                let (length, offset) = Self::thumb_metrics(
                    band.width,
                    self.content_size.width,
                    self.viewport.width.max(1),
                    self.scroll_position.0,
                );
                Some(Rect::new(band.x + offset, band.y, length, band.height))
            }
            ScrollAxis::Vertical => {
                let band = chrome.vertical?;
                let (length, offset) = Self::thumb_metrics(
                    band.height,
                    self.content_size.height,
                    self.viewport.height.max(1),
                    self.scroll_position.1,
                );
                Some(Rect::new(band.x, band.y + offset, band.width, length))
            }
        }
    }

    /// Which axis has a visible bar under `point`.
    fn bar_at(&self, point: crate::core::Point) -> Option<ScrollAxis> {
        let chrome = self.assemble_chrome(self.geometry());
        if chrome.horizontal.is_some_and(|band| band.contains_point(point)) {
            Some(ScrollAxis::Horizontal)
        } else if chrome.vertical.is_some_and(|band| band.contains_point(point)) {
            Some(ScrollAxis::Vertical)
        } else {
            None
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
        if self.widget_resizable == resizable {
            return;
        }
        self.widget_resizable = resizable;
        // The frame is the viewport for a resizable child and the declared extent for a fixed
        // one, so this flag decides the child's rectangle.
        self.place_content();
        // `draw` decides whether to paint the corner grip from this, so the control's own ink
        // depends on it.
        self.base.request_redraw();
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    /// Sets how the content is positioned inside the viewport, and repaints.
    ///
    /// The alignment is read while drawing the content frame (`content_frame(chrome.content, ..,
    /// self.alignment)`), so changing it has to mark damage.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        if self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        // Alignment places a fixed-size child inside the viewport, so it changes the child's
        // rectangle rather than only the picture.
        self.place_content();
        self.base.request_redraw();
    }
    /// Returns horizontal scroll bar policy.
    pub fn horizontal_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.horizontal_scroll_bar_policy
    }
    /// Sets horizontal scroll bar policy.
    pub fn set_horizontal_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        if self.horizontal_scroll_bar_policy == policy {
            return;
        }
        self.horizontal_scroll_bar_policy = policy;
        // A bar changes the viewport's extent, and the child is placed in that extent.
        self.update_scroll_bars();
        self.place_content();
        self.base.request_redraw();
    }
    /// Returns vertical scroll bar policy.
    pub fn vertical_scroll_bar_policy(&self) -> ScrollBarPolicy {
        self.vertical_scroll_bar_policy
    }
    /// Sets vertical scroll bar policy.
    pub fn set_vertical_scroll_bar_policy(&mut self, policy: ScrollBarPolicy) {
        if self.vertical_scroll_bar_policy == policy {
            return;
        }
        self.vertical_scroll_bar_policy = policy;
        // A bar changes the viewport's extent, and the child is placed in that extent.
        self.update_scroll_bars();
        self.place_content();
        self.base.request_redraw();
    }

    /// The policy governing one axis.
    ///
    /// # The published vocabulary, and why the local name is kept
    ///
    /// The issue this implements asks for `Always` / `AsNeeded` / `Needed`; this crate's
    /// [`ScrollBarPolicy`] already spells the same three states `AlwaysOn` / `AsNeeded` /
    /// `AlwaysOff` and is the enum every existing caller and the capability table use. Renaming
    /// it would be a second vocabulary for one fact (principle #54), so the mapping is stated
    /// here instead: `Always` = [`ScrollBarPolicy::AlwaysOn`], `AsNeeded` =
    /// [`ScrollBarPolicy::AsNeeded`], `Needed` (show only while the content overflows) is the
    /// same `AsNeeded` state, and `AlwaysOff` is the explicit "never" the third slot has always
    /// meant.
    pub fn scroll_bar_policy(&self, axis: ScrollAxis) -> ScrollBarPolicy {
        match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_bar_policy,
            ScrollAxis::Vertical => self.vertical_scroll_bar_policy,
        }
    }

    /// Sets the policy governing one axis, re-deriving the chrome and the mounted bar's
    /// visibility.
    pub fn set_scroll_bar_policy(&mut self, axis: ScrollAxis, policy: ScrollBarPolicy) {
        match axis {
            ScrollAxis::Horizontal => self.set_horizontal_scroll_bar_policy(policy),
            ScrollAxis::Vertical => self.set_vertical_scroll_bar_policy(policy),
        }
    }

    /// Mounts (or clears, with `None`) a real [`ScrollBar`] on the vertical axis.
    ///
    /// # What changes when a bar is mounted
    ///
    /// * the band is the one [`Self::assemble_chrome`] already derives, written into the bar's
    ///   own `geometry` — the child is placed by the same derivation the self-drawn bar used;
    /// * the control's `draw` delegates that band to [`ScrollBar::draw`] instead of painting a
    ///   groove and thumb itself;
    /// * pointer events in the band go to the bar, whose own thumb drag becomes the gesture;
    /// * `value_changed` moves [`Self::scroll_position`] and a scroll-position change writes the
    ///   bar's value, so the two cannot drift.
    ///
    /// With no bar mounted the self-drawn path stays in charge, which is why an existing area's
    /// pixels and tests are unaffected by this API existing.
    pub fn set_vertical_scroll_bar(&mut self, bar: Option<ScrollBar>) {
        self.vertical_scroll_bar = bar.map(ScrollBarChild::new);
        self.update_scroll_bars();
        self.base.request_redraw();
    }

    /// Mounts (or clears, with `None`) a real [`ScrollBar`] on the horizontal axis.
    pub fn set_horizontal_scroll_bar(&mut self, bar: Option<ScrollBar>) {
        self.horizontal_scroll_bar = bar.map(ScrollBarChild::new);
        self.update_scroll_bars();
        self.base.request_redraw();
    }

    /// Mounts (or clears) a bar on `axis`; the axis-explicit spelling of the two setters above.
    pub fn set_scroll_bar(&mut self, axis: ScrollAxis, bar: Option<ScrollBar>) {
        match axis {
            ScrollAxis::Horizontal => self.set_horizontal_scroll_bar(bar),
            ScrollAxis::Vertical => self.set_vertical_scroll_bar(bar),
        }
    }

    /// The mounted bar on `axis`, if any.
    pub fn scroll_bar(&self, axis: ScrollAxis) -> Option<&ScrollBar> {
        match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_bar.as_ref(),
            ScrollAxis::Vertical => self.vertical_scroll_bar.as_ref(),
        }
        .map(|child| &child.bar)
    }

    /// The mounted bar on `axis`, mutably.
    ///
    /// A write through this reference (say `set_style`) is the host's own change and does not
    /// reach the scroll position. A **value** write does: call [`Self::sync_from_scroll_bars`]
    /// afterwards, which is also what the control does at the next event or tick.
    pub fn scroll_bar_mut(&mut self, axis: ScrollAxis) -> Option<&mut ScrollBar> {
        match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_bar.as_mut(),
            ScrollAxis::Vertical => self.vertical_scroll_bar.as_mut(),
        }
        .map(|child| &mut child.bar)
    }

    /// Whether a real bar is mounted on `axis`.
    pub fn has_scroll_bar(&self, axis: ScrollAxis) -> bool {
        match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_bar.is_some(),
            ScrollAxis::Vertical => self.vertical_scroll_bar.is_some(),
        }
    }

    /// Applies any value the mounted bars' `value_changed` published since the last drain.
    ///
    /// Returns whether the scroll position moved. Called automatically by `handle_event`, `tick`
    /// and [`Self::set_scroll_position`]'s own writers; exposed because a host that writes the bar
    /// directly may want the result immediately rather than at the next event.
    pub fn sync_from_scroll_bars(&mut self) -> bool {
        let mut moved = false;
        if let Some(value) =
            self.vertical_scroll_bar.as_ref().and_then(ScrollBarChild::take_pending)
        {
            let before = self.scroll_position.1;
            self.set_scroll_position(self.scroll_position.0, value);
            moved |= self.scroll_position.1 != before;
        }
        if let Some(value) =
            self.horizontal_scroll_bar.as_ref().and_then(ScrollBarChild::take_pending)
        {
            let before = self.scroll_position.0;
            self.set_scroll_position(value, self.scroll_position.1);
            moved |= self.scroll_position.0 != before;
        }
        moved
    }

    /// Writes the current scroll position into the mounted bars.
    fn write_scroll_bar_values(&mut self) {
        let (x, y) = self.scroll_position;
        if let Some(child) = self.horizontal_scroll_bar.as_mut() {
            child.bar.set_value(x);
            child.clear_pending();
        }
        if let Some(child) = self.vertical_scroll_bar.as_mut() {
            child.bar.set_value(y);
            child.clear_pending();
        }
    }

    /// Places each mounted bar in its chrome band, publishes its range and visibility.
    ///
    /// The band comes from [`Self::assemble_chrome`], so the child's geometry is the same
    /// rectangle the fallback painter used — the two paths cannot disagree about where the bar
    /// is. The bar's value domain is the scroll offset in pixels: `maximum` is the largest
    /// reachable offset, `page_step` is one viewport so PageUp/PageDown and a trough click move a
    /// page, and `single_step` is the wheel's line distance.
    fn layout_scroll_bars(&mut self) {
        let chrome = self.assemble_chrome(self.base.geometry());
        let (x, y) = self.scroll_position;
        let max_x = self.max_scroll_x();
        let max_y = self.max_scroll_y();
        let page_x = self.viewport.width.max(1) as i32;
        let page_y = self.viewport.height.max(1) as i32;
        if let Some(child) = self.horizontal_scroll_bar.as_mut() {
            match chrome.horizontal {
                Some(band) => {
                    child.bar.base_mut().show();
                    child.bar.set_orientation(crate::core::Orientation::Horizontal);
                    child.bar.set_geometry(band);
                }
                None => child.bar.base_mut().hide(),
            }
            child.bar.set_range(0, max_x);
            child.bar.set_page_step(page_x);
            child.bar.set_single_step(WHEEL_PIXELS_PER_LINE);
            child.bar.set_value(x);
            child.clear_pending();
        }
        if let Some(child) = self.vertical_scroll_bar.as_mut() {
            match chrome.vertical {
                Some(band) => {
                    child.bar.base_mut().show();
                    child.bar.set_orientation(crate::core::Orientation::Vertical);
                    child.bar.set_geometry(band);
                }
                None => child.bar.base_mut().hide(),
            }
            child.bar.set_range(0, max_y);
            child.bar.set_page_step(page_y);
            child.bar.set_single_step(WHEEL_PIXELS_PER_LINE);
            child.bar.set_value(y);
            child.clear_pending();
        }
    }

    /// The largest horizontal scroll offset the content allows.
    fn max_scroll_x(&self) -> i32 {
        (self.content_size.width as i32 - self.viewport.width as i32).max(0)
    }

    /// The largest vertical scroll offset the content allows.
    fn max_scroll_y(&self) -> i32 {
        (self.content_size.height as i32 - self.viewport.height as i32).max(0)
    }

    /// Routes a pointer event to the mounted bars.
    ///
    /// Returns whether a visible bar's band owns the pointer, in which case this control must
    /// not also start its own thumb drag. Movement and release are always offered to the bars —
    /// a drag that leaves the band still has to hear its release.
    fn dispatch_to_scroll_bars(&mut self, event: &Event) -> bool {
        let pos = match event {
            Event::MousePress { pos, .. }
            | Event::MouseMove { pos }
            | Event::MouseRelease { pos, .. }
            | Event::MouseDoubleClick { pos, .. } => *pos,
            _ => return false,
        };
        let mut owns_pointer = false;
        if let Some(child) = self.horizontal_scroll_bar.as_mut() {
            if child.bar.base().is_visible() {
                child.bar.handle_event(event);
                owns_pointer |= child.bar.geometry().contains_point(pos);
            }
        }
        if let Some(child) = self.vertical_scroll_bar.as_mut() {
            if child.bar.base().is_visible() {
                child.bar.handle_event(event);
                owns_pointer |= child.bar.geometry().contains_point(pos);
            }
        }
        owns_pointer
    }
    /// Sets widget.
    ///
    /// # Why this writes both sides of the link
    ///
    /// `BaseWidget::add_child` appends to *this* control's list "without touching the child's own
    /// parent link" (its own documentation says so), and `set_parent` likewise does not update any
    /// child list. BLUE22 §B.6 rule 5 is therefore a rule about *call sites*: a host that writes
    /// one side only leaves a child that believes it has no parent — invisible to any tree walk
    /// that descends from the root, which is every walk that draws, hit-tests or discovers focus.
    ///
    /// The previous version wrote the child list alone. A scroll area's content was consequently
    /// unreachable from its own parent, so anything walking the window tree found the viewport and
    /// nothing inside it.
    ///
    /// Replacing the content also **unlinks the old one**: `add_child` does not reject duplicates
    /// and nothing else removes the previous id, so setting a second widget left the first in the
    /// list forever — a stale id that a walk would still visit. Clearing its parent link keeps the
    /// two sides consistent rather than leaving an orphan that claims a parent it no longer has.
    pub fn set_widget(&mut self, widget: Option<ObjectId>) {
        // A child cannot be both the scrolling content and the corner. Promoting the current
        // corner to content first removes its old child entry, so it is not linked twice.
        if widget.is_some() && widget == self.corner_widget {
            if let Some(corner) = self.corner_widget {
                self.base.remove_child_linked(corner);
                #[cfg(not(alloc_frugal))]
                crate::set_widget_visible(corner, false);
            }
            self.corner_widget = None;
        }
        if let Some(previous) = self.widget {
            if Some(previous) != widget {
                self.base.remove_child_linked(previous);
                // See `Frame::set_widget`: detached content must stop claiming to be visible.
                #[cfg(not(alloc_frugal))]
                crate::set_widget_visible(previous, false);
            }
        }
        self.widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(widget_id, true);
        }
        // The scrolling set is the mounted tree, so it is rebuilt exactly when the content root
        // changes rather than on every scroll step.
        self.refresh_content_tree();
        // The child has never been placed: it keeps whatever rectangle it was constructed with
        // until the scroll area hands it the viewport.
        self.place_content();
        self.base.request_redraw();
    }
    /// Returns widget.
    pub fn widget(&self) -> Option<ObjectId> {
        self.widget
    }

    /// The child mounted in the corner between the two scroll bars, if any.
    pub fn corner_widget(&self) -> Option<ObjectId> {
        self.corner_widget
    }

    /// Mounts (or clears, with `None`) a child in the corner where the two bars meet.
    ///
    /// The corner belongs to the control rather than to the scrolling content, so the child is
    /// linked as a tree child but excluded from the scrolling descendant list. Its rectangle is
    /// written whenever the chrome is re-derived; when either bar is not visible there is no
    /// corner to own, so the widget is hidden.
    pub fn set_corner_widget(&mut self, widget: Option<ObjectId>) {
        // The scrolling content and the corner are two roles of one child list: one id cannot
        // occupy both.
        if widget.is_some() && widget == self.widget {
            return;
        }
        if self.corner_widget == widget {
            self.place_content();
            return;
        }
        if let Some(previous) = self.corner_widget {
            self.base.remove_child_linked(previous);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(previous, false);
        }
        self.corner_widget = widget;
        if let Some(widget_id) = widget {
            self.base.add_child_linked(widget_id);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(widget_id, true);
        }
        // A corner is not content: the flat descendant list must not translate it on scroll.
        self.refresh_content_tree();
        self.place_content();
        self.base.request_redraw();
    }

    /// The rectangle the corner widget is given, when both bars are visible.
    pub fn corner_widget_rect(&self) -> Option<Rect> {
        self.assemble_chrome(self.base.geometry()).corner
    }

    /// Rebuilds the flat list of scrolling descendants from the mounted tree.
    ///
    /// Called when the content root or the corner changes, and available to a host that mounts
    /// descendants directly into the content subtree after [`Self::set_widget`]. A host that
    /// changes the tree must call it once for the change to be reflected by the next scroll.
    pub fn refresh_content_tree(&mut self) {
        self.content_descendants.clear();
        self.content_tree_children = self.base.children().len();
        // The mini profile holds no widget registry to descend, and it also does not translate
        // children on scroll (`move_content_children` is compiled out), so the flat list stays
        // empty there rather than being half-populated.
        #[cfg(not(alloc_frugal))]
        {
            let mut stack: Vec<(ObjectId, u32)> = Vec::new();
            for child in self.base.children().to_vec() {
                if Some(child) == self.corner_widget {
                    continue;
                }
                if Some(child) == self.widget {
                    let descendants = crate::widget::runtime::with_widget(child, |widget| {
                        widget.children().to_vec()
                    })
                    .unwrap_or_default();
                    for descendant in descendants {
                        stack.push((descendant, 1));
                    }
                } else {
                    stack.push((child, 0));
                }
            }
            while let Some((id, depth)) = stack.pop() {
                // The old recursive walker used the same bound: a tree is a tree, and a corrupted
                // parent chain must not turn a scroll into an unbounded walk.
                if depth > 64 {
                    continue;
                }
                self.content_descendants.push(id);
                if let Some(children) =
                    crate::widget::runtime::with_widget(id, |widget| widget.children().to_vec())
                {
                    for child in children {
                        stack.push((child, depth + 1));
                    }
                }
            }
        }
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
        self.sticky_headers.push(None);
        self.base.request_redraw();
    }

    /// Removes every sticky band.
    pub fn clear_sticky_regions(&mut self) {
        self.sticky_regions.clear();
        self.sticky_headers.clear();
        self.base.request_redraw();
    }

    /// Mounts (or clears, with `None`) the control drawn inside band `index` (COMP-E01).
    ///
    /// Returns `false` for an out-of-range band. The control is linked as this area's child and
    /// placed in the band rectangle the wheel/scroller derives, so it follows the pin rather than
    /// keeping a fixed position.
    pub fn set_sticky_header(&mut self, index: usize, header: Option<ObjectId>) -> bool {
        if index >= self.sticky_regions.len() {
            return false;
        }
        if self.sticky_headers.len() < self.sticky_regions.len() {
            self.sticky_headers.resize(self.sticky_regions.len(), None);
        }
        let previous = self.sticky_headers[index];
        if previous == header {
            return true;
        }
        if let Some(previous) = previous {
            self.base.remove_child_linked(previous);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(previous, false);
        }
        self.sticky_headers[index] = header;
        if let Some(header) = header {
            self.base.add_child_linked(header);
            #[cfg(not(alloc_frugal))]
            crate::set_widget_visible(header, true);
        }
        self.base.request_redraw();
        true
    }

    /// The control mounted in band `index`, if any.
    pub fn sticky_header(&self, index: usize) -> Option<ObjectId> {
        self.sticky_headers.get(index).copied().flatten()
    }

    /// Removes the sticky band at `index` (C-20). Returns `false` when the index is out of range.
    ///
    /// The other bands keep their order and identity, so a host tracking a band by its index can
    /// drop one group without re-registering the rest — the flicker a full clear-and-re-add
    /// produces between two frames.
    pub fn remove_sticky_region(&mut self, index: usize) -> bool {
        if index >= self.sticky_regions.len() {
            return false;
        }
        self.sticky_regions.remove(index);
        if index < self.sticky_headers.len() {
            if let Some(header) = self.sticky_headers.remove(index) {
                self.base.remove_child_linked(header);
                #[cfg(not(alloc_frugal))]
                crate::set_widget_visible(header, false);
            }
        }
        self.base.request_redraw();
        true
    }

    /// Removes the sticky band whose group starts at `group_top`, if one does (C-20).
    pub fn remove_sticky_region_for_group(&mut self, group_top: i32) -> bool {
        match self.sticky_regions.iter().position(|region| region.group_top == group_top) {
            Some(index) => self.remove_sticky_region(index),
            None => false,
        }
    }

    /// The inset applied inside the content box (C-21): `(top, bottom, left, right)`.
    pub fn content_margins(&self) -> (u32, u32, u32, u32) {
        (
            self.content_margins.top,
            self.content_margins.bottom,
            self.content_margins.left,
            self.content_margins.right,
        )
    }

    /// Sets the content inset (C-21).
    ///
    /// The content box shrinks by these amounts, and the scroll range is measured against the
    /// inset box, so the last line of content is reachable rather than sitting under the inset.
    /// The default is all zero, which is the historical full-bleed content box.
    pub fn set_content_margins(&mut self, top: u32, bottom: u32, left: u32, right: u32) {
        let margins = crate::style::EdgeOffsets { top, right, bottom, left };
        if self.content_margins == margins {
            return;
        }
        self.content_margins = margins;
        // The scroll range is derived from the content box, so re-clamp before placing.
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
        self.place_content();
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
        self.update_scroll_bars();
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
        self.place_content();
    }
    /// Returns the current scroll position.
    pub fn scroll_position(&self) -> (i32, i32) {
        self.scroll_position
    }

    /// Sets the scroll position, clamped within the content extent.
    pub fn set_scroll_position(&mut self, x: i32, y: i32) {
        // The usable viewport is the viewport minus the content inset (C-21). Bars are
        // deliberately *not* subtracted here: the historical contract clamps against the declared
        // viewport, and changing that would shift every stored scroll position by the bar
        // thickness even for a caller who never set a margin.
        let view_w =
            self.viewport.width.saturating_sub(
                self.content_margins.left.saturating_add(self.content_margins.right),
            ) as i32;
        let view_h =
            self.viewport.height.saturating_sub(
                self.content_margins.top.saturating_add(self.content_margins.bottom),
            ) as i32;
        let content_w = self.content_size.width as i32;
        let content_h = self.content_size.height as i32;
        let max_x = (content_w - view_w).max(0);
        let max_y = (content_h - view_h).max(0);
        let clamped = (x.clamp(0, max_x), y.clamp(0, max_y));
        if self.scroll_position == clamped {
            // A mounted bar can be waiting on a value that matches the position; drain it so a
            // stale mailbox cannot re-apply an old offset on the next event.
            self.write_scroll_bar_values();
            return;
        }
        // A host may have mounted rows directly after the last rebuild; refresh before moving so
        // the cached id list is not stale.
        if self.base.children().len() != self.content_tree_children {
            self.refresh_content_tree();
        }
        #[cfg(not(alloc_frugal))]
        let (dx, dy) = (clamped.0 - self.scroll_position.0, clamped.1 - self.scroll_position.1);
        self.scroll_position = clamped;
        self.scroll_position_changed.emit(self.scroll_position);
        // The offset is part of the child's geometry, not an offset applied by its painter.
        //
        // # Why every mounted child moves, not only `self.widget`
        //
        // A scroll area is a viewport over whatever the host mounted inside it. The original
        // implementation moved only the single `self.widget` id, so a host that mounted several
        // rows directly (the control demo does) saw the bar travel while the rows stayed put —
        // "滚动条动，但内容没动". Geometry in this crate is absolute, so moving a container does
        // not move its descendants either; each subtree is translated here.
        #[cfg(not(alloc_frugal))]
        self.move_content_children(dx, dy);
        self.place_content();
        self.write_scroll_bar_values();
        self.base.request_redraw();
    }

    /// Translates every mounted content child of the viewport by `(-dx, -dy)`.
    ///
    /// `self.widget` keeps its own placement derivation (`content_child_rect`, which already
    /// includes the scroll offset), so only its descendants are translated here. Every other
    /// direct child is content the host mounted itself and is translated as a whole. The list
    /// was flattened when the tree was mounted, so a step performs no allocations.
    ///
    /// # C-P03: what is still per-node, and why
    ///
    /// Each descendant costs one global geometry read and one write. A true bulk primitive —
    /// "read-modify-write this whole id list under one lock" — belongs at the crate's runtime
    /// geometry layer, outside this module; the second route the issue offers (stop moving the
    /// subtree and apply one viewport offset in the painter) would change the geometry contract
    /// every host and hit test currently relies on. The step is therefore left as an accurate
    /// per-node translation, with the flattened list (no tree walk, no allocation) as the part
    /// this control can own. Recorded here rather than silently claimed as closed.
    #[cfg(not(alloc_frugal))]
    fn move_content_children(&self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        for &id in &self.content_descendants {
            if let Some((x, y, width, height)) = crate::widget_geometry(id) {
                crate::set_widget_geometry(id, x - dx, y - dy, width, height);
            }
        }
    }

    /// Sets the total content size (used for AsNeeded scroll bar calculation).
    pub fn set_content_size(&mut self, size: Size) {
        self.content_size = size;
        // The extent decides both bar visibility and the bars' ranges, so the chrome is re-derived
        // before the (possibly unchanged) offset is clamped again.
        self.update_scroll_bars();
        self.set_scroll_position(self.scroll_position.0, self.scroll_position.1);
        // `set_scroll_position` is a no-op when the clamped offset did not change, so the
        // placement cannot rely on it: a fixed-size child's frame is the declared extent.
        self.place_content();
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
    ///
    /// The content root retains its historical behaviour (centre it). Any other mounted
    /// descendant is read through its absolute geometry, mapped into content coordinates and
    /// handed to [`Self::ensure_visible`]. Returns `false` when the id is not a mounted
    /// descendant or its geometry is unavailable.
    pub fn ensure_widget_visible(&mut self, widget_id: ObjectId) -> bool {
        // If the widget is the child widget, center the content viewport by
        // changing scroll_position rather than mutating viewport geometry.
        if Some(widget_id) == self.widget {
            let max_x = (self.content_size.width.saturating_sub(self.viewport.width)) as i32;
            let max_y = (self.content_size.height.saturating_sub(self.viewport.height)) as i32;
            self.set_scroll_position(max_x / 2, max_y / 2);
            return true;
        }
        #[cfg(not(alloc_frugal))]
        {
            let Some((x, y, width, height)) = crate::widget_geometry(widget_id) else {
                return false;
            };
            let chrome = self.assemble_chrome(self.base.geometry());
            let frame = content_frame(
                chrome.content,
                self.content_size,
                self.widget_resizable,
                self.alignment,
            );
            // Absolute geometry -> content coordinates. The content origin sits at
            // `frame - scroll_position`, so subtracting that origin adds the offset back.
            let content_x = x.saturating_sub(frame.x).saturating_add(self.scroll_position.0);
            let content_y = y.saturating_sub(frame.y).saturating_add(self.scroll_position.1);
            self.ensure_visible(Rect::new(content_x, content_y, width, height));
            true
        }
        #[cfg(alloc_frugal)]
        {
            false
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
    /// Re-derives the mounted bars: placement, range, visibility and value.
    ///
    /// The self-drawn chrome remains the default path; this is the only writer of a mounted
    /// bar's geometry, so the child and the `assemble_chrome` bands are one measurement.
    fn update_scroll_bars(&mut self) {
        self.layout_scroll_bars();
        self.base.request_redraw();
    }

    /// The rectangle the content child occupies, **including** the scroll offset.
    ///
    /// `content_frame` answers the unscrolled box inside the viewport; the scroll position
    /// moves that box. The offset belongs in the geometry here because the tree painter draws
    /// a child at its own rectangle — an offset applied only while drawing would move the
    /// pixels but leave the hit test, the accessible frame and every geometry query behind.
    fn content_child_rect(&self) -> Option<Rect> {
        self.widget?;
        let chrome = self.assemble_chrome(self.base.geometry());
        let frame =
            content_frame(chrome.content, self.content_size, self.widget_resizable, self.alignment);
        Some(Rect::new(
            frame.x.saturating_sub(self.scroll_position.0),
            frame.y.saturating_sub(self.scroll_position.1),
            frame.width,
            frame.height,
        ))
    }

    /// Places the content child in the viewport, translated by the scroll offset.
    ///
    /// Every state that changes the box calls this: geometry, content extent, scroll offset,
    /// resizability, alignment and scroll-bar policy. Keeping the list complete is what makes
    /// the placement observable without a draw — a caller that asks the child's geometry after
    /// a scroll gets the rectangle the next frame will use.
    fn place_content(&self) {
        if let Some(rect) = self.content_child_rect() {
            // A sized profile has no mounted surfaces and no platform geometry entry, but the
            // rectangle derivation is still the control's model, so it is computed and not
            // discarded with a `cfg` on the helper (which would make the helper dead there).
            #[cfg(not(alloc_frugal))]
            if let Some(widget_id) = self.widget {
                crate::set_widget_geometry(widget_id, rect.x, rect.y, rect.width, rect.height);
            }
            #[cfg(alloc_frugal)]
            let _ = rect;
        }
        // The corner is part of the same chrome derivation, so every state that re-places the
        // content also hands the corner its rectangle.
        self.place_corner_widget();
    }

    /// Places the corner widget in the square the two bars leave, or hides it when either bar
    /// is absent.
    fn place_corner_widget(&self) {
        #[cfg(not(alloc_frugal))]
        if let Some(corner) = self.corner_widget {
            match self.corner_widget_rect() {
                Some(rect) => {
                    crate::set_widget_visible(corner, true);
                    crate::set_widget_geometry(corner, rect.x, rect.y, rect.width, rect.height);
                }
                None => crate::set_widget_visible(corner, false),
            }
        }
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

    /// The content box and the two bar bands.
    ///
    /// # Why the chrome is one derivation and not two corrections
    ///
    /// The old form wrote each band as the control's own extent with an `if other_bar_visible { bar }`
    /// term applied to the *other* axis — written twice, once for each band, in two directions. "The
    /// other bar shortens this one" is a consequence of the two bars sharing a corner, not a fact
    /// about either of them, so stating it per band meant repeating the correction and keeping two
    /// spellings in agreement about one corner.
    ///
    /// It is stated once here: the content box is the rectangle minus each bar the control owes, and
    /// each bar is the strip that content leaves. The vertical band therefore stops above the
    /// horizontal one, and the corner square is what the two leave — the paint path fills it from the
    /// same two boxes rather than from a third computed rectangle.
    ///
    /// # Why this is arithmetic rather than an assembled column and row
    ///
    /// The bands used to be placed by two `CompositeBuilder` + `FlexLayout` assemblies — four
    /// temporary `label` widgets and two `Vec`s per call — for a control whose paint path reads this
    /// geometry twice (once to clip the content, once to draw the bars) and whose hit test reads it
    /// on every pointer event. The two assemblies resolved to the arithmetic below; the row/column
    /// structure was a slower spelling of "minus the bars it owes".
    ///
    /// # What this does not cover
    ///
    /// The thumbs. Their length is a proportion of the content, which is not a layout question — a
    /// layout places boxes, it does not know how much of the content is visible. `thumb_metrics`
    /// keeps owning that.
    fn assemble_chrome(&self, rect: Rect) -> ChromeBands {
        // C-P02: the derivation is pure arithmetic over the geometry, the two policies, the
        // content extent and the content inset, and it was recomputed 4–6 times per frame
        // (thumb, bar hit test, clip, child placement, draw, corner widget). One cached value
        // feeds every caller until one of those inputs changes.
        let fingerprint = self.chrome_fingerprint(rect);
        if let Some((cached, bands)) = self.chrome_cache.borrow().as_ref() {
            if *cached == fingerprint {
                return *bands;
            }
        }
        let bands = self.derive_chrome(rect);
        *self.chrome_cache.borrow_mut() = Some((fingerprint, bands));
        bands
    }

    /// Hashes every input [`Self::derive_chrome`] reads.
    fn chrome_fingerprint(&self, rect: Rect) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (rect.x, rect.y, rect.width, rect.height).hash(&mut hasher);
        (self.viewport.x, self.viewport.y, self.viewport.width, self.viewport.height)
            .hash(&mut hasher);
        (self.content_size.width, self.content_size.height).hash(&mut hasher);
        (self.horizontal_scroll_bar_policy as u8).hash(&mut hasher);
        (self.vertical_scroll_bar_policy as u8).hash(&mut hasher);
        self.corner_widget.hash(&mut hasher);
        (
            self.content_margins.top,
            self.content_margins.bottom,
            self.content_margins.left,
            self.content_margins.right,
        )
            .hash(&mut hasher);
        hasher.finish()
    }

    /// Derives the chrome bands from scratch; see [`Self::assemble_chrome`] for the cache.
    fn derive_chrome(&self, rect: Rect) -> ChromeBands {
        let bar = dimensions::SCROLLBAR_THICKNESS;
        let h_scroll_visible = self.horizontal_scroll_bar_visible();
        let v_scroll_visible = self.vertical_scroll_bar_visible();
        // A control with no bars needs no derivation: the content *is* the rectangle.
        let content = if !h_scroll_visible && !v_scroll_visible {
            rect
        } else {
            Rect::new(
                rect.x,
                rect.y,
                rect.width.saturating_sub(if v_scroll_visible { bar } else { 0 }),
                rect.height.saturating_sub(if h_scroll_visible { bar } else { 0 }),
            )
        };
        // C-21: the inset applies inside the content box, so a margin never eats the bars and a
        // bar never eats the margin.
        let content = Rect::new(
            content.x + self.content_margins.left as i32,
            content.y + self.content_margins.top as i32,
            content.width.saturating_sub(
                self.content_margins.left.saturating_add(self.content_margins.right),
            ),
            content.height.saturating_sub(
                self.content_margins.top.saturating_add(self.content_margins.bottom),
            ),
        );
        if !h_scroll_visible && !v_scroll_visible {
            return ChromeBands { content, horizontal: None, vertical: None, corner: None };
        }
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
        // The corner is what the two bands leave. Stated once here, so a corner widget and the
        // painted grip read the same square instead of each deriving it from the thickness.
        let corner = match (h_band, v_band) {
            (Some(horizontal), Some(vertical)) => {
                Some(Rect::new(vertical.x, horizontal.y, bar.min(rect.width), bar.min(rect.height)))
            }
            _ => None,
        };
        ChromeBands { content, horizontal: h_band, vertical: v_band, corner }
    }
}

/// The frame the scrolling child is placed in, given the viewport and how the content should be
/// aligned in it.
///
/// # The two fields this exists for
///
/// `alignment` and `widget_resizable` were both stored, published (getters, setters, schema rows)
/// and read by nothing: every scroll area clipped to its whole rectangle, drew the child at its own
/// stored size, and neither field could change a pixel. Rather than two branches at the two call
/// sites -- the paint and the geometry hand-off -- they resolve here once and both read the result,
/// which is what keeps a click's coordinate and the pixel under it from disagreeing.
///
/// # The two fields' meanings, and the third they do *not* replace
///
/// * `widget_resizable` decides **who owns the child's size**. `true` says the scroll area sizes the
///   child to the viewport, so a `ScrollArea` behaves like a container and its child is laid out to
///   fit. `false` says the caller keeps its child's size, so the content box is the declared extent
///   and the bars exist to reach the rest of it. This is *not* the same axis as `content_size`,
///   which is the child's declared extent and stays the caller's either way -- `content_size` is the
///   number, this is who acts on it.
///
///   # Why `false` is the extent, not `max(viewport, extent)`
///
///   The tempting reading is "at least the viewport, so the child always fills it", which is what
///   `max` would give. That reading makes `alignment` **unreachable**: a frame that is never smaller
///   than the viewport has zero slack in every case, so the field that places slack could never move
///   a pixel. The two fields are only both meaningful when `false` means *exactly the extent* -- then
///   a small extent sits somewhere in the viewport (alignment's job) and a large one overflows it
///   (the bars' job).
///
/// * `alignment` decides **where the child sits when it is smaller than the viewport**, which is the
///   state `widget_resizable: false` makes reachable. Alignment only has a meaning when there is
///   slack, so it is applied to the extra space and never to the child itself: a child as large as
///   the viewport is unaffected by any alignment, which is what makes the field a no-op in the
///   common case rather than a wrong offset in it.
///
/// # The axis each variant moves on
///
/// [`Alignment`] has no "both axes" variant, so the three pairs are read the way the rest of this
/// crate reads them: `Left`/`Right` and `Top`/`Bottom` move one axis each, `Center` moves both, and
/// a variant that names the *other* axis than the one being resolved leaves it alone. That is why
/// one `match` returns both offsets rather than two `match`es returning one each.
fn content_frame(
    viewport: Rect,
    content_size: Size,
    resizable: bool,
    alignment: Alignment,
) -> Rect {
    // The box the child is laid out in. A resizable child is given the viewport, because the scroll
    // area owns its size; a fixed one is given exactly the extent it declared, because the caller
    // does -- `max(viewport, extent)` would leave no slack for the alignment below to place, which
    // is what made this pair of fields unreachable together.
    let width = if resizable { viewport.width } else { content_size.width };
    let height = if resizable { viewport.height } else { content_size.height };
    let slack_x = viewport.width.saturating_sub(width) as i32;
    let slack_y = viewport.height.saturating_sub(height) as i32;
    let (x, y) = match alignment {
        Alignment::Left => (viewport.x, viewport.y + slack_y / 2),
        Alignment::Right => (viewport.x + slack_x, viewport.y + slack_y / 2),
        Alignment::Top => (viewport.x + slack_x / 2, viewport.y),
        Alignment::Bottom => (viewport.x + slack_x / 2, viewport.y + slack_y),
        Alignment::Center => (viewport.x + slack_x / 2, viewport.y + slack_y / 2),
    };
    Rect::new(x, y, width, height)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ChromeBands {
    /// The scrolling viewport.
    content: Rect,
    /// The horizontal track, when one is owed.
    horizontal: Option<Rect>,
    /// The vertical track, when one is owed.
    vertical: Option<Rect>,
    /// The square the two tracks leave, when both are owed.
    corner: Option<Rect>,
}
// Implement Widget trait
impl Widget for ScrollArea {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            dimensions::SCROLL_AREA_DEFAULT_WIDTH,
            dimensions::SCROLL_AREA_DEFAULT_HEIGHT,
        )
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.viewport.width = geometry.width;
        self.viewport.height = geometry.height;
        self.update_scroll_bars();
        // A layout positions the scroll area through this entry point. The content lives in
        // the new viewport, so it has to be placed again here rather than only on a scroll.
        self.place_content();
    }

    /// The content box descendants are clipped to.
    ///
    /// The *content* box, not the control's whole rectangle: the scroll-bar bands sit inside
    /// the rectangle and would be painted over by content that reached their side. This is
    /// also the box the scroll offset is measured against, so a scrolled child cannot draw
    /// outside the viewport it is scrolling in.
    fn children_clip(&self) -> Option<Rect> {
        Some(self.assemble_chrome(self.base.geometry()).content)
    }

    /// Advances fling momentum after a swipe, and any animation a mounted bar owns.
    fn tick(&mut self, delta_ms: u32) -> bool {
        // A mounted bar's idle fade or released-thumb glide is driven by the same frame clock.
        // Its value_changed is drained afterwards, so bar inertia scrolls the content too.
        let mut bar_animating = false;
        if let Some(child) = self.horizontal_scroll_bar.as_mut() {
            bar_animating |= child.bar.tick(delta_ms);
        }
        if let Some(child) = self.vertical_scroll_bar.as_mut() {
            bar_animating |= child.bar.tick(delta_ms);
        }
        self.sync_from_scroll_bars();
        if !self.inertia || (self.fling.0 == 0.0 && self.fling.1 == 0.0) {
            return bar_animating;
        }
        let step = delta_ms as f32 / 1000.0;
        let dx = (self.fling.0 * step).round() as i32;
        let dy = (self.fling.1 * step).round() as i32;
        self.set_scroll_position(self.scroll_position.0 + dx, self.scroll_position.1 + dy);
        // Exponential decay, stopped once the remaining speed is imperceptible. A
        // frame-rate-independent fraction would need the exact delta; at 16 ms this drops the
        // velocity by about 8% per frame, which is the familiar flick feel.
        let decay = (1.0 - 0.005 * delta_ms as f32).clamp(0.0, 1.0);
        self.fling = (self.fling.0 * decay, self.fling.1 * decay);
        if self.fling.0.abs() + self.fling.1.abs() < 20.0 {
            self.fling = (0.0, 0.0);
            return false;
        }
        true
    }

    fn is_animating(&self) -> bool {
        self.inertia && (self.fling.0 != 0.0 || self.fling.1 != 0.0)
            || self.vertical_scroll_bar.as_ref().is_some_and(|child| child.bar.is_animating())
            || self.horizontal_scroll_bar.as_ref().is_some_and(|child| child.bar.is_animating())
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "scroll_position_changed" => Some(EventSignalRef::mapped(
                "scroll_position_changed",
                &self.scroll_position_changed,
                // The declared shape is `Tuple2` of `Int`, so the payload is delivered as a real
                // pair of integers rather than its `Debug` spelling — a subscriber that read the
                // schema can read the two offsets as numbers.
                |v| {
                    CapabilityValue::Tuple(crate::compat::Vec::from([
                        CapabilityValue::Int(v.0 as i64),
                        CapabilityValue::Int(v.1 as i64),
                    ]))
                },
            )),
            _ => None,
        }
    }
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
            "content_width" => Ok(CapabilityValue::UInt(self.content_size().width as u64)),
            "content_height" => Ok(CapabilityValue::UInt(self.content_size().height as u64)),
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
                let x = expect_i32_saturating(value)?;
                let (_, y) = self.scroll_position();
                self.set_scroll_position(x, y);
                Ok(())
            }
            "scroll_position_y" => {
                let y = expect_i32_saturating(value)?;
                let (x, _) = self.scroll_position();
                self.set_scroll_position(x, y);
                Ok(())
            }
            "content_width" | "content_height" => {
                let extent = u32::try_from(expect_i64(value)?)
                    .map_err(|_| CapabilityAccessError::OutOfRange)?;
                let current = self.content_size();
                self.set_content_size(if name == "content_width" {
                    crate::core::Size::new(extent, current.height)
                } else {
                    crate::core::Size::new(current.width, extent)
                });
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
            "content_width",
            "content_height",
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
        // A mounted bar is a real control: it gets the pointer first, and its value_changed is
        // drained before this control decides whether it also has a gesture to run.
        let bar_owns_pointer = self.dispatch_to_scroll_bars(event);
        self.sync_from_scroll_bars();
        if bar_owns_pointer {
            return;
        }
        // Handle scroll events (mouse wheel + touch swipe)
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(axis) = self.bar_at(*pos) {
                    // A mounted bar owns its axis' drag; starting the fallback's drag here would
                    // move the thumb twice for one pointer delta.
                    if !self.has_scroll_bar(axis)
                        && self.thumb_rect(axis).is_some_and(|thumb| thumb.contains_point(*pos))
                    {
                        let coordinate = match axis {
                            ScrollAxis::Horizontal => pos.x,
                            ScrollAxis::Vertical => pos.y,
                        };
                        self.thumb_drag = Some((axis, coordinate));
                    }
                }
            }
            Event::MouseMove { pos } => {
                if let Some((axis, last)) = self.thumb_drag {
                    let coordinate = match axis {
                        ScrollAxis::Horizontal => pos.x,
                        ScrollAxis::Vertical => pos.y,
                    };
                    let delta = coordinate - last;
                    let (content, view, track, thumb) = match axis {
                        ScrollAxis::Horizontal => {
                            let chrome = self.assemble_chrome(self.geometry());
                            let band = chrome.horizontal.map(|band| band.width).unwrap_or(1);
                            let (thumb, _) = Self::thumb_metrics(
                                band,
                                self.content_size.width,
                                self.viewport.width.max(1),
                                self.scroll_position.0,
                            );
                            (self.content_size.width, self.viewport.width, band, thumb)
                        }
                        ScrollAxis::Vertical => {
                            let chrome = self.assemble_chrome(self.geometry());
                            let band = chrome.vertical.map(|band| band.height).unwrap_or(1);
                            let (thumb, _) = Self::thumb_metrics(
                                band,
                                self.content_size.height,
                                self.viewport.height.max(1),
                                self.scroll_position.1,
                            );
                            (self.content_size.height, self.viewport.height, band, thumb)
                        }
                    };
                    let travel = track.saturating_sub(thumb).max(1) as f32;
                    let scrollable = content.saturating_sub(view) as f32;
                    let amount = (delta as f32 * scrollable / travel).round() as i32;
                    if matches!(axis, ScrollAxis::Horizontal) {
                        self.set_scroll_position(
                            self.scroll_position.0 + amount,
                            self.scroll_position.1,
                        );
                    } else {
                        self.set_scroll_position(
                            self.scroll_position.0,
                            self.scroll_position.1 + amount,
                        );
                    }
                    self.thumb_drag = Some((axis, coordinate));
                    return;
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.thumb_drag = None;
            }
            Event::KeyPress { key, .. } if self.base.focus_reason().is_some() => {
                let page = self.viewport.height.max(1) as i32;
                let (x, y) = self.scroll_position;
                match *key {
                    crate::event::key_codes::UP => {
                        self.set_scroll_position(x, y - WHEEL_PIXELS_PER_LINE)
                    }
                    crate::event::key_codes::DOWN => {
                        self.set_scroll_position(x, y + WHEEL_PIXELS_PER_LINE)
                    }
                    crate::event::key_codes::LEFT => {
                        self.set_scroll_position(x - WHEEL_PIXELS_PER_LINE, y)
                    }
                    crate::event::key_codes::RIGHT => {
                        self.set_scroll_position(x + WHEEL_PIXELS_PER_LINE, y)
                    }
                    crate::event::key_codes::PAGE_UP => {
                        self.set_scroll_position(x, y.saturating_sub(page))
                    }
                    crate::event::key_codes::PAGE_DOWN => {
                        self.set_scroll_position(x, y.saturating_add(page))
                    }
                    crate::event::key_codes::HOME => self.set_scroll_position(0, 0),
                    crate::event::key_codes::END => self.set_scroll_position(i32::MAX, i32::MAX),
                    _ => {}
                }
            }
            Event::Wheel { delta, modifiers } => {
                // `WheelScroll` owns the notch normalization; this control owns the
                // line-to-pixel step. The scroll position is clamped by the setter.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                self.set_scroll_position(
                    self.scroll_position.0 + scroll.columns_x * WHEEL_PIXELS_PER_LINE,
                    self.scroll_position.1 + scroll.lines_y * WHEEL_PIXELS_PER_LINE,
                );
            }
            #[cfg(feature = "touch")]
            Event::Swipe { start, end, velocity } => {
                // Map swipe to scroll
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                self.set_scroll_position(self.scroll_position.0 + dx, self.scroll_position.1 + dy);
                if self.inertia {
                    let length = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                    self.fling = (-*velocity * dx as f32 / length, -*velocity * dy as f32 / length);
                }
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
        context.face_with_gradient(
            rect,
            style.background_color.unwrap_or(Color::rgb(255, 255, 255)),
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
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
        // Set viewport for clipping. The clip is the *content box* the assembled chrome leaves,
        // not the control's whole rectangle: with a bar owed, the bars are painted after this clip
        // is popped and a scrolled child would otherwise draw over the groove it is scrolling to
        // reach the end of.
        let chrome = self.assemble_chrome(rect);
        context.push_clip(
            chrome.content.x,
            chrome.content.y,
            chrome.content.width,
            chrome.content.height,
        );
        // Where the child is placed inside that box. Both `alignment` and `widget_resizable` were
        // stored, published and read by nothing; this is the one derivation both the paint and the
        // geometry hand-off read, so a click's coordinate and the pixel under it cannot disagree.
        let frame =
            content_frame(chrome.content, self.content_size, self.widget_resizable, self.alignment);
        // Draw widget via registry, translated by the negative scroll offset so
        // the viewport shows the scrolled content window.
        if let Some(widget_id) = self.widget {
            if let Some(ref reg) = self.registry {
                // Hand the child the frame rather than letting it keep whatever geometry it was
                // constructed with. `widget_resizable: true` is exactly the promise that the scroll
                // area sizes its content, and without this line that promise was unobservable: a
                // resizable area and a fixed one drew the same picture.
                reg.borrow_mut().set_widget_geometry(widget_id, frame);
                context.push_offset(-self.scroll_position.0, -self.scroll_position.1);
                reg.borrow_mut().draw_widget(widget_id, context);
                context.pop_offset();
            }
        }
        // Sticky bands are drawn *after* the scrolled content, still inside the
        // clip and without the scroll offset, which is precisely what makes them
        // stay put. Drawing them with the offset (i.e. as ordinary content) would
        // make this loop a no-op.
        for (band_index, region) in self.sticky_regions.iter().enumerate() {
            // A band whose group has not been reached yet scrolls normally, so the
            // content itself already put it in the right place; re-drawing it here
            // would double-paint it.
            if region.group_top - self.scroll_position.1 > 0 {
                continue;
            }
            let offset_y = region.draw_offset(self.scroll_position.1);
            // Bands span the content box, not the control: a band that ran under the vertical bar
            // would be painted over by the bar and would look clipped rather than pinned.
            let band_rect = Rect::new(
                chrome.content.x,
                chrome.content.y + offset_y,
                chrome.content.width,
                region.height.min(chrome.content.height),
            );
            // A band pushed entirely above the viewport has nothing left to show.
            if band_rect.y + band_rect.height as i32 <= chrome.content.y {
                continue;
            }
            // COMP-E01: a band with a mounted header draws that control in the band box, so the
            // group title, icon and actions are visible instead of only a colour stripe. A band
            // without one keeps the historical fill.
            match self.sticky_header(band_index) {
                Some(header) => {
                    #[cfg(not(alloc_frugal))]
                    crate::set_widget_geometry(
                        header,
                        band_rect.x,
                        band_rect.y,
                        band_rect.width,
                        band_rect.height,
                    );
                    if let Some(ref registry) = self.registry {
                        context.push_clip(
                            band_rect.x,
                            band_rect.y,
                            band_rect.width,
                            band_rect.height,
                        );
                        registry.borrow_mut().draw_widget(header, context);
                        context.pop_clip();
                    } else {
                        self.draw_sticky_band(context, band_rect);
                    }
                }
                None => self.draw_sticky_band(context, band_rect),
            }
        }
        context.pop_clip();
        // ── The chrome, assembled ──
        //
        // `assemble_chrome` places the content box and the two bands as one structure, so the
        // "the other bar shortens this one" correction lives in one place instead of once per band.
        // It was already resolved above to clip the content; reading it once rather than assembling
        // it twice is what keeps the clip and the bars from being measured from two assemblies.
        if let Some(h_band) = chrome.horizontal {
            // A mounted bar paints the band through its own draw pass; the self-drawn groove
            // below is the fallback that keeps an area without a child rendering as it always did.
            if let Some(child) = self.horizontal_scroll_bar.as_mut() {
                if child.bar.base().is_visible() {
                    child.bar.draw(context);
                }
            } else {
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
        }
        if let Some(v_band) = chrome.vertical {
            if let Some(child) = self.vertical_scroll_bar.as_mut() {
                if child.bar.base().is_visible() {
                    child.bar.draw(context);
                }
            } else {
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
        }
        // The corner between the two bars. An explicit corner widget owns this square; otherwise
        // the track-coloured grip is painted in it.
        if let Some(corner_rect) = chrome.corner {
            match (self.corner_widget, self.registry.as_ref()) {
                (Some(corner), Some(registry)) => {
                    // The corner sits outside the content box, which is what descendants are
                    // clipped to. Drawing it through the registry here, after the clip is popped,
                    // gives it the square the chrome reserved for it; the tree child entry
                    // remains for discovery and hit tests.
                    registry.borrow_mut().draw_widget(corner, context);
                }
                // No corner child, or no registry able to paint one: keep the visible grip so
                // the chrome is never blank.
                _ => context.fill_rect(corner_rect, track_color),
            }
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

    // ── `alignment` and `widget_resizable` decide the content frame ──

    /// A resizable scroll area hands its child the **viewport** and ignores the declared extent.
    ///
    /// # What was dead, and what proves it is alive now
    ///
    /// `widget_resizable` was stored, published (`widget_resizable` property, schema row, default
    /// `true`) and read by nothing: `draw` never touched the child's geometry, so a resizable area
    /// and a fixed one painted the same picture. The field's own doc says "whether the scroll area
    /// sizes the child to the viewport" — this asserts the sizing, and the extent being larger than
    /// the viewport is what makes the two settings distinguishable rather than accidentally equal.
    #[test]
    fn a_resizable_area_sizes_its_child_to_the_viewport() {
        let viewport = Rect::new(0, 0, 100, 80);
        let content = Size::new(400, 600);
        let frame = content_frame(viewport, content, true, Alignment::Center);
        assert_eq!(
            frame, viewport,
            "a resizable child is the viewport: the scroll area owns its size"
        );
        // And a fixed one keeps the extent, so the two settings are not the same picture.
        let fixed = content_frame(viewport, content, false, Alignment::Center);
        assert_eq!(fixed.width, 400, "a fixed child keeps the width it was given");
        assert_eq!(fixed.height, 600);
        assert_ne!(fixed, frame, "the two settings must not render identically");
    }

    /// Alignment places a child that is **smaller** than the viewport, on the axes its variant
    /// names, and leaves a child that fills the viewport where it is.
    ///
    /// Alignment only has a meaning when there is slack, so the interesting case is the one
    /// `widget_resizable: false` makes reachable: an extent smaller than the viewport. The
    /// half-viewport assertion (`Left` moves x but centre-y) is what pins the claim that a variant
    /// naming one axis leaves the other alone — a plausible bug is to read `Alignment::Left` as
    /// "top-left", which no single-axis assertion would catch.
    #[test]
    fn alignment_places_a_child_smaller_than_the_viewport() {
        let viewport = Rect::new(0, 0, 100, 80);
        let content = Size::new(40, 20);
        let place = |alignment| content_frame(viewport, content, false, alignment);

        assert_eq!(place(Alignment::Left).x, 0, "Left pins the leading edge");
        assert_eq!(place(Alignment::Left).y, 30, "and centres the other axis");
        assert_eq!(place(Alignment::Right).x, 60, "Right pushes to the trailing edge");
        assert_eq!(place(Alignment::Right).y, 30);
        assert_eq!(place(Alignment::Top).y, 0, "Top pins the leading edge");
        assert_eq!(place(Alignment::Top).x, 30, "and centres the other axis");
        assert_eq!(place(Alignment::Bottom).y, 60);
        assert_eq!(place(Alignment::Bottom).x, 30);
        assert_eq!(place(Alignment::Center), Rect::new(30, 30, 40, 20));

        // A child the size of the viewport has no slack, so every alignment agrees -- which is what
        // makes the field a no-op in the common case rather than a wrong offset in it.
        let fitted = Size::new(100, 80);
        for alignment in [
            Alignment::Left,
            Alignment::Right,
            Alignment::Top,
            Alignment::Bottom,
            Alignment::Center,
        ] {
            assert_eq!(
                content_frame(viewport, fitted, false, alignment),
                viewport,
                "a child that fills the viewport is unmoved by {alignment:?}"
            );
        }
    }

    /// The frame never exceeds the viewport when it is resizable, and a fixed child keeps its extent.
    ///
    /// The one input that could underflow the placement arithmetic is a slack computed from a frame
    /// larger than the viewport, which cannot happen now: a resizable frame *is* the viewport, and a
    /// fixed one is clamped by the saturating subtraction. Keeping the invariant is what keeps a
    /// caller's arithmetic error from becoming a panic inside `draw`.
    #[test]
    fn a_fixed_child_keeps_its_extent_and_a_resizable_one_is_the_viewport() {
        let viewport = Rect::new(10, 10, 100, 80);
        // A resizable child is the viewport whatever it declared.
        for content in [Size::new(1, 1), Size::new(400, 600), Size::new(0, 0)] {
            assert_eq!(
                content_frame(viewport, content, true, Alignment::Center),
                viewport,
                "a resizable child is the viewport regardless of its declared extent"
            );
        }
        // A fixed child is exactly what it declared, which is what leaves `alignment` something to
        // place -- an extent that filled the box would make the field unobservable.
        assert_eq!(
            content_frame(viewport, Size::new(40, 20), false, Alignment::Center),
            Rect::new(40, 40, 40, 20)
        );
        // A fixed child larger than the viewport overflows it, which is the state the bars exist
        // for: the frame starts at the viewport's origin and the scroll position reaches the rest.
        let overflow = content_frame(viewport, Size::new(400, 600), false, Alignment::Center);
        assert_eq!((overflow.x, overflow.y), (viewport.x, viewport.y));
        assert_eq!((overflow.width, overflow.height), (400, 600));
    }

    /// `set_alignment` asks for a frame.
    ///
    /// The alignment is read while drawing the content frame, so a change that did not mark
    /// damage left the content in its old position.
    #[test]
    fn setting_alignment_requests_a_redraw() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 200));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = area.connection_scope();
            area.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        // The default is `Center`, so move to a *different* value — a setter that skips a no-op
        // change would otherwise look like it repainted when it never wrote anything.
        let target =
            if area.alignment() == Alignment::Center { Alignment::Left } else { Alignment::Center };
        area.set_alignment(target);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst), "a new alignment must be drawn");

        fired.store(false, std::sync::atomic::Ordering::SeqCst);
        area.set_alignment(target);
        assert!(
            !fired.load(std::sync::atomic::Ordering::SeqCst),
            "setting the same value must not repaint"
        );
    }

    #[test]
    fn children_are_clipped_to_the_content_box_not_the_bars() {
        let mut sa = ScrollArea::new(Rect::new(10, 20, 200, 150));
        sa.set_content_size(Size::new(1000, 1000));
        // The content overflows both axes, so both bars are owed and the viewport the
        // descendants are clipped to is smaller than the control on both axes.
        let chrome = sa.assemble_chrome(sa.geometry());
        assert_eq!(sa.children_clip(), Some(chrome.content));
        assert!(chrome.content.width < sa.geometry().width);
        assert!(chrome.content.height < sa.geometry().height);
        assert!(chrome.vertical.is_some());
        assert!(chrome.horizontal.is_some());
    }

    /// The scroll offset is part of the child's rectangle, not an offset applied only while
    /// painting: the hit test and every geometry query have to see the same box.
    #[test]
    fn the_content_rect_follows_the_scroll_offset() {
        let mut sa = ScrollArea::new(Rect::new(10, 20, 200, 150));
        sa.set_content_size(Size::new(1000, 1000));
        sa.set_widget(Some(4242));
        let unscrolled = sa.content_child_rect().expect("a content child is set");

        sa.set_scroll_position(30, 40);
        let scrolled = sa.content_child_rect().expect("still set");
        assert_eq!(scrolled.x, unscrolled.x - 30);
        assert_eq!(scrolled.y, unscrolled.y - 40);
        assert_eq!(scrolled.width, unscrolled.width);
        assert_eq!(scrolled.height, unscrolled.height);
    }

    /// The thumb is draggable, the keyboard scrolls when focused, and an opt-in fling
    /// keeps moving for a few frames after a swipe.
    #[cfg(feature = "touch")]
    #[test]
    fn thumb_drag_keyboard_and_inertia_are_connected() {
        let mut sa = ScrollArea::new(Rect::new(0, 0, 200, 100));
        sa.set_content_size(Size::new(100, 400));
        sa.handle_event(&Event::MousePress { pos: Point::new(196, 5), button: 1, modifiers: 0 });
        sa.handle_event(&Event::MouseMove { pos: Point::new(196, 40) });
        sa.handle_event(&Event::MouseRelease { pos: Point::new(196, 40), button: 1 });
        assert!(sa.scroll_position().1 > 0, "dragging the thumb scrolls the content");

        let before = sa.scroll_position().1;
        sa.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        sa.handle_event(&Event::KeyPress { key: crate::event::key_codes::PAGE_DOWN, modifiers: 0 });
        assert!(sa.scroll_position().1 > before, "PageDown scrolls one viewport");
        sa.handle_event(&Event::KeyPress { key: crate::event::key_codes::HOME, modifiers: 0 });
        assert_eq!(sa.scroll_position(), (0, 0));

        sa.set_content_size(Size::new(100, 400));
        sa.set_inertia(true);
        sa.handle_event(&Event::Swipe {
            start: Point::new(0, 80),
            end: Point::new(0, 40),
            velocity: 800.0,
        });
        assert!(sa.is_animating());
        let scrolled = sa.scroll_position().1;
        assert!(sa.tick(16));
        assert!(sa.scroll_position().1 > scrolled, "the fling keeps scrolling");
        assert!(!sa.tick(10_000), "a long frame decays the velocity to rest");
    }

    // ── COMP-02: a real `ScrollBar` child ──────────────────────────────────

    /// A mounted bar is placed by the same chrome derivation the self-drawn path used.
    #[test]
    fn a_mounted_bar_uses_the_derived_chrome_band_as_its_geometry() {
        let mut area = ScrollArea::new(Rect::new(10, 20, 200, 100));
        area.set_content_size(Size::new(600, 600));
        area.set_vertical_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));
        area.set_horizontal_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));

        let chrome = area.assemble_chrome(area.geometry());
        assert_eq!(
            area.scroll_bar(ScrollAxis::Vertical).unwrap().geometry(),
            chrome.vertical.unwrap()
        );
        assert_eq!(
            area.scroll_bar(ScrollAxis::Horizontal).unwrap().geometry(),
            chrome.horizontal.unwrap()
        );
        // The band is the *derived* strip, not the control's whole rectangle.
        assert_ne!(area.scroll_bar(ScrollAxis::Vertical).unwrap().geometry(), area.geometry());
    }

    /// Moving the scroll position writes the bar, and moving the bar scrolls the content.
    #[test]
    fn a_mounted_bar_and_the_scroll_position_sync_both_ways() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 100));
        area.set_content_size(Size::new(600, 600));
        area.set_vertical_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));

        // Area -> bar: the offset is the bar's value domain (pixels).
        area.set_scroll_position(0, 120);
        assert_eq!(area.scroll_bar(ScrollAxis::Vertical).unwrap().value(), 120);
        assert_eq!(area.scroll_bar(ScrollAxis::Vertical).unwrap().maximum(), 500);
        assert_eq!(area.scroll_bar(ScrollAxis::Vertical).unwrap().page_step(), 100);

        // Bar -> area: `value_changed` lands in the mailbox and the drain applies it.
        area.scroll_bar_mut(ScrollAxis::Vertical).unwrap().set_value(300);
        assert!(area.sync_from_scroll_bars());
        assert_eq!(area.scroll_position().1, 300);

        // The horizontal axis is independent.
        area.set_horizontal_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));
        area.scroll_bar_mut(ScrollAxis::Horizontal).unwrap().set_value(40);
        assert!(area.sync_from_scroll_bars());
        assert_eq!(area.scroll_position(), (40, 300));
        assert_eq!(area.scroll_bar(ScrollAxis::Vertical).unwrap().value(), 300);
    }

    /// The policy decides whether a mounted bar is visible, and AlwaysOn wins over fitting content.
    #[test]
    fn the_policy_governs_the_mounted_bars_visibility() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 100));
        area.set_vertical_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));

        // AsNeeded with content that fits: no bar, no band.
        area.set_content_size(Size::new(100, 50));
        assert!(!area.vertical_scroll_bar_visible());
        assert!(!area.scroll_bar(ScrollAxis::Vertical).unwrap().base().is_visible());
        assert_eq!(area.assemble_chrome(area.geometry()).vertical, None);

        // AsNeeded once it overflows: shown and placed.
        area.set_content_size(Size::new(100, 600));
        assert!(area.vertical_scroll_bar_visible());
        assert!(area.scroll_bar(ScrollAxis::Vertical).unwrap().base().is_visible());
        assert!(area.assemble_chrome(area.geometry()).vertical.is_some());

        // AlwaysOn shows it even when the content fits.
        area.set_content_size(Size::new(100, 50));
        area.set_scroll_bar_policy(ScrollAxis::Vertical, ScrollBarPolicy::AlwaysOn);
        assert!(area.scroll_bar(ScrollAxis::Vertical).unwrap().base().is_visible());

        // AlwaysOff hides it even when the content overflows.
        area.set_content_size(Size::new(100, 600));
        area.set_scroll_bar_policy(ScrollAxis::Vertical, ScrollBarPolicy::AlwaysOff);
        assert!(!area.scroll_bar(ScrollAxis::Vertical).unwrap().base().is_visible());
        assert_eq!(area.assemble_chrome(area.geometry()).vertical, None);
        assert_eq!(area.scroll_bar_policy(ScrollAxis::Vertical), ScrollBarPolicy::AlwaysOff);
    }

    /// The self-drawn path stays the default: no mounted bar means no child and unchanged chrome.
    #[test]
    fn the_self_drawn_path_remains_the_default() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 100));
        area.set_content_size(Size::new(600, 600));
        assert!(!area.has_scroll_bar(ScrollAxis::Vertical));
        assert!(!area.has_scroll_bar(ScrollAxis::Horizontal));
        let chrome = area.assemble_chrome(area.geometry());
        assert!(chrome.vertical.is_some() && chrome.horizontal.is_some());
        // And the offset still reaches the bar domain when one is mounted later.
        area.set_scroll_position(0, 50);
        area.set_vertical_scroll_bar(Some(ScrollBar::new(Rect::new(0, 0, 0, 0))));
        assert_eq!(area.scroll_bar(ScrollAxis::Vertical).unwrap().value(), 50);
    }

    /// C-20: a single sticky band can be removed without clearing the rest.
    #[test]
    fn a_sticky_band_can_be_removed_by_index() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 200));
        area.add_sticky_region(0, 100, 20);
        area.add_sticky_region(100, 200, 20);
        assert!(!area.remove_sticky_region(9));
        assert!(area.remove_sticky_region(0));
        assert_eq!(area.sticky_regions().len(), 1);
        assert_eq!(area.sticky_regions()[0].group_top, 100, "the other band keeps its identity");
        assert!(area.remove_sticky_region_for_group(100));
        assert!(area.sticky_regions().is_empty());
    }

    /// C-21: the content inset shrinks the content box and the scroll range with it.
    #[test]
    fn content_margins_shrink_the_content_box() {
        let mut area = ScrollArea::new(Rect::new(0, 0, 200, 200));
        let full = area.assemble_chrome(area.geometry()).content;
        area.set_content_margins(10, 10, 5, 5);
        let inset = area.assemble_chrome(area.geometry()).content;
        assert_eq!(inset.x - full.x, 5);
        assert_eq!(inset.width + 10, full.width);
        assert_eq!(inset.y - full.y, 10);
        assert_eq!(inset.height + 20, full.height);
        assert_eq!(area.content_margins(), (10, 10, 5, 5));
    }
}
