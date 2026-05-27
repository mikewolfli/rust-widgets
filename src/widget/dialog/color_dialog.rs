//! Color dialog widget.
use crate::core::{Color, Font, Point, Rect};
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
            current_color: Color::from_rgb(255, 255, 255),
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
}
impl Widget for ColorDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for ColorDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::KeyPress { key, .. } = event {
            if *key == 13 {
                self.accept();
            } else if *key == 27 {
                self.reject();
            }
        }
    }
}
impl Draw for ColorDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(245, 245, 245),
        );
        context.draw_rect(
            Rect::new(rect.x, rect.y, rect.width, rect.height),
            Color::from_rgb(160, 160, 160),
        );
        context.fill_rect(
            Rect::new(rect.x, rect.y, rect.width, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + 8, rect.y + 14),
            &tr!("color_dialog.title"),
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        // Color picker area (simplified)
        let picker_rect_x = rect.x + 10;
        let picker_rect_y = rect.y + 38;
        let picker_w = rect.width - 20;
        let picker_h = rect.height.saturating_sub(120);
        context.fill_rect(
            Rect::new(picker_rect_x, picker_rect_y, picker_w, picker_h),
            Color::from_rgb(200, 200, 200),
        );
        context.draw_rect(
            Rect::new(picker_rect_x, picker_rect_y, picker_w, picker_h),
            Color::from_rgb(100, 100, 100),
        );
        // Color preview
        let preview_y = rect.y as f32 + rect.height as f32 - 80.0;
        context.fill_rect(
            Rect::new(rect.x + 10, preview_y as i32, 60, 30),
            self.current_color,
        );
        context.draw_rect(
            Rect::new(rect.x + 10, preview_y as i32, 60, 30),
            Color::from_rgb(0, 0, 0),
        );
        context.draw_text(
            Point::new(rect.x + 80, (preview_y + 15.0) as i32),
            &tr!("color_dialog.current_color"),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // OK/Cancel buttons
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, btn_w, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("common.button.ok"),
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, btn_w, 28),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 48, (btn_y + 14.0) as i32),
            &tr!("common.button.cancel"),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
