//! Color dialog widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Color dialog for picking RGBA colors.
pub struct ColorDialog {
    base: BaseWidget,
    current_color: Color,
    options_alpha: bool,
    modal: bool,
    pub color_selected: Signal1<Color>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl ColorDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ColorDialog, geometry, "ColorDialog"),
            current_color: Color::rgb(255, 255, 255),
            options_alpha: false,
            modal: true,
            color_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }
    pub fn current_color(&self) -> Color {
        self.current_color
    }
    pub fn options_alpha(&self) -> bool {
        self.options_alpha
    }
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
        self.color_selected.emit(color);
        self.base.request_redraw();
    }
    pub fn set_options_alpha(&mut self, enabled: bool) {
        self.options_alpha = enabled;
    }
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    pub fn get_color(&self) -> Color {
        self.current_color
    }

    fn picker_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x + 10, rect.y + 38, rect.width - 20, rect.height.saturating_sub(120))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }

    fn pick_color_from_point(&self, pos: Point) -> Option<Color> {
        let picker = self.picker_rect();
        if !Self::point_in_rect(pos, picker) {
            return None;
        }
        let w = picker.width.max(1) as f32;
        let h = picker.height.max(1) as f32;
        let rx = ((pos.x - picker.x) as f32 / w).clamp(0.0, 1.0);
        let ry = ((pos.y - picker.y) as f32 / h).clamp(0.0, 1.0);
        let r = (rx * 255.0).round() as u8;
        let g = ((1.0 - ry) * 255.0).round() as u8;
        let b = ((1.0 - rx) * 255.0).round() as u8;
        let a = if self.options_alpha { self.current_color.a } else { 255 };
        Some(Color::rgba(r, g, b, a))
    }

    fn nudge_rgb(&mut self, dr: i16, dg: i16, db: i16) {
        let next = Color::rgba(
            (self.current_color.r as i16 + dr).clamp(0, 255) as u8,
            (self.current_color.g as i16 + dg).clamp(0, 255) as u8,
            (self.current_color.b as i16 + db).clamp(0, 255) as u8,
            self.current_color.a,
        );
        self.set_current_color(next);
    }
}
impl Widget for ColorDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(400, 300)
    }
}
impl EventHandler for ColorDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(color) = self.pick_color_from_point(*pos) {
                    self.set_current_color(color);
                }
            }
            Event::KeyPress { key, .. } => {
                if *key == 13 {
                    self.accept();
                } else if *key == 27 {
                    self.reject();
                } else if *key == 37 {
                    self.nudge_rgb(-5, 0, 0);
                } else if *key == 39 {
                    self.nudge_rgb(5, 0, 0);
                } else if *key == 38 {
                    self.nudge_rgb(0, 5, 0);
                } else if *key == 40 {
                    self.nudge_rgb(0, -5, 0);
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ColorDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::rgb(160, 160, 160),
        );
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28), Color::rgb(0, 120, 215));
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &tr!("color_dialog.title"),
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        // Color picker area (simplified)
        let picker_rect = self.picker_rect();
        context.fill_rect(picker_rect, Color::rgb(200, 200, 200));
        context.draw_rect(picker_rect, Color::rgb(100, 100, 100));
        // Color preview
        let preview_y = rect.y as f32 + rect.height as f32 - 80.0;
        context.fill_rect(Rect::new(rect.x + 10, preview_y as i32, 60, 30), self.current_color);
        context
            .draw_rect(Rect::new(rect.x + 10, preview_y as i32, 60, 30), Color::rgb(0, 0, 0));
        context.draw_text(
            Point::new(rect.x + 80, (preview_y + 15.0) as i32),
            &format!("{} {}", tr!("color_dialog.current_color"), self.current_color.to_hex_rgba()),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // OK/Cancel buttons
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, btn_w, 28),
            Color::rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("common.button.ok"),
            &Font::default(),
            Color::rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            &tr!("common.button.cancel"),
            &Font::default(),
            Color::rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn mouse_pick_updates_color() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.handle_event(&Event::mouse_press(60, 80, 1));
        assert_ne!(dialog.current_color(), Color::rgb(255, 255, 255));
    }

    #[test]
    fn arrow_keys_nudge_channels() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        dialog.set_current_color(Color::rgb(100, 100, 100));
        dialog.handle_event(&Event::key_press(39, 0));
        assert_eq!(dialog.current_color().r, 105);
        dialog.handle_event(&Event::key_press(38, 0));
        assert_eq!(dialog.current_color().g, 105);
    }

    #[test]
    fn set_current_color_emits_signal() {
        let mut dialog = ColorDialog::new(Rect::new(0, 0, 300, 260));
        let emitted = Arc::new(Mutex::new(Vec::<Color>::new()));
        let sink = emitted.clone();
        dialog.color_selected.connect(move |color| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*color);
            }
        });

        dialog.set_current_color(Color::rgb(1, 2, 3));

        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec![Color::rgb(1, 2, 3)]);
    }
}
