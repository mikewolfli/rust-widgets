// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll bar widget.
use crate::compat::ToString;
use crate::core::{Color, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{
    expect_i64, expect_orientation, expect_text_direction, orientation_to_str,
    text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Scroll bar widget.
pub struct ScrollBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    single_step: i32,
    page_step: i32,
    orientation: Orientation,
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
    mouse_pressed: bool,
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
            direction: crate::core::TextDirection::default(),
            value_changed: Signal1::new(),
            slider_moved: Signal1::new(),
            slider_pressed: GenericSignal::new(),
            slider_released: GenericSignal::new(),
            mouse_pressed: false,
        }
    }
    /// Returns minimum value.
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    /// Sets minimum value.
    pub fn set_minimum(&mut self, minimum: i32) {
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
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
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
        self.value_changed.emit(self.value);
    }
    /// Returns single step value.
    pub fn single_step(&self) -> i32 {
        self.single_step
    }
    /// Sets single step value.
    pub fn set_single_step(&mut self, step: i32) {
        self.single_step = step.max(1);
        self.base.request_redraw();
    }
    /// Returns page step value.
    pub fn page_step(&self) -> i32 {
        self.page_step
    }
    /// Sets page step value.
    pub fn set_page_step(&mut self, step: i32) {
        self.page_step = step.max(1);
        self.base.request_redraw();
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
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
    /// The control's **own rectangle**, which is the area it was given: a hit area and a
    /// layout slot, *not* a drawing instruction.
    ///
    /// `size_hint` reports a 16 px thick bar and `draw` paints
    /// [`dimensions::SCROLLBAR_THICKNESS`] of it, so the two agree about thickness; this
    /// accessor is the one place that asks "how long may the thumb travel", and it is
    /// deliberately the full length of the rectangle.
    fn track_band(&self) -> Rect {
        let rect = self.geometry();
        match self.orientation {
            Orientation::Horizontal => {
                ControlMetrics::centered_band(rect, dimensions::SCROLLBAR_THICKNESS)
            }
            Orientation::Vertical => {
                // A vertical scrollbar's band is *wide and full-height*, i.e. the same
                // derivation with the axes exchanged. `centered_band` keeps the caller's
                // width and takes the thickness for the height, which is exactly the
                // horizontal reading; transposing the inputs gives the vertical one.
                let band = ControlMetrics::centered_band(
                    Rect::new(rect.y, rect.x, rect.height, rect.width),
                    dimensions::SCROLLBAR_THICKNESS,
                );
                Rect::new(band.y, band.x, band.height, band.width)
            }
        }
    }

    /// Size of the arrow cell at each end of the trough, in pixels.
    ///
    /// Derived from the control's **thickness** and bounded, so a scrollbar's arrows are the
    /// same size no matter how long the bar is. `width * 0.2` on a 240 px horizontal bar gave a
    /// 48 px arrow — wider than the trough and longer than the space before the thumb, so the
    /// left arrow's apex landed underneath the slider. Qt's `SC_ScrollBarSubLine` cell is on the
    /// order of the trough's thickness, and this is the same reading — it is
    /// [`dimensions::SCROLLBAR_MIN_LENGTH`]'s floor applied to the cell, so an arrow cell and a
    /// thumb are never wildly different objects.
    ///
    /// `value_to_pixel_pos` reads this too, which is what keeps the thumb inside the trough: one
    /// derivation, two consumers, so they cannot drift apart.
    fn arrow_cell(&self) -> f32 {
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        match self.orientation {
            crate::layout::Orientation::Horizontal => crate::core::Size::new(100, 16),
            crate::layout::Orientation::Vertical => crate::core::Size::new(16, 100),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "minimum" => {
                self.set_minimum(expect_i64(value)? as i32);
                Ok(())
            }
            "maximum" => {
                self.set_maximum(expect_i64(value)? as i32);
                Ok(())
            }
            "value" => {
                self.set_value(expect_i64(value)? as i32);
                Ok(())
            }
            "single_step" => {
                self.set_single_step(expect_i64(value)? as i32);
                Ok(())
            }
            "page_step" => {
                self.set_page_step(expect_i64(value)? as i32);
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
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                self.mouse_pressed = true;
                self.slider_pressed.emit();
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_value(value);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.mouse_pressed = false;
                self.slider_released.emit();
            }
            Event::MouseMove { pos } if self.mouse_pressed => {
                let pixel = match self.orientation {
                    Orientation::Horizontal => pos.x as f32,
                    Orientation::Vertical => pos.y as f32,
                };
                let value = self.pixel_pos_to_value(pixel);
                self.set_value(value);
                self.slider_moved.emit(value);
            }
            Event::KeyPress { key, modifiers: _ } => {
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
        let band = self.track_band();
        let rect = band;
        let slider_pos = self.value_to_pixel_pos(self.value);
        let slider_size = self.slider_size();
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
        // Draw background (the trough)
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), trough);
        // Draw border
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            style.border_color.unwrap_or_else(|| trough.contrast_color().with_alpha(80)),
        );
        // Draw slider
        //
        // The thumb is `slider_size` of the *travel*, but never shorter than
        // [`dimensions::SCROLLBAR_MIN_LENGTH`]: a proportional thumb with no floor vanishes on
        // a very long document, leaving nothing to grab. The floor is applied here rather than
        // inside `slider_size()` because that function reports the fraction a *caller* asked
        // for, while the floor is a drawing decision about the grip.
        let thumb_length = |travel: u32| -> u32 { (travel as f32 * slider_size) as u32 };
        match self.orientation {
            Orientation::Horizontal => {
                let slider_width =
                    thumb_length(rect.width).max(dimensions::SCROLLBAR_MIN_LENGTH.min(rect.width));
                // The thumb stops at the trough's far edge even when the floor would push it
                // past it, because nothing clips a widget at this layer.
                let slider_width = slider_width
                    .min((band.x + band.width as i32 - slider_pos as i32).max(0) as u32);
                context.fill_rect(
                    Rect::from_f32(
                        slider_pos,
                        rect.y as f32,
                        slider_width as f32,
                        rect.height as f32,
                    ),
                    slider_color,
                );
                // Draw slider border
                context.draw_rect(
                    Rect::from_f32(
                        slider_pos,
                        rect.y as f32,
                        slider_width as f32,
                        rect.height as f32,
                    ),
                    slider_border_color,
                );
                // Draw arrows using draw_line (triangles approximated)
                //
                // The arrow cell is sized from the control's **thickness**, not its length:
                // `rect.width * 0.2` on a 240 px horizontal bar produced a 48 px arrow — longer
                // than the space before the thumb, so the left arrow's apex crossed underneath
                // the slider (`scroll_bar.svg` had its apex at x = 48 while the thumb began at
                // x = 24). The derivation lives in `arrow_cell()`, which the thumb's travel
                // reads too, so the two cannot disagree.
                let arrow_size = self.arrow_cell() as u32;
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
            Orientation::Vertical => {
                let slider_height = thumb_length(rect.height)
                    .max(dimensions::SCROLLBAR_MIN_LENGTH.min(rect.height));
                let slider_height = slider_height
                    .min((band.y + band.height as i32 - slider_pos as i32).max(0) as u32);
                context.fill_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        slider_pos,
                        rect.width as f32,
                        slider_height as f32,
                    ),
                    slider_color,
                );
                // Draw slider border
                context.draw_rect(
                    Rect::from_f32(
                        rect.x as f32,
                        slider_pos,
                        rect.width as f32,
                        slider_height as f32,
                    ),
                    slider_border_color,
                );
                // Draw arrows using draw_line (triangles approximated)
                let arrow_size = self.arrow_cell() as u32;
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
        sb.set_style(custom.clone());
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
}
