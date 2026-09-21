// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ColorWell widget — a compact color swatch that displays the current color
//! and emits a click signal when pressed.
//!
//! The ColorWell shows a filled rectangle with the selected color. When the
//! color has alpha transparency, a checkerboard pattern is rendered behind it
//! to indicate the transparent regions. An optional border frames the swatch.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// A compact color swatch widget that displays a color and emits a signal
/// when clicked.
pub struct ColorWell {
    base: BaseWidget,
    color: Color,
    show_border: bool,
    /// Emitted when the color well is clicked.
    pub clicked: GenericSignal,
}

impl ColorWell {
    /// Creates a new ColorWell widget with the given color and geometry.
    ///
    /// The border is enabled by default.
    pub fn new(color: Color, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorWell, geometry, "ColorWell"),
            color,
            show_border: true,
            clicked: GenericSignal::new(),
        }
    }

    /// Returns the current color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Sets the current color and requests a redraw.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.base.request_redraw();
    }

    /// Returns whether the border is visible.
    pub fn show_border(&self) -> bool {
        self.show_border
    }

    /// Sets whether the border is visible and requests a redraw.
    pub fn set_show_border(&mut self, show_border: bool) {
        if self.show_border != show_border {
            self.show_border = show_border;
            self.base.request_redraw();
        }
    }
}

impl Widget for ColorWell {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(40, 24)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `ColorWell`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. The colour travels as the
/// `#RRGGBBAA` hex string [`Color::to_hex_rgba`] produces, and a write accepts
/// anything [`Color::parse_hex`] understands.
impl WidgetProperties for ColorWell {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The property's declared kind is `Color`, so it is read back as one. It
            // previously travelled as a `String`, which meant a caller had to know the
            // spelling and could not tell the value's type from the schema.
            "color" => Ok(CapabilityValue::Color(self.color())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "color" => {
                // Accept only the declared kind. A string is still accepted because the
                // C ABI and JSON surfaces carry colours as CSS text, and both parse it
                // into `Color` before it reaches here; a bare string arriving means a
                // caller reached past those layers, and refusing it would break code
                // that has not migrated yet.
                let color = match value {
                    CapabilityValue::Color(color) => color,
                    CapabilityValue::String(raw) => {
                        let Some(color) = Color::parse_hex(&raw) else {
                            return Err(CapabilityAccessError::TypeMismatch);
                        };
                        color
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_color(color);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["color", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `color_well` publishes.
    ///
    /// `set_color` is the only published name and it assigns the swatch's colour, so
    /// it needs an argument a command carries none of. It is refused as
    /// [`CapabilityAccessError::OutOfRange`] — the name is right, the value belongs on
    /// the property route (`set("color", ..)`) — rather than reported unknown.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_color" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for ColorWell {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // The well's chrome — the checkerboard that marks transparency and the
        // framing border — resolves explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously both were hardcoded, so
        // light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("color_well");
        // The swatch itself (`self.color`) is the datum the control exists to show,
        // so it is painted verbatim; only the surface behind a translucent swatch
        // and the border around it follow the appearance.
        //
        // `color_well` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — the window's own
        // colour. The well's frame is therefore a step toward the foreground, or it
        // would be indistinguishable from the window behind it.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| resolved.blend(&Color::BLACK, 0.35));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let background = resolved.blend(&text_color, 0.08);

        // Draw checkerboard for transparency indication: two steps of the resolved
        // surface, so a translucent swatch composites over a pattern that follows the
        // appearance rather than a fixed grey-and-white pair.
        let checker_size = 4u32;
        let even = background;
        let odd = background.blend(&border, 0.25);

        for y in (rect.y..(rect.y + rect.height as i32)).step_by(checker_size as usize) {
            for x in (rect.x..(rect.x + rect.width as i32)).step_by(checker_size as usize) {
                let tile_x = (x - rect.x) / checker_size as i32;
                let tile_y = (y - rect.y) / checker_size as i32;
                let tile_color = if (tile_x + tile_y) % 2 == 0 { even } else { odd };
                let tile_w = checker_size.min((rect.x + rect.width as i32 - x) as u32);
                let tile_h = checker_size.min((rect.y + rect.height as i32 - y) as u32);
                context.fill_rect(Rect::new(x, y, tile_w, tile_h), tile_color);
            }
        }

        // The swatch is inset inside the well rather than flooding it. The swatch is
        // the datum the control exists to show, so it is painted verbatim — but a
        // control whose *entire* area is caller data has no chrome left to follow an
        // appearance switch, and the well reads as a bare colour rather than as a
        // colour *control*. The housing around the swatch is what makes the well a
        // control: it is the resolved surface, so it follows the appearance, and it
        // is large enough to be the colour the eye reads first.
        let inset = (rect.width.min(rect.height) / 3).clamp(3, 32);
        let swatch = Rect::new(
            rect.x + inset as i32,
            rect.y + inset as i32,
            rect.width.saturating_sub(inset * 2),
            rect.height.saturating_sub(inset * 2),
        );
        // A translucent swatch composites over the checkerboard; an opaque one
        // covers it. Either way the surrounding housing stays the resolved surface.
        context.fill_rect(swatch, self.color);

        // The housing between the well's edge and the swatch is the resolved surface,
        // drawn last so it reads as the control's chrome rather than as a hole.
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, inset), background);
        context.fill_rect(
            Rect::new(rect.x, swatch.y + swatch.height as i32, rect.width, inset),
            background,
        );
        context.fill_rect(Rect::new(rect.x, swatch.y, inset, swatch.height), background);
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - inset as i32, swatch.y, inset, swatch.height),
            background,
        );

        // Draw border if enabled
        if self.show_border {
            context.draw_rect_stroke(rect, border, 1);
        }
    }
}

impl EventHandler for ColorWell {
    /// Emits `clicked` only for a press that lands on the well.
    ///
    /// The position used to be discarded (`pos: _`), so a left press **anywhere**
    /// emitted this control's `clicked` — a press on a neighbouring control fired the
    /// well's colour-changed path. 43 other widgets in this crate guard the press with
    /// [`Rect::contains_point`](crate::core::Rect::contains_point); this one now does too.
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    self.clicked.emit();
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn color_well_default_color() {
        let cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert_eq!(cw.color(), Color::RED);
        assert!(cw.show_border());
        assert_eq!(cw.kind(), WidgetKind::ColorWell);
    }

    #[test]
    fn color_well_set_color() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        cw.set_color(Color::BLUE);
        assert_eq!(cw.color(), Color::BLUE);
    }

    #[test]
    fn color_well_signal_emitted_on_click() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_border_toggle() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        assert!(cw.show_border());

        cw.set_show_border(false);
        assert!(!cw.show_border());

        cw.set_show_border(true);
        assert!(cw.show_border());
    }

    #[test]
    fn color_well_disabled_blocks_events() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let clicked = Arc::new(AtomicBool::new(false));
        cw.clicked.connect({
            let clicked = Arc::clone(&clicked);
            move || {
                clicked.store(true, Ordering::SeqCst);
            }
        });

        cw.set_enabled(false);
        cw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn color_well_svg_output() {
        let mut cw = ColorWell::new(Color::RED, Rect::new(0, 0, 40, 40));
        let svg = crate::widget::svg::render_to_svg(&mut cw);
        assert!(svg.starts_with("<svg"));
    }
}
