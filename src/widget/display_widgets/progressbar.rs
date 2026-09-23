// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Progress bar widget.
use crate::compat::{format, String, ToString};
use crate::core::{Color, Font, HorizontalAlignment, Orientation, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    expect_bool, expect_i64, expect_orientation, expect_text_direction, orientation_to_str,
    text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::numeric::ordered_clamp_i32;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Progress bar widget.
pub struct ProgressBar {
    base: BaseWidget,
    minimum: i32,
    maximum: i32,
    value: i32,
    text_visible: bool,
    orientation: Orientation,
    inverted_appearance: bool,
    /// The writing direction the value axis runs in.
    ///
    /// # Why this is not `inverted_appearance`
    ///
    /// `inverted_appearance` is a *presentational* choice — "anchor the fill to the far end" — and it
    /// is what an indeterminate or a trailing-edge indicator wants. Direction is a *reading* fact: in
    /// an Arabic or Hebrew interface the run begins at the right edge because that is where the user
    /// starts reading, so the fill grows leftward and "a third of the way through" is measured from
    /// the right. The two coincide for RTL — which is why one bool could stand in for both — but they
    /// are different statements, and collapsing them would mean a control could not be right-anchored
    /// in a left-to-right locale. BLUE22 §5.1 rejects a full `LayoutMirroring`; it does not reject
    /// "a control knows which end its line begins at", which is what `TextDirection` is for.
    ///
    /// Defaults to left-to-right, so a bar that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// Emitted with the new value after any change to `value` — from
    /// `set_value`, the steppers, or keyboard/wheel input. Not emitted when the
    /// value is set to the value it already had.
    pub value_changed: Signal1<i32>,
}
impl ProgressBar {
    /// Creates a progress bar with default range 0-100.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressBar, geometry, "ProgressBar"),
            minimum: 0,
            maximum: 100,
            value: 0,
            text_visible: true,
            orientation: Orientation::Horizontal,
            inverted_appearance: false,
            direction: crate::core::TextDirection::default(),
            value_changed: Signal1::new(),
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
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    ///
    /// Not gated by `enabled` for the same reason as [`ProgressBar::set_value`].
    pub fn set_range(&mut self, minimum: i32, maximum: i32) {
        self.minimum = minimum;
        self.maximum = maximum.max(minimum);
        self.set_value(self.value); // Re-clamp
    }
    /// Returns current value.
    pub fn value(&self) -> i32 {
        self.value
    }
    /// Sets value, clamped to valid range.
    ///
    /// `value_changed` is deliberately not gated by `enabled`: it reports the value the
    /// host just wrote, and a disabled progress bar is still a data display its host
    /// reads. The `enabled` contract exists to stop a disabled control from acting on
    /// *user* input, which this path never involves.
    pub fn set_value(&mut self, value: i32) {
        let clamped = ordered_clamp_i32(value, self.minimum, self.maximum);
        if self.value == clamped {
            return;
        }
        self.value = clamped;
        self.value_changed.emit(self.value);
        self.base.request_redraw();
    }
    /// Resets progress bar to minimum value.
    pub fn reset(&mut self) {
        self.set_value(self.minimum);
    }
    /// Returns whether text is visible.
    pub fn is_text_visible(&self) -> bool {
        self.text_visible
    }
    /// Sets text visibility.
    pub fn set_text_visible(&mut self, visible: bool) {
        self.text_visible = visible;
        self.base.request_redraw();
    }
    /// Returns orientation.
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    /// Sets orientation.
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
        self.base.request_redraw();
    }
    /// Returns whether appearance is inverted.
    pub fn is_inverted_appearance(&self) -> bool {
        self.inverted_appearance
    }
    /// Sets inverted appearance.
    pub fn set_inverted_appearance(&mut self, inverted: bool) {
        self.inverted_appearance = inverted;
        self.base.request_redraw();
    }

    /// Returns the writing direction the value axis runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the value axis runs in, and repaints.
    ///
    /// A bar that runs right-to-left fills from the right edge, so its filled run is measured from
    /// the beginning of the line — the edge the reader starts at — rather than from the left.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        self.direction = direction;
        self.base.request_redraw();
    }
    /// Returns progress as percentage (0 to 1).
    pub fn progress(&self) -> f32 {
        if self.maximum == self.minimum {
            return 0.0;
        }
        // Use saturating_sub to prevent integer overflow.
        ((self.value.saturating_sub(self.minimum)) as f32)
            / ((self.maximum.saturating_sub(self.minimum)) as f32)
    }
    /// Returns formatted text for display.
    fn format_text(&self) -> String {
        if !self.text_visible {
            return String::new();
        }
        let percentage = self.progress() * 100.0;
        format!("{}%", percentage.round() as i32)
    }
}
// Implement Widget trait
impl Widget for ProgressBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        match self.orientation() {
            Orientation::Horizontal => Size::new(120, 20),
            Orientation::Vertical => Size::new(20, 120),
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ProgressBar`'s property contract.
///
/// `progress` is derived from `minimum`/`maximum`/`value`, so it is readable but
/// deliberately not writable — the same split the schema records, and the same
/// answer the previous centralised writer gave.
impl WidgetProperties for ProgressBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "minimum" => Ok(CapabilityValue::Int(self.minimum() as i64)),
            "maximum" => Ok(CapabilityValue::Int(self.maximum() as i64)),
            "value" => Ok(CapabilityValue::Int(self.value() as i64)),
            "text_visible" => Ok(CapabilityValue::Bool(self.is_text_visible())),
            "orientation" => {
                Ok(CapabilityValue::String(orientation_to_str(self.orientation()).to_string()))
            }
            "inverted_appearance" => Ok(CapabilityValue::Bool(self.is_inverted_appearance())),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            "progress" => Ok(CapabilityValue::Float(self.progress() as f64)),
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
            "text_visible" => {
                self.set_text_visible(expect_bool(value)?);
                Ok(())
            }
            "orientation" => {
                self.set_orientation(expect_orientation(value)?);
                Ok(())
            }
            "inverted_appearance" => {
                self.set_inverted_appearance(expect_bool(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            // `progress` has no setter: it is a function of the range. Reporting it
            // as unsupported keeps the read-only contract explicit.
            "progress" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `PROGRESS_BAR_PROPERTIES`.
        property_names_of![
            "minimum",
            "maximum",
            "value",
            "text_visible",
            "orientation",
            "inverted_appearance",
            "direction",
            "progress",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `progress_bar` publishes.
    ///
    /// Every published name carries a payload (`value`, `orientation`, or the
    /// two-number range), so each is answered through the property route with a
    /// value — `value` / `orientation` / `minimum` + `maximum`. Reporting
    /// `OutOfRange` for a payload-less call is the same convention the sibling
    /// display controls (`lcd_number`, `scrollbar`, `slider`) already use, and it
    /// is what an absent `command` override cannot do: the trait default answers
    /// `UnknownCommand`, which `invoke_command` reports as a registry/
    /// implementation disagreement for a name the capability does publish.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_value" | "set_orientation" | "set_range" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ProgressBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Progress bar is usually non-interactive
    }
}
impl Draw for ProgressBar {
    fn draw(&mut self, context: &mut RenderContext) {
        // Draw base widget
        let rect = self.geometry();
        let progress = self.progress();
        let style = self.style().clone();
        // The track is the bar's *groove*: the empty run behind the fill. It used to be
        // `style.background_color` with a light-grey literal fallback, but `progress_bar`
        // classifies as `WidgetRole::Accent` and `role_colors` writes the theme's **accent**
        // colour there. The groove therefore painted the same saturated orange as the fill
        // would, so `progress_bar.svg` was a solid slab in which the value was invisible —
        // the control's whole purpose. A groove must be low-emphasis, so it is *derived*
        // from the resolved surface rather than read from a field that carries the accent.
        // The same derivation, for the same reason, is in `range_slider.rs`.
        let window_fill = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| active.colors.background).unwrap_or(Color::WHITE)
        };
        let ink = style.text_color.unwrap_or_else(|| window_fill.contrast_color());
        // A caller-set background is the groove; a theme-derived one is the window fill
        // (or the accent) and is replaced by one visible step from the surface. The filter
        // is on the **resolved** value, not only on the provenance, because a control whose
        // role resolves to the window fill paints an invisible groove either way.
        let groove_from_surface = window_fill.blend(&ink, 0.14);
        let track_color = match if style.theme_derived { None } else { style.background_color } {
            Some(resolved) if resolved != window_fill => resolved,
            _ => groove_from_surface,
        };
        // The fill resolves the explicit style first, then the theme's own resolved style
        // for this control — the accent — and only then the literal, so a build without a
        // theme renders exactly what it used to.
        //
        // The filled portion is **chrome**, not data: it expresses "how much of this task is
        // done", and that reading is carried by its *extent*, not by its hue. Hardcoding it
        // meant a light and a dark window showed the same blue bar, so the switch did
        // nothing.
        let themed = crate::style::resolved_theme_style("progress_bar");
        let fill = style
            .background_color
            .filter(|_| !style.theme_derived)
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
            .or_else(|| crate::style::semantic_color(crate::style::SemanticColor::Info))
            .unwrap_or(Color::rgb(0, 120, 215));
        // The bar is a **fixed-height** rounded track centred in `rect`, not the whole of
        // it. Using `rect.height` made a 240x120 census cell a 240x120 slab, which is a
        // filled rectangle rather than a progress bar; Material's linear indicator is 4px
        // tall with `height / 2` rounded ends. `rect` stays the widget's occupancy — its
        // hit area and layout slot — and only the drawn chrome takes the constant.
        //
        // The height comes from the shared `dimensions` table and the box from
        // `ControlMetrics::centered_band`, the derivation for "a line of this thickness
        // across the middle of my area". A local `const BAR_HEIGHT: u32 = 4` was the same
        // fact written a second time: the table already names it, so the two could drift
        // and nothing would have connected a progress bar's thickness to its neighbour's.
        let bar_rect = ControlMetrics::centered_band(rect, dimensions::PROGRESS_HEIGHT);
        let bar_height = bar_rect.height;
        // How much of the run is filled, in pixels along the bar's own axis.
        let filled_len = match self.orientation {
            Orientation::Horizontal => (bar_rect.width as f32 * progress) as u32,
            Orientation::Vertical => (bar_rect.height as f32 * progress) as u32,
        }
        .min(match self.orientation {
            Orientation::Horizontal => bar_rect.width,
            Orientation::Vertical => bar_rect.height,
        });
        // Draw background (the groove).
        //
        // The groove is the control's *own chrome* — the empty run the fill is measured against — so
        // it is drawn unconditionally, including at value 0. Only the fill below is conditional.
        context.fill_rounded_rect(bar_rect, bar_height / 2, track_color);
        // Draw border
        if let Some(border_color) = style.border_color {
            context.draw_rect(bar_rect, border_color);
        }
        // Draw the filled run — but only when there is one.
        //
        // A zero-extent rectangle is a *degenerate* element, not a small one. The SVG backend emits
        // it faithfully (`<rect width="0" ...>`), where it paints nothing, so a bar at value 0
        // claimed a fill and drew none: `snapshots/svg/progress_bar.svg` carried `width="0"` on the
        // fill line. Worse, the two backends *disagreed* about the same command —
        // `SoftwareSurface::fill_rounded_rect` returns early for a zero-extent rect — so the
        // snapshot documented chrome no rasteriser ever produced. Guarding here fixes both: the
        // emitted stream no longer contains an element that means nothing, and it now matches what
        // the rasteriser does with the identical command.
        //
        // The guard is `filled_len > 0`, and the same for both arms. A guard on `progress > 0.0`
        // alone would be wrong: a long bar at a *tiny* non-zero value floors to `filled_len == 0`
        // too, and the degenerate rect would come straight back.
        if filled_len > 0 {
            match self.orientation {
                Orientation::Horizontal => {
                    // `inverted_appearance` and the direction are two ways to reach the far end, and
                    // they compose: an inverted bar in an RTL locale anchors to the left, which is
                    // what XOR expresses and what a naive `||` would get wrong. The vertical arm is
                    // unaffected — the block axis is not mirrored in either direction, which is the
                    // same rule `slider` applies (its vertical run is top-to-bottom in every
                    // direction).
                    let from_far_end = self.inverted_appearance ^ self.direction.is_right_to_left();
                    let x = if from_far_end {
                        bar_rect.x + bar_rect.width as i32 - filled_len as i32
                    } else {
                        bar_rect.x
                    };
                    context.fill_rounded_rect(
                        Rect::new(x, bar_rect.y, filled_len, bar_height),
                        bar_height / 2,
                        fill,
                    );
                }
                Orientation::Vertical => {
                    let y = if self.inverted_appearance {
                        bar_rect.y
                    } else {
                        bar_rect.y + bar_rect.height as i32 - filled_len as i32
                    };
                    context.fill_rounded_rect(
                        Rect::new(bar_rect.x, y, bar_rect.width, filled_len),
                        bar_height / 2,
                        fill,
                    );
                }
            }
        }
        // Draw text if visible
        //
        // The band is the whole control, not the 4px bar: a label centred on the bar alone
        // would be clipped to four rows. `text_line` derives the glyph box from the band, so
        // the label is centred rather than starting on the band's middle line.
        //
        // The ink is the contrast colour of the surface actually behind the label. The
        // label is centred on the control, so the question is whether the filled run
        // covers that centre: if it does, `fill.contrast_color()` is the legible choice;
        // if it does not, the label sits on the groove and the groove's contrast colour is.
        // It used to be a hardcoded `Color::rgb(0, 0, 0)`, which is 1.12:1 against the dark
        // theme's background — the label was unreadable on exactly the appearance the
        // census renders. Same rule, and the same mistake it removes, as `roller.rs`.
        let text_color = if style.theme_derived || style.text_color.is_none() {
            // The filled run, as a half-open interval along its own axis.
            let (start, end) = match self.orientation {
                Orientation::Horizontal if self.inverted_appearance => (
                    bar_rect.x + bar_rect.width as i32 - filled_len as i32,
                    bar_rect.x + bar_rect.width as i32,
                ),
                Orientation::Horizontal => (bar_rect.x, bar_rect.x + filled_len as i32),
                Orientation::Vertical if !self.inverted_appearance => (
                    bar_rect.y + bar_rect.height as i32 - filled_len as i32,
                    bar_rect.y + bar_rect.height as i32,
                ),
                Orientation::Vertical => (bar_rect.y, bar_rect.y + filled_len as i32),
            };
            let label_centre = match self.orientation {
                Orientation::Horizontal => rect.x + rect.width as i32 / 2,
                Orientation::Vertical => rect.y + rect.height as i32 / 2,
            };
            if filled_len > 0 && label_centre >= start && label_centre < end {
                fill.contrast_color()
            } else {
                track_color.contrast_color()
            }
        } else {
            ink
        };
        let text = self.format_text();
        if !text.is_empty() {
            context.draw_text_line(
                rect,
                &text,
                &Font::default(),
                text_color,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `Vec` comes from `crate::compat` rather than from `std`, because the `alloc_frugal`
    // profile has no `std::vec` and a test that named it directly would not compile there.
    use crate::compat::Vec;
    use crate::core::{Color, Orientation, Rect, Size};
    use crate::style::WidgetStyle;

    #[test]
    fn progressbar_creation_defaults() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert_eq!(pb.value(), 0);
        assert_eq!(pb.minimum(), 0);
        assert_eq!(pb.maximum(), 100);
        assert!(pb.is_text_visible());
        assert_eq!(pb.orientation(), Orientation::Horizontal);
        assert!(!pb.is_inverted_appearance());
    }

    #[test]
    fn progressbar_set_value() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        assert_eq!(pb.value(), 50);
        pb.set_value(200); // clamp to max
        assert_eq!(pb.value(), 100);
        pb.set_value(-10); // clamp to min
        assert_eq!(pb.value(), 0);
    }

    #[test]
    fn progressbar_set_range() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_minimum(10);
        pb.set_maximum(200);
        assert_eq!(pb.minimum(), 10);
        assert_eq!(pb.maximum(), 200);
    }

    #[test]
    fn progressbar_set_range_reclamps_value() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        pb.set_range(60, 100);
        assert_eq!(pb.value(), 60);
    }

    #[test]
    fn progressbar_orientation() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_orientation(Orientation::Vertical);
        assert_eq!(pb.orientation(), Orientation::Vertical);
        pb.set_orientation(Orientation::Horizontal);
        assert_eq!(pb.orientation(), Orientation::Horizontal);
    }

    #[test]
    fn progressbar_text_visible() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_text_visible());
        pb.set_text_visible(false);
        assert!(!pb.is_text_visible());
        pb.set_text_visible(true);
        assert!(pb.is_text_visible());
    }

    #[test]
    fn progressbar_inverted_appearance() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(!pb.is_inverted_appearance());
        pb.set_inverted_appearance(true);
        assert!(pb.is_inverted_appearance());
        pb.set_inverted_appearance(false);
        assert!(!pb.is_inverted_appearance());
    }

    #[test]
    fn progressbar_reset() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(75);
        assert_eq!(pb.value(), 75);
        pb.reset();
        assert_eq!(pb.value(), 0);
    }

    #[test]
    fn progressbar_progress_percentage() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!((pb.progress() - 0.0).abs() < f32::EPSILON);

        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_value(50);
        assert!((pb.progress() - 0.5).abs() < f32::EPSILON);

        pb.set_value(100);
        assert!((pb.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progressbar_geometry_delegation() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(pb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn progressbar_visibility() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_visible());
        pb.hide();
        assert!(!pb.is_visible());
        pb.show();
        assert!(pb.is_visible());
    }

    #[test]
    fn progressbar_enabled() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.is_enabled());
        pb.set_enabled(false);
        assert!(!pb.is_enabled());
        pb.set_enabled(true);
        assert!(pb.is_enabled());
    }

    #[test]
    fn progressbar_tooltip_roundtrip() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert!(pb.tooltip().is_empty());
        pb.set_tooltip("Progress info".to_string());
        assert_eq!(pb.tooltip(), "Progress info");
        pb.set_tooltip(String::new());
        assert!(pb.tooltip().is_empty());
    }

    #[test]
    fn progressbar_style_roundtrip() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        assert_eq!(*pb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(220, 220, 220));
        pb.set_style(custom.clone());
        assert_eq!(*pb.style(), custom);
    }

    #[test]
    fn progressbar_id_kind() {
        let pb_a = ProgressBar::new(Rect::new(0, 0, 100, 20));
        let pb_b = ProgressBar::new(Rect::new(0, 0, 100, 20));
        assert_ne!(pb_a.id(), pb_b.id());
        assert_eq!(pb_a.kind(), WidgetKind::ProgressBar);
        assert_eq!(pb_b.kind(), WidgetKind::ProgressBar);
    }

    #[test]
    fn progressbar_signal_accessors() {
        let pb = ProgressBar::new(Rect::new(0, 0, 100, 20));
        let _value_changed = &pb.value_changed;
    }

    #[test]
    fn progressbar_size_hint_horizontal() {
        let pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        let hint = pb.size_hint();
        assert_eq!(hint, Size::new(120, 20));
    }

    #[test]
    fn progressbar_size_hint_vertical() {
        let mut pb = ProgressBar::new(Rect::new(0, 0, 200, 20));
        pb.set_orientation(Orientation::Vertical);
        let hint = pb.size_hint();
        assert_eq!(hint, Size::new(20, 120));
    }

    /// Returns the `(x, y, width, height)` of every `<rect>` whose `fill` is `fill`.
    ///
    /// Parsed per line rather than with a regex so the test needs no extra dependency; the backend
    /// emits exactly one element per line, so this is the whole element stream.
    fn rects_with_fill(svg: &str, fill: &str) -> Vec<(i32, i32, i32, i32)> {
        svg.lines()
            .filter(|line| line.contains("<rect") && line.contains(fill))
            .map(|line| {
                let attr = |name: &str| -> i32 {
                    line.split(&std::format!("{name}=\""))
                        .nth(1)
                        .and_then(|rest| rest.split('"').next())
                        .and_then(|value| value.parse().ok())
                        .unwrap_or_else(|| panic!("no {name} on: {line}"))
                };
                (attr("x"), attr("y"), attr("width"), attr("height"))
            })
            .collect()
    }

    /// The colour `draw` resolves for the filled run, read the same way `draw` reads it.
    ///
    /// Derived rather than hardcoded because the value is theme-dependent (the fill is chrome and
    /// follows the resolved theme style). Asserting a literal here would make the test fail on a
    /// theme change for a reason unrelated to what it pins.
    fn fill_color() -> Color {
        crate::style::resolved_theme_style("progress_bar")
            .and_then(|resolved| resolved.background_color)
            .or_else(|| crate::style::semantic_color(crate::style::SemanticColor::Info))
            .unwrap_or(Color::rgb(0, 120, 215))
    }

    /// A bar at value 0 must emit **nothing** for its fill, in either orientation.
    ///
    /// A zero-extent `<rect>` is degenerate: it is present in the stream, it is not a drawing, and
    /// the software rasteriser skips the identical command — so the snapshot showed an element the
    /// raster never produced (`snapshots/svg/progress_bar.svg` carried `width="0"`). The groove
    /// underneath is the control's own chrome and must still be there: "no fill" is the assertion,
    /// "no drawing at all" is not.
    #[test]
    fn progressbar_at_zero_emits_no_fill_but_keeps_its_groove() {
        let fill = fill_color();
        for orientation in [Orientation::Horizontal, Orientation::Vertical] {
            for inverted in [false, true] {
                let mut pb = ProgressBar::new(Rect::new(0, 0, 240, 120));
                pb.set_orientation(orientation);
                pb.set_inverted_appearance(inverted);
                pb.set_value(0);
                assert_eq!(pb.value(), 0, "the precondition of this test is a 0 value");

                let svg = crate::widget::svg::render_to_svg(&mut pb);
                let filled =
                    rects_with_fill(&svg, &crate::render::svg::convert::color_to_rgba(&fill));
                assert!(
                    filled.is_empty(),
                    "a 0-value {orientation:?} bar (inverted={inverted}) emitted a fill: {filled:?}\n{svg}"
                );

                // The groove is unconditional chrome, so it must survive the guard above. The
                // window background is the only other rect, and it is the full control rect.
                let drawn: Vec<_> = svg
                    .lines()
                    .filter(|line| line.contains("<rect") && line.contains("rx=\""))
                    .collect();
                assert!(
                    !drawn.is_empty(),
                    "a 0-value bar still draws its own groove, not just the window background: {svg}"
                );

                // The groove is also a *fixed-height band* centred in the control, not the
                // control itself: the 240x120 census cell used to be painted as a 240x120
                // slab, which is a filled rectangle rather than a progress bar. This pins
                // the shared `centered_band` derivation now that it supplies the geometry.
                assert!(
                    svg.contains(&format!("height=\"{}\"", dimensions::PROGRESS_HEIGHT)),
                    "the groove is {0}px tall, not the whole cell: {svg}",
                    dimensions::PROGRESS_HEIGHT
                );
            }
        }
    }

    /// The complement of the test above: a non-zero value that reaches a whole pixel of run **does**
    /// emit a fill, and the emitted extent is the filled length, never zero.
    #[test]
    fn progressbar_above_zero_emits_a_non_degenerate_fill() {
        let fill = fill_color();
        let mut pb = ProgressBar::new(Rect::new(0, 0, 240, 120));
        pb.set_value(50);

        let svg = crate::widget::svg::render_to_svg(&mut pb);
        let filled = rects_with_fill(&svg, &crate::render::svg::convert::color_to_rgba(&fill));
        assert_eq!(filled.len(), 1, "half a bar is one filled run: {svg}");
        let (_, _, width, height) = filled[0];
        assert!(width > 0 && height > 0, "a drawn fill is never degenerate: {svg}");
        assert_eq!(width, 120, "half of a 240-wide census cell is 120px: {svg}");
    }

    /// A right-to-left bar fills from the **right** edge, by the same length.
    ///
    /// # What this pins
    ///
    /// BLUE22 · F-4. A progress bar maps a value onto a line, and the line runs from where the reader
    /// starts to where they finish — so in an Arabic or Hebrew interface a third of the way through a
    /// task is a third of the way in from the *right*. Before this the control had no way to say so,
    /// and the only knob was `inverted_appearance`, which is a presentational choice rather than a
    /// reading fact.
    ///
    /// The assertion is deliberately about the **length as well as the origin**: a mirrored bar that
    /// also changed its length would look like it was at a different value, which is the "two
    /// directions must share one inset" lesson of BLUE22 §4.3 in its simplest form. The horizontal
    /// arm is the one that mirrors; the vertical arm must not.
    #[test]
    fn a_right_to_left_bar_fills_from_the_right_edge() {
        let fill = fill_color();
        let rgba = crate::render::svg::convert::color_to_rgba(&fill);

        let mut ltr = ProgressBar::new(Rect::new(0, 0, 240, 120));
        ltr.set_value(50);
        let ltr_svg = crate::widget::svg::render_to_svg(&mut ltr);
        let (ltr_x, _, ltr_w, _) = rects_with_fill(&ltr_svg, &rgba)[0];

        let mut rtl = ProgressBar::new(Rect::new(0, 0, 240, 120));
        rtl.set_value(50);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        let rtl_svg = crate::widget::svg::render_to_svg(&mut rtl);
        let (rtl_x, _, rtl_w, _) = rects_with_fill(&rtl_svg, &rgba)[0];

        assert_eq!(rtl_w, ltr_w, "the fill's length is a fact about the value, not the direction");
        assert_eq!(ltr_x, 0, "an LTR bar begins at its leading (left) edge");
        assert_eq!(
            rtl_x + rtl_w,
            240,
            "an RTL bar's fill ends at its leading (right) edge: x={rtl_x} w={rtl_w}"
        );
    }

    /// A vertical bar is **not** mirrored by the direction.
    ///
    /// The block axis is not a reading direction: text runs down the page the same way in Arabic and
    /// in English, so a vertical progress bar fills bottom-to-top in both. `slider` states the same
    /// rule in its own RTL arm, and the two controls must agree or a form would have one vertical
    /// indicator that mirrors and one that does not.
    #[test]
    fn a_vertical_bar_is_not_mirrored_by_direction() {
        let fill = fill_color();
        let rgba = crate::render::svg::convert::color_to_rgba(&fill);

        let mut ltr = ProgressBar::new(Rect::new(0, 0, 120, 240));
        ltr.set_orientation(Orientation::Vertical);
        ltr.set_value(50);
        let ltr_svg = crate::widget::svg::render_to_svg(&mut ltr);
        let (_, ltr_y, _, ltr_h) = rects_with_fill(&ltr_svg, &rgba)[0];

        let mut rtl = ProgressBar::new(Rect::new(0, 0, 120, 240));
        rtl.set_orientation(Orientation::Vertical);
        rtl.set_value(50);
        rtl.set_direction(crate::core::TextDirection::RightToLeft);
        let rtl_svg = crate::widget::svg::render_to_svg(&mut rtl);
        let (_, rtl_y, _, rtl_h) = rects_with_fill(&rtl_svg, &rgba)[0];

        assert_eq!(ltr_y, rtl_y, "the vertical fill starts where it did");
        assert_eq!(ltr_h, rtl_h, "and is the same length");
    }

    /// Direction and `inverted_appearance` compose rather than one winning.
    ///
    /// They are two ways to reach the far end — one a reading fact, one a presentation choice — so a
    /// bar that is *both* inverted and RTL must anchor to the **near** edge. Treating them as one flag
    /// (`||`) would make the second setting undo the first, which is the kind of interaction that is
    /// invisible until a locale is combined with an indeterminate indicator.
    #[test]
    fn direction_and_inverted_appearance_compose() {
        let fill = fill_color();
        let rgba = crate::render::svg::convert::color_to_rgba(&fill);
        let mut both = ProgressBar::new(Rect::new(0, 0, 240, 120));
        both.set_value(50);
        both.set_inverted_appearance(true);
        both.set_direction(crate::core::TextDirection::RightToLeft);
        let svg = crate::widget::svg::render_to_svg(&mut both);
        let (x, _, w, _) = rects_with_fill(&svg, &rgba)[0];
        assert_eq!((x, w), (0, 120), "inverted and RTL cancel, anchoring to the near edge");
    }
}
