// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll bar widget.
use crate::compat::ToString;
use crate::core::{Color, Orientation, Point, Rect};
use crate::event::{wheel::WheelScroll, Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::{
    expect_f32, expect_i32_saturating, expect_orientation, expect_text_direction, expect_u32,
    orientation_to_str, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, focus_ring_color, ControlMetrics, FocusRing, FOCUS_RING_WIDTH,
};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// How long the bar stays fully opaque after the last interaction, in milliseconds.
///
/// Material's scrollbar waits **600 ms** of inactivity before it begins to fade. The value is a
/// duration the theme does not price (it is a *delay*, not a transition), so it is a named
/// constant rather than a `Motion` slot — the same distinction `ReferenceToolkit`'s scrollbar
/// draws between its hide-delay and its hide animation.
const SCROLLBAR_IDLE_DELAY_MS: u32 = 600;

/// What the bar fades *to* while it is idle, as a fraction of its drawn opacity.
///
/// Not zero: a scrollbar that vanishes entirely gives no hint that the region scrolls, and a
/// pointer returning to the bar has nothing to aim at. A faint trough that stays is what every
/// toolkit settles on — the bar dims, it does not disappear.
///
/// This is the **arrows** style's floor. An overlay bar is chrome *above* the content and
/// deliberately disappears when idle; see [`ScrollBar::idle_opacity`].
const SCROLLBAR_IDLE_OPACITY: f32 = 0.35;

/// How long an arrow cell must be held before it starts auto-repeating, in milliseconds.
///
/// The first step fires on the press itself; this delay is what separates "one click" from
/// "keep going". It is long enough that a deliberate single step is not doubled.
const SCROLLBAR_ARROW_REPEAT_DELAY_MS: u32 = 400;

/// How often a held arrow cell repeats its line step, in milliseconds.
const SCROLLBAR_ARROW_REPEAT_INTERVAL_MS: u32 = 60;

/// Per-millisecond decay applied to a released thumb's velocity while it glides.
///
/// A fraction rather than a fixed duration, so the movement "continues briefly and settles"
/// regardless of the frame pacing: at 16 ms a frame the carried velocity is roughly halved
/// every five frames.
const SCROLLBAR_INERTIA_DECAY_PER_MS: f32 = 0.006;

/// The velocity, in pixels per millisecond, below which a glide is considered finished.
const SCROLLBAR_INERTIA_STOP_PX_PER_MS: f32 = 0.02;

/// A held arrow cell and how long it has been held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArrowRepeat {
    action: ScrollBarAction,
    held_ms: u32,
    /// How many repeats have already been performed since the press.
    fired: u32,
}

/// How a scroll bar is drawn.
///
/// # Why two shapes on one control
///
/// Both are the same scrollbar behaviour — one range, one thumb, one drag — drawn as two
/// different pieces of chrome. [`Arrows`](Self::Arrows) is the desktop form with fixed arrow
/// cells at each end; [`Overlay`](Self::Overlay) floats over the content with no arrow cells,
/// a thinner band, and it fades to fully transparent while idle. Keeping the behaviour in one
/// implementation means the two shapes cannot drift on the parts that are not about ink.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollBarStyle {
    /// A trough with arrow cells at both ends; the default desktop shape.
    #[default]
    Arrows,
    /// A thin, arrow-less thumb that fades away when idle.
    Overlay,
}

/// Scroll bar widget.
pub struct ScrollBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    orientation: Orientation,
    /// The drawn shape: arrow-ended trough or floating overlay thumb.
    style: ScrollBarStyle,
    /// The writing direction the trough runs in.
    ///
    /// # Why a scroll bar needs this
    ///
    /// The bar places a *value* on a line, exactly as a slider does, and in a right-to-left interface
    /// the line's beginning is its right edge: value `minimum` belongs on the right, and the `LineUp`
    /// action — "move toward the minimum" — must walk leftward. Every mapping below read the trough
    /// left-to-right, so the thumb sat at the mirrored position and the arrows moved it the wrong
    /// way.
    ///
    /// Only the **horizontal** trough is a line of text direction. A vertical bar's axis is the block
    /// flow, which is not reversed by a right-to-left script, so `direction` is ignored when the
    /// orientation is vertical — the same scoping [`crate::core::TextDirection`] documents.
    ///
    /// Defaults to left-to-right, so a bar that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// Emitted with the new value when the scroll position changes, whether
    /// from user input or a programmatic setter.
    pub value_changed: Signal1<i32>,
    /// Emitted with the new value when the scroll *thumb* is dragged, as opposed
    /// to any other way the value can change. Lets a consumer distinguish direct
    /// manipulation from, say, a wheel scroll.
    pub slider_moved: Signal1<i32>,
    /// Emitted when the thumb is pressed. Carries no payload.
    pub slider_pressed: GenericSignal,
    /// Emitted when the thumb is released. Emitted even if the pointer left the
    /// widget before releasing, since the widget tracks its own press state.
    pub slider_released: GenericSignal,
    /// Whether a pointer button is currently held on this bar.
    mouse_pressed: bool,
    /// Whether a press on the thumb may turn into a drag.
    ///
    /// A press on the trough pages instead of grabbing, so a following pointer move must not
    /// drag the thumb; this flag is what distinguishes the two gestures.
    dragging_thumb: bool,
    /// The pointer's pixel position at the last press/move, for sampling drag velocity.
    drag_last_pixel: f32,
    /// Pixels the pointer travelled since the last tick, consumed by the velocity estimate.
    drag_pending_px: f32,
    /// The thumb's recent velocity, in pixels per millisecond, while it is being dragged.
    drag_velocity_px_per_ms: f32,
    /// Whether released-thumb inertia is enabled.
    inertia: bool,
    /// The velocity a released thumb is still carrying, in pixels per millisecond.
    inertia_velocity: Option<f32>,
    /// Fractional value left over between inertia frames, so a slow glide still advances.
    inertia_remainder: f32,
    /// The arrow cell currently held, if any; it repeats a line step while it is.
    repeat: Option<ArrowRepeat>,
    /// Milliseconds since the last interaction that should keep the bar fully opaque.
    ///
    /// # Why the bar needs an idle clock at all
    ///
    /// A scrollbar is chrome the reader only needs *while scrolling*: left fully opaque it is a
    /// permanent stripe competing with the content it frames. Every toolkit that has solved this
    /// fades it — Material after 600 ms idle, 参考工具包 through the scroll bar's own hide-delay — and
    /// the mechanism has two halves that must both exist: a clock that notices the quiet, and an
    /// opacity the draw path reads. Without them the bar is either always loud or (if simply
    /// hidden) undiscoverable.
    idle_ms: u32,
    /// How long the bar stays fully opaque after the last interaction, in milliseconds.
    idle_delay_ms: u32,
    /// An explicit idle opacity, or `None` for the style's own floor.
    ///
    /// An arrows bar keeps [`SCROLLBAR_IDLE_OPACITY`] and an overlay bar fades to zero; a host
    /// that wants a longer-visible touch scrollbar can now say so per control.
    idle_opacity_override: Option<f32>,
    /// The bar's drawn opacity, `1.0` while active and [`SCROLLBAR_IDLE_OPACITY`] once idle.
    ///
    /// A driver rather than a bare float so the fade is a **transition** that can be seen rather
    /// than a step, and so its resting end is expressible: the driver is built at `1.0` because a
    /// freshly laid-out bar has just appeared and must not fade *from* invisibility.
    opacity: PropertyDriver,
}
impl ScrollBar {
    /// Creates a scroll bar with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ScrollBar, geometry, "ScrollBar"),
            minimum: 0,
            maximum: 100,
            value: 0,
            single_step: 1,
            page_step: 10,
            orientation: Orientation::Horizontal,
            style: ScrollBarStyle::default(),
            direction: crate::core::TextDirection::default(),
            value_changed: Signal1::new(),
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
            mouse_pressed: false,
            dragging_thumb: false,
            drag_last_pixel: 0.0,
            drag_pending_px: 0.0,
            drag_velocity_px_per_ms: 0.0,
            inertia: false,
            inertia_velocity: None,
            inertia_remainder: 0.0,
            repeat: None,
            idle_ms: 0,
            idle_delay_ms: SCROLLBAR_IDLE_DELAY_MS,
            idle_opacity_override: None,
            opacity: PropertyDriver::at(1.0, MotionSlot::Normal),
        }
    }
    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
        if self.minimum == minimum {
            return;
        }
        self.minimum = minimum;
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        self.set_value(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Returns maximum value.
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    /// Sets maximum value.
    pub fn set_maximum(&mut self, maximum: i32) {
        if self.maximum == maximum {
            return;
        }
        self.maximum = maximum;
        if self.minimum > self.maximum {
            self.minimum = self.maximum;
        }
        self.set_value(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        let maximum = maximum.max(minimum);
        if self.minimum == minimum && self.maximum == maximum {
            return;
        }
        self.minimum = minimum;
        self.maximum = maximum;
        self.set_value(self.value); // Re-clamp
        self.base.request_redraw();
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Sets value, clamped to valid range.
    pub fn set_value(&mut self, value: i32) {
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        // A moving thumb is the reader scrolling: the bar comes back to full opacity and its idle
        // clock restarts, which is what keeps the fade from fighting the scroll it is meant to
        // accompany.
        self.touch_activity();
        self.value_changed.emit(self.value);
    }
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        let step = step.max(1);
        if self.single_step == step {
            return;
        }
        self.single_step = step;
        self.base.request_redraw();
    }
    /// Returns page step value.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Sets page step value.
    pub fn set_page_step(&mut self, step: i32) {
        let step = step.max(1);
        if self.page_step == step {
            return;
        }
        self.page_step = step;
        self.base.request_redraw();
    }

    /// Whether a released thumb keeps gliding before it settles.
    pub fn inertia(&self) -> bool {
        self.inertia
    }

    /// Enables or disables released-thumb inertia.
    ///
    /// When enabled, a thumb that is dragged and then released carries its velocity briefly
    /// and settles under a decaying glide; [`ScrollBar::tick`] advances it and
    /// [`ScrollBar::is_animating`] reports the frames it owes. The default is `false`, which
    /// is the historical behaviour: the thumb stops exactly where the pointer released it.
    ///
    /// Disabling inertia stops a glide in progress at its current value rather than rewinding
    /// it, so the picture after the setter is the picture that was on screen.
    pub fn set_inertia(&mut self, on: bool) {
        if self.inertia == on {
            return;
        }
        self.inertia = on;
        if !on {
            self.inertia_velocity = None;
            self.inertia_remainder = 0.0;
        }
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    /// Sets whether the trough runs horizontally or vertically, and repaints.
    ///
    /// The orientation selects which axis the bar measures and which way the arrows point, so
    /// both the paint path and the hit test branch on it.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if self.orientation == orientation {
            return;
        }
        self.orientation = orientation;
        self.base.request_redraw();
    }

    /// Returns the writing direction the horizontal trough runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the horizontal trough runs in, and repaints.
    ///
    /// A right-to-left bar puts `minimum` at the right end and grows leftward, so a drag and the
    /// arrow actions move the way the reader's eye does. The vertical orientation ignores it: the
    /// block axis is not reversed by a right-to-left script. See the field for the reasoning.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }
    /// Returns slider size as percentage of visible area.
    pub fn slider_size(&self) -> f32 {
        if self.maximum == self.minimum {
            return 1.0;
        }
        let page_size = self.page_step as f32;
        let total_range = (self.maximum - self.minimum) as f32;
        (page_size / total_range).clamp(0.1, 0.9)
    }
    /// Returns slider position as percentage.
    pub fn slider_position(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0.0;
        }
        ((self.value - self.minimum) as f32) / ((self.maximum - self.minimum) as f32)
    }
    /// Returns the drawn shape of the bar.
    ///
    /// Named `bar_style` rather than `style` so it cannot shadow the inherited
    /// [`Widget::style`] accessor that reports the resolved [`crate::style::WidgetStyle`].
    pub fn bar_style(&self) -> ScrollBarStyle {
        self.style
    }

    /// Selects the drawn shape and repaints.
    ///
    /// Switching to [`ScrollBarStyle::Overlay`] collapses the arrow cells to zero and makes
    /// the idle fade reach full transparency; switching back restores the desktop form. The
    /// value and travel mappings read the same shape, so a bar cannot be dragged by one
    /// geometry while being drawn by another.
    pub fn set_style(&mut self, style: ScrollBarStyle) {
        if self.style == style {
            return;
        }
        self.style = style;
        self.base.request_redraw();
    }

    /// The thickness of the band the bar measures with.
    ///
    /// This is the *hit* geometry. The overlay's painted band can widen on hover, but that
    /// extra width is never part of what the pointer targets (see
    /// [`Self::paint_band`]).
    fn band_thickness(&self) -> u32 {
        match self.style {
            ScrollBarStyle::Arrows => dimensions::SCROLLBAR_THICKNESS,
            ScrollBarStyle::Overlay => dimensions::SCROLLBAR_OVERLAY_THICKNESS,
        }
    }

    /// The band the bar paints, which in overlay style widens under the pointer.
    ///
    /// The widening is deliberately paint-only: growing the measured band with it made the
    /// bar oscillate as the pointer crossed its edge, because every frame's hit test used the
    /// previous frame's width.
    fn paint_band(&self) -> Rect {
        let thickness = match self.style {
            ScrollBarStyle::Arrows => dimensions::SCROLLBAR_THICKNESS,
            ScrollBarStyle::Overlay if self.base.is_hovered() || self.mouse_pressed => {
                dimensions::SCROLLBAR_OVERLAY_HOVER_THICKNESS
            }
            ScrollBarStyle::Overlay => dimensions::SCROLLBAR_OVERLAY_THICKNESS,
        };
        self.band_with_thickness(thickness)
    }

    /// The opaque band `thickness` px thick, centred in the control's rectangle and running
    /// along the bar's long axis.
    fn band_with_thickness(&self, thickness: u32) -> Rect {
        let rect = self.geometry();
        match self.orientation {
            Orientation::Horizontal => ControlMetrics::centered_band(rect, thickness),
            Orientation::Vertical => {
                // A vertical scrollbar's band is *wide and full-height*, i.e. the same
                // derivation with the axes exchanged. `centered_band` keeps the caller's
                // width and takes the thickness for the height, which is exactly the
                // horizontal reading; transposing the inputs gives the vertical one.
                let band = ControlMetrics::centered_band(
                    Rect::new(rect.y, rect.x, rect.height, rect.width),
                    thickness,
                );
                Rect::new(band.y, band.x, band.height, band.width)
            }
        }
    }

    /// The band the bar **measures** against: the control's rectangle reduced to the bar's
    /// thickness and centred, running the full long axis.
    ///
    /// `size_hint` reports the same thickness and `draw` paints [`Self::paint_band`], so the
    /// two agree about how thick the bar is; this accessor is the one place that asks "how
    /// long may the thumb travel", and it is deliberately the full length of the rectangle.
    /// In overlay style the band is thinner and has no arrow cells, which is why the
    /// thickness is asked of [`Self::band_thickness`] rather than named here.
    fn track_band(&self) -> Rect {
        self.band_with_thickness(self.band_thickness())
    }

    /// Size of the arrow cell at each end of the trough, in pixels.
    ///
    /// Derived from the control's **thickness** and bounded, so a scrollbar's arrows are the
    /// same size no matter how long the bar is. `width * 0.2` on a 240 px horizontal bar gave a
    /// 48 px arrow — wider than the trough and longer than the space before the thumb, so the
    /// left arrow's apex landed underneath the slider. The decrement-line scroll step is on the
    /// order of the trough's thickness, and this is the same reading — it is
    /// [`dimensions::SCROLLBAR_MIN_LENGTH`]'s floor applied to the cell, so an arrow cell and a
    /// thumb are never wildly different objects.
    ///
    /// `value_to_pixel_pos` reads this too, which is what keeps the thumb inside the trough: one
    /// derivation, two consumers, so they cannot drift apart.
    fn arrow_cell(&self) -> f32 {
        // An overlay bar has no arrow cells: every consumer (travel, thumb length, arrow
        // paint) reads this one function, so returning zero makes the whole band available
        // without branching in three places.
        if self.style == ScrollBarStyle::Overlay {
            return 0.0;
        }
        let rect = self.geometry();
        match self.orientation {
            Orientation::Horizontal => {
                (rect.height as f32).min((rect.width as f32) / 3.0).clamp(4.0, 16.0)
            }
            Orientation::Vertical => {
                (rect.width as f32).min((rect.height as f32) / 3.0).clamp(4.0, 16.0)
            }
        }
    }

    /// The line action an arrow cell at `pos` performs, or `None` when the point is not on
    /// an arrow cell.
    ///
    /// The cells are the two ends of the drawn band. Which end is "toward the minimum"
    /// follows the written direction for a horizontal trough — in a right-to-left interface
    /// the minimum sits at the right edge, so the right cell steps down — and a vertical
    /// trough's top cell always steps toward the minimum.
    fn arrow_action_at(&self, pos: Point) -> Option<ScrollBarAction> {
        if self.style == ScrollBarStyle::Overlay {
            return None;
        }
        let cell = self.arrow_cell() as i32;
        if cell <= 0 {
            return None;
        }
        let band = self.track_band();
        if !band.contains(pos) {
            return None;
        }
        match self.orientation {
            Orientation::Horizontal => {
                let min_at_left = self.direction == crate::core::TextDirection::LeftToRight;
                let at_left = pos.x < band.x + cell;
                let at_right = pos.x >= band.x + band.width as i32 - cell;
                if at_left {
                    Some(if min_at_left {
                        ScrollBarAction::LineUp
                    } else {
                        ScrollBarAction::LineDown
                    })
                } else if at_right {
                    Some(if min_at_left {
                        ScrollBarAction::LineDown
                    } else {
                        ScrollBarAction::LineUp
                    })
                } else {
                    None
                }
            }
            Orientation::Vertical => {
                if pos.y < band.y + cell {
                    Some(ScrollBarAction::LineUp)
                } else if pos.y >= band.y + band.height as i32 - cell {
                    Some(ScrollBarAction::LineDown)
                } else {
                    None
                }
            }
        }
    }

    /// The length of the thumb the draw pass paints, in pixels.
    ///
    /// The draw pass sizes the thumb from the **track between the arrow cells** and floors it at
    /// [`dimensions::SCROLLBAR_MIN_LENGTH`], so a long document still leaves something to grab.
    /// It also stops the thumb at the trough's far edge. Both of those are drawing decisions, but
    /// they describe the *grip the user actually sees*, and a conversion that assumed a different
    /// grip could not be inverted — so this is the same measure the draw pass takes, and
    /// [`Self::travel_band`] is the single consumer of it.
    fn thumb_length(&self) -> f32 {
        let band = self.track_band();
        let cell = self.arrow_cell();
        let track = match self.orientation {
            Orientation::Horizontal => band.width as f32,
            Orientation::Vertical => band.height as f32,
        };
        let between_arrows = (track - cell * 2.0).max(0.0);
        let proportional = between_arrows * self.slider_size();
        // Mirrors the draw pass exactly: floor first, then clamp inside the trough, so the length
        // here never exceeds the room it has. The floor is expressed against `track` rather than
        // `between_arrows` for the same reason the draw pass does: a floor that could exceed the
        // space available would make a short bar's thumb overshoot the trough it sits in.
        let floor = dimensions::SCROLLBAR_MIN_LENGTH.min(track as u32) as f32;
        let floored = proportional.max(floor);
        floored.min(between_arrows.max(1.0))
    }

    /// The thumb's own rectangle, in control coordinates.
    ///
    /// The single derivation the paint pass and the keyboard focus ring read, so the grip
    /// the ring frames is exactly the grip that was painted. The length repeats the draw
    /// pass's two rules — floor at [`dimensions::SCROLLBAR_MIN_LENGTH`], then stop at the
    /// trough's far edge — because the ring must not outline a thumb that is not there.
    fn thumb_rect(&self) -> Rect {
        let band = self.track_band();
        let rect = self.paint_band();
        let slider_pos = self.value_to_pixel_pos(self.value);
        let fraction = self.slider_size();
        match self.orientation {
            Orientation::Horizontal => {
                let length = ((rect.width as f32) * fraction) as u32;
                let length = length.max(dimensions::SCROLLBAR_MIN_LENGTH.min(rect.width));
                let length =
                    length.min((band.x + band.width as i32 - slider_pos as i32).max(0) as u32);
                Rect::from_f32(slider_pos, rect.y as f32, length as f32, rect.height as f32)
            }
            Orientation::Vertical => {
                let length = ((rect.height as f32) * fraction) as u32;
                let length = length.max(dimensions::SCROLLBAR_MIN_LENGTH.min(rect.height));
                let length =
                    length.min((band.y + band.height as i32 - slider_pos as i32).max(0) as u32);
                Rect::from_f32(rect.x as f32, slider_pos, rect.width as f32, length as f32)
            }
        }
    }

    /// The **one** origin and length the two value/pixel conversions are defined against.
    ///
    /// # Why this exists (BLUE22 · G-2)
    ///
    /// `pixel_pos_to_value` and `value_to_pixel_pos` used to derive *different* travel from the
    /// same trough: the reader measured from the band's edge over the band's whole length, while
    /// the writer first skipped an arrow cell and scaled the rest by `1 - slider_size`. Neither
    /// was wrong on its own, but they were not inverses of each other — a value drawn at `x` read
    /// back as a different value, so clicking the thumb did not select the value it showed, and
    /// the error grew with the range rather than being a rounding artefact.
    ///
    /// `slider` had already been through exactly this and its comment states the rule: the two
    /// functions must share the inset, because that is what makes them exact inverses. The same
    /// reading applies here, with one correction the slider does not face — this control's thumb
    /// is a *fraction* of the track, not a constant, so the shared travel has to be
    /// **track minus thumb** rather than *track scaled by a fraction*. That is also the honest
    /// reading of the geometry: the thumb stops when its leading edge reaches the far end of the
    /// trough, so its centre travels `track - thumb`, and its own length is what it cannot travel.
    ///
    /// One derivation, two consumers, so they cannot drift apart.
    fn travel_band(&self) -> (f32, f32) {
        let band = self.track_band();
        let cell = self.arrow_cell();
        let (origin, length) = match self.orientation {
            Orientation::Horizontal => (band.x as f32, band.width as f32),
            Orientation::Vertical => (band.y as f32, band.height as f32),
        };
        // The travel is the **band between the two arrow cells**, not the whole trough: the arrows
        // occupy fixed cells at each end, so a thumb travelling the full length would pass beneath
        // them. The drawn thumb is already sized against that same span, which is what makes the
        // subtraction below a statement about *this* control rather than a second convention.
        let span = (length - cell * 2.0).max(0.0);
        // The thumb travels the span **minus its own length**: it is the only part that cannot be
        // traversed, and it is read from the same derivation the draw pass paints with.
        let travel = (span - self.thumb_length()).max(0.0);
        (origin + cell, travel)
    }

    /// Returns value for a given pixel position.
    ///
    /// The exact inverse of [`Self::value_to_pixel_pos`]: both read [`Self::travel_band`], so the
    /// value a drag reads back is the value the thumb was drawn at.
    fn pixel_pos_to_value(&self, pos: f32) -> i32 {
        let range = (self.maximum - self.minimum) as f32;
        if range == 0.0 {
            return self.minimum;
        }
        let (origin, travel) = self.travel_band();
        // A trough with no room left travels nowhere, so every position on it is the minimum.
        // That is the same answer `value_to_pixel_pos` gives for the same trough, which is what
        // keeps the pair total rather than merely monotonic.
        if travel <= 0.0 {
            return self.minimum;
        }
        let relative = (pos - origin) / travel;
        // The pointer arrives in the left-edge frame and the value lives in reading order, so this
        // is the one place the conversion belongs — and `value_to_pixel_pos` applies the same one in
        // the other direction, which is what keeps a drag reading back the value the thumb was drawn
        // at. Only a horizontal trough is a line of text direction; see the field.
        let relative = if self.orientation == Orientation::Horizontal {
            self.direction.left_fraction_to_begin_fraction(relative)
        } else {
            relative
        };
        let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
        value.round() as i32
    }
    /// Returns pixel position for a given value.
    ///
    /// Read the trough from [`Self::track_band`], the cells from [`Self::arrow_cell`] and the
    /// travel from [`Self::travel_band`], so the thumb, the arrows and the hit test are consumers
    /// of **one** geometry rather than derivations that can drift.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        let range = (self.maximum - self.minimum) as f32;
        let (origin, travel) = self.travel_band();
        if range == 0.0 {
            return origin;
        }
        let mut relative = (clamped - self.minimum) as f32 / range;
        if self.orientation == Orientation::Horizontal {
            relative = self.direction.begin_fraction_to_left_fraction(relative);
        }
        origin + travel * relative
    }
    /// Triggers a scroll action.
    pub fn trigger_action(&mut self, action: ScrollBarAction) {
        match action {
            ScrollBarAction::LineUp => {
                self.set_value(self.value - self.single_step);
            }
            ScrollBarAction::LineDown => {
                self.set_value(self.value + self.single_step);
            }
            ScrollBarAction::PageUp => {
                self.set_value(self.value - self.page_step);
            }
            ScrollBarAction::PageDown => {
                self.set_value(self.value + self.page_step);
            }
            ScrollBarAction::SliderMove => {
                // Handled by mouse events
            }
            ScrollBarAction::SliderPageStepAdd => {
                self.set_value(self.value + self.page_step);
            }
            ScrollBarAction::SliderPageStepSub => {
                self.set_value(self.value - self.page_step);
            }
            ScrollBarAction::SliderToMinimum => {
                self.set_value(self.minimum);
            }
            ScrollBarAction::SliderToMaximum => {
                self.set_value(self.maximum);
            }
        }
    }

    /// The bar's drawn opacity, `1.0` while active and dimmed once idle.
    ///
    /// The draw path multiplies every colour it paints by this, so the two halves of the
    /// behaviour — "notice the quiet" and "paint quieter" — are read from one value.
    pub fn opacity(&self) -> f32 {
        self.opacity.value()
    }

    /// How long the bar stays fully opaque after the last interaction, in milliseconds.
    pub fn idle_delay_ms(&self) -> u32 {
        self.idle_delay_ms
    }

    /// Sets the idle delay, clamped to keep the two-phase fade meaningful.
    ///
    /// A zero delay is legal (fade immediately), which is what an overlay bar on a touch device
    /// wants; the value is a duration, so there is no other invalid input.
    pub fn set_idle_delay_ms(&mut self, delay_ms: u32) {
        if self.idle_delay_ms == delay_ms {
            return;
        }
        self.idle_delay_ms = delay_ms;
    }

    /// What the bar fades toward for the current shape.
    ///
    /// An explicit [`Self::set_idle_opacity`] wins; otherwise an overlay bar fades to fully
    /// transparent and an arrow-ended bar keeps the faint floor that tells the reader the region
    /// scrolls.
    pub fn idle_opacity(&self) -> f32 {
        self.idle_opacity_override.unwrap_or(match self.style {
            ScrollBarStyle::Arrows => SCROLLBAR_IDLE_OPACITY,
            ScrollBarStyle::Overlay => 0.0,
        })
    }

    /// Sets the opacity the bar fades to, clamped to `0.0..=1.0`.
    pub fn set_idle_opacity(&mut self, opacity: f32) {
        let opacity = if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { return };
        if self.idle_opacity_override == Some(opacity) {
            return;
        }
        self.idle_opacity_override = Some(opacity);
    }

    /// The style's own idle floor, used by the docs and by tests.
    fn resolved_idle_opacity(&self) -> f32 {
        self.idle_opacity()
    }

    /// Marks the bar as newly interacted with, waking it back to full opacity.
    ///
    /// Called from every path that changes what the reader is looking at: the value moving, the
    /// thumb being grabbed, the pointer arriving. It re-aims the driver and restarts the idle
    /// clock, and it deliberately does **not** request a frame: while the bar is already opaque
    /// there is nothing to animate, and while it is dimmed `tick_animations` reports the movement
    /// itself.
    fn touch_activity(&mut self) {
        self.idle_ms = 0;
        self.opacity.set_target(1.0);
    }

    /// Advances the idle clock, the held-arrow repeat, released-thumb inertia and the
    /// opacity fade by `delta_ms`.
    ///
    /// # The two-phase fade, and why the delta is *split*
    ///
    /// The opacity holds at `1.0` for `SCROLLBAR_IDLE_DELAY_MS`, and only then begins to fade
    /// toward `SCROLLBAR_IDLE_OPACITY`. That ordering is what makes the bar feel *responsive*
    /// rather than twitchy: fading on the first idle millisecond would dim the bar while the
    /// reader is still reading what they just scrolled to.
    ///
    /// A single `Transition` cannot express the delay, so there are two fields — but they must not
    /// both consume the whole delta. Feeding the entire `delta_ms` to the fade on the frame that
    /// *crosses* the delay made a single `tick(600)` drop straight to the floor: the delay is a
    /// duration the fade has not yet started for, so only the part of the delta **past** the
    /// threshold belongs to the fade. A long frame therefore still arrives at the right place, and
    /// a frame that lands exactly on the boundary leaves the bar untouched.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // Held arrows and released-thumb inertia are advances of their own; both are additive
        // with the fade rather than alternatives to it, because a gliding bar can also be
        // fading.
        let repeated = self.advance_auto_repeat(delta_ms);
        let glided = self.advance_inertia(delta_ms);
        // Sample the drag velocity from the pointer movement accumulated since the previous
        // frame. A tick with no pointer movement contributes a zero sample, which pulls the
        // estimate toward rest — exactly what a paused pointer means.
        if self.mouse_pressed && self.dragging_thumb {
            let sample = self.drag_pending_px / delta_ms.max(1) as f32;
            self.drag_velocity_px_per_ms = self.drag_velocity_px_per_ms * 0.6 + sample * 0.4;
            self.drag_pending_px = 0.0;
        }
        let fade_delta = if self.idle_ms < self.idle_delay_ms {
            let until_idle = self.idle_delay_ms - self.idle_ms;
            self.idle_ms = self.idle_ms.saturating_add(delta_ms);
            if self.idle_ms >= self.idle_delay_ms {
                self.opacity.set_target(self.resolved_idle_opacity());
                delta_ms.saturating_sub(until_idle)
            } else {
                // Inside the hold. The clock is what the delay consumes, but the fade-in after a
                // wake also happens here: a bar whose opacity is below full is climbing toward it
                // and must be advanced, or a wake would stall at whatever the fade left behind.
                self.opacity.set_target(1.0);
                delta_ms
            }
        } else {
            self.opacity.set_target(self.resolved_idle_opacity());
            delta_ms
        };
        if fade_delta == 0 {
            return repeated || glided;
        }
        let faded = self.opacity.tick(fade_delta);
        repeated || glided || faded
    }

    /// Repeats the held arrow's line step once the hold has lasted long enough.
    ///
    /// The press itself already performed the first step, so this only fires the repeats. A
    /// very long frame is capped at a bounded number of repeats rather than looping for as
    /// many intervals as the delta contains, which would stall the frame that delivered it.
    fn advance_auto_repeat(&mut self, delta_ms: u32) -> bool {
        let (action, count) = {
            let Some(state) = self.repeat.as_mut() else {
                return false;
            };
            state.held_ms = state.held_ms.saturating_add(delta_ms);
            if state.held_ms < SCROLLBAR_ARROW_REPEAT_DELAY_MS {
                return false;
            }
            // Repeats due since the press: one when the delay elapses, then one per interval.
            // Counting the *total* due and subtracting what already fired keeps the cadence
            // independent of how the frames happen to land.
            let since_delay = state.held_ms - SCROLLBAR_ARROW_REPEAT_DELAY_MS;
            let due = 1 + since_delay / SCROLLBAR_ARROW_REPEAT_INTERVAL_MS;
            let count = due.saturating_sub(state.fired).min(64);
            state.fired = due;
            (state.action, count)
        };
        if count == 0 {
            return false;
        }
        for _ in 0..count {
            self.trigger_action(action);
        }
        true
    }

    /// Carries a released thumb's velocity forward and lets it settle.
    ///
    /// The glide moves in whole value units and keeps the fractional remainder between
    /// frames, so a slow release still advances rather than rounding to nothing forever.
    /// Velocity decays geometrically and is abandoned below
    /// [`SCROLLBAR_INERTIA_STOP_PX_PER_MS`] or at either bound.
    fn advance_inertia(&mut self, delta_ms: u32) -> bool {
        let Some(mut velocity) = self.inertia_velocity else {
            return false;
        };
        let range = (self.maximum as i64 - self.minimum as i64) as f32;
        let (_, travel) = self.travel_band();
        if range <= 0.0 || travel <= 0.0 {
            self.inertia_velocity = None;
            self.inertia_remainder = 0.0;
            return false;
        }
        // Pixels to values, with the horizontal written direction's sign applied: a rightward
        // flick raises the value left-to-right and lowers it right-to-left.
        let mut values_per_px = range / travel;
        if self.orientation == Orientation::Horizontal
            && self.direction == crate::core::TextDirection::RightToLeft
        {
            values_per_px = -values_per_px;
        }
        let step = velocity * delta_ms as f32 * values_per_px + self.inertia_remainder;
        let whole = step.trunc();
        self.inertia_remainder = step - whole;
        if whole != 0.0 {
            self.set_value(self.value.saturating_add(whole as i32));
        }
        velocity *= (1.0 - SCROLLBAR_INERTIA_DECAY_PER_MS * delta_ms as f32).max(0.0);
        let at_bound = self.value <= self.minimum || self.value >= self.maximum;
        if velocity.abs() < SCROLLBAR_INERTIA_STOP_PX_PER_MS || at_bound {
            self.inertia_velocity = None;
            self.inertia_remainder = 0.0;
            return false;
        }
        self.inertia_velocity = Some(velocity);
        true
    }

    /// Whether the bar is between two opacities, gliding, or repeating a held arrow -- answers
    /// only, never advances.
    pub fn is_animating(&self) -> bool {
        self.opacity.is_moving() || self.inertia_velocity.is_some() || self.repeat.is_some()
    }

    /// Submits one interaction frame as damage bounded by the painted box plus the
    /// focus-ring and halo margin.
    ///
    /// Interaction paths pass `None` and let the runtime derive the box. A control that
    /// is unmounted falls back to [`BaseWidget::request_redraw`], which keeps the redraw
    /// signal contract for host-owned controls. An interaction frame in
    /// [`crate::widget::runtime::RepaintMode::Full`] still routes through
    /// [`crate::widget::runtime::request_repaint_rect`], whose documented fallback repaints
    /// the whole control.
    fn request_frame_repaint(&self, bounds: Option<Rect>) {
        let _ = bounds;
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.id()) {
                let narrow = crate::widget::runtime::repaint_mode(id)
                    != crate::widget::runtime::RepaintMode::Full;
                // Interaction paths may take the documented whole-control fallback in
                // `Full`; `bounds` lets a caller that already has the rectangle skip the
                // runtime lookup.
                if narrow || bounds.is_none() {
                    let damage = bounds
                        .or_else(|| crate::widget::runtime::painted_box_with_damage_margin(id))
                        .unwrap_or_else(|| self.damage_bounds());
                    crate::widget::runtime::request_repaint_rect(id, damage);
                    // The replaced `request_redraw` emitted this signal; keep it.
                    self.base.redraw_requested.emit();
                    return;
                }
            }
        }
        self.base.request_redraw();
    }

    /// This bar's own rectangle grown by
    /// [`crate::widget::metrics::DAMAGE_MARGIN`].
    ///
    /// Used when the runtime cannot read the mounted geometry back, which is the case
    /// while an interaction event holds the control out of the registry. Deriving the
    /// rectangle from `Widget::geometry` yields the same answer the runtime returns for
    /// the mounted control.
    #[cfg(not(alloc_frugal))]
    fn damage_bounds(&self) -> Rect {
        let rect = self.geometry();
        let margin = crate::widget::metrics::DAMAGE_MARGIN;
        let offset = margin as i32;
        Rect::new(
            rect.x.saturating_sub(offset),
            rect.y.saturating_sub(offset),
            rect.width.saturating_add(margin).saturating_add(margin),
            rect.height.saturating_add(margin).saturating_add(margin),
        )
    }
}
/// Scroll bar actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarAction {
    /// Move up/left by line
    LineUp,
    /// Move down/right by line
    LineDown,
    /// Move up/left by page
    PageUp,
    /// Move down/right by page
    PageDown,
    /// Move slider
    SliderMove,
    /// Move slider by page step up/right
    SliderPageStepAdd,
    /// Move slider by page step down/left
    SliderPageStepSub,
    /// Move slider to minimum
    SliderToMinimum,
    /// Move slider to maximum
    SliderToMaximum,
}
// Implement Widget trait
impl Widget for ScrollBar {
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
        let long = crate::widget::metrics::dimensions::SCROLLBAR_DEFAULT_LENGTH;
        let thick = self.band_thickness();
        match self.orientation {
            crate::layout::Orientation::Horizontal => crate::core::Size::new(long, thick),
            crate::layout::Orientation::Vertical => crate::core::Size::new(thick, long),
        }
    }

    // The idle fade is the control's own animation; the trait spelling is what the frame bus
    // reaches through `&mut dyn Widget`, which is the only way the fade actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        ScrollBar::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        ScrollBar::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |v| {
                    CapabilityValue::Int(*v as i64)
                }))
            }
            "slider_moved" => {
                Some(EventSignalRef::mapped("slider_moved", &self.slider_moved, |v| {
                    CapabilityValue::Int(*v as i64)
                }))
            }
            "slider_pressed" => Some(EventSignalRef::unit("slider_pressed", &self.slider_pressed)),
            "slider_released" => {
                Some(EventSignalRef::unit("slider_released", &self.slider_released))
            }
            _ => None,
        }
    }
}

/// `ScrollBar`'s property contract.
///
/// `slider_size` and `slider_position` are derived from the range and the page
/// step, so they are read-only, matching the schema and the old dispatch.
impl WidgetProperties for ScrollBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "single_step" => Ok(CapabilityValue::Int(self.single_step() as i64)),
            "page_step" => Ok(CapabilityValue::Int(self.page_step() as i64)),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            "slider_size" => Ok(CapabilityValue::Float(self.slider_size() as f64)),
            "slider_position" => Ok(CapabilityValue::Float(self.slider_position() as f64)),
            "idle_delay_ms" => Ok(CapabilityValue::UInt(u64::from(self.idle_delay_ms()))),
            "idle_opacity" => Ok(CapabilityValue::Float(f64::from(self.idle_opacity()))),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "minimum" => {
                self.set_minimum(expect_i32_saturating(value)?);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i32_saturating(value)?);
                Ok(())
            }
            "value" => {
                self.set_value(expect_i32_saturating(value)?);
                Ok(())
            }
            "single_step" => {
                self.set_single_step(expect_i32_saturating(value)?);
                Ok(())
            }
            "page_step" => {
                self.set_page_step(expect_i32_saturating(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            // Derived from the range: no setter exists, so the contract says so.
            "slider_size" | "slider_position" => Err(CapabilityAccessError::ReadOnlyProperty),
            "idle_delay_ms" => {
                self.set_idle_delay_ms(expect_u32(value)?);
                Ok(())
            }
            "idle_opacity" => {
                self.set_idle_opacity(expect_f32(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SCROLL_BAR_PROPERTIES`.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "single_step",
            "page_step",
            "orientation",
            "direction",
            "slider_size",
            "slider_position",
            "idle_delay_ms",
            "idle_opacity",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `scroll_bar` publishes.
    ///
    /// All four assign state: `set_range` the bounds, `set_value` the position,
    /// `set_steps` the two increments and `set_orientation` the axis. Each needs an
    /// argument a command carries none of, so the whole set is answered through the
    /// property route and refused here as [`CapabilityAccessError::OutOfRange`] rather
    /// than reported as unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_range" | "set_value" | "set_steps" | "set_orientation" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ScrollBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // The pointer arriving over the bar wakes it, because a reader whose pointer is on a
        // scrollbar is about to grab it. Read from the base's own hover fact rather than from a
        // second copy of the enter/leave arms, so this cannot drift from what `widget_state()`
        // reports. Value changes wake it too, through `set_value`.
        if self.base.is_hovered() {
            self.touch_activity();
        }
        match event {
            Event::MouseEnter { .. } | Event::MouseLeave { .. } => {
                // Hover widens an overlay bar, so the arrival and the departure both
                // change pixels even when the idle fade has nothing to advance.
                self.request_frame_repaint(None);
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                // An arrow cell is a line step, and holding it repeats. The press itself is
                // the first step, so a single click is exactly one line.
                if let Some(action) = self.arrow_action_at(*pos) {
                    self.repeat = Some(ArrowRepeat { action, held_ms: 0, fired: 0 });
                    self.dragging_thumb = false;
                    self.trigger_action(action);
                    self.request_frame_repaint(None);
                    return;
                }
                self.repeat = None;
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                self.drag_last_pixel = pixel;
                self.drag_pending_px = 0.0;
                self.drag_velocity_px_per_ms = 0.0;
                self.inertia_velocity = None;
                self.inertia_remainder = 0.0;
                self.dragging_thumb = self.thumb_rect().contains(*pos);
                if self.dragging_thumb {
                    // Grabbing the thumb selects the value under the pointer, which is the
                    // historical drag behaviour and stays the exact inverse of the drawing.
                    let value = self.pixel_pos_to_value(pixel);
                    self.set_value(value);
                } else {
                    // A trough click is a **page step in the click's direction**, not a jump to
                    // the clicked position: one visible page per click is the gesture every
                    // scrollbar publishes, and a jump made a mis-aimed click lose the reading
                    // position the user was holding.
                    let clicked = self.pixel_pos_to_value(pixel);
                    let action = if clicked < self.value {
                        Some(ScrollBarAction::PageUp)
                    } else if clicked > self.value {
                        Some(ScrollBarAction::PageDown)
                    } else {
                        None
                    };
                    if let Some(action) = action {
                        self.trigger_action(action);
                    }
                }
                self.request_frame_repaint(None);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.mouse_pressed = false;
                self.repeat = None;
                self.slider_released.emit();
                // A released thumb may keep gliding. A release with no sampled movement falls
                // back to the movement accumulated since the last frame so a fast flick that
                // happened entirely between two ticks is still carried.
                if self.inertia && self.dragging_thumb {
                    let mut velocity = self.drag_velocity_px_per_ms;
                    if velocity == 0.0 && self.drag_pending_px != 0.0 {
                        velocity = self.drag_pending_px / 16.0;
                    }
                    if velocity.abs() >= SCROLLBAR_INERTIA_STOP_PX_PER_MS {
                        self.inertia_velocity = Some(velocity);
                        self.inertia_remainder = 0.0;
                    }
                }
                self.dragging_thumb = false;
                self.drag_pending_px = 0.0;
                self.drag_velocity_px_per_ms = 0.0;
                self.request_frame_repaint(None);
            }
            Event::MouseMove { pos } if self.mouse_pressed && self.dragging_thumb => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                // Accumulated here and sampled in `tick`, which is the only place a time base
                // exists: a pointer move carries no timestamp.
                self.drag_pending_px += pixel - self.drag_last_pixel;
                self.drag_last_pixel = pixel;
                let value = self.pixel_pos_to_value(pixel);
                self.set_value(value);
                self.slider_moved.emit(value);
                self.request_frame_repaint(None);
            }
            Event::Wheel { delta, modifiers, .. } => {
                // This arm predates the surrounding work: a wheel notch already stepped the
                // value by one line, and it is kept as-is rather than reimplemented.
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    self.set_value(self.value + scroll.lines_y * self.single_step);
                    self.request_frame_repaint(None);
                }
            }
            // Keyboard stepping is reachable only while the bar holds focus: a scroll bar
            // embedded in a page must not consume the arrows of whatever the user is editing.
            Event::KeyPress { key, modifiers: _ } if self.base.focus_reason().is_some() => {
                match *key {
                    37 => {
                        // Left arrow (or up arrow for vertical)
                        self.trigger_action(ScrollBarAction::LineUp);
                    }
                    38 => {
                        // Up arrow (or right arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(ScrollBarAction::LineUp);
                        } else {
                            self.trigger_action(ScrollBarAction::LineDown);
                        }
                    }
                    39 => {
                        // Right arrow (or down arrow for vertical)
                        self.trigger_action(ScrollBarAction::LineDown);
                    }
                    40 => {
                        // Down arrow (or left arrow for horizontal)
                        if self.orientation == Orientation::Vertical {
                            self.trigger_action(ScrollBarAction::LineDown);
                        } else {
                            self.trigger_action(ScrollBarAction::LineUp);
                        }
                    }
                    33 => {
                        // Page up
                        self.trigger_action(ScrollBarAction::PageUp);
                    }
                    34 => {
                        // Page down
                        self.trigger_action(ScrollBarAction::PageDown);
                    }
                    36 => {
                        // Home
                        self.trigger_action(ScrollBarAction::SliderToMinimum);
                    }
                    35 => {
                        // End
                        self.trigger_action(ScrollBarAction::SliderToMaximum);
                    }
                    _ => { /* Other keys are not relevant */ }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
/// The active theme's window fill, or a light-theme default when no theme is installed.
///
/// Used only as a **guard**: a control whose resolved background equals this has not been given
/// a surface of its own, so painting it verbatim would make the control the window. Keeping
/// the query in one place means the guard reads the same fact every control does.
fn color_theme_window_fill() -> Color {
    crate::style::theme_manager()
        .current_theme()
        .map(|theme| theme.colors.background)
        .unwrap_or(Color::rgb(240, 240, 240))
}

impl Draw for ScrollBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        //
        // The **band**, not the control's rectangle: the trough is
        // [`dimensions::SCROLLBAR_THICKNESS`] thick and centred, exactly as `size_hint`
        // describes it. Painting `rect` made a 240x120 census cell a 240x120 trough with a
        // 240x120 *thumb* (`scroll_bar.svg` carried `<rect x="16" y="0" width="24"
        // height="120"/>` — a full-height slab, not a grip), which is a picture of a
        // scrollbar-shaped rectangle rather than a scrollbar.
        // The painted band is the measured band except in overlay style, where hovering widens
        // the ink only. Its long axis (and therefore every thumb position and length) is the
        // same, so the value/pixel mapping stays the one the hit test uses.
        let rect = self.paint_band();
        // The thumb is derived once, through `thumb_rect`, so the keyboard focus ring frames
        // the same rectangle the paint below fills.
        let thumb = self.thumb_rect();
        let style = self.style();
        // The slider is the control's *surface*; the arrows are its *foreground*.
        //
        // Both used to be literals, so a themed scrollbar kept a light trough and
        // dark glyphs in a dark theme: the theme resolved the colours, handed them
        // to the widget, and the widget painted its own grey anyway. The slider is a
        // background, and the arrows are ink over it, so they read `text_color` —
        // the colour the theme resolves for exactly that.
        // The **thumb** is the part the user drags; the **trough** is the track it travels in.
        //
        // Both used to read `style.background_color`. Since the theme resolves one
        // `(bg, fg, border)` triple per role, that made them the same colour by construction: two
        // byte-identical rectangles, so the control rendered as a plain bar with a movable
        // region the user could not see. The trough is the control's own surface and the thumb is
        // a raised affordance on it, so the thumb is derived one step away from the trough — the
        // same `!= window_fill` guard the slider uses to keep its own track off the window fill.
        // The theme's `Input` role (this control's role) already resolves the trough away from
        // the window colour; the guard covers a style that never met the theme.
        let window_fill = { color_theme_window_fill() };
        let trough = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => style
                .background_color
                .unwrap_or(Color::rgb(240, 240, 240))
                .blend(&style.text_color.unwrap_or(Color::rgb(100, 100, 100)), 0.06),
        };
        let slider_color = style
            .border_color
            .filter(|resolved| *resolved != trough)
            .unwrap_or_else(|| trough.blend(&trough.contrast_color(), 0.32));
        let slider_border_color = trough.blend(&slider_color, 0.5);
        let arrow_color = style.text_color.unwrap_or_else(|| trough.contrast_color());
        // The idle fade is applied as one multiply over every colour the bar paints, so the two
        // halves of the behaviour read from one value and no paint site can forget the fade. The
        // alpha is scaled rather than the colour *replaced*, so a colour that was already
        // translucent (a themed trough with its own alpha) keeps its own transparency.
        let fade = self.opacity.value();
        let faded = |color: Color| -> Color {
            if fade >= 1.0 {
                color
            } else {
                color.with_alpha((color.a as f32 * fade) as u8)
            }
        };
        let trough = faded(trough);
        let slider_color = faded(slider_color);
        let slider_border_color = faded(slider_border_color);
        let arrow_color = faded(arrow_color);
        // Draw background (the trough)
        context.face_with_gradient_corners(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            trough,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().resolved_corner_radii().to_array(),
            Color::BLACK,
        );
        // Draw border, rounded to the trough's own face: a square 1 px stroke over the pill
        // groove left the four corners outside the drawn surface.
        crate::widget::draw::stroke_surface_border(
            context,
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            self.style().resolved_corner_radii().to_array(),
            style.border_color.unwrap_or_else(|| trough.contrast_color().with_alpha(80)),
            1,
        );
        // Draw slider
        //
        // The grip is [`Self::thumb_rect`]'s answer: `slider_size` of the *travel*, but never
        // shorter than [`dimensions::SCROLLBAR_MIN_LENGTH`], so a proportional thumb with no
        // floor cannot vanish on a very long document. That derivation is shared with the
        // keyboard focus ring below, so the ring frames the grip that was painted.
        match self.orientation {
            Orientation::Horizontal => {
                context.fill_rect(thumb, slider_color);
                // Draw slider border
                context.draw_rect(thumb, slider_border_color);
                // Draw arrows using draw_line (triangles approximated)
                //
                // The arrow cell is sized from the control's **thickness**, not its length:
                // `rect.width * 0.2` on a 240 px horizontal bar produced a 48 px arrow — longer
                // than the space before the thumb, so the left arrow's apex crossed underneath
                // the slider (`scroll_bar.svg` had its apex at x = 48 while the thumb began at
                // x = 24). The derivation lives in `arrow_cell()`, which the thumb's travel
                // reads too, so the two cannot disagree.
                let arrow_size = self.arrow_cell() as u32;
                // An overlay bar has no arrow cells; skipping the glyphs rather than drawing
                // zero-length lines is what keeps a stray mark off the trough's ends.
                if arrow_size > 0 {
                    // The arrows sit on the band's own middle line, so a centred band draws them
                    // on the trough rather than on the control's edges.
                    let mid_y = rect.y as f32 + rect.height as f32 / 2.0;
                    // Left arrow head
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + arrow_size as f32 / 2.0, mid_y),
                        Point::from_f32(
                            rect.x as f32 + arrow_size as f32,
                            rect.y as f32 + rect.height as f32 / 4.0,
                        ),
                        arrow_color,
                    );
                    context.draw_line(
                        Point::from_f32(rect.x as f32 + arrow_size as f32 / 2.0, mid_y),
                        Point::from_f32(
                            rect.x as f32 + arrow_size as f32,
                            rect.y as f32 + rect.height as f32 * 3.0 / 4.0,
                        ),
                        arrow_color,
                    );
                    // Right arrow head
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - arrow_size as f32 / 2.0,
                            mid_y,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - arrow_size as f32,
                            rect.y as f32 + rect.height as f32 / 4.0,
                        ),
                        arrow_color,
                    );
                    context.draw_line(
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - arrow_size as f32 / 2.0,
                            mid_y,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 - arrow_size as f32,
                            rect.y as f32 + rect.height as f32 * 3.0 / 4.0,
                        ),
                        arrow_color,
                    );
                }
            }
            Orientation::Vertical => {
                context.fill_rect(thumb, slider_color);
                // Draw slider border
                context.draw_rect(thumb, slider_border_color);
                // Draw arrows using draw_line (triangles approximated)
                let arrow_size = self.arrow_cell() as u32;
                // No arrow cells on the overlay shape; see the horizontal arm.
                if arrow_size > 0 {
                    // On the band's own middle line, as above.
                    let mid_x = rect.x as f32 + rect.width as f32 / 2.0;
                    // Up arrow head
                    context.draw_line(
                        Point::from_f32(mid_x, rect.y as f32 + arrow_size as f32 / 2.0),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 / 4.0,
                            rect.y as f32 + arrow_size as f32,
                        ),
                        arrow_color,
                    );
                    context.draw_line(
                        Point::from_f32(mid_x, rect.y as f32 + arrow_size as f32 / 2.0),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 * 3.0 / 4.0,
                            rect.y as f32 + arrow_size as f32,
                        ),
                        arrow_color,
                    );
                    // Down arrow head
                    context.draw_line(
                        Point::from_f32(
                            mid_x,
                            rect.y as f32 + rect.height as f32 - arrow_size as f32 / 2.0,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 / 4.0,
                            rect.y as f32 + rect.height as f32 - arrow_size as f32,
                        ),
                        arrow_color,
                    );
                    context.draw_line(
                        Point::from_f32(
                            mid_x,
                            rect.y as f32 + rect.height as f32 - arrow_size as f32 / 2.0,
                        ),
                        Point::from_f32(
                            rect.x as f32 + rect.width as f32 * 3.0 / 4.0,
                            rect.y as f32 + rect.height as f32 - arrow_size as f32,
                        ),
                        arrow_color,
                    );
                }
            }
        }

        // ── Keyboard focus ring ──
        //
        // The thumb is what the arrow keys and Home/End move, so the ring frames it rather
        // than the whole trough. Gated on the *reason* focus arrived: a pointer press focuses
        // the bar without painting a ring under the cursor, while Tab and Shortcut draw one.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(thumb, 0);
            if ring.is_drawable() {
                // The fallback is the thumb's contrasting tone; an active theme overrides it
                // with its declared `outline` token.
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(slider_color.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::String;
    use crate::core::{Color, Orientation, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn scrollbar_creation_defaults() {
        let sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(sb.minimum(), 0);
        assert_eq!(sb.maximum(), 100);
        assert_eq!(sb.value(), 0);
        assert_eq!(sb.single_step(), 1);
        assert_eq!(sb.page_step(), 10);
        assert_eq!(sb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn scrollbar_set_value() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        assert_eq!(sb.value(), 50);
        sb.set_value(200); // clamp to max
        assert_eq!(sb.value(), 100);
        sb.set_value(-10); // clamp to min
        assert_eq!(sb.value(), 0);
    }

    #[test]
    fn scrollbar_set_range() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_minimum(10);
        sb.set_maximum(200);
        assert_eq!(sb.minimum(), 10);
        assert_eq!(sb.maximum(), 200);
    }

    #[test]
    fn scrollbar_set_range_reclamps_value() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        sb.set_range(60, 100);
        assert_eq!(sb.value(), 60);
    }

    #[test]
    fn scrollbar_single_step() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_single_step(5);
        assert_eq!(sb.single_step(), 5);
        sb.set_single_step(0); // floors at 1
        assert_eq!(sb.single_step(), 1);
    }

    #[test]
    fn scrollbar_page_step() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_page_step(25);
        assert_eq!(sb.page_step(), 25);
        sb.set_page_step(0); // floors at 1
        assert_eq!(sb.page_step(), 1);
    }

    #[test]
    fn scrollbar_orientation() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_orientation(Orientation::Vertical);
        assert_eq!(sb.orientation(), Orientation::Vertical);
        sb.set_orientation(Orientation::Horizontal);
        assert_eq!(sb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn scrollbar_slider_position() {
        let sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!((sb.slider_position() - 0.0).abs() < f32::EPSILON);

        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);
        assert!((sb.slider_position() - 0.5).abs() < f32::EPSILON);

        sb.set_value(100);
        assert!((sb.slider_position() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scrollbar_trigger_action() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_value(50);

        sb.trigger_action(ScrollBarAction::LineUp);
        assert_eq!(sb.value(), 49);

        sb.trigger_action(ScrollBarAction::LineDown);
        assert_eq!(sb.value(), 50);

        sb.trigger_action(ScrollBarAction::PageUp);
        assert_eq!(sb.value(), 40);

        sb.trigger_action(ScrollBarAction::PageDown);
        assert_eq!(sb.value(), 50);

        sb.trigger_action(ScrollBarAction::SliderToMinimum);
        assert_eq!(sb.value(), 0);

        sb.trigger_action(ScrollBarAction::SliderToMaximum);
        assert_eq!(sb.value(), 100);
    }

    #[test]
    fn scrollbar_geometry_delegation() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(sb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn scrollbar_visibility() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.is_visible());
        sb.hide();
        assert!(!sb.is_visible());
        sb.show();
        assert!(sb.is_visible());
    }

    #[test]
    fn scrollbar_enabled() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.is_enabled());
        sb.set_enabled(false);
        assert!(!sb.is_enabled());
        sb.set_enabled(true);
        assert!(sb.is_enabled());
    }

    #[test]
    fn scrollbar_tooltip_roundtrip() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert!(sb.tooltip().is_empty());
        sb.set_tooltip("Scroll".to_string());
        assert_eq!(sb.tooltip(), "Scroll");
        sb.set_tooltip(String::new());
        assert!(sb.tooltip().is_empty());
    }

    #[test]
    fn scrollbar_style_roundtrip() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(*sb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(200, 200, 200));
        // The bar's own `set_style` selects the shape; the resolved widget style goes through
        // the `Widget` contract with a fully qualified call.
        crate::widget::Widget::set_style(&mut sb, custom.clone());
        assert_eq!(*sb.style(), custom);
    }

    #[test]
    fn scrollbar_id_kind() {
        let sb_a = ScrollBar::new(Rect::new(0, 0, 100, 16));
        let sb_b = ScrollBar::new(Rect::new(0, 0, 100, 16));
        assert_ne!(sb_a.id(), sb_b.id());
        assert_eq!(sb_a.kind(), WidgetKind::ScrollBar);
        assert_eq!(sb_b.kind(), WidgetKind::ScrollBar);
    }

    #[test]
    fn scrollbar_signal_accessors() {
        let sb = ScrollBar::new(Rect::new(0, 0, 100, 16));
        let _value_changed = &sb.value_changed;
        let _slider_moved = &sb.slider_moved;
        let _slider_pressed = &sb.slider_pressed;
        let _slider_released = &sb.slider_released;
    }

    /// A right-to-left trough puts its **minimum** at the right end.
    ///
    /// # The defect this pins
    ///
    /// The bar maps a value onto a line, and the line it maps onto runs from where the reader starts
    /// to where they finish. Reading the trough left-to-right unconditionally put the thumb at the
    /// mirrored position in an Arabic or Hebrew interface, so "scrolled to the end" looked like
    /// "scrolled to the start".
    #[test]
    fn a_right_to_left_trough_puts_the_minimum_on_the_right() {
        use crate::core::TextDirection;
        let geometry = Rect::new(0, 0, 200, 16);
        let mut ltr = ScrollBar::new(geometry);
        let mut rtl = ScrollBar::new(geometry);
        for sb in [&mut ltr, &mut rtl] {
            sb.set_range(0, 100);
            sb.set_value(0);
        }
        rtl.set_direction(TextDirection::RightToLeft);

        let band = ltr.track_band();
        assert_eq!(band, rtl.track_band(), "the direction must not move the trough itself");
        let left_end = band.x as f32 + 1.0;
        let right_end = band.right() as f32 - 1.0;

        // The trough's two ends are the two extremes, and the direction decides which is which.
        assert!(
            ltr.pixel_pos_to_value(left_end) < ltr.pixel_pos_to_value(right_end),
            "LTR: values must grow left to right"
        );
        assert!(
            rtl.pixel_pos_to_value(left_end) > rtl.pixel_pos_to_value(right_end),
            "RTL: values must grow right to left"
        );

        // And the thumb follows: the RTL bar parks the minimum to the right of the maximum.
        assert!(
            rtl.value_to_pixel_pos(0) > rtl.value_to_pixel_pos(100),
            "an RTL bar must draw the minimum to the right of the maximum"
        );
        assert!(
            ltr.value_to_pixel_pos(0) < ltr.value_to_pixel_pos(100),
            "the default direction must be unchanged"
        );
    }

    /// The pointer → value and value → pointer mappings must stay **ordered the same way** in both
    /// directions, so a drag toward the end the user is heading for raises the value.
    ///
    /// The direction is what this test is about; the two functions being exact inverses is pinned
    /// separately by [`the_two_conversions_are_exact_inverses`] (BLUE22 · G-2).
    #[test]
    fn the_value_and_pixel_mappings_agree_about_which_way_is_forward() {
        use crate::core::TextDirection;
        for direction in [TextDirection::LeftToRight, TextDirection::RightToLeft] {
            let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
            sb.set_range(0, 1000);
            sb.set_direction(direction);
            for value in (0..1000).step_by(50) {
                let here = sb.value_to_pixel_pos(value);
                let next = sb.value_to_pixel_pos(value + 50);
                let forward = next - here;
                assert!(
                    forward.abs() > 0.0,
                    "{direction:?}: value {value} and {} drew at the same pixel",
                    value + 50
                );
                let expected_sign = if direction.is_right_to_left() { -1.0 } else { 1.0 };
                assert_eq!(
                    forward.signum(),
                    expected_sign,
                    "{direction:?}: {value} → {} must move {expected_sign}px, moved {forward}",
                    value + 50
                );
            }
            // And reading the pixel back returns the value itself, not merely the same half of the
            // range: the two halves agreeing was all the pre-G-2 mapping could promise, because it
            // measured the trough by two different rules.
            for value in (0..=1000).step_by(50) {
                let back = sb.pixel_pos_to_value(sb.value_to_pixel_pos(value));
                assert_eq!(
                    back, value,
                    "{direction:?}: value {value} was drawn and read back as {back}"
                );
            }
        }
    }

    /// The two conversions are **exact inverses** of each other, in both orientations.
    ///
    /// # The defect this pins (BLUE22 · G-2)
    ///
    /// `pixel_pos_to_value` measured the pointer from the band's edge across the band's whole
    /// length, while `value_to_pixel_pos` skipped an arrow cell and scaled the remainder by
    /// `1 - slider_size`. Two different rules over the same trough cannot be inverses: a value
    /// drawn at `x` read back as a different value, so clicking the thumb did not select the value
    /// it was showing, and the error grew with the range rather than being a rounding artefact.
    ///
    /// `slider` had already been through this and fixed it by sharing the inset. The rule is the
    /// same here, and it is stated in one place — [`ScrollBar::travel_band`] — which both functions
    /// now read. This test is what keeps a future edit from re-deriving either half.
    #[test]
    fn the_two_conversions_are_exact_inverses() {
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            let geometry = match orientation {
                Orientation::Horizontal => Rect::new(0, 0, 200, 16),
                Orientation::Vertical => Rect::new(0, 0, 16, 200),
            };
            // Several ranges, because `slider_size()` is a fraction of the range: the old defect was
            // invisible for one range and large for another, which is exactly why it survived.
            for (minimum, maximum) in [(0, 100), (0, 1000), (-50, 50), (0, 20)] {
                let mut sb = ScrollBar::new(geometry);
                sb.set_orientation(orientation);
                sb.set_range(minimum, maximum);
                let span = maximum - minimum;
                for step in 0..=10 {
                    let value = minimum + span * step / 10;
                    let drawn_at = sb.value_to_pixel_pos(value);
                    let read_back = sb.pixel_pos_to_value(drawn_at);
                    assert_eq!(
                        read_back, value,
                        "{orientation:?} range {minimum}..{maximum}: {value} was drawn at \
                         {drawn_at} but read back as {read_back}"
                    );
                }
            }
        }
    }

    /// A trough too narrow to hold its own arrows still answers both conversions, and answers them
    /// consistently.
    ///
    /// The travel collapses to zero, so there is no position that means anything other than the
    /// minimum. The pre-G-2 pair disagreed even here — the reader divided by a positive number while
    /// the writer had already run out of room — so the degenerate trough is where the two halves were
    /// furthest apart, not where they were safest.
    #[test]
    fn a_trough_with_no_travel_answers_both_conversions_with_the_minimum() {
        // 8 px wide with 4 px arrow cells leaves nothing between them, and the 48 px thumb floor is
        // clamped to the span, so the travel is zero.
        let mut sb = ScrollBar::new(Rect::new(0, 0, 8, 16));
        sb.set_range(0, 100);
        let (origin, travel) = sb.travel_band();
        assert_eq!(travel, 0.0, "this trough must have no travel for the test to mean anything");
        assert_eq!(sb.value_to_pixel_pos(0), origin);
        assert_eq!(sb.value_to_pixel_pos(100), origin);
        assert_eq!(sb.pixel_pos_to_value(origin), 0);
        assert_eq!(sb.pixel_pos_to_value(origin + 500.0), 0);
    }

    /// The thumb's drawn position, its drawn length and the travel all agree with one another.
    ///
    /// This is the geometric statement behind the inverse property: the thumb starts at the travel's
    /// origin for the minimum, and its **trailing edge** reaches the far end of the trough for the
    /// maximum. The old mapping placed the maximum's thumb so that the thumb itself could not finish
    /// its journey — the reader and the writer disagreed about where the end was.
    #[test]
    fn the_thumb_starts_at_one_end_and_finishes_at_the_other() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_range(0, 1000);
        let band = sb.track_band();
        let cell = sb.arrow_cell();
        let (origin, travel) = sb.travel_band();
        let thumb = sb.thumb_length();

        assert_eq!(
            sb.value_to_pixel_pos(0),
            origin,
            "the minimum's thumb begins where the travel begins"
        );
        assert_eq!(
            sb.value_to_pixel_pos(1000),
            origin + travel,
            "the maximum's thumb ends where the travel ends"
        );
        // The far end of the travel plus the thumb is the trough's own far edge, so the grip the user
        // sees never leaves the track it runs in.
        assert_eq!(
            origin + travel + thumb,
            band.x as f32 + band.width as f32 - cell,
            "the thumb's trailing edge must stop at the trough's far arrow cell"
        );
    }

    /// The vertical bar is the block axis, which a right-to-left script does not reverse, so the
    /// direction must not move its thumb.
    #[test]
    fn a_vertical_trough_ignores_the_direction() {
        use crate::core::TextDirection;
        let geometry = Rect::new(0, 0, 16, 200);
        let mut ltr = ScrollBar::new(geometry);
        let mut rtl = ScrollBar::new(geometry);
        for sb in [&mut ltr, &mut rtl] {
            sb.set_orientation(Orientation::Vertical);
            sb.set_range(0, 100);
        }
        rtl.set_direction(TextDirection::RightToLeft);
        for value in [0, 25, 50, 100] {
            assert_eq!(
                ltr.value_to_pixel_pos(value),
                rtl.value_to_pixel_pos(value),
                "a vertical bar moved when only the direction changed (value {value})"
            );
        }
    }

    /// The direction is reachable from the property API and the token written is the token read back.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(sb.get("direction").unwrap().as_str(), Some("ltr"));
        sb.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
        assert_eq!(sb.direction(), crate::core::TextDirection::RightToLeft);
        assert_eq!(sb.get("direction").unwrap().as_str(), Some("rtl"));
    }

    /// An untouched bar is fully opaque and owes no frames.
    ///
    /// The resting end, asserted rather than inferred: a bar built at the *idle* opacity would
    /// fade *in* on its first frame, and a bar whose idle clock started mid-way would begin
    /// dimming before the reader had done anything. This is the crate's single answer to
    /// "what is the first value", and it is also what keeps `scroll_bar.svg` unchanged. Note that
    /// "owes no frames" is about the *first* moment: after the idle delay the bar does begin to
    /// fade, which is the whole point — the delay starts when the bar appears.
    #[test]
    fn a_freshly_built_bar_is_opaque_and_still() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(sb.opacity(), 1.0, "a bar that has just appeared must not be dimmed");
        assert!(!sb.is_animating(), "so it owes no frames");
        assert!(!sb.tick(16), "nor on its first frame");
        assert_eq!(sb.opacity(), 1.0, "and it is still opaque well inside the delay");
    }

    /// The bar holds full opacity through the idle delay, then fades to a visible floor.
    ///
    /// # The defect this pins
    ///
    /// The bar had no idle behaviour at all: it was permanently opaque chrome competing with the
    /// content it frames. The assertion is the two-phase shape itself, because both halves are
    /// load-bearing and they fail differently — a fade with no delay dims the bar while the reader
    /// is still looking at what they scrolled to, and a delay with no fade never gets out of the
    /// way. The middle sample ("still opaque once the delay has just elapsed") is what separates
    /// them: it is a value a fade-on-first-idle-millisecond cannot produce.
    #[test]
    fn the_bar_holds_then_fades_after_the_idle_delay() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        // Scrolling wakes it, and it is already at full opacity, so there is nothing to see yet.
        sb.set_value(50);
        assert_eq!(sb.opacity(), 1.0);
        assert!(!sb.is_animating(), "waking an opaque bar is not an animation");

        // Still fully opaque at the moment the delay elapses — this is the sample a fade with no
        // delay fails, because it would already be part-way down.
        assert!(!sb.tick(SCROLLBAR_IDLE_DELAY_MS), "the hold is not an animation");
        assert_eq!(sb.opacity(), 1.0, "the bar must hold through the whole idle delay");

        // Past the delay it starts to move, and it moves *downward* toward the floor.
        assert!(sb.tick(60), "past the delay the bar owes frames");
        let mid = sb.opacity();
        assert!(
            mid < 1.0 && mid > SCROLLBAR_IDLE_OPACITY,
            "the fade must pass through interior opacities (got {mid})"
        );
        while sb.tick(60) {}
        assert_eq!(sb.opacity(), SCROLLBAR_IDLE_OPACITY, "and settle at the visible floor");
        assert!(sb.opacity() > 0.0, "an invisible scrollbar gives no hint the region scrolls");

        // Interacting again wakes the bar: the idle clock restarts immediately and the opacity
        // animates back up from wherever the fade got to. Note the two are separate facts — the
        // clock restarts at once (so the bar stops fading), while the opacity *rises* over the
        // frames that follow, which is a fade-in rather than a snap.
        sb.set_value(60);
        assert!(sb.is_animating(), "a wake starts fading back up");
        while sb.tick(60) {}
        assert_eq!(sb.opacity(), 1.0, "and it returns to full opacity");
        // The wake restarted the clock, so the bar is opaque for a *fresh* delay rather than
        // resuming the old one. Sampled inside that window: after a full delay and a bit, it will
        // have started fading again, which is the cycle working rather than a defect.
        sb.set_value(70);
        assert_eq!(sb.opacity(), 1.0);
        assert!(!sb.tick(SCROLLBAR_IDLE_DELAY_MS - 1), "a fresh delay begins from the wake");
        assert_eq!(sb.opacity(), 1.0, "so it is opaque throughout that delay");
    }

    /// The fade reaches the pixels: a dimmed bar paints translucent ink, not opaque ink.
    ///
    /// This is the assertion that separates "the opacity is stored" from "the opacity is drawn"
    /// — the defect shape this crate has hit repeatedly (`audio_visualizer`, `data_grid`, the
    /// bezier handles). It reads the emitted SVG for a partial alpha rather than comparing whole
    /// documents, so a bar that renders differently for some unrelated reason cannot satisfy it.
    #[test]
    fn the_fade_reaches_the_painted_ink() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        use crate::core::Size;
        use crate::render::{PaintBackend, RenderContext, SvgPaintBackend};

        let render = |bar: &mut ScrollBar| -> String {
            let mut backend = SvgPaintBackend::new(Size::new(200, 16));
            backend.begin_frame(Color::rgb(0, 0, 0));
            {
                let mut ctx = RenderContext::new(&mut backend);
                bar.draw(&mut ctx);
            }
            backend.end_frame();
            backend.finish()
        };

        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 1000);
        bar.set_value(500);
        let active = render(&mut bar);

        // Drive the fade to its floor and render the same bar again.
        bar.tick(SCROLLBAR_IDLE_DELAY_MS);
        while bar.tick(60) {}
        assert_eq!(bar.opacity(), SCROLLBAR_IDLE_OPACITY);
        let idle = render(&mut bar);

        assert_ne!(active, idle, "a dimmed bar must not paint like an active one");
        // The frame background is the first fill the *backend* emits, so the bar's own trough is
        // the second. Naming the trough rather than "any fill" is what keeps the assertion from
        // being satisfied by the background the harness paints (the sampling mistake this crate's
        // log records repeatedly).
        let nth_fill = |svg: &str, n: usize| -> String {
            let mut search = 0usize;
            for _ in 0..=n {
                let Some(offset) = svg[search..].find("fill=\"") else {
                    return String::new();
                };
                search += offset + 6;
            }
            let rest = &svg[search..];
            rest[..rest.find('"').unwrap_or(0)].to_string()
        };
        let active_fill = nth_fill(&active, 1);
        let idle_fill = nth_fill(&idle, 1);
        assert!(
            active_fill.ends_with("1.00)"),
            "an active bar paints opaque ink (got {active_fill})"
        );
        assert!(
            !idle_fill.ends_with("1.00)"),
            "an idle bar must paint translucent ink, not the same opaque fill (got {idle_fill})"
        );
    }

    /// `set_orientation` asks for a frame.
    ///
    /// The orientation selects which axis the bar measures and which way its arrows point, so the
    /// paint path and the hit test both branch on it — a silent write left the old trough drawn.
    #[test]
    fn setting_orientation_requests_a_redraw() {
        let mut sc = ScrollBar::new(Rect::new(0, 0, 200, 16));
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = sc.connection_scope();
            sc.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        let other = if sc.orientation() == Orientation::Horizontal {
            Orientation::Vertical
        } else {
            Orientation::Horizontal
        };
        sc.set_orientation(other);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst), "a new orientation must be drawn");

        fired.store(false, std::sync::atomic::Ordering::SeqCst);
        sc.set_orientation(other);
        assert!(
            !fired.load(std::sync::atomic::Ordering::SeqCst),
            "setting the same value must not repaint"
        );
    }

    /// The desktop shape stays the default and keeps its arrow cells.
    #[test]
    fn the_default_scrollbar_style_is_arrows() {
        let sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        assert_eq!(sb.bar_style(), ScrollBarStyle::Arrows);
        assert!(sb.arrow_cell() > 0.0, "the desktop shape reserves cells for its arrows");
        assert_eq!(sb.size_hint().height, dimensions::SCROLLBAR_THICKNESS);
    }

    /// The overlay shape has no arrow cells, so the thumb can use the whole band.
    #[test]
    fn an_overlay_bar_travels_the_whole_band_and_the_conversions_are_inverse() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_style(ScrollBarStyle::Overlay);
        sb.set_range(0, 1000);

        assert_eq!(sb.arrow_cell(), 0.0, "an overlay bar paints no arrows");
        let band = sb.track_band();
        assert_eq!(band.width, 200, "both ends of the trough are travel, not arrow cells");
        let (origin, travel) = sb.travel_band();
        assert_eq!(origin, band.x as f32, "travel starts at the trough's own edge");
        assert_eq!(
            origin + travel + sb.thumb_length(),
            band.x as f32 + band.width as f32,
            "the thumb reaches the far edge with no arrow cell to stop at"
        );

        for value in [0, 250, 500, 750, 1000] {
            let drawn_at = sb.value_to_pixel_pos(value);
            assert_eq!(sb.pixel_pos_to_value(drawn_at), value, "value {value} must round-trip");
        }
    }

    /// An idle overlay bar fades to fully transparent, and an interaction brings it back.
    #[test]
    fn an_overlay_bar_fades_all_the_way_out_when_idle() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_style(ScrollBarStyle::Overlay);
        sb.set_value(50);

        assert!(!sb.tick(SCROLLBAR_IDLE_DELAY_MS), "the hold is not an animation");
        assert_eq!(sb.opacity(), 1.0, "the bar holds through the idle delay");
        assert!(sb.tick(60), "past the delay the bar owes frames");
        while sb.tick(60) {}
        assert_eq!(sb.opacity(), 0.0, "an overlay bar disappears entirely at rest");

        sb.set_value(60);
        assert!(sb.is_animating(), "a scroll fades the bar back in");
        while sb.tick(60) {}
        assert_eq!(sb.opacity(), 1.0, "and it returns to full strength");
    }

    /// Hover widens the painted overlay band only; the measured band is unchanged.
    #[test]
    fn hovering_widens_the_overlay_paint_but_not_the_measured_band() {
        let mut sb = ScrollBar::new(Rect::new(0, 0, 200, 16));
        sb.set_style(ScrollBarStyle::Overlay);
        let resting_paint = sb.paint_band();
        let resting_band = sb.track_band();
        assert_eq!(resting_paint.height, dimensions::SCROLLBAR_OVERLAY_THICKNESS);
        assert_eq!(resting_band.height, dimensions::SCROLLBAR_OVERLAY_THICKNESS);

        sb.handle_event(&Event::MouseEnter { pos: Point::new(100, 8) });
        assert_eq!(
            sb.paint_band().height,
            dimensions::SCROLLBAR_OVERLAY_HOVER_THICKNESS,
            "hovering thickens the drawn band"
        );
        assert_eq!(
            sb.track_band().height,
            dimensions::SCROLLBAR_OVERLAY_THICKNESS,
            "but the hit/travel band must not move with it"
        );

        sb.handle_event(&Event::MouseLeave { pos: Point::new(100, 8) });
        assert_eq!(sb.paint_band().height, dimensions::SCROLLBAR_OVERLAY_THICKNESS);
    }

    /// Keyboard focus paints the ring on the thumb; a pointer press does not.
    ///
    /// The bar reported `WidgetState::Focused` for keyboard focus but painted no ring, so the
    /// state had no visible consumer. The ring is the only 2 px stroke the control emits,
    /// which makes its presence observable in the render.
    #[test]
    fn a_keyboard_focused_bar_paints_the_ring_and_pointer_focus_does_not() {
        // The ring's colour reads the process-wide theme; hold the guard so a concurrent
        // appearance switch cannot change this render mid-assertion.
        let _guard = crate::style::theme_test_guard();
        let make = || {
            let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
            bar.set_range(0, 1000);
            bar.set_value(500);
            bar
        };

        let mut keyboard = make();
        keyboard.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        assert!(keyboard.base.draws_focus_ring(), "Tab focus must ask for a ring");
        let focused = crate::widget::svg::render_to_svg(&mut keyboard);
        assert!(
            focused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "keyboard focus must paint the ring around the thumb: {focused}"
        );

        let mut pointer = make();
        pointer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        let unfocused = crate::widget::svg::render_to_svg(&mut pointer);
        assert!(
            !unfocused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "pointer focus must not paint a ring: {unfocused}"
        );
    }

    /// The ring frames exactly the thumb the paint pass fills.
    #[test]
    fn the_focus_ring_follows_the_painted_thumb() {
        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 1000);
        bar.set_value(0);
        let at_start = bar.thumb_rect();
        bar.set_value(1000);
        let at_end = bar.thumb_rect();
        assert!(
            at_end.x > at_start.x,
            "the thumb must travel with the value: {:?} vs {:?}",
            at_end,
            at_start
        );
        assert_eq!(at_end.y, at_start.y, "a horizontal bar's thumb keeps one row");
        assert_eq!(
            at_start.height,
            bar.paint_band().height,
            "the ring must frame the painted grip, not a second estimate"
        );
    }

    /// A wheel notch moves the value by one single step and clamps at both ends.
    #[test]
    fn the_wheel_steps_the_value_by_one_line_and_clamps() {
        use crate::event::{Event, EventHandler};

        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 10);
        bar.set_value(5);

        bar.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(bar.value(), 6);
        bar.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(bar.value(), 5);

        bar.set_value(10);
        bar.handle_event(&Event::wheel(0, 1, 0));
        assert_eq!(bar.value(), 10, "the maximum clamps the wheel");
        bar.set_value(0);
        bar.handle_event(&Event::wheel(0, -1, 0));
        assert_eq!(bar.value(), 0, "the minimum clamps the wheel");
    }

    /// A wheel interaction submits damage bounded by the painted box and the margin.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_wheel_frame_submits_the_painted_box_with_the_damage_margin() {
        use crate::widget::runtime;

        let id =
            runtime::register(crate::compat::Box::new(ScrollBar::new(Rect::new(0, 0, 200, 16))))
                .expect("registry");
        assert!(runtime::set_repaint_mode(id, runtime::RepaintMode::Dirty));

        assert!(runtime::dispatch_event(id, &Event::wheel(0, 1, 0)));

        let expected = runtime::painted_box_with_damage_margin(id).expect("painted box");
        assert_eq!(
            runtime::dirty_rects(id),
            vec![expected],
            "a value frame must damage the painted box plus the margin"
        );
        runtime::unregister(id);
    }

    /// `Full` keeps the whole-control path and records no damage rectangle.
    #[cfg(not(alloc_frugal))]
    #[test]
    fn a_wheel_frame_in_full_mode_keeps_the_whole_control_repaint() {
        use crate::widget::runtime;

        let id =
            runtime::register(crate::compat::Box::new(ScrollBar::new(Rect::new(0, 0, 200, 16))))
                .expect("registry");
        assert_eq!(runtime::repaint_mode(id), runtime::RepaintMode::Full);

        assert!(runtime::dispatch_event(id, &Event::wheel(0, 1, 0)));
        assert!(
            runtime::dirty_rects(id).is_empty(),
            "Full mode records no damage, so the control repaints whole"
        );
        runtime::unregister(id);
    }

    /// A trough click moves by exactly one page, in the click's direction.
    ///
    /// The old handler mapped the click's pixel straight to a value, so a click in the empty
    /// trough teleported the thumb onto the pointer. A scrollbar pages instead: the reading
    /// position the user was holding is preserved and moves by one visible page.
    #[test]
    fn a_trough_click_pages_instead_of_jumping() {
        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 100);
        bar.set_value(50);
        assert_eq!(bar.page_step(), 10);

        // A point to the right of the thumb (and clear of the 16 px arrow cell).
        bar.handle_event(&Event::MousePress { pos: Point::new(160, 8), button: 1, modifiers: 0 });
        assert_eq!(bar.value(), 60, "a trough click beyond the thumb pages down");

        // The page step, not the distance to the pointer, is what moved it.
        bar.handle_event(&Event::MouseRelease { pos: Point::new(160, 8), button: 1 });
        bar.handle_event(&Event::MousePress { pos: Point::new(30, 8), button: 1, modifiers: 0 });
        assert_eq!(bar.value(), 50, "a trough click before the thumb pages up");
    }

    /// An arrow cell is a line step per click, and holding it repeats while it is held.
    #[test]
    fn an_arrow_cell_steps_a_line_and_repeats_while_held() {
        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 100);
        bar.set_value(50);

        // The left cell of a left-to-right bar steps toward the minimum.
        bar.handle_event(&Event::MousePress { pos: Point::new(8, 8), button: 1, modifiers: 0 });
        assert_eq!(bar.value(), 49, "the press itself is one line step");
        assert!(bar.is_animating(), "a held arrow owes frames for its repeats");

        assert!(bar.tick(SCROLLBAR_ARROW_REPEAT_DELAY_MS), "holding repeats");
        assert_eq!(bar.value(), 48, "the first repeat fires when the delay elapses");
        assert!(bar.tick(SCROLLBAR_ARROW_REPEAT_INTERVAL_MS * 3), "the repeat continues");
        assert_eq!(bar.value(), 45, "three intervals are three more steps");

        bar.handle_event(&Event::MouseRelease { pos: Point::new(8, 8), button: 1 });
        assert!(!bar.is_animating(), "a released arrow stops repeating");
        bar.tick(SCROLLBAR_ARROW_REPEAT_INTERVAL_MS * 3);
        assert_eq!(bar.value(), 45, "and no step lands after the release");
    }

    /// A released thumb keeps gliding when inertia is on, and settles.
    #[test]
    fn a_released_thumb_glides_briefly_with_inertia() {
        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 16));
        bar.set_range(0, 1000);
        bar.set_value(100);
        assert!(!bar.inertia(), "inertia is opt-in");
        bar.set_inertia(true);
        assert!(bar.inertia());

        let thumb = bar.thumb_rect();
        let grab = Point::new(thumb.x + thumb.width as i32 / 2, 8);
        bar.handle_event(&Event::MousePress { pos: grab, button: 1, modifiers: 0 });

        // Two moving frames build a velocity estimate (a pointer move carries no timestamp).
        bar.handle_event(&Event::MouseMove { pos: Point::new(grab.x + 10, 8) });
        bar.tick(16);
        bar.handle_event(&Event::MouseMove { pos: Point::new(grab.x + 20, 8) });
        bar.tick(16);

        bar.handle_event(&Event::MouseRelease { pos: Point::new(grab.x + 20, 8), button: 1 });
        let released_at = bar.value();
        assert!(bar.is_animating(), "a flicked thumb owes glide frames");

        assert!(bar.tick(16), "the glide advances past the release point");
        assert!(bar.value() > released_at, "and keeps moving in the drag's direction");

        while bar.tick(16) {}
        assert!(!bar.is_animating(), "the glide settles");
        assert!(bar.value() <= bar.maximum());
    }

    /// The idle delay and floor are per-control fields whose defaults are the old constants.
    #[test]
    fn the_idle_fade_parameters_are_configurable() {
        let mut bar = ScrollBar::new(Rect::new(0, 0, 200, 12));
        assert_eq!(bar.idle_delay_ms(), SCROLLBAR_IDLE_DELAY_MS);
        assert_eq!(bar.idle_opacity(), SCROLLBAR_IDLE_OPACITY, "the arrows floor");

        bar.set_idle_delay_ms(1200);
        bar.set_idle_opacity(0.7);
        assert_eq!(bar.idle_delay_ms(), 1200);
        assert_eq!(bar.idle_opacity(), 0.7);

        // The fade now holds past the old constant and settles at the configured floor.
        assert!(!bar.tick(600), "inside the longer hold nothing animates");
        bar.tick(5_000);
        while bar.tick(16) {}
        assert!((bar.opacity() - 0.7).abs() < 1e-3, "got {}", bar.opacity());

        // An out-of-range write is clamped, and the property route agrees.
        bar.set_idle_opacity(2.0);
        assert!((bar.idle_opacity() - 1.0).abs() < f32::EPSILON);
        assert_eq!(bar.get("idle_delay_ms").unwrap(), CapabilityValue::UInt(1200));
    }

    /// Arrow and page keys step the bar only while it holds focus.
    #[test]
    fn the_keyboard_steps_only_while_focused() {
        use crate::event::FocusReason;

        let mut bar = ScrollBar::new(Rect::new(0, 0, 16, 200));
        // The constructor defaults to horizontal like every control in this crate; the
        // tall shape is a vertical bar only once it is stated, which is what the arrow
        // directions below are written against.
        bar.set_orientation(Orientation::Vertical);
        bar.set_value(50);
        bar.set_single_step(1);
        bar.set_page_step(10);

        // Unfocused: the key belongs to whoever holds focus.
        bar.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(bar.value(), 50, "an unfocused bar ignores the key");

        bar.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        bar.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(bar.value(), 51, "a focused bar takes one line step");
        bar.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
        assert_eq!(bar.value(), 61, "PageDown takes one page step");
        bar.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
        assert_eq!(bar.value(), 51, "PageUp reverses it");

        bar.handle_event(&Event::FocusLost);
        bar.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(bar.value(), 51, "losing focus restores the gate");
    }
}
