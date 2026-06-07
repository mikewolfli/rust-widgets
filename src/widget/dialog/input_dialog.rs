//! Input dialog widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Input dialog input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Text,
    Integer,
    Double,
    Item,
}
/// Input dialog for simple user input.
pub struct InputDialog {
    base: BaseWidget,
    modal: bool,
    title: String,
    label_text: String,
    mode: InputMode,
    text_value: String,
    int_value: i64,
    double_value: f64,
    items: Vec<String>,
    current_item: usize,
    int_min: i64,
    int_max: i64,
    int_step: i64,
    double_min: f64,
    double_max: f64,
    _double_step: f64,
    double_decimals: u8,
    pub text_value_changed: Signal1<String>,
    pub int_value_changed: Signal1<i64>,
    pub double_value_changed: Signal1<f64>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl InputDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::InputDialog, geometry, "InputDialog"),
            modal: true,
            title: String::new(),
            label_text: String::new(),
            mode: InputMode::Text,
            text_value: String::new(),
            int_value: 0,
            double_value: 0.0,
            items: Vec::new(),
            current_item: 0,
            int_min: i64::MIN,
            int_max: i64::MAX,
            int_step: 1,
            double_min: f64::MIN,
            double_max: f64::MAX,
            _double_step: 1.0,
            double_decimals: 1,
            text_value_changed: Signal1::new(),
            int_value_changed: Signal1::new(),
            double_value_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn get_text(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.text_value = default.into();
        d.mode = InputMode::Text;
        d
    }
    pub fn get_int(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.int_value = value.clamp(min, max);
        d.int_min = min;
        d.int_max = max;
        d.int_step = step;
        d.mode = InputMode::Integer;
        d
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    pub fn mode(&self) -> InputMode {
        self.mode
    }
    pub fn text_value(&self) -> &str {
        &self.text_value
    }
    pub fn int_value(&self) -> i64 {
        self.int_value
    }
    pub fn double_value(&self) -> f64 {
        self.double_value
    }
    pub fn current_item(&self) -> usize {
        self.current_item
    }
    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn current_item_text(&self) -> Option<&str> {
        self.items.get(self.current_item).map(|s| s.as_str())
    }
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.title = t.into();
    }
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        self.label_text = t.into();
    }
    pub fn set_mode(&mut self, mode: InputMode) {
        self.mode = mode;
    }
    pub fn set_text_value(&mut self, v: impl Into<String>) {
        self.text_value = v.into();
    }
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_item = 0;
    }
    pub fn set_int_value(&mut self, v: i64) {
        self.int_value = v.clamp(self.int_min, self.int_max);
    }
    pub fn set_double_value(&mut self, v: f64) {
        self.double_value = v.clamp(self.double_min, self.double_max);
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
}
impl Widget for InputDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for InputDialog {
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
impl Draw for InputDialog {
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
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, 28), Color::from_rgb(0, 120, 215));
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
        // Input field
        let input_y = rect.y + 60;
        context.fill_rect(
            Rect::new(rect.x + 10, input_y, rect.width - 20, 26),
            Color::from_rgb(255, 255, 255),
        );
        context.draw_rect(
            Rect::new(rect.x + 10, input_y, rect.width - 20, 26),
            Color::from_rgb(150, 150, 150),
        );
        let display_text = match self.mode {
            InputMode::Text => self.text_value.clone(),
            InputMode::Integer => self.int_value.to_string(),
            InputMode::Double => {
                format!("{:.prec$}", self.double_value, prec = self.double_decimals as usize)
            }
            InputMode::Item => self.current_item_text().unwrap_or("").to_string(),
        };
        context.draw_text(
            Point::new(rect.x + 14, input_y + 13),
            &display_text,
            &Font::default(),
            Color::from_rgb(0, 0, 0),
        );
        // OK/Cancel
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, 80, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("common.button.ok"),
            &Font::default(),
            Color::from_rgb(255, 255, 255),
        );
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28),
            Color::from_rgb(225, 225, 225),
        );
        context.draw_rect(
            Rect::new(rect.x + rect.width as i32 - 88, btn_y as i32, 80, 28),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn get_int_clamps_and_sets_integer_mode() {
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 500, 0, 100, 5);
        assert_eq!(dialog.mode(), InputMode::Integer);
        assert_eq!(dialog.int_value(), 100);
    }

    #[test]
    fn enter_and_escape_emit_accept_reject() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        let accepted = Arc::new(Mutex::new(0usize));
        let rejected = Arc::new(Mutex::new(0usize));

        let a = Arc::clone(&accepted);
        dialog.accepted.connect(move || {
            if let Ok(mut n) = a.lock() {
                *n += 1;
            }
        });

        let r = Arc::clone(&rejected);
        dialog.rejected.connect(move || {
            if let Ok(mut n) = r.lock() {
                *n += 1;
            }
        });

        dialog.show();
        dialog.handle_event(&Event::key_press(13, 0));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        dialog.handle_event(&Event::key_press(27, 0));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }
}
