//! ColorPicker widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
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
            base: BaseWidget::new(WidgetKind::ColorDialog, geometry, "ColorPicker"),
            color: Color::from_rgb(255, 0, 0),
            hue: 0,
            saturation: 255,
            value: 255,
            alpha: 255,
            show_alpha: true,
            presets: vec![
                Color::from_rgb(244, 67, 54),
                Color::from_rgb(33, 150, 243),
                Color::from_rgb(76, 175, 80),
                Color::from_rgb(255, 193, 7),
                Color::from_rgb(156, 39, 176),
                Color::from_rgb(96, 125, 139),
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

    fn palette_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(
            rect.x + 8,
            rect.y + 8,
            rect.width.saturating_sub(48),
            rect.height.saturating_sub(44),
        )
    }

    fn hue_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x + rect.width as i32 - 34, rect.y + 8, 12, rect.height.saturating_sub(44))
    }

    fn alpha_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x + rect.width as i32 - 18, rect.y + 8, 10, rect.height.saturating_sub(44))
    }

    fn preset_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.presets.len() {
            return None;
        }
        let rect = self.geometry();
        let x = rect.x + 8 + (index as i32) * 22;
        let y = rect.y + rect.height as i32 - 28;
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
        context.fill_rect(rect, Color::from_rgb(247, 249, 252));
        context.draw_rect(rect, Color::from_rgb(186, 195, 208));

        let palette = self.palette_rect();
        let base_hue = hsv_to_color(self.hue, 255, 255, 255);
        context.fill_rect(palette, base_hue.blend(&Color::WHITE, 0.25));
        context.draw_rect(palette, Color::from_rgb(142, 153, 170));

        let hue_rect = self.hue_rect();
        context.fill_rect(hue_rect, Color::from_rgb(210, 214, 223));
        context.draw_rect(hue_rect, Color::from_rgb(138, 147, 161));

        if self.show_alpha {
            let alpha_rect = self.alpha_rect();
            context.fill_rect(alpha_rect, Color::from_rgb(226, 229, 236));
            context.draw_rect(alpha_rect, Color::from_rgb(150, 159, 172));
        }

        for (index, color) in self.presets.iter().enumerate() {
            let Some(preset_rect) = self.preset_rect(index) else {
                continue;
            };
            context.fill_rect(preset_rect, *color);
            context.draw_rect(preset_rect, Color::from_rgb(107, 116, 131));
        }

        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 70, rect.y + rect.height as i32 - 28, 56, 18),
            self.color,
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 70, rect.y + rect.height as i32 - 28, 56, 18),
            Color::from_rgb(40, 48, 63),
        );

        context.draw_text(
            Point::new(rect.x + 8, rect.y + rect.height as i32 - 12),
            &self.hex_rgba(),
            &Font::default(),
            Color::from_rgb(53, 66, 84),
            HorizontalAlignment::Left,
        );
    }
}

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

    Color::from_rgba(
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
        assert_eq!(picker.color(), Color::from_rgba(0x33, 0x66, 0x99, 0xCC));
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
