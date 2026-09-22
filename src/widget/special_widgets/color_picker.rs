// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! ColorPicker widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Interactive color picker with HSV controls and preset swatches.
pub struct ColorPicker {
    base: BaseWidget,
    color: Color,
    hue: u8,
    saturation: u8,
    value: u8,
    alpha: u8,
    show_alpha: bool,
    presets: Vec<Color>,
    /// Emitted when selected color changes.
    pub color_changed: Signal1<Color>,
    /// Emitted when hex text changes.
    pub hex_changed: Signal1<String>,
}

impl ColorPicker {
    /// Creates a color picker.
    pub fn new(geometry: Rect) -> Self {
        let mut picker = Self {
            base: BaseWidget::new(WidgetKind::ColorPicker, geometry, "ColorPicker"),
            color: Color::rgb(255, 0, 0),
            hue: 0,
            saturation: 255,
            value: 255,
            alpha: 255,
            show_alpha: true,
            presets: vec![
                Color::rgb(244, 67, 54),
                Color::rgb(33, 150, 243),
                Color::rgb(76, 175, 80),
                Color::rgb(255, 193, 7),
                Color::rgb(156, 39, 176),
                Color::rgb(96, 125, 139),
            ],
            color_changed: Signal1::new(),
            hex_changed: Signal1::new(),
        };
        picker.sync_color_from_hsva();
        picker
    }

    /// Returns current color.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Sets current color directly.
    pub fn set_color(&mut self, color: Color) {
        self.color = color;
        self.alpha = color.a;
        self.color_changed.emit(self.color);
        self.hex_changed.emit(self.color.to_hex_rgba());
        self.base.request_redraw();
    }

    /// Sets HSVA components and updates color.
    pub fn set_hsva(&mut self, hue: u8, saturation: u8, value: u8, alpha: u8) {
        self.hue = hue;
        self.saturation = saturation;
        self.value = value;
        self.alpha = alpha;
        self.sync_color_from_hsva();
    }

    /// Returns `(h, s, v, a)`.
    pub fn hsva(&self) -> (u8, u8, u8, u8) {
        (self.hue, self.saturation, self.value, self.alpha)
    }

    /// Enables/disables alpha strip rendering.
    pub fn set_show_alpha(&mut self, show_alpha: bool) {
        if self.show_alpha == show_alpha {
            return;
        }
        self.show_alpha = show_alpha;
        self.base.request_redraw();
    }

    /// Returns whether alpha strip is visible.
    pub fn show_alpha(&self) -> bool {
        self.show_alpha
    }

    /// Sets color by hex text (`#RRGGBB` or `#RRGGBBAA`).
    pub fn set_hex(&mut self, hex: &str) -> bool {
        let Some(color) = Color::parse_hex(hex) else {
            return false;
        };
        self.set_color(color);
        true
    }

    /// Returns current color hex string.
    pub fn hex_rgba(&self) -> String {
        self.color.to_hex_rgba()
    }

    /// Applies a preset by index.
    pub fn apply_preset(&mut self, index: usize) -> bool {
        let Some(color) = self.presets.get(index).copied() else {
            return false;
        };
        self.set_color(color);
        true
    }

    /// Returns preset count.
    pub fn preset_count(&self) -> usize {
        self.presets.len()
    }

    /// Height of the bottom chrome stack below the palette: the hex readout's line box, the
    /// preset swatch row, and the margins around them.
    ///
    /// Derived from the parts rather than being one literal, because `44` was too small for
    /// them: it did not account for the readout's own 14 px line, so the palette extended to
    /// within 6 px of the swatch row and the readout was drawn straight over the palette's
    /// bottom edge (the census measured `#FF0000FF` in `225,225,225` on the palette's
    /// `255,64,64`, 2.65:1). The palette is a spectrum, so text over any of it is wrong
    /// regardless of the ratio.
    const BOTTOM_STACK: u32 = READOUT_LINE + PRESET_ROW + MARGIN * 3;

    fn palette_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + 8,
            rect.y + 8,
            rect.width.saturating_sub(48),
            rect.height.saturating_sub(Self::BOTTOM_STACK + 8),
        )
    }

    fn hue_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + rect.width as i32 - 34,
            rect.y + 8,
            12,
            rect.height.saturating_sub(Self::BOTTOM_STACK + 8),
        )
    }

    fn alpha_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + rect.width as i32 - 18,
            rect.y + 8,
            10,
            rect.height.saturating_sub(Self::BOTTOM_STACK + 8),
        )
    }

    fn preset_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.presets.len() {
            return None;
        }
        let rect = self.geometry();
        let x = rect.x + 8 + (index as i32) * 22;
        let y = rect.y + rect.height as i32 - (PRESET_ROW + MARGIN) as i32;
        Some(Rect::new(x, y, 18, 18))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn sync_color_from_hsva(&mut self) {
        self.color = hsv_to_color(self.hue, self.saturation, self.value, self.alpha);
        self.color_changed.emit(self.color);
        self.hex_changed.emit(self.color.to_hex_rgba());
        self.base.request_redraw();
    }

    fn set_from_palette_point(&mut self, pos: Point) {
        let palette = self.palette_rect();
        let width = palette.width.max(1) as f32;
        let height = palette.height.max(1) as f32;
        let sat_ratio = ((pos.x - palette.x) as f32 / width).clamp(0.0, 1.0);
        let val_ratio = (1.0 - ((pos.y - palette.y) as f32 / height)).clamp(0.0, 1.0);
        self.saturation = (sat_ratio * 255.0).round() as u8;
        self.value = (val_ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }

    fn set_from_hue_point(&mut self, pos: Point) {
        let hue_rect = self.hue_rect();
        let height = hue_rect.height.max(1) as f32;
        let ratio = ((pos.y - hue_rect.y) as f32 / height).clamp(0.0, 1.0);
        self.hue = (ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }

    fn set_from_alpha_point(&mut self, pos: Point) {
        let alpha_rect = self.alpha_rect();
        let height = alpha_rect.height.max(1) as f32;
        let ratio = (1.0 - ((pos.y - alpha_rect.y) as f32 / height)).clamp(0.0, 1.0);
        self.alpha = (ratio * 255.0).round() as u8;
        self.sync_color_from_hsva();
    }
}

impl Widget for ColorPicker {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Paints itself, so it can be mounted into a host surface.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 200)
    }
}

/// `ColorPicker`'s property contract.
///
/// These are the properties the `WidgetKind::ColorDialog` arms used to serve: the
/// old dispatch keyed on `ColorDialog` but downcast to `ColorPicker`, so the
/// contract belongs here, next to the fields it reads. Read/write semantics are
/// carried over unchanged from `access_read_dialog.in.rs` /
/// `access_write_dialog.in.rs`.
///
/// Since BLUE16 phase E-6 the control declares `WidgetKind::ColorPicker` instead,
/// so it no longer shares the dialog's kind. The history above is kept because the
/// contract was written under the old key and the semantics still come from there;
/// what changed is *which kind answers*, not what the properties mean.
impl WidgetProperties for ColorPicker {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "hex_rgba" => Ok(CapabilityValue::String(self.hex_rgba())),
            "show_alpha" => Ok(CapabilityValue::Bool(self.show_alpha())),
            "preset_count" => Ok(CapabilityValue::UInt(self.preset_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "hex_rgba" => {
                // `set_hex` reports a parse failure by returning `false`; the old
                // dispatch mapped that to `TypeMismatch`, so the meaning is kept.
                if self.set_hex(&expect_string(value)?) {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::TypeMismatch)
                }
            }
            "show_alpha" => {
                self.set_show_alpha(expect_bool(value)?);
                Ok(())
            }
            // The preset palette is fixed at construction and applied by index
            // through `apply_preset`, so the count is a derived read.
            "preset_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["hex_rgba", "show_alpha", "preset_count", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `color_picker` publishes.
    ///
    /// `set_hex` takes the hex text and `apply_preset` takes the preset index, so
    /// neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_hex" | "apply_preset" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for ColorPicker {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                for index in 0..self.presets.len() {
                    let Some(preset_rect) = self.preset_rect(index) else {
                        continue;
                    };
                    if Self::point_in_rect(*pos, preset_rect) {
                        let _ = self.apply_preset(index);
                        return;
                    }
                }

                if Self::point_in_rect(*pos, self.palette_rect()) {
                    self.set_from_palette_point(*pos);
                } else if Self::point_in_rect(*pos, self.hue_rect()) {
                    self.set_from_hue_point(*pos);
                } else if self.show_alpha && Self::point_in_rect(*pos, self.alpha_rect()) {
                    self.set_from_alpha_point(*pos);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => {
                    self.hue = self.hue.saturating_sub(2);
                    self.sync_color_from_hsva();
                }
                39 => {
                    self.hue = self.hue.saturating_add(2);
                    self.sync_color_from_hsva();
                }
                38 => {
                    self.value = self.value.saturating_add(2);
                    self.sync_color_from_hsva();
                }
                40 => {
                    self.value = self.value.saturating_sub(2);
                    self.sync_color_from_hsva();
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for ColorPicker {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // The picker's frame and its guide rails are chrome and resolve through the theme; the
        // **spectrum** inside them is the value being picked and stays the caller's data (that is
        // what the `hue-wheel` exemption is about). Every literal below used to be a light-theme
        // grey, so the panel did not move with the appearance at all and a dark-theme picker was
        // a white card.
        let style = self.style().clone();
        let theme = crate::style::resolved_theme_style("color_picker");
        let panel = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(247, 249, 252));
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or_else(|| panel.contrast_color());
        // The rails frame a spectrum whose own brightness varies along its length, so they are
        // held at a fixed ± step from the panel rather than derived from the spectrum.
        let frame = panel.blend(&ink, 0.22);
        let rail = panel.blend(&ink, 0.10);

        context.fill_rect(rect, panel);
        context.draw_rect(rect, frame);

        let palette = self.palette_rect();
        let base_hue = hsv_to_color(self.hue, 255, 255, 255);
        context.fill_rect(palette, base_hue.blend(&Color::WHITE, 0.25));
        context.draw_rect(palette, panel.blend(&ink, 0.40));

        let hue_rect = self.hue_rect();
        context.fill_rect(hue_rect, rail);
        context.draw_rect(hue_rect, panel.blend(&ink, 0.35));

        if self.show_alpha {
            let alpha_rect = self.alpha_rect();
            context.fill_rect(alpha_rect, rail);
            context.draw_rect(alpha_rect, panel.blend(&ink, 0.30));
        }

        for (index, color) in self.presets.iter().enumerate() {
            let Some(preset_rect) = self.preset_rect(index) else {
                continue;
            };
            // The preset swatch IS a colour value, so it is passed through; only its outline is
            // chrome, and it is chosen against the swatch so a dark preset still has an edge.
            context.fill_rect(preset_rect, *color);
            context.draw_rect(preset_rect, color.contrast_color().with_alpha(90));
        }

        // The current-value swatch: its outline is its own contrast colour rather than a fixed
        // dark grey, which only outlined light swatches.
        let swatch_rect = Rect::new(
            rect.x + rect.width as i32 - 70,
            rect.y + rect.height as i32 - (PRESET_ROW + MARGIN) as i32,
            56,
            PRESET_ROW,
        );
        context.fill_rect(swatch_rect, self.color);
        context.draw_rect(swatch_rect, self.color.contrast_color().with_alpha(120));

        // The hex readout takes the row immediately below the palette, which the palette's own
        // rectangle now leaves for it.
        let hex_font = Font::default();
        let hex_text = self.hex_rgba();
        let hex_height = context.measure_text(&hex_text, &hex_font).height as i32;
        let palette = self.palette_rect();
        let hex_band = Rect::new(
            rect.x + 8,
            palette.y + palette.height as i32,
            rect.width.saturating_sub(16),
            hex_height.max(1) as u32,
        );
        context.draw_text_fitted(
            hex_band,
            &hex_text,
            &hex_font,
            // The readout is chrome sitting on the panel, so it is the resolved ink rather than
            // a literal that was tuned for one appearance.
            ink.legible_on(panel, 4.5),
            HorizontalAlignment::Left,
        );
    }
}

/// Height of the bottom chrome stack below the palette: the hex readout's line box, the
/// preset swatch row, and the margins around them.
///
/// Derived from the parts rather than being one literal, because `44` was too small for
/// them: it did not account for the readout's own 14 px line, so the palette extended to
/// within 6 px of the swatch row and the readout was drawn straight over the palette's
/// bottom edge (the census measured `#FF0000FF` in `225,225,225` on the palette's
/// `255,64,64`, 2.65:1). The palette is a spectrum, so text over any of it is wrong
/// regardless of the ratio.
const READOUT_LINE: u32 = 14;
const PRESET_ROW: u32 = 18;
const MARGIN: u32 = 4;

fn hsv_to_color(h: u8, s: u8, v: u8, a: u8) -> Color {
    let hf = (h as f32 / 255.0) * 360.0;
    let sf = s as f32 / 255.0;
    let vf = v as f32 / 255.0;

    let c = vf * sf;
    let x = c * (1.0 - (((hf / 60.0) % 2.0) - 1.0).abs());
    let m = vf - c;

    let (r1, g1, b1) = if hf < 60.0 {
        (c, x, 0.0)
    } else if hf < 120.0 {
        (x, c, 0.0)
    } else if hf < 180.0 {
        (0.0, c, x)
    } else if hf < 240.0 {
        (0.0, x, c)
    } else if hf < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    Color::rgba(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
        a,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn hsva_red_maps_to_red_color() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        picker.set_hsva(0, 255, 255, 255);
        let color = picker.color();
        assert!(color.r >= 250);
        assert!(color.g <= 5);
        assert!(color.b <= 5);
    }

    #[test]
    fn set_hex_updates_color() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));
        assert!(picker.set_hex("#336699CC"));
        assert_eq!(picker.color(), Color::rgba(0x33, 0x66, 0x99, 0xCC));
    }

    #[test]
    fn apply_preset_emits_color_changed() {
        let mut picker = ColorPicker::new(Rect::new(0, 0, 260, 220));

        let emitted = Arc::new(Mutex::new(Vec::<Color>::new()));
        let sink = emitted.clone();
        picker.color_changed.connect(move |color| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*color);
            }
        });

        assert!(picker.apply_preset(1));
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert!(!got.is_empty());
    }
}
