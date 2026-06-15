//! Font dialog widget.
use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::tr;

use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Font selection dialog.
pub struct FontDialog {
    base: BaseWidget,
    current_font: Font,
    modal: bool,
    pub font_selected: Signal1<Font>,
    pub accepted: GenericSignal,
    pub rejected: GenericSignal,
}
impl FontDialog {
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::FontDialog, geometry, "FontDialog"),
            current_font: Font::default(),
            modal: true,
            font_selected: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
        }
    }
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    pub fn set_current_font(&mut self, font: Font) {
        self.current_font = font.clone();
        self.font_selected.emit(font);
    }
    pub fn accept(&mut self) {
        self.font_selected.emit(self.current_font.clone());
        self.accepted.emit();
        self.hide();
    }
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
    pub fn get_font(&self) -> Font {
        self.current_font.clone()
    }
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    pub fn set_modal(&mut self, modal: bool) {
        self.modal = modal;
    }
}
impl Widget for FontDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for FontDialog {
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
impl Draw for FontDialog {
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
            &tr!("dialog.font.select_font"),
            &Font::default(),
            Color::from_rgb(255, 255, 255),
            HorizontalAlignment::Left,
        );
        let col_w = (rect.width / 3).saturating_sub(6);
        let list_y = rect.y + 38;
        let list_h = rect.height.saturating_sub(120);
        // Family, Style, Size columns
        let col_labels =
            [tr!("dialog.font.font_family"), tr!("dialog.font.style"), tr!("dialog.font.size")];
        for (i, label) in col_labels.iter().enumerate() {
            let col_x = rect.x as f32 + 4.0 + i as f32 * (col_w as f32 + 4.0);
            context.draw_text(
                Point::new(col_x as i32, list_y - 10),
                label.as_str(),
                &Font::default(),
                Color::from_rgb(0, 0, 0),
                HorizontalAlignment::Left,
            );
            context.fill_rect(
                Rect::new(col_x as i32, list_y, col_w, list_h),
                Color::from_rgb(255, 255, 255),
            );
            context.draw_rect(
                Rect::new(col_x as i32, list_y, col_w, list_h),
                Color::from_rgb(150, 150, 150),
            );
        }
        // Preview area
        let prev_y = list_y + list_h as i32 + 8;
        let bw = rect.width.saturating_sub(8);
        context.fill_rect(Rect::new(rect.x + 4, prev_y, bw, 36), Color::from_rgb(255, 255, 255));
        context.draw_rect(Rect::new(rect.x + 4, prev_y, bw, 36), Color::from_rgb(150, 150, 150));
        context.draw_text(
            Point::new(rect.x + 10, prev_y + 18),
            "AaBbYyZz 0123",
            &self.current_font,
            Color::from_rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
        // OK/Cancel
        let btn_y = rect.y as f32 + rect.height as f32 - 40.0;
        context.fill_rect(
            Rect::new(rect.x + rect.width as i32 - 176, btn_y as i32, 80, 28),
            Color::from_rgb(0, 120, 215),
        );
        context.draw_text(
            Point::new(rect.x + rect.width as i32 - 136, (btn_y + 14.0) as i32),
            &tr!("dialog.ok"),
            &Font::default(),
            Color::from_rgb(255, 255, 255),
            HorizontalAlignment::Left,
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
            &tr!("dialog.cancel"),
            &Font::default(),
            Color::from_rgb(0, 0, 0),
            HorizontalAlignment::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn set_current_font_emits_font_selected() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
        let seen = Arc::new(Mutex::new(Font::default()));
        let seen_clone = Arc::clone(&seen);

        dialog.font_selected.connect(move |font| {
            if let Ok(mut f) = seen_clone.lock() {
                *f = (*font).clone();
            }
        });

        let target = Font::simple("Sans", 18.0);
        dialog.set_current_font(target.clone());
        assert_eq!(dialog.current_font(), &target);
        assert_eq!(*seen.lock().expect("seen lock"), target);
    }

    #[test]
    fn enter_accepts_and_escape_rejects() {
        let mut dialog = FontDialog::new(Rect::new(0, 0, 420, 320));
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
