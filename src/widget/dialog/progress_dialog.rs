//! Progress dialog widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::tr;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Progress dialog widget.
pub struct ProgressDialog {
    base: BaseWidget,
    title: String,
    label_text: String,
    value: i32,
    minimum: i32,
    maximum: i32,
    cancel_button_text: String,
    was_canceled: bool,
    auto_close: bool,
    auto_reset: bool,
    modal: bool,
    pub canceled: GenericSignal,
}
impl ProgressDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ProgressDialog, geometry, "ProgressDialog"),
            title: String::new(),
            label_text: String::new(),
            value: 0,
            minimum: 0,
            maximum: 100,
            cancel_button_text: tr!("common.button.cancel"),
            was_canceled: false,
            auto_close: true,
            auto_reset: true,
            modal: true,
            canceled: GenericSignal::new(),
        }
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    pub fn value(&self) -> i32 {
        self.value
    }
    pub fn minimum(&self) -> i32 {
        self.minimum
    }
    pub fn maximum(&self) -> i32 {
        self.maximum
    }
    pub fn was_canceled(&self) -> bool {
        self.was_canceled
    }
    pub fn auto_close(&self) -> bool {
        self.auto_close
    }
    pub fn auto_reset(&self) -> bool {
        self.auto_reset
    }
    pub fn cancel_button_text(&self) -> &str {
        &self.cancel_button_text
    }
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.title = t.into();
    }
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        self.label_text = t.into();
    }
    pub fn set_minimum(&mut self, min: i32) {
        self.minimum = min;
    }
    pub fn set_maximum(&mut self, max: i32) {
        self.maximum = max;
    }
    /// Sets both minimum and maximum in one call.
    /// This is a convenience writer; query bounds via `minimum()` and `maximum()`.
    pub fn set_range(&mut self, min: i32, max: i32) {
        self.minimum = min;
        self.maximum = max;
    }
    pub fn set_auto_close(&mut self, v: bool) {
        self.auto_close = v;
    }
    pub fn set_auto_reset(&mut self, v: bool) {
        self.auto_reset = v;
    }
    pub fn set_cancel_button_text(&mut self, t: impl Into<String>) {
        self.cancel_button_text = t.into();
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }
    pub fn set_value(&mut self, value: i32) {
        self.value = value.clamp(self.minimum, self.maximum);
        if self.auto_close && self.value >= self.maximum {
            self.hide();
        }
    }
    pub fn reset(&mut self) {
        self.value = self.minimum;
        self.was_canceled = false;
    }
    pub fn cancel(&mut self) {
        self.was_canceled = true;
        self.canceled.emit();
        self.hide();
    }
    pub fn progress_fraction(&self) -> f32 {
        let range = self.maximum - self.minimum;
        if range <= 0 {
            return 1.0;
        }
        (self.value - self.minimum) as f32 / range as f32
    }
}
impl Widget for ProgressDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for ProgressDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::KeyPress { key, .. } if *key == 27 => self.cancel(),
            _ => {}
        }
    }
}
impl Draw for ProgressDialog {
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
            &self.title,
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        // Label
        context.draw_text(
            Point::new(rect.x + 10, rect.y + 48),
            &self.label_text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // Progress bar
        let bar_y = rect.y + 62;
        let bar_w = rect.width.saturating_sub(20);
        let bar_h: u32 = 20;
        context.fill_rect(
            Rect::new(rect.x + 10, bar_y, bar_w, bar_h),
            Color::from_rgb(220, 220, 220),
        );
        context.draw_rect(
            Rect::new(rect.x + 10, bar_y, bar_w, bar_h),
            Color::from_rgb(150, 150, 150),
        );
        let fill_w = (bar_w as f32 * self.progress_fraction()) as i32;
        if fill_w > 0 {
            context.fill_rect(
                Rect::new(rect.x + 10, bar_y, fill_w.max(0) as u32, bar_h),
                Color::from_rgb(6, 176, 37),
            );
        }
        // Percentage text
        let pct = (self.progress_fraction() * 100.0) as i32;
        context.draw_text(
            Point::new(rect.x + 10 + (bar_w as i32 / 2), bar_y + (bar_h as i32 / 2)),
            &format!("{}%", pct),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // Cancel button
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        let btn_w = 80;
        context.fill_rect(
            Rect::new(
                rect.x + rect.width as i32 / 2 - btn_w / 2,
                btn_y as i32,
                btn_w as u32,
                28u32,
            ),
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(
                rect.x + rect.width as i32 / 2 - btn_w / 2,
                btn_y as i32,
                btn_w as u32,
                28u32,
            ),
            Color::from_rgb(100, 100, 100),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 / 2, (btn_y + 14.0) as i32),
            &self.cancel_button_text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
    }
}
