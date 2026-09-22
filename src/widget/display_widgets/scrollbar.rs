// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Scroll bar widget.
use crate::compat::ToString;
use crate::core::{Color, Orientation, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_i64, expect_orientation, orientation_to_str};
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

    /// Returns value for a given pixel position.
    ///
    /// The inverse of [`Self::value_to_pixel_pos`]: both read the same **track band**, so the
    /// value a drag reads back is the value the thumb was drawn at.
    fn pixel_pos_to_value(&self, pos: f32) -> i32 {
        let band = self.track_band();
        let slider_size = self.slider_size();
        let range = (self.maximum - self.minimum) as f32;
        // The available travel is the band's length, not the control's: with the band centred
        // in a 120 px cell, measuring the pointer from the *control's* edge would map a click
        // 60 px above the bar to a position on it. The `slider_size` term is deliberately
        // *not* subtracted, because `value_to_pixel_pos` does not subtract it either — a
        // scrollbar's thumb does not travel its own length, and the two must be inverses.
        let (origin, length) = match self.orientation {
            Orientation::Horizontal => (band.x as f32, band.width as f32),
            Orientation::Vertical => (band.y as f32, band.height as f32),
        };
        let available = (length * (1.0 - slider_size)).max(0.0);
        if available == 0.0 {
            return self.minimum;
        }
        let relative = (pos - origin) / available;
        let value = self.minimum as f32 + range * relative.clamp(0.0, 1.0);
        value.round() as i32
    }
    /// Returns pixel position for a given value.
    ///
    /// Read the trough from [`Self::track_band`] and the cells from [`Self::arrow_cell`], so
    /// the thumb, the arrows and the hit test are three consumers of **one** geometry rather
    /// than three derivations that can drift.
    fn value_to_pixel_pos(&self, value: i32) -> f32 {
        let band = self.track_band();
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        let slider_size = self.slider_size();
        let range = (self.maximum - self.minimum) as f32;
        //
        // The travel is the **band between the two arrow cells**, not the whole control: the
        // arrows occupy fixed cells at each end, so a thumb travelling the full length would
        // pass underneath them.
        let cell = self.arrow_cell();
        let (origin, length) = match self.orientation {
            Orientation::Horizontal => (band.x as f32, band.width as f32),
            Orientation::Vertical => (band.y as f32, band.height as f32),
        };
        if range == 0.0 {
            return origin + cell;
        }
        let relative = (clamped - self.minimum) as f32 / range;
        let available = (length - cell * 2.0).max(0.0) * (1.0 - slider_size);
        origin + cell + available * relative
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
}
