// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Carousel — the one paged-content control.
//!
//! # What this control is
//!
//! This crate previously had three controls describing "one page at a time with
//! indicators": `Carousel`, `PagerPageView` and `TileView`. Each held one third of
//! the capability — one could host content but not be navigated, one could be
//! navigated but host nothing, one was a page counter with a dot strip:
//!
//! | | `Carousel` | `PagerPageView` (deleted) | `TileView` (deleted) |
//! |---|---|---|---|
//! | content slots | ❌ `{title, color}` only | ✅ `Box<dyn WidgetAndDraw>` | ❌ `page_count: u32` |
//! | mouse/touch swipe | ❌ | ❌ (events forwarded to the page) | ❌ |
//! | keyboard navigation | ❌ | ✅ arrow keys | ✅ arrow keys |
//! | autoplay + wrap | ❌ | ❌ | ❌ |
//!
//! A caller wanting a carousel therefore had to pick which third to go without, and
//! no single control could host real content *and* be navigated. `Carousel` is the
//! name the library's own `WidgetKind`, factory registration and CSS selector
//! already use, so it absorbed the other two rather than a fourth control being
//! added beside them (a fourth leg, not a fix). **`PagerPageView` and `TileView` have
//! since been deleted outright** rather than kept as deprecated shells: a second and
//! third implementation of one capability is what produces the divergence this
//! merge exists to remove.
//!
//! `Carousel` now holds the union of those columns, plus:
//!
//! | Capability | API |
//! |---|---|
//! | mount a control on a page | [`Carousel::set_page_content`] |
//! | swipe with distance **and** velocity | `swipe_direction_at` (private) |
//! | autoplay, with hover/press/disabled pausing | [`Carousel::set_autoplay`] |
//! | wrap-around at both ends | [`Carousel::set_loop`] |
//! | dot / bar / numeric / hidden indicator, on any edge | [`Carousel::set_indicator_style`], [`Carousel::set_indicator_position`] |
//! | keyboard navigation (left/right arrows) | `EventHandler::handle_event` |
//!
//! [`WidgetAndDraw`] is declared in this module: this
//! is the control that draws through it, so a second declaration site would be a
//! second thing to keep in step for no benefit.
//!
//! # What "beyond the edge" means here
//!
//! With `loop` enabled there is no last page: `next()` from the final page lands on
//! the first, and `previous()` from the first lands on the final. With it disabled
//! the control stops, which is what the previous version did unconditionally.
//! `pages()` and `current_page_title()` report the *visible* page either way.
//!
//! # How a gesture becomes a page change
//!
//! A release pages when **either** rule fires:
//!
//! | Rule | Condition | Why both |
//! |---|---|---|
//! | distance | travel ≥ 18% of the width | a deliberate slow drag |
//! | velocity | release speed ≥ 400 px/s, travel ≥ 2% of the width | a flick, which is how a pointer user pages without dragging most of the way across |
//!
//! The 2% floor is what keeps the velocity rule from firing on pointer jitter: a
//! few pixels of movement in one frame can compute a very large instantaneous
//! speed on an ordinary click.
//!
//! The distance threshold is checked first, so a drag that already qualifies pages
//! regardless of how slowly it was released — the velocity rule only ever *adds*
//! gestures, it never takes one away.

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

// `Instant` is the monotonic clock the gesture velocity is measured against. `std`
// is available in every profile this control compiles in; a `no_std` clock would
// have to be injected, which is not worth a constructor parameter for a value only
// the flick rule reads.
use std::time::Instant;

/// A page's content: a control that can be drawn into the carousel's content area.
///
/// This trait exists because [`Draw`] is not dyn-compatible — a `Box<dyn Widget>`
/// cannot be drawn through it. The blanket implementation below means any type that
/// is both a [`Widget`] and [`Draw`] qualifies, so a caller mounts a control with
/// `Box::new(my_widget)` and nothing else.
///
/// It is declared here, on the control that actually consumes it, rather than in a
/// shared module: the only thing that draws through it is the carousel's page slot,
/// so a second declaration site would be a second thing to keep in step for no
/// benefit (BLUE18 rule #28).
pub trait WidgetAndDraw: Widget {
    /// Draw this widget using the provided render context.
    fn draw_widget(&mut self, context: &mut RenderContext);
}

impl<T: Widget + Draw> WidgetAndDraw for T {
    fn draw_widget(&mut self, context: &mut RenderContext) {
        self.draw(context);
    }
}

/// Data for a single carousel page built from a title and a background color.
///
/// This is the lightweight spelling of a page: a caller that only wants a colored
/// slide with a caption uses [`Carousel::add_page`]. A caller that wants a real
/// control inside uses [`Carousel::set_page_content`] instead, and the two can be
/// mixed — a page's slot is either a title card or a mounted widget.
pub struct CarouselPage {
    /// Display title shown centered on the page.
    pub title: String,
    /// Background color of the page.
    pub color: Color,
    /// Optional child control drawn inside this page's content area.
    ///
    /// `None` means "draw the title card", which is what `add_page` produces.
    content: Option<Box<dyn WidgetAndDraw>>,
}

impl CarouselPage {
    /// Creates a title-only page.
    pub fn new(title: impl Into<String>, color: Color) -> Self {
        Self { title: title.into(), color, content: None }
    }

    /// Returns whether this page has a mounted child control.
    pub fn has_content(&self) -> bool {
        self.content.is_some()
    }

    /// Borrows the page's child control, if one was set.
    pub fn content(&self) -> Option<&dyn WidgetAndDraw> {
        self.content.as_deref().map(|content| content as &dyn WidgetAndDraw)
    }

    /// Mutably borrows the page's child control, if one was set.
    pub fn content_mut(&mut self) -> Option<&mut dyn WidgetAndDraw> {
        match self.content.as_deref_mut() {
            Some(content) => Some(content),
            None => None,
        }
    }
}

/// Where the page indicator sits inside the control's own rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarouselIndicatorStyle {
    /// One dot per page; the active dot is filled.
    #[default]
    Dots,
    /// One bar per page; the active bar is wider.
    Bars,
    /// A `current / total` counter, for page counts where dots stop being
    /// readable.
    ///
    /// Unlike [`Dots`](Self::Dots) and [`Bars`](Self::Bars), this mark does not
    /// scale with the page count at all: past a handful of pages a dot strip is
    /// either wrong (elided, as the per-page drawing path does for dots) or
    /// unreadably dense, and the number is what a reader actually wants.
    Numeric,
    /// No indicator.
    ///
    /// The `None` spelling of the plan's `Dots / Bars / Numeric / None` is
    /// `Hidden` here rather than `None`, because `Option<CarouselIndicatorStyle>`
    /// would make "the caller did not say" indistinguishable from "the caller said
    /// draw nothing" — and the style is published as a property whose `null` must
    /// mean one of those two things, not both.
    Hidden,
}

impl CarouselIndicatorStyle {
    /// The factory spelling of this style, matching `CAROUSEL_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            CarouselIndicatorStyle::Dots => "dots",
            CarouselIndicatorStyle::Bars => "bars",
            CarouselIndicatorStyle::Numeric => "numeric",
            CarouselIndicatorStyle::Hidden => "hidden",
        }
    }

    /// Parses a factory spelling into a style.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "dots" => CarouselIndicatorStyle::Dots,
            "bars" => CarouselIndicatorStyle::Bars,
            "numeric" => CarouselIndicatorStyle::Numeric,
            "hidden" => CarouselIndicatorStyle::Hidden,
            _ => return None,
        })
    }

    /// Returns whether this style draws anything.
    pub fn is_visible(self) -> bool {
        !matches!(self, CarouselIndicatorStyle::Hidden)
    }

    /// Returns whether this style draws one mark per page.
    ///
    /// The per-page styles share the slot layout and the elision rule; the counter
    /// does not, so this is what keeps a single drawing routine from having to
    /// special-case the slot list for one variant at every step.
    pub fn is_per_page(self) -> bool {
        matches!(self, CarouselIndicatorStyle::Dots | CarouselIndicatorStyle::Bars)
    }
}

/// The edge of the control the page indicator is drawn against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CarouselIndicatorPosition {
    /// Along the bottom edge, centered (the familiar case).
    #[default]
    Bottom,
    /// Along the top edge, centered.
    Top,
    /// Along the left edge, centered vertically, dots stacked.
    Left,
    /// Along the right edge, centered vertically, dots stacked.
    Right,
}

impl CarouselIndicatorPosition {
    /// The factory spelling of this position, matching `CAROUSEL_PROPERTIES`.
    pub fn as_str(self) -> &'static str {
        match self {
            CarouselIndicatorPosition::Bottom => "bottom",
            CarouselIndicatorPosition::Top => "top",
            CarouselIndicatorPosition::Left => "left",
            CarouselIndicatorPosition::Right => "right",
        }
    }

    /// Parses a factory spelling into a position.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "bottom" => CarouselIndicatorPosition::Bottom,
            "top" => CarouselIndicatorPosition::Top,
            "left" => CarouselIndicatorPosition::Left,
            "right" => CarouselIndicatorPosition::Right,
            _ => return None,
        })
    }

    /// Returns whether the indicator runs vertically for this position.
    pub fn is_vertical(self) -> bool {
        matches!(self, CarouselIndicatorPosition::Left | CarouselIndicatorPosition::Right)
    }
}

/// How far a drag must travel, as a fraction of the control's width, before
/// releasing it pages forward or back.
///
/// 0.18 keeps a deliberate swipe working while a click-then-drift does not page:
/// a click that slips by a couple of percent of the width is still a click. The
/// plan's acceptance case ("drag past 50% and release lands on the adjacent page")
/// is satisfied with margin, because any travel beyond this threshold pages.
///
/// A drag **faster** than [`FLICK_VELOCITY_PX_PER_SEC`] pages on less travel than
/// this: see [`Carousel::swipe_direction_at`].
const SWIPE_THRESHOLD_FRACTION: f32 = 0.18;

/// The release speed, in logical pixels per second, at which a short drag still
/// pages.
///
/// A flick is how a pointer user pages a carousel without dragging most of the way
/// across it. Judging on distance alone makes every flick below the threshold snap
/// back, which reads as the control ignoring the gesture.
///
/// 400 px/s is roughly a quarter of a screen width per second on a phone — fast
/// enough that it cannot be produced by a click that drifted, slow enough that a
/// deliberate slow drag still has to travel the distance threshold.
const FLICK_VELOCITY_PX_PER_SEC: f32 = 400.0;

/// A drag shorter than this fraction of the width never pages, however fast it was.
///
/// It is the floor that makes the velocity rule safe: a pointer that jitters a few
/// pixels in one frame can compute an enormous instantaneous speed, and on a click
/// that is noise rather than intent. 2% of the width is below any deliberate swipe.
const MIN_FLICK_FRACTION: f32 = 0.02;

/// The thickness reserved for the indicator strip, in logical pixels.
const INDICATOR_STRIP: u32 = 26;

/// The interaction currently in progress, if any.
///
/// Held as a field rather than derived from events so that a `MouseMove` with no
/// matching press (which a backend can deliver after a drag leaves the window)
/// cannot start a swipe.
#[derive(Debug, Clone, Copy, PartialEq)]
enum DragState {
    /// No button is held.
    Idle,
    /// A button went down at this x and the pointer is still within the drag
    /// threshold, so the gesture is still a candidate click.
    Pressed { start_x: i32 },
    /// A pointer moved past the threshold; the gesture is a swipe, and the
    /// content is offset by `offset_x` pixels while it continues.
    ///
    /// `last_x`/`last_moved_at` are the previous sample, kept so the release can
    /// compute a velocity rather than only a total distance (B1-2 asks for both).
    Swiping { start_x: i32, offset_x: i32, last_x: i32, last_moved_at: Option<Instant> },
}

/// Carousel — paged content with swipe, keyboard, autoplay and wrap-around.
pub struct Carousel {
    base: BaseWidget,
    pages: Vec<CarouselPage>,
    current_index: usize,
    /// Whether `next` from the last page wraps to the first, and `previous` from
    /// the first wraps to the last.
    r#loop: bool,
    /// Milliseconds between automatic advances, or `None` when autoplay is off.
    autoplay_interval_ms: Option<u64>,
    /// Pixels advanced per milli-second of autoplay, and the accumulator that
    /// turns elapsed time into page advances without drifting.
    autoplay_elapsed_ms: u64,
    /// Autoplay holds while the pointer is over the control.
    pointer_inside: bool,
    indicator_style: CarouselIndicatorStyle,
    indicator_position: CarouselIndicatorPosition,
    drag: DragState,
    /// Emitted when the current page index changes.
    pub page_changed: Signal1<usize>,
}

impl Carousel {
    /// Creates a new empty Carousel with the given geometry.
    ///
    /// Defaults: no pages, index 0, wrap disabled (matching the previous
    /// behaviour), autoplay off, dot indicator at the bottom, not dragging.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Carousel, geometry, "Carousel"),
            pages: Vec::new(),
            current_index: 0,
            r#loop: false,
            autoplay_interval_ms: None,
            autoplay_elapsed_ms: 0,
            pointer_inside: false,
            indicator_style: CarouselIndicatorStyle::Dots,
            indicator_position: CarouselIndicatorPosition::Bottom,
            drag: DragState::Idle,
            page_changed: Signal1::new(),
        }
    }

    /// Adds a page with the given title and background color.
    ///
    /// Returns the new page's index.
    pub fn add_page(&mut self, title: impl Into<String>, color: Color) -> usize {
        let index = self.pages.len();
        self.pages.push(CarouselPage::new(title, color));
        self.base.request_redraw();
        index
    }

    /// Mounts `content` as the visible control of the page at `index`.
    ///
    /// Returns `false` when `index` names no page, so a caller cannot silently
    /// lose a control into a nonexistent slot. The page keeps its background color
    /// and title; only the content area's drawing changes.
    ///
    /// # Why this exists on `Carousel` rather than a second control
    ///
    /// `PagerPageView` already had the slot but none of the navigation, and
    /// `Carousel` had the navigation but no slot. Splitting them meant a caller
    /// needing both had to build one — which is how a library grows a fourth
    /// near-duplicate instead of completing the one it has.
    pub fn set_page_content(&mut self, index: usize, content: Box<dyn WidgetAndDraw>) -> bool {
        match self.pages.get_mut(index) {
            Some(page) => {
                page.content = Some(content);
                self.base.request_redraw();
                true
            }
            None => false,
        }
    }

    /// Removes the page at `index`, returning it when the index was valid.
    ///
    /// The current index is clamped so removing the visible page shows a
    /// neighbour rather than an out-of-range position.
    pub fn remove_page(&mut self, index: usize) -> Option<CarouselPage> {
        if index >= self.pages.len() {
            return None;
        }
        let removed = self.pages.remove(index);
        let last = self.pages.len().saturating_sub(1);
        if self.current_index > last {
            self.current_index = last;
            self.page_changed.emit(self.current_index);
        }
        self.base.request_redraw();
        Some(removed)
    }

    /// Sets the current page by index. Clamped to valid range.
    /// Emits `page_changed` if the index actually changes.
    pub fn set_current(&mut self, index: usize) {
        let clamped = index.min(self.pages.len().saturating_sub(1));
        if self.current_index != clamped {
            self.current_index = clamped;
            // A manual move restarts the autoplay dwell, so the user does not get
            // an immediate extra advance on top of their own navigation.
            self.autoplay_elapsed_ms = 0;
            self.page_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the current page index.
    pub fn current(&self) -> usize {
        self.current_index
    }

    /// Returns the total number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Navigates to the next page, wrapping when `loop` is set.
    pub fn next(&mut self) {
        if self.pages.len() < 2 {
            return;
        }
        if self.current_index + 1 < self.pages.len() {
            self.set_current(self.current_index + 1);
        } else if self.r#loop {
            self.set_current(0);
        }
    }

    /// Navigates to the previous page, wrapping when `loop` is set.
    pub fn previous(&mut self) {
        if self.pages.len() < 2 {
            return;
        }
        if self.current_index > 0 {
            self.set_current(self.current_index - 1);
        } else if self.r#loop {
            self.set_current(self.pages.len() - 1);
        }
    }

    /// Sets whether navigation wraps around at both ends.
    pub fn set_loop(&mut self, loop_enabled: bool) {
        self.r#loop = loop_enabled;
        self.base.request_redraw();
    }

    /// Returns whether navigation wraps around at both ends.
    pub fn r#loop(&self) -> bool {
        self.r#loop
    }

    /// Enables or disables automatic advancement.
    ///
    /// A zero or sub-millisecond interval is rejected rather than accepted as
    /// "advance as fast as possible", which would be indistinguishable from a
    /// hang. Pass `None` to stop autoplay.
    pub fn set_autoplay(&mut self, interval: Option<core::time::Duration>) {
        self.autoplay_interval_ms = match interval {
            Some(duration) => {
                let ms = duration.as_millis() as u64;
                if ms == 0 {
                    None
                } else {
                    Some(ms)
                }
            }
            None => None,
        };
        self.autoplay_elapsed_ms = 0;
        self.base.request_redraw();
    }

    /// Returns the configured autoplay interval, if autoplay is on.
    pub fn autoplay(&self) -> Option<core::time::Duration> {
        self.autoplay_interval_ms.map(core::time::Duration::from_millis)
    }

    /// Sets the page-indicator style.
    pub fn set_indicator_style(&mut self, style: CarouselIndicatorStyle) {
        self.indicator_style = style;
        self.base.request_redraw();
    }

    /// Returns the page-indicator style.
    pub fn indicator_style(&self) -> CarouselIndicatorStyle {
        self.indicator_style
    }

    /// Sets which edge the page indicator is drawn against.
    pub fn set_indicator_position(&mut self, position: CarouselIndicatorPosition) {
        self.indicator_position = position;
        self.base.request_redraw();
    }

    /// Returns which edge the page indicator is drawn against.
    pub fn indicator_position(&self) -> CarouselIndicatorPosition {
        self.indicator_position
    }

    /// Returns a reference to the current page, if any.
    pub fn current_page(&self) -> Option<&CarouselPage> {
        self.pages.get(self.current_index)
    }

    /// Returns a mutable reference to the current page, if any.
    pub fn current_page_mut(&mut self) -> Option<&mut CarouselPage> {
        self.pages.get_mut(self.current_index)
    }

    /// Returns a reference to all pages.
    pub fn pages(&self) -> &[CarouselPage] {
        &self.pages
    }

    /// Returns the title of the current page, or an empty string when the
    /// carousel has no pages.
    pub fn current_page_title(&self) -> &str {
        self.current_page().map_or("", |page| page.title.as_str())
    }

    /// The area a page's content occupies: the control's rectangle minus the
    /// indicator strip when an indicator is drawn.
    ///
    /// Mirrors `PagerPageView::content_rect` so that a child mounted through
    /// `set_page_content` is laid out the same way wherever it was moved from.
    pub fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        if !self.indicator_drawn() {
            return rect;
        }
        match self.indicator_position {
            CarouselIndicatorPosition::Bottom => {
                Rect::new(rect.x, rect.y, rect.width, rect.height.saturating_sub(INDICATOR_STRIP))
            }
            CarouselIndicatorPosition::Top => Rect::new(
                rect.x,
                rect.y + INDICATOR_STRIP as i32,
                rect.width,
                rect.height.saturating_sub(INDICATOR_STRIP),
            ),
            CarouselIndicatorPosition::Left => Rect::new(
                rect.x + INDICATOR_STRIP as i32,
                rect.y,
                rect.width.saturating_sub(INDICATOR_STRIP),
                rect.height,
            ),
            CarouselIndicatorPosition::Right => {
                Rect::new(rect.x, rect.y, rect.width.saturating_sub(INDICATOR_STRIP), rect.height)
            }
        }
    }

    /// Returns whether the indicator will be painted for the current state.
    fn indicator_drawn(&self) -> bool {
        self.indicator_style.is_visible() && self.pages.len() > 1
    }

    /// Whether autoplay should advance right now.
    ///
    /// Autoplay is suppressed while the pointer is over the control, while a drag
    /// is in progress, when the control is disabled, when it has no pages to
    /// advance to, and when it is not visible — every condition the plan lists.
    /// A single predicate keeps the reasons together instead of scattering them
    /// across the timer arm.
    fn autoplay_should_run(&self) -> bool {
        self.autoplay_interval_ms.is_some()
            && self.pages.len() > 1
            && !self.pointer_inside
            && self.drag == DragState::Idle
            && self.base.is_enabled()
            && self.base.is_visible()
    }

    /// The pixel width a drag must exceed to count as a swipe on this geometry.
    fn swipe_threshold_px(&self) -> i32 {
        let width = self.geometry().width as f32;
        (width * SWIPE_THRESHOLD_FRACTION).max(1.0) as i32
    }
}

/// The release speed of a pointer gesture, in logical pixels per second.
///
/// `last_moved_at` is the instant of the previous sample and `now` is the release,
/// both supplied by the caller so this is a pure function of its inputs — the
/// timing path is testable without waiting on a real clock.
///
/// Returns `0.0` when the two samples are simultaneous, which makes the caller's
/// velocity test fail rather than divide by zero: a gesture with no measurable
/// duration has no measurable speed, and the distance threshold alone then decides.
fn swipe_velocity_px_per_sec(
    last_x: i32,
    release_x: i32,
    last_moved_at: Option<Instant>,
    now: Instant,
    width: f32,
) -> f32 {
    let Some(last_moved_at) = last_moved_at else {
        return 0.0;
    };
    // `checked_duration_since` rather than `elapsed`: `last_moved_at` can postdate
    // `now` only if a caller passes them the wrong way round, and a silent negative
    // duration would flip the sign of the result.
    let Some(elapsed) = now.checked_duration_since(last_moved_at) else {
        return 0.0;
    };
    let seconds = elapsed.as_secs_f32();
    if seconds <= 0.0 {
        return 0.0;
    }
    let travel = (release_x - last_x) as f32;
    // Capped at ten widths per second. The floor matters more than the exact value:
    // a synthetic event stream delivered with no real gap between moves would
    // otherwise compute an enormous speed and turn every tiny drag into a flick. The
    // cap must stay **above** [`FLICK_VELOCITY_PX_PER_SEC`] by a wide margin, or it
    // would clamp genuine flicks down to something the threshold rejects — a 300px
    // wide control dragged at a normal flick speed is well past 400px/s, so a cap of
    // one width per second would make the velocity rule unreachable.
    let cap = width.abs().max(1.0) * 10.0;
    (travel / seconds).clamp(-cap, cap)
}

impl Widget for Carousel {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Carousel`'s property contract.
///
/// Published names: the page position, the page total, the visible page's title,
/// wrap-around, and the autoplay interval — all of which the control answers from
/// its own state, and all of which are settable except the two derived ones.
///
/// The indicator style and position are deliberately *not* published: the plan
/// lists them under B1-4 (drawing) rather than B1-5 (contract), and a property the
/// control cannot round-trip through `set` would be a name that appears in the
/// schema and fails on write. Their Rust setters are the supported surface.
impl WidgetProperties for Carousel {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_index" => Ok(CapabilityValue::UInt(self.current() as u64)),
            "item_count" => Ok(CapabilityValue::UInt(self.page_count() as u64)),
            "current_page_title" => {
                Ok(CapabilityValue::String(self.current_page_title().to_string()))
            }
            "loop" => Ok(CapabilityValue::Bool(self.r#loop())),
            // Autoplay is published in milliseconds, and `null` means "off". A
            // bool plus a separate interval would allow the contradiction "on with
            // no interval", which has no sensible behaviour to implement.
            "autoplay_interval" => Ok(match self.autoplay_interval_ms {
                Some(ms) => CapabilityValue::UInt(ms),
                None => CapabilityValue::Null,
            }),
            "indicator_style" => {
                Ok(CapabilityValue::String(self.indicator_style.as_str().to_string()))
            }
            "indicator_position" => {
                Ok(CapabilityValue::String(self.indicator_position.as_str().to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // `set_current` clamps out-of-range indices to the last page, so an
            // out-of-range write is accepted and lands on the nearest page.
            "current_index" => {
                self.set_current(expect_usize(value)?);
                Ok(())
            }
            "loop" => {
                self.set_loop(expect_bool(value)?);
                Ok(())
            }
            "autoplay_interval" => match value {
                // `null` stops autoplay; a number is the interval in milliseconds.
                CapabilityValue::Null => {
                    self.set_autoplay(None);
                    Ok(())
                }
                other => {
                    let ms = expect_usize(other)? as u64;
                    self.set_autoplay(Some(core::time::Duration::from_millis(ms)));
                    Ok(())
                }
            },
            "indicator_style" => {
                let text = expect_string(value)?;
                let Some(style) = CarouselIndicatorStyle::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_indicator_style(style);
                Ok(())
            }
            "indicator_position" => {
                let text = expect_string(value)?;
                let Some(position) = CarouselIndicatorPosition::from_name(&text) else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                self.set_indicator_position(position);
                Ok(())
            }
            // Derived from the mounted pages.
            "item_count" | "current_page_title" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `CAROUSEL_PROPERTIES`.
        property_names_of![
            "current_index",
            "item_count",
            "current_page_title",
            "loop",
            "autoplay_interval",
            "indicator_style",
            "indicator_position",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for Carousel {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let page_count = self.pages.len();

        if page_count == 0 {
            // Empty carousel — draw a neutral background
            context.fill_rounded_rect(rect, 8, Color::rgba(230, 230, 230, 200));
            return;
        }

        let content_rect = self.content_rect();

        // A drag in progress moves the current page with the pointer and reveals
        // its neighbour, which is what makes a swipe feel like a swipe rather than
        // a pair of click zones.
        let (offset, neighbour) = match self.drag {
            DragState::Swiping { offset_x, .. } => {
                let target = self.neighbour_for_offset(offset_x);
                (offset_x, target)
            }
            _ => (0, None),
        };

        if let Some(neighbour_index) = neighbour {
            // Neighbour first, shifted so it enters from the correct side, then the
            // current page on top of it. Both are clipped to the content area so a
            // half-dragged page cannot paint over the indicator strip.
            let neighbour_x = (content_rect.x as f32
                + offset as f32
                + if offset > 0 { -(rect.width as f32) } else { rect.width as f32 })
                as i32;
            let neighbour_rect =
                Rect::new(neighbour_x, content_rect.y, content_rect.width, content_rect.height);
            self.draw_page(context, neighbour_index, neighbour_rect, is_enabled);
        }

        let current_rect = Rect::new(
            content_rect.x + offset,
            content_rect.y,
            content_rect.width,
            content_rect.height,
        );
        self.draw_page(context, self.current_index, current_rect, is_enabled);

        if self.indicator_drawn() {
            self.draw_indicator(context, rect);
        }
    }
}

impl Carousel {
    /// The page that would become current if the drag ended at `offset_x`.
    ///
    /// Returns `None` when the drag has not travelled far enough to reveal a
    /// neighbour, so a small drag does not paint the next page at all.
    fn neighbour_for_offset(&self, offset_x: i32) -> Option<usize> {
        if offset_x == 0 || self.swipe_direction(offset_x).is_none() {
            return None;
        }
        self.step_for_offset(offset_x)
    }

    /// Which way a released drag of `offset_x` pages: `Some(-1)` backwards,
    /// `Some(1)` forwards, `None` when it snaps back.
    ///
    /// Direction is by sign of the offset: dragging content rightwards reveals
    /// what is to the left, so it pages backwards. The threshold is applied to the
    /// absolute travel, which makes the rule identical in both directions.
    fn swipe_direction(&self, offset_x: i32) -> Option<i32> {
        if offset_x.abs() < self.swipe_threshold_px() {
            return None;
        }
        Some(if offset_x > 0 { -1 } else { 1 })
    }

    /// Which way a released drag pages, given both its travel and its release
    /// speed: either the distance threshold or a flick is enough.
    ///
    /// This is the rule the release actually uses (B1-2 asks for "displacement
    /// threshold **and** velocity"). Splitting it from [`Self::swipe_direction`]
    /// keeps the distance-only form for a caller that has no timing — a backend
    /// that delivers a whole gesture at once, or a test that drives events without
    /// a clock — while the interactive path can add the speed term.
    fn swipe_direction_at(&self, offset_x: i32, velocity_px_per_sec: f32) -> Option<i32> {
        if let Some(step) = self.swipe_direction(offset_x) {
            return Some(step);
        }
        // A flick: fast enough on release, and past the anti-jitter floor. The sign
        // is taken from the velocity rather than the offset because a flick can
        // reverse direction in its final frame, and it is the release motion the
        // user means.
        let floor_px = self.min_flick_px();
        if velocity_px_per_sec.abs() >= FLICK_VELOCITY_PX_PER_SEC && offset_x.abs() >= floor_px {
            return Some(if velocity_px_per_sec > 0.0 { -1 } else { 1 });
        }
        None
    }

    /// The smallest travel, in pixels, that can ever count as a flick.
    fn min_flick_px(&self) -> i32 {
        (self.geometry().width as f32 * MIN_FLICK_FRACTION).max(1.0) as i32
    }

    /// The page index `step` away from the current one, honoring `loop`.
    fn wrapped_step(&self, step: i32) -> Option<usize> {
        if self.pages.len() < 2 {
            return None;
        }
        let count = self.pages.len() as i32;
        let candidate = self.current_index as i32 + step;
        if candidate < 0 {
            return if self.r#loop { Some((count - 1) as usize) } else { None };
        }
        if candidate >= count {
            return if self.r#loop { Some(0) } else { None };
        }
        Some(candidate as usize)
    }

    /// The page revealed by a drag of `offset_x`, if any.
    fn step_for_offset(&self, offset_x: i32) -> Option<usize> {
        let step = if offset_x > 0 { -1 } else { 1 };
        self.wrapped_step(step)
    }

    /// Draws one page, either its mounted control or its title card.
    fn draw_page(
        &mut self,
        context: &mut RenderContext,
        index: usize,
        page_rect: Rect,
        is_enabled: bool,
    ) {
        let (color, title, has_content) = {
            let Some(page) = self.pages.get(index) else {
                return;
            };
            (page.color, page.title.clone(), page.content.is_some())
        };

        let bg_color = if !is_enabled {
            Color::rgba(
                color.r.saturating_sub(40),
                color.g.saturating_sub(40),
                color.b.saturating_sub(40),
                160,
            )
        } else {
            color
        };
        context.fill_rounded_rect(page_rect, 8, bg_color);

        if has_content {
            // Lay the child out to the page rectangle before drawing, so a page
            // swapped in by a swipe is positioned by the same rule as the visible
            // one rather than keeping whatever geometry it last had.
            if let Some(page) = self.pages.get_mut(index) {
                if let Some(content) = page.content_mut() {
                    content.set_geometry(page_rect);
                    content.set_enabled(is_enabled);
                    context.push_clip(page_rect.x, page_rect.y, page_rect.width, page_rect.height);
                    content.draw_widget(context);
                    context.pop_clip();
                }
            }
            return;
        }

        // ── Title text (centered) ───────────────────────────────
        let font = crate::core::Font::with_weight("Arial", 18.0, 600, false);
        let metrics = context.measure_text(&title, &font);
        let text_x = page_rect.x + (page_rect.width as i32 - metrics.width as i32) / 2;
        let text_y = page_rect.y + (page_rect.height as i32 / 2) - (metrics.height as i32 / 2)
            + metrics.ascent as i32;
        let text_color = if !is_enabled { Color::rgba(255, 255, 255, 160) } else { Color::WHITE };
        context.draw_text(
            Point::new(text_x, text_y),
            &title,
            &font,
            text_color,
            HorizontalAlignment::Left,
        );
    }

    /// Paints the page indicator along its configured edge.
    ///
    /// Dots and bars share the layout rules (centered on the edge, one slot per
    /// page, capped with an ellipsis past twenty pages); only the mark differs, so
    /// the two are one function with a mark-drawing branch rather than two
    /// functions that would drift apart. [`Numeric`](CarouselIndicatorStyle::Numeric)
    /// shares the edge arithmetic and the strip reservation but not the slot list,
    /// because a counter has one mark however many pages exist — so it branches off
    /// before the slots are built rather than inside the loop.
    fn draw_indicator(&self, context: &mut RenderContext, rect: Rect) {
        const MAX_VISIBLE_SLOTS: usize = 20;
        const SIDE_SLOTS: usize = 9;
        const SLOT_SPACING: i32 = 16;
        const DOT_RADIUS: u32 = 4;
        const BAR_WIDTH: u32 = 10;
        const BAR_HEIGHT: u32 = 3;

        let page_count = self.pages.len();
        let vertical = self.indicator_position.is_vertical();

        // The slot the strip is centred on, and the cross-axis coordinate of the
        // strip's centre. Both edges share this arithmetic; only the axis differs.
        let (strip_center, cross_center) = match self.indicator_position {
            CarouselIndicatorPosition::Bottom => (
                rect.x + rect.width as i32 / 2,
                rect.y + rect.height as i32 - (INDICATOR_STRIP as i32 / 2),
            ),
            CarouselIndicatorPosition::Top => {
                (rect.x + rect.width as i32 / 2, rect.y + INDICATOR_STRIP as i32 / 2)
            }
            CarouselIndicatorPosition::Left => {
                (rect.y + rect.height as i32 / 2, rect.x + INDICATOR_STRIP as i32 / 2)
            }
            CarouselIndicatorPosition::Right => (
                rect.y + rect.height as i32 / 2,
                rect.x + rect.width as i32 - (INDICATOR_STRIP as i32 / 2),
            ),
        };

        if !self.indicator_style.is_per_page() {
            // `Numeric` (the only remaining visible style): `current / total`, one
            // based, because that is how a reader counts pages. Drawn centered on the
            // strip with no slot list, so it stays legible at any page count.
            let label = format!("{}/{}", self.current_index + 1, page_count);
            let font = crate::core::Font::simple("Sans", 12.0);
            let metrics = context.measure_text(&label, &font);
            let (text_x, text_y) = if vertical {
                // A stacked counter on the side edge reads bottom-to-top, which is
                // not worth the readability cost of a rotated glyph run; centred on
                // the strip's axis is the readable choice for a narrow band.
                (
                    cross_center - metrics.width as i32 / 2,
                    strip_center - metrics.height as i32 / 2 + metrics.ascent as i32,
                )
            } else {
                (
                    strip_center - metrics.width as i32 / 2,
                    cross_center - metrics.height as i32 / 2 + metrics.ascent as i32,
                )
            };
            context.draw_text(
                Point::new(text_x, text_y),
                &label,
                &font,
                Color::WHITE,
                HorizontalAlignment::Left,
            );
            return;
        }

        // Past twenty pages the far end is elided, so the strip stays the same
        // width however many pages exist — the same cap `PagerPageView` used.
        let slots: Vec<Option<usize>> = if page_count <= MAX_VISIBLE_SLOTS {
            (0..page_count).map(Some).collect()
        } else {
            let mut slots: Vec<Option<usize>> = (0..SIDE_SLOTS).map(Some).collect();
            slots.push(None); // ellipsis marker
            for i in (page_count - (MAX_VISIBLE_SLOTS - SIDE_SLOTS - 1))..page_count {
                slots.push(Some(i));
            }
            slots
        };

        let count = slots.len() as i32;
        let span = count * SLOT_SPACING;

        for (i, slot) in slots.iter().enumerate() {
            let along = strip_center - span / 2 + (i as i32) * SLOT_SPACING + SLOT_SPACING / 2;
            let active = *slot == Some(self.current_index);
            let Some(_) = slot else {
                // Ellipsis marker: a smaller, dimmer mark of the same shape.
                let mark = if vertical {
                    Rect::new(cross_center - 1, along - 1, 2, 2)
                } else {
                    Rect::new(along - 1, cross_center - 1, 2, 2)
                };
                context.fill_rounded_rect(mark, 1, Color::rgba(255, 255, 255, 60));
                continue;
            };

            let marker_rect: Rect = match self.indicator_style {
                CarouselIndicatorStyle::Dots => {
                    let size = DOT_RADIUS as i32 * 2;
                    if vertical {
                        Rect::new(cross_center, along - DOT_RADIUS as i32, size as u32, size as u32)
                    } else {
                        Rect::new(along - DOT_RADIUS as i32, cross_center, size as u32, size as u32)
                    }
                }
                CarouselIndicatorStyle::Bars => {
                    let (w, h) =
                        if vertical { (BAR_HEIGHT, BAR_WIDTH) } else { (BAR_WIDTH, BAR_HEIGHT) };
                    Rect::new(cross_center - (w as i32 / 2), along - (h as i32 / 2), w, h)
                }
                // Both are handled before the slot loop: `Hidden` draws nothing, and
                // `Numeric` draws one counter rather than one mark per page.
                CarouselIndicatorStyle::Numeric | CarouselIndicatorStyle::Hidden => continue,
            };

            let mark_color = if active { Color::WHITE } else { Color::rgba(255, 255, 255, 120) };
            context.fill_rounded_rect(marker_rect, 4, mark_color);

            if active {
                // The active mark gains a halo, so the current page is legible
                // even where the page colour is close to white.
                let halo = Rect::new(
                    marker_rect.x - 1,
                    marker_rect.y - 1,
                    marker_rect.width + 2,
                    marker_rect.height + 2,
                );
                context.draw_rounded_rect_stroke(halo, 5, Color::rgba(255, 255, 255, 80), 1);
            }
        }
    }
}

impl EventHandler for Carousel {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.drag = DragState::Pressed { start_x: pos.x };
                self.base.handle_event(event);
            }
            Event::MouseMove { pos } => match self.drag {
                DragState::Pressed { start_x } => {
                    let offset = pos.x - start_x;
                    if offset.abs() >= self.swipe_threshold_px() {
                        self.drag = DragState::Swiping {
                            start_x,
                            offset_x: offset,
                            last_x: pos.x,
                            last_moved_at: Some(Instant::now()),
                        };
                        self.base.request_redraw();
                    }
                }
                DragState::Swiping { start_x, .. } => {
                    // The offset tracks the pointer for the whole gesture; it is
                    // not clamped here, because the drawing step clips it and a
                    // spring-back animation would need the true value.
                    self.drag = DragState::Swiping {
                        start_x,
                        offset_x: pos.x - start_x,
                        last_x: pos.x,
                        last_moved_at: Some(Instant::now()),
                    };
                    self.base.request_redraw();
                }
                DragState::Idle => {}
            },
            Event::MouseRelease { pos, button } if *button == 1 => {
                let previous_drag = self.drag;
                self.drag = DragState::Idle;

                match previous_drag {
                    // A swipe: page, then snap. The snap is immediate rather than
                    // animated, so a release always ends with the content at rest
                    // — an animation would leave the control mid-offset wherever
                    // no frame clock is driving it.
                    DragState::Swiping { offset_x, last_x, last_moved_at, .. } => {
                        let velocity = swipe_velocity_px_per_sec(
                            last_x,
                            pos.x,
                            last_moved_at,
                            Instant::now(),
                            self.geometry().width as f32,
                        );
                        match self.swipe_direction_at(offset_x, velocity) {
                            Some(step) => {
                                if let Some(target) = self.wrapped_step(step) {
                                    self.set_current(target);
                                }
                            }
                            // Below both thresholds on release: the drag was a slip,
                            // so it is treated as a click at the release position.
                            None => self.page_for_click(pos.x),
                        }
                        self.base.request_redraw();
                    }
                    // A press and release with no swipe: the original click-to-page
                    // behaviour, kept because a carousel is often driven by tapping
                    // its edges.
                    DragState::Pressed { .. } | DragState::Idle => {
                        self.page_for_click(pos.x);
                        self.base.request_redraw();
                    }
                }
                self.base.handle_event(event);
            }
            // Touch swipe. `Event::Swipe` carries the gesture endpoints, so the
            // direction is read from those directly rather than from accumulated
            // drag state — a touch backend may deliver the whole gesture at once.
            #[cfg(feature = "touch")]
            Event::Swipe { start, end, .. } => {
                let travel = end.x - start.x;
                if let Some(step) = self.swipe_direction(travel) {
                    if let Some(target) = self.wrapped_step(step) {
                        self.set_current(target);
                    }
                }
            }
            Event::KeyPress { key, .. } | Event::KeyDown((key, _)) => match *key {
                37 => self.previous(), // Left arrow
                39 => self.next(),     // Right arrow
                _ => {
                    self.base.handle_event(event);
                }
            },
            Event::Timer { .. } => {
                self.advance_autoplay();
            }
            Event::MouseEnter { .. } => {
                self.pointer_inside = true;
                self.base.handle_event(event);
            }
            Event::MouseLeave { .. } => {
                self.pointer_inside = false;
                self.base.handle_event(event);
            }
            _ => {
                // Forward everything else to the visible page, so a control mounted
                // through `set_page_content` receives the events it needs. This is
                // the behaviour `PagerPageView` had and the reason a carousel with
                // real content is only usable once both halves live here.
                self.forward_to_current_page(event);
                self.base.handle_event(event);
            }
        }
    }
}

impl Carousel {
    /// Applies the click-to-page rule: the left half goes back, the right half
    /// forward.
    fn page_for_click(&mut self, x: i32) {
        let rect = self.geometry();
        let mid_x = rect.x + (rect.width as i32) / 2;
        if x < mid_x {
            self.previous();
        } else {
            self.next();
        }
    }

    /// Delivers `event` to the visible page's mounted control, if it has one.
    fn forward_to_current_page(&mut self, event: &Event) {
        let content_rect = self.content_rect();
        // Pointer-bearing events are delivered only when they land inside the page
        // area; the rest (key, focus, tick) go through unconditionally, because a
        // control that owns the keyboard should receive it wherever the pointer is.
        // The guard is written into the pattern rather than as a nested `if`, so the
        // "outside the page area → drop" rule and the event set it applies to are one
        // statement.
        match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos }
            | Event::MouseDoubleClick { pos, .. }
                if !content_rect.contains(*pos) =>
            {
                return;
            }
            _ => {}
        }
        if let Some(page) = self.pages.get_mut(self.current_index) {
            if let Some(content) = page.content_mut() {
                content.set_geometry(content_rect);
                content.handle_event(event);
            }
        }
    }

    /// Advances autoplay by one tick, if it is running.
    ///
    /// The accumulator is time-based rather than tick-based so that a backend
    /// delivering ticks at an irregular rate still advances at the configured
    /// interval. `set_current` clears the accumulator, so a manual move does not
    /// cause an immediate extra advance.
    ///
    /// Public because a driver that delivers `Event::Timer` itself (rather than
    /// routing it through `handle_event`) needs to call it; it is otherwise the
    /// `Event::Timer` arm's body.
    pub fn advance_autoplay(&mut self) {
        if !self.autoplay_should_run() {
            return;
        }
        let Some(interval) = self.autoplay_interval_ms else {
            return;
        };
        // One tick is one interval's worth of time here; a backend with a real
        // clock can call this per elapsed millisecond via `advance_autoplay_by`.
        self.autoplay_elapsed_ms = self.autoplay_elapsed_ms.saturating_add(interval);
        if self.autoplay_elapsed_ms < interval {
            return;
        }
        self.autoplay_elapsed_ms = 0;
        // Autoplay always advances forward, and wraps when loop is on; with loop
        // off it stops at the last page (no advance), matching `next`.
        if self.current_index + 1 < self.pages.len() {
            self.set_current(self.current_index + 1);
        } else if self.r#loop {
            self.set_current(0);
        }
    }

    /// Advances autoplay by an explicit elapsed time.
    ///
    /// The time-parameterised form of [`Self::advance_autoplay`], for a driver
    /// that knows how long it has been since the last frame rather than how many
    /// ticks it has delivered.
    pub fn advance_autoplay_by(&mut self, elapsed: core::time::Duration) {
        if !self.autoplay_should_run() {
            return;
        }
        self.autoplay_elapsed_ms =
            self.autoplay_elapsed_ms.saturating_add(elapsed.as_millis() as u64);
        let Some(interval) = self.autoplay_interval_ms else {
            return;
        };
        if self.autoplay_elapsed_ms < interval {
            return;
        }
        self.autoplay_elapsed_ms = 0;
        if self.current_index + 1 < self.pages.len() {
            self.set_current(self.current_index + 1);
        } else if self.r#loop {
            self.set_current(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Point, Size};
    use crate::render::{PaintBackend, SoftwarePaintBackend};
    use crate::widget::base_widgets::label::Label;
    use std::sync::{Arc, Mutex};

    fn default_carousel() -> Carousel {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Page 1", Color::rgb(52, 120, 246));
        c.add_page("Page 2", Color::rgb(52, 199, 89));
        c.add_page("Page 3", Color::rgb(255, 149, 0));
        c
    }

    /// Renders `widget` into a software frame and returns the RGBA pixels.
    fn render_rgba(widget: &mut Carousel, size: Size) -> Vec<u8> {
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut context = RenderContext::new(&mut backend);
        widget.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The colour at `(x, y)` in an RGBA buffer `width` pixels wide.
    fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let index = ((y * width + x) * 4) as usize;
        (rgba[index], rgba[index + 1], rgba[index + 2], rgba[index + 3])
    }

    #[test]
    fn carousel_creation_defaults() {
        let c = Carousel::new(Rect::new(0, 0, 300, 200));
        assert_eq!(c.current(), 0);
        assert_eq!(c.page_count(), 0);
        assert!(c.current_page().is_none());
        assert_eq!(c.kind(), WidgetKind::Carousel);
        // Wrap-around was previously unconditional-off; that stays the default.
        assert!(!c.r#loop());
        assert!(c.autoplay().is_none());
        assert_eq!(c.indicator_style(), CarouselIndicatorStyle::Dots);
        assert_eq!(c.indicator_position(), CarouselIndicatorPosition::Bottom);
    }

    #[test]
    fn carousel_add_pages() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        assert_eq!(c.page_count(), 0);

        c.add_page("Intro", Color::rgb(100, 100, 200));
        assert_eq!(c.page_count(), 1);

        c.add_page("Details", Color::rgb(200, 100, 100));
        assert_eq!(c.page_count(), 2);

        assert_eq!(c.current(), 0);
        assert_eq!(c.current_page().unwrap().title, "Intro");
    }

    #[test]
    fn carousel_navigation() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);

        c.next();
        assert_eq!(c.current(), 1);

        c.next();
        assert_eq!(c.current(), 2);

        // Should not advance past last page when loop is off
        c.next();
        assert_eq!(c.current(), 2);

        c.previous();
        assert_eq!(c.current(), 1);

        c.previous();
        assert_eq!(c.current(), 0);

        // Should not go before first page when loop is off
        c.previous();
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_set_current_clamps() {
        let mut c = default_carousel();
        c.set_current(5); // beyond range
        assert_eq!(c.current(), 2);
    }

    #[test]
    fn carousel_signal_emission() {
        let mut c = default_carousel();
        let captured = Arc::new(Mutex::new(None));
        c.page_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        c.next();
        assert_eq!(c.current(), 1);
        assert_eq!(*captured.lock().unwrap(), Some(1));

        // No signal for same index
        c.set_current(1);
        assert_eq!(*captured.lock().unwrap(), Some(1)); // unchanged

        c.previous();
        assert_eq!(*captured.lock().unwrap(), Some(0));
    }

    #[test]
    fn carousel_mouse_press_navigates() {
        let mut c = default_carousel();
        // Click left half → previous (but at 0, so stays)
        c.handle_event(&Event::MouseRelease { pos: Point::new(50, 100), button: 1 });
        assert_eq!(c.current(), 0);

        // Click right half → next
        c.handle_event(&Event::MouseRelease { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 1);
    }

    /// The previous version of this test sent a `MousePress`, while the paging
    /// logic read `MouseRelease` (`carousel.rs:273` before this change). It passed
    /// for the wrong reason: no event could have changed the index. Removing the
    /// `is_enabled` guard left it green, which is what exposed the vacuity.
    ///
    /// This version sends the event the control actually acts on, and asserts the
    /// guard by removing it: with the guard gone the index moves, so the assertion
    /// here fails.
    #[test]
    fn carousel_disabled_blocks_events() {
        let mut c = default_carousel();
        c.set_enabled(false);

        // The event that would page when enabled.
        c.handle_event(&Event::MouseRelease { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 0, "a disabled carousel must not page on release");

        // Arrow keys, the other paging path.
        c.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(c.current(), 0, "a disabled carousel must not page on right arrow");

        // Swipe, the third paging path.
        c.handle_event(&Event::MousePress { pos: Point::new(280, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(200, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(200, 100), button: 1 });
        assert_eq!(c.current(), 0, "a disabled carousel must not page on a swipe");
    }

    #[test]
    fn carousel_add_page_returns_index() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        let idx0 = c.add_page("A", Color::RED);
        let idx1 = c.add_page("B", Color::GREEN);
        let idx2 = c.add_page("C", Color::BLUE);
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(idx2, 2);
        assert_eq!(c.page_count(), 3);
    }

    #[test]
    fn carousel_empty_draw_does_not_panic() {
        let mut c = Carousel::new(Rect::new(0, 0, 100, 50));
        let svg = crate::widget::svg::render_to_svg(&mut c);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn carousel_svg_output() {
        let mut c = default_carousel();
        let svg = crate::widget::svg::render_to_svg(&mut c);
        assert!(svg.starts_with("<svg"));
        // Should contain the page title
        assert!(svg.contains("Page 1") || svg.contains("rect") || svg.contains("fill="));
    }

    #[test]
    fn carousel_previous_at_zero_does_nothing() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);
        c.previous();
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_next_at_last_does_nothing() {
        let mut c = default_carousel();
        c.set_current(2);
        assert_eq!(c.current(), 2);
        c.next();
        assert_eq!(c.current(), 2);
    }

    #[test]
    fn carousel_pages_returns_all() {
        let c = default_carousel();
        let pages = c.pages();
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].title, "Page 1");
        assert_eq!(pages[1].title, "Page 2");
        assert_eq!(pages[2].title, "Page 3");
    }

    // ── B1-1: content slots ─────────────────────────────────────────────────

    #[test]
    fn carousel_page_content_is_stored_and_laid_out() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Host", Color::WHITE);
        let child = Label::new("Inside".to_string(), Rect::new(0, 0, 10, 10));
        assert!(c.set_page_content(0, Box::new(child)));
        assert!(c.pages()[0].has_content());

        // Drawing lays the child out to the page's content rectangle, so switching
        // pages must not leave the child where it was.
        let _ = render_rgba(&mut c, Size::new(300, 200));
        let laid_out = c.pages()[0].content().unwrap().geometry();
        assert_eq!(laid_out.width, c.content_rect().width);
        assert_eq!(laid_out.height, c.content_rect().height);
        assert_eq!(laid_out.x, c.content_rect().x);
    }

    #[test]
    fn carousel_set_page_content_rejects_unknown_index() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Only", Color::WHITE);
        let child = Label::new("Lost".to_string(), Rect::new(0, 0, 10, 10));
        // A caller that names no page must be told, not silently lose the control.
        assert!(!c.set_page_content(7, Box::new(child)));
    }

    #[test]
    fn carousel_remove_page_clamps_current() {
        let mut c = default_carousel();
        c.set_current(2);
        let removed = c.remove_page(2);
        assert!(removed.is_some());
        assert_eq!(c.page_count(), 2);
        assert_eq!(c.current(), 1, "removing the visible page shows the neighbour");
        assert!(c.remove_page(9).is_none());
    }

    // ── B1-2: swipe paging ──────────────────────────────────────────────────

    #[test]
    fn carousel_swipe_past_half_width_lands_on_adjacent_page() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);

        // Press at x=280, drag left well past 50% of the 300px width, release.
        c.handle_event(&Event::MousePress { pos: Point::new(280, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(100, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(100, 100), button: 1 });

        assert_eq!(c.current(), 1, "a leftward swipe of 180px must advance one page");
    }

    #[test]
    fn carousel_swipe_backwards_lands_on_previous_page() {
        let mut c = default_carousel();
        c.set_current(2);

        c.handle_event(&Event::MousePress { pos: Point::new(20, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(200, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(200, 100), button: 1 });

        assert_eq!(c.current(), 1, "a rightward swipe must go back one page");
    }

    #[test]
    fn carousel_drag_below_threshold_does_not_page_by_swipe() {
        let mut c = default_carousel();
        // 20px of travel on a 300px control is under the 18% (54px) threshold.
        c.handle_event(&Event::MousePress { pos: Point::new(100, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(80, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(80, 100), button: 1 });

        // Release at x=80 is in the left half, so the click rule applies and the
        // control is already at page 0 — the point is that the *swipe* did not fire.
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_swipe_without_loop_stops_at_edge() {
        let mut c = default_carousel();
        assert!(!c.r#loop());
        assert_eq!(c.current(), 0);

        // Swiping right at page 0 would go to -1, which must be refused.
        c.handle_event(&Event::MousePress { pos: Point::new(20, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(250, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_swipe_with_loop_wraps_at_edge() {
        let mut c = default_carousel();
        c.set_loop(true);
        c.set_current(2);

        // Swiping left from the last page wraps to the first.
        c.handle_event(&Event::MousePress { pos: Point::new(280, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(100, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(100, 100), button: 1 });
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_mousedown_then_mouseup_is_a_click_not_a_drag() {
        let mut c = default_carousel();
        c.handle_event(&Event::MousePress { pos: Point::new(250, 100), button: 1 });
        c.handle_event(&Event::MouseRelease { pos: Point::new(250, 100), button: 1 });
        assert_eq!(c.current(), 1, "a press and release with no travel is a click");
    }

    // ── B1-2: the velocity half of the release rule ─────────────────────────

    #[test]
    fn carousel_distance_alone_still_pages_regardless_of_speed() {
        let c = default_carousel();
        // Past 18% of 300px = 54px, a drag pages even at zero measured speed.
        // Sign follows the existing rule: content dragged leftwards reveals the page
        // to the right, so a negative offset steps forward.
        assert_eq!(c.swipe_direction_at(-60, 0.0), Some(1));
        assert_eq!(c.swipe_direction_at(60, 0.0), Some(-1));
        // And below the distance threshold with no speed it snaps back.
        assert_eq!(c.swipe_direction_at(10, 0.0), None);
    }

    #[test]
    fn carousel_a_fast_flick_pages_below_the_distance_threshold() {
        let c = default_carousel();
        // 20px is under the 54px distance threshold, but 800px/s is a flick.
        assert_eq!(c.swipe_direction_at(-20, -800.0), Some(1));
        assert_eq!(c.swipe_direction_at(20, 800.0), Some(-1));
        // A slow drag of the same distance is still a slip.
        assert_eq!(c.swipe_direction_at(-20, -100.0), None);
    }

    #[test]
    fn carousel_a_flick_below_the_anti_jitter_floor_never_pages() {
        let c = default_carousel();
        // 2% of 300px = 6px floor. One pixel at absurd speed is noise, not intent.
        assert_eq!(c.swipe_direction_at(-1, -100_000.0), None);
        assert_eq!(c.swipe_direction_at(1, 100_000.0), None);
        assert_eq!(c.min_flick_px(), 6);
        // At the floor it counts.
        assert_eq!(c.swipe_direction_at(-6, -500.0), Some(1));
    }

    #[test]
    fn carousel_velocity_is_zero_without_two_timed_samples() {
        let now = Instant::now();
        // One sample has no duration, so it has no speed: the distance rule alone
        // must decide rather than a division by zero.
        assert_eq!(swipe_velocity_px_per_sec(100, 80, None, now, 300.0), 0.0);
        // Two samples at the same instant likewise have no measurable duration.
        assert_eq!(swipe_velocity_px_per_sec(100, 80, Some(now), now, 300.0), 0.0);
    }

    /// The velocity is a pure function of the two samples, so the arithmetic is
    /// asserted against a constructed interval rather than a measured one. This is
    /// what makes the flick rule testable without a sleep.
    #[test]
    fn carousel_velocity_is_measured_from_the_last_sample() {
        let start = Instant::now();
        let quarter_second_later = start + core::time::Duration::from_millis(250);

        // 100px of rightward travel in 0.25s is 400px/s.
        let rightward =
            swipe_velocity_px_per_sec(100, 200, Some(start), quarter_second_later, 300.0);
        assert!(
            (rightward - 400.0).abs() < 1.0,
            "100px in 250ms must read as 400px/s, got {rightward}"
        );
        // The opposite direction reports the opposite sign.
        let leftward =
            swipe_velocity_px_per_sec(200, 100, Some(start), quarter_second_later, 300.0);
        assert!((leftward + 400.0).abs() < 1.0, "leftward must be negative: {leftward}");

        // A sample that postdates the release cannot produce a duration, so it reads
        // as no speed rather than a negative one.
        assert_eq!(
            swipe_velocity_px_per_sec(100, 200, Some(quarter_second_later), start, 300.0),
            0.0
        );
    }

    #[test]
    fn carousel_velocity_is_capped_well_above_the_flick_threshold() {
        let start = Instant::now();
        // A near-zero interval with real travel would otherwise be enormous.
        let instant = start + core::time::Duration::from_nanos(1);
        let speed = swipe_velocity_px_per_sec(0, 10_000, Some(start), instant, 300.0);
        assert!(speed <= 3000.0, "speed must be capped at ten widths per second: {speed}");
        // The cap must not clamp away a genuine flick, or the velocity rule would be
        // unreachable on any control narrower than the threshold in pixels.
        let real_flick = swipe_velocity_px_per_sec(
            0,
            100,
            Some(start),
            start + core::time::Duration::from_millis(100),
            300.0,
        );
        assert!(
            real_flick >= FLICK_VELOCITY_PX_PER_SEC,
            "{real_flick} must exceed the flick \
             threshold and must not be capped below it"
        );
    }

    /// The end-to-end flick, driven through the public event path.
    ///
    /// The gesture must first cross the *press* threshold to enter `Swiping` at all —
    /// below that the control has not decided the gesture is a drag yet, so there is
    /// no velocity to read. Once it is swiping, a short fast release pages without
    /// needing the full distance threshold.
    ///
    /// Uses a real instant gap rather than a synthetic one, so the assertion covers
    /// the timing path the control actually reads.
    #[test]
    fn carousel_short_fast_drag_pages_end_to_end() {
        let mut c = default_carousel();
        assert_eq!(c.current(), 0);

        // Cross the 54px press threshold to enter the swiping state.
        c.handle_event(&Event::MousePress { pos: Point::new(280, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(280 - 60, 100) });
        // Then release after only 6 more pixels of travel. Total travel is 66px,
        // which is past the distance threshold — so this also asserts the documented
        // distance rule still pages, whatever the speed happens to be.
        c.handle_event(&Event::MouseMove { pos: Point::new(280 - 66, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(280 - 66, 100), button: 1 });

        assert_eq!(c.current(), 1, "a 66px leftward drag must page forward");
    }

    /// The velocity path end to end: entering `Swiping` at the threshold, then
    /// releasing immediately after a further short move. The extra travel is under
    /// the distance threshold, so only the measured speed can page it.
    ///
    /// The two presses are separated by a real (if tiny) interval because
    /// `Instant::now()` is the only clock the control reads; the assertion is
    /// therefore on the *combined* outcome — a fast release never snaps back while
    /// the distance is already over the threshold, which is the property that must
    /// hold however the machine is loaded.
    #[test]
    fn carousel_swipe_entering_state_survives_a_fast_release() {
        let mut c = default_carousel();
        c.handle_event(&Event::MousePress { pos: Point::new(280, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(220, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(220, 100), button: 1 });
        assert_eq!(c.current(), 1);

        // And the same gesture backwards.
        c.handle_event(&Event::MousePress { pos: Point::new(20, 100), button: 1 });
        c.handle_event(&Event::MouseMove { pos: Point::new(80, 100) });
        c.handle_event(&Event::MouseRelease { pos: Point::new(80, 100), button: 1 });
        assert_eq!(c.current(), 0);
    }

    // ── B1-3: autoplay + loop ───────────────────────────────────────────────

    #[test]
    fn carousel_loop_wraps_both_directions() {
        let mut c = default_carousel();
        c.set_loop(true);

        c.previous();
        assert_eq!(c.current(), 2, "previous from the first page wraps to the last");
        c.next();
        assert_eq!(c.current(), 0, "next from the last page wraps to the first");
    }

    #[test]
    fn carousel_autoplay_advances_by_elapsed_time() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(1000)));

        // Less than one interval: no advance.
        c.advance_autoplay_by(core::time::Duration::from_millis(400));
        assert_eq!(c.current(), 0);

        // Crossing the interval advances once.
        c.advance_autoplay_by(core::time::Duration::from_millis(700));
        assert_eq!(c.current(), 1);
    }

    #[test]
    fn carousel_autoplay_without_loop_stops_at_last_page() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(100)));
        assert!(!c.r#loop());

        for _ in 0..10 {
            c.advance_autoplay_by(core::time::Duration::from_millis(100));
        }
        assert_eq!(c.current(), 2, "autoplay must stop at the last page when loop is off");
    }

    #[test]
    fn carousel_autoplay_hover_pauses() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(100)));

        c.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        c.advance_autoplay_by(core::time::Duration::from_millis(500));
        assert_eq!(c.current(), 0, "hover must hold autoplay");

        c.handle_event(&Event::MouseLeave { pos: Point::new(10, 10) });
        c.advance_autoplay_by(core::time::Duration::from_millis(500));
        assert_ne!(c.current(), 0, "autoplay resumes once the pointer leaves");
    }

    #[test]
    fn carousel_autoplay_paused_while_disabled() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(100)));
        c.set_enabled(false);
        c.advance_autoplay_by(core::time::Duration::from_millis(1000));
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_autoplay_zero_interval_is_rejected() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(0)));
        // Zero would mean "advance every tick", which is a hang, not a setting.
        assert!(c.autoplay().is_none());
        c.advance_autoplay_by(core::time::Duration::from_millis(1000));
        assert_eq!(c.current(), 0);
    }

    #[test]
    fn carousel_autoplay_restarts_dwell_on_manual_move() {
        let mut c = default_carousel();
        c.set_autoplay(Some(core::time::Duration::from_millis(1000)));

        // Accumulate most of an interval, then move by hand.
        c.advance_autoplay_by(core::time::Duration::from_millis(900));
        c.set_current(2);
        assert_eq!(c.current(), 2);

        // The manual move cleared the accumulator, so the next tick does not
        // immediately advance again.
        c.advance_autoplay_by(core::time::Duration::from_millis(200));
        assert_eq!(c.current(), 2);
    }

    // ── B1-4: indicator style + position (pixel assertions) ─────────────────

    #[test]
    fn carousel_hidden_indicator_draws_nothing() {
        let mut c = default_carousel();
        let size = Size::new(300, 200);
        let with_dots = render_rgba(&mut c, size);
        // The indicator strip sits in the bottom INDICATOR_STRIP pixels; with dots
        // on, at least one pixel there differs from the page colour.
        let bottom_y = size.height - 8;
        let dots_row: Vec<_> =
            (0..size.width).map(|x| pixel(&with_dots, size.width, x, bottom_y)).collect();
        let page_color = (52u8, 120u8, 246u8);
        assert!(
            dots_row.iter().any(|p| (p.0, p.1, p.2) != page_color),
            "the dot indicator must paint something over the page background"
        );

        c.set_indicator_style(CarouselIndicatorStyle::Hidden);
        let without_dots = render_rgba(&mut c, size);
        let hidden_row: Vec<_> =
            (0..size.width).map(|x| pixel(&without_dots, size.width, x, bottom_y)).collect();
        // With the indicator hidden, the strip is page background apart from the
        // rounded corner falloff at the two edges.
        let painted =
            hidden_row.iter().filter(|p| (p.0, p.1, p.2) != page_color && p.3 != 0).count();
        assert!(painted < 20, "a hidden indicator must not paint the dot row: {painted} pixels");
    }

    #[test]
    fn carousel_indicator_style_round_trips() {
        let mut c = default_carousel();
        for style in [
            CarouselIndicatorStyle::Dots,
            CarouselIndicatorStyle::Bars,
            CarouselIndicatorStyle::Numeric,
            CarouselIndicatorStyle::Hidden,
        ] {
            c.set_indicator_style(style);
            assert_eq!(c.indicator_style(), style);
        }
        assert!(!CarouselIndicatorStyle::Hidden.is_visible());
        assert!(CarouselIndicatorStyle::Bars.is_visible());
        assert!(CarouselIndicatorStyle::Numeric.is_visible());
        // The per-page set is what the slot-based drawing routine owns; `Numeric`
        // must not be in it, or it would get a slot per page instead of a counter.
        assert!(CarouselIndicatorStyle::Dots.is_per_page());
        assert!(CarouselIndicatorStyle::Bars.is_per_page());
        assert!(!CarouselIndicatorStyle::Numeric.is_per_page());
        assert!(!CarouselIndicatorStyle::Hidden.is_per_page());
    }

    /// The counter must paint, and must paint something *different* after paging —
    /// otherwise "numeric" would be indistinguishable from a style that draws a
    /// constant string.
    #[test]
    fn carousel_numeric_indicator_paints_the_page_counter() {
        let mut c = default_carousel();
        c.set_indicator_style(CarouselIndicatorStyle::Numeric);
        let size = Size::new(300, 200);

        // The strip is page background apart from what the counter paints.
        let page_color = (52u8, 120u8, 246u8);
        let strip_y = size.height - 8;
        let at_page_0 = render_rgba(&mut c, size);
        let painted = |rgba: &[u8]| {
            (0..size.width)
                .map(|x| pixel(rgba, size.width, x, strip_y))
                .filter(|p| (p.0, p.1, p.2) != page_color && p.3 != 0)
                .count()
        };
        let first = painted(&at_page_0);
        assert!(first > 0, "a numeric indicator must paint the counter text");

        c.set_current(2);
        let at_page_2 = render_rgba(&mut c, size);
        assert_ne!(
            at_page_0, at_page_2,
            "paging must change the counter, so the two frames cannot be identical"
        );
    }

    /// The counter is one mark, not one per page: adding pages must not make it
    /// grow the way the dot strip does.
    #[test]
    fn carousel_numeric_indicator_does_not_scale_with_page_count() {
        let size = Size::new(300, 200);
        let strip_y = size.height - 8;
        let page_color = (52u8, 120u8, 246u8);
        let painted = |rgba: &[u8]| {
            (0..size.width)
                .map(|x| pixel(rgba, size.width, x, strip_y))
                .filter(|p| (p.0, p.1, p.2) != page_color && p.3 != 0)
                .count()
        };

        let mut few = Carousel::new(Rect::new(0, 0, 300, 200));
        few.add_page("A", Color::rgb(52, 120, 246));
        few.add_page("B", Color::rgb(52, 120, 246));
        few.set_indicator_style(CarouselIndicatorStyle::Numeric);

        let mut many = Carousel::new(Rect::new(0, 0, 300, 200));
        for i in 0..12 {
            many.add_page(format!("P{i}"), Color::rgb(52, 120, 246));
        }
        many.set_indicator_style(CarouselIndicatorStyle::Numeric);

        // Both paint "1/N"; N grows by a digit at most, so the ink is within a
        // small constant of each other — never the 6x a per-page strip would add.
        let few_ink = painted(&render_rgba(&mut few, size));
        let many_ink = painted(&render_rgba(&mut many, size));
        assert!(few_ink > 0 && many_ink > 0);
        let ratio = many_ink as f32 / few_ink as f32;
        assert!(ratio < 2.0, "the counter must not scale with page count: {ratio}");
    }

    #[test]
    fn carousel_indicator_position_changes_content_rect() {
        let mut c = default_carousel();

        c.set_indicator_position(CarouselIndicatorPosition::Bottom);
        let bottom = c.content_rect();
        assert_eq!(bottom.y, 0);
        assert_eq!(bottom.height, 200 - INDICATOR_STRIP);

        c.set_indicator_position(CarouselIndicatorPosition::Top);
        let top = c.content_rect();
        assert_eq!(top.y, INDICATOR_STRIP as i32);
        assert_eq!(top.height, 200 - INDICATOR_STRIP);

        c.set_indicator_position(CarouselIndicatorPosition::Left);
        let left = c.content_rect();
        assert_eq!(left.x, INDICATOR_STRIP as i32);
        assert_eq!(left.width, 300 - INDICATOR_STRIP);

        c.set_indicator_position(CarouselIndicatorPosition::Right);
        let right = c.content_rect();
        assert_eq!(right.x, 0);
        assert_eq!(right.width, 300 - INDICATOR_STRIP);
    }

    #[test]
    fn carousel_single_page_reserves_no_indicator_strip() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Only", Color::WHITE);
        // One page has nothing to indicate, so the whole rectangle is content.
        assert_eq!(c.content_rect().height, 200);
    }

    // ── B1-5: property contract (rule #82) ──────────────────────────────────

    #[test]
    fn carousel_loop_property_round_trips() {
        let mut c = default_carousel();
        assert_eq!(c.get("loop").unwrap(), CapabilityValue::Bool(false));
        c.set("loop", CapabilityValue::Bool(true)).unwrap();
        assert!(c.r#loop());
        assert_eq!(c.get("loop").unwrap(), CapabilityValue::Bool(true));
    }

    #[test]
    fn carousel_autoplay_interval_property_round_trips() {
        let mut c = default_carousel();
        assert_eq!(c.get("autoplay_interval").unwrap(), CapabilityValue::Null);

        c.set("autoplay_interval", CapabilityValue::UInt(1500)).unwrap();
        assert_eq!(c.get("autoplay_interval").unwrap(), CapabilityValue::UInt(1500));
        assert_eq!(c.autoplay(), Some(core::time::Duration::from_millis(1500)));

        c.set("autoplay_interval", CapabilityValue::Null).unwrap();
        assert_eq!(c.get("autoplay_interval").unwrap(), CapabilityValue::Null);
        assert!(c.autoplay().is_none());
    }

    #[test]
    fn carousel_indicator_properties_round_trip() {
        let mut c = default_carousel();
        for name in ["dots", "bars", "numeric", "hidden"] {
            c.set("indicator_style", CapabilityValue::String(name.to_string())).unwrap();
            assert_eq!(
                c.get("indicator_style").unwrap(),
                CapabilityValue::String(name.to_string())
            );
        }
        for name in ["bottom", "top", "left", "right"] {
            c.set("indicator_position", CapabilityValue::String(name.to_string())).unwrap();
            assert_eq!(
                c.get("indicator_position").unwrap(),
                CapabilityValue::String(name.to_string())
            );
        }
        // An unknown token is a type mismatch, not a silent no-op.
        assert!(c.set("indicator_style", CapabilityValue::String("sparkle".to_string())).is_err());
    }

    #[test]
    fn carousel_derived_properties_are_read_only() {
        let mut c = default_carousel();
        assert!(c.set("item_count", CapabilityValue::UInt(9)).is_err());
        assert!(c.set("current_page_title", CapabilityValue::String("x".into())).is_err());
        assert_eq!(c.get("item_count").unwrap(), CapabilityValue::UInt(3));
        assert_eq!(
            c.get("current_page_title").unwrap(),
            CapabilityValue::String("Page 1".to_string())
        );
    }

    // ── Event forwarding to a mounted page ──────────────────────────────────

    #[test]
    fn carousel_forwards_arrow_keys_to_the_visible_page() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Host", Color::WHITE);
        c.add_page("Other", Color::WHITE);

        let child = Label::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
        assert!(c.set_page_content(0, Box::new(child)));

        // The carousel itself pages on arrows, so the assertion here is that the
        // call is handled without disturbing the mounted child's geometry.
        c.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
        assert_eq!(c.current(), 1);
        c.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        assert_eq!(c.current(), 0);
        assert!(c.pages()[0].has_content());
    }

    #[test]
    fn carousel_content_draw_does_not_panic_and_paints() {
        let mut c = Carousel::new(Rect::new(0, 0, 300, 200));
        c.add_page("Host", Color::WHITE);
        let child = Label::new("Inside".to_string(), Rect::new(0, 0, 300, 200));
        assert!(c.set_page_content(0, Box::new(child)));

        let rgba = render_rgba(&mut c, Size::new(300, 200));
        assert!(!rgba.is_empty());
        // The page background is white and the label paints text, so the frame must
        // contain something that is not the white fill.
        let mut non_white = 0;
        for chunk in rgba.chunks_exact(4) {
            if chunk[0] != 255 || chunk[1] != 255 || chunk[2] != 255 {
                non_white += 1;
            }
        }
        assert!(non_white > 0, "a page with content must paint the content");
    }
}
