use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};

use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Widget, WidgetKind};
/// Font combo box widget for font selection.
pub struct FontComboBox {
    base: BaseWidget,
    current_font: Font,
    fonts: Vec<String>,
    current_index: i32,
    editable: bool,
    max_visible_items: i32,
    expanded: bool,
    /// Emitted when the current font changes.
    pub current_font_changed: Signal1<Font>,
    /// Emitted when the current index changes.
    pub current_index_changed: Signal1<i32>,
    /// Emitted when the combo box is activated.
    pub activated: Signal1<i32>,
    /// Emitted when the text is edited (if editable).
    pub text_edited: Signal1<String>,
    /// Emitted when the popup is shown.
    pub popup_shown: GenericSignal,
    /// Emitted when the popup is hidden.
    pub popup_hidden: GenericSignal,
}
impl FontComboBox {
    pub fn new(geometry: Rect) -> Self {
        let default_font = Font::default();
        Self {
            base: BaseWidget::new(WidgetKind::FontComboBox, geometry, "FontComboBox"),
            current_font: default_font.clone(),
            fonts: Vec::new(),
            current_index: -1,
            editable: false,
            max_visible_items: 10,
            expanded: false,
            current_font_changed: Signal1::new(),
            current_index_changed: Signal1::new(),
            activated: Signal1::new(),
            text_edited: Signal1::new(),
            popup_shown: GenericSignal::new(),
            popup_hidden: GenericSignal::new(),
        }
    }
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }
    pub fn count(&self) -> i32 {
        self.fonts.len() as i32
    }
    pub fn set_current_font(&mut self, font: Font) {
        if self.current_font != font {
            self.current_font = font.clone();
            self.current_font_changed.emit(font);
            self.base.request_redraw();
        }
    }
    pub fn set_current_index(&mut self, index: i32) {
        let clamped = index.clamp(-1, self.fonts.len() as i32 - 1);
        if self.current_index != clamped {
            self.current_index = clamped;
            self.current_index_changed.emit(clamped);
            if clamped >= 0 && clamped < self.fonts.len() as i32 {
                // Update current font based on selection
                if let Some(font_name) = self.fonts.get(clamped as usize) {
                    let new_font = Font::new(font_name, self.current_font.size(), false, false);
                    self.set_current_font(new_font);
                }
            }
            self.base.request_redraw();
        }
    }
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        self.base.request_redraw();
    }
    pub fn set_max_visible_items(&mut self, max_items: i32) {
        self.max_visible_items = max_items.max(1);
    }
    pub fn add_font(&mut self, font_name: String) {
        self.fonts.push(font_name);
        self.base.request_redraw();
    }
    pub fn remove_font(&mut self, index: i32) {
        if index >= 0 && index < self.fonts.len() as i32 {
            self.fonts.remove(index as usize);
            if self.current_index == index {
                self.set_current_index(-1);
            } else if self.current_index > index {
                self.current_index -= 1;
            }
            self.base.request_redraw();
        }
    }
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.set_current_index(-1);
        self.base.request_redraw();
    }
    pub fn show_popup(&mut self) {
        self.expanded = true;
        self.popup_shown.emit();
        self.base.request_redraw();
    }
    pub fn hide_popup(&mut self) {
        self.expanded = false;
        self.popup_hidden.emit();
        self.base.request_redraw();
    }
    pub fn current_text(&self) -> String {
        if self.current_index >= 0 && self.current_index < self.fonts.len() as i32 {
            self.fonts[self.current_index as usize].clone()
        } else {
            String::new()
        }
    }
}
impl Widget for FontComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
}
use crate::render::RenderContext;
use crate::widget::Draw;

impl EventHandler for FontComboBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } if button == &1 => {
                // Show the dropdown list
                self.show_popup();
                self.base.clicked.emit();
            }
            Event::MouseRelease { pos: _, button }
                if button == &1
                    // Cycle to next font on release
                    && !self.fonts.is_empty() =>
            {
                let next = (self.current_index + 1) % self.fonts.len() as i32;
                self.set_current_index(next);
                self.activated.emit(next);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for FontComboBox {
    fn draw(&mut self, ctx: &mut RenderContext) {
        let g = self.geometry();
        ctx.fill_rect(g, Color::WHITE);
        ctx.draw_rect(g, Color::rgb(200, 200, 200));
        // Draw drop-down arrow indicator
        let arrow_right_x = g.x + g.width as i32 - 14;
        let arrow_y = g.y + g.height as i32 / 2;
        ctx.draw_line(
            Point::new(arrow_right_x - 3, arrow_y - 2),
            Point::new(arrow_right_x, arrow_y + 2),
            Color::rgb(100, 100, 100),
        );
        ctx.draw_line(
            Point::new(arrow_right_x, arrow_y + 2),
            Point::new(arrow_right_x + 3, arrow_y - 2),
            Color::rgb(100, 100, 100),
        );
        let font_name = self.current_font().family().to_string();
        ctx.draw_text(
            Point::new(g.x + 4, g.y + g.height as i32 / 2 + 5),
            &font_name,
            &Font::default_ui(),
            Color::BLACK,
            HorizontalAlignment::Left,
        );
        // Draw popup list when expanded
        if self.expanded && !self.fonts.is_empty() {
            let popup_item_height = 22i32;
            let visible_count = self.fonts.len().min(self.max_visible_items as usize);
            let popup_height = (visible_count as i32) * popup_item_height;
            let popup_rect = Rect::new(g.x, g.y + g.height as i32, g.width, popup_height as u32);
            ctx.fill_rect(popup_rect, Color::WHITE);
            ctx.draw_rect(popup_rect, Color::rgb(180, 180, 180));
            let display_font = Font::default_ui();
            for i in 0..visible_count {
                let item_y = popup_rect.y + (i as i32) * popup_item_height;
                let item_rect =
                    Rect::new(popup_rect.x, item_y, popup_rect.width, popup_item_height as u32);
                if i as i32 == self.current_index {
                    ctx.fill_rect(item_rect, Color::rgb(0, 120, 215));
                    if let Some(name) = self.fonts.get(i) {
                        ctx.draw_text(
                            Point::new(g.x + 4, item_y + popup_item_height / 2 + 3),
                            name,
                            &display_font,
                            Color::WHITE,
                            HorizontalAlignment::Left,
                        );
                    }
                } else if let Some(name) = self.fonts.get(i) {
                    ctx.draw_text(
                        Point::new(g.x + 4, item_y + popup_item_height / 2 + 3),
                        name,
                        &display_font,
                        Color::BLACK,
                        HorizontalAlignment::Left,
                    );
                }
            }
        }
    }
    fn uses_custom_drawing(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn fontcombobox_creation_defaults() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.fonts().is_empty());
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.current_index(), -1);
        assert!(fcb.current_text().is_empty());
        assert!(!fcb.is_editable());
        assert_eq!(fcb.max_visible_items(), 10);
    }

    #[test]
    fn fontcombobox_add_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        assert_eq!(fcb.count(), 2);
        assert_eq!(fcb.fonts()[0], "Arial");
        assert_eq!(fcb.fonts()[1], "Helvetica");
    }

    #[test]
    fn fontcombobox_remove_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.remove_font(0);
        assert_eq!(fcb.count(), 1);
        assert_eq!(fcb.fonts()[0], "Helvetica");
    }

    #[test]
    fn fontcombobox_clear() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        fcb.clear();
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.current_index(), -1);
    }

    #[test]
    fn fontcombobox_set_current_index() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        assert_eq!(fcb.current_index(), 0);
        assert_eq!(fcb.current_text(), "Arial");
    }

    #[test]
    fn fontcombobox_set_current_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let font = Font::new("Arial", 12.0, false, false);
        fcb.set_current_font(font.clone());
        assert_eq!(fcb.current_font().family(), "Arial");
    }

    #[test]
    fn fontcombobox_editable() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!fcb.is_editable());
        fcb.set_editable(true);
        assert!(fcb.is_editable());
        fcb.set_editable(false);
        assert!(!fcb.is_editable());
    }

    #[test]
    fn fontcombobox_max_visible_items() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(fcb.max_visible_items(), 10);
        fcb.set_max_visible_items(5);
        assert_eq!(fcb.max_visible_items(), 5);
        fcb.set_max_visible_items(0); // floors at 1
        assert_eq!(fcb.max_visible_items(), 1);
    }

    #[test]
    fn fontcombobox_show_hide_popup() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.show_popup();
        fcb.hide_popup();
        // Should not panic
    }

    #[test]
    fn fontcombobox_geometry_delegation() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(fcb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn fontcombobox_visibility() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_visible());
        fcb.hide();
        assert!(!fcb.is_visible());
        fcb.show();
        assert!(fcb.is_visible());
    }

    #[test]
    fn fontcombobox_enabled() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_enabled());
        fcb.set_enabled(false);
        assert!(!fcb.is_enabled());
        fcb.set_enabled(true);
        assert!(fcb.is_enabled());
    }

    #[test]
    fn fontcombobox_id_kind() {
        let fcb_a = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let fcb_b = FontComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(fcb_a.id(), fcb_b.id());
        assert_eq!(fcb_a.kind(), WidgetKind::FontComboBox);
        assert_eq!(fcb_b.kind(), WidgetKind::FontComboBox);
    }

    #[test]
    fn fontcombobox_signal_accessors() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &fcb.current_font_changed;
        let _ = &fcb.current_index_changed;
        let _ = &fcb.activated;
        let _ = &fcb.text_edited;
        let _ = &fcb.popup_shown;
        let _ = &fcb.popup_hidden;
    }

    #[test]
    fn font_combo_box_draw_produces_output() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let svg = crate::widget::svg::render_to_svg(&mut fcb);
        assert!(svg.starts_with("<svg"));
    }
}
