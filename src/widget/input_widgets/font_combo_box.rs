// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};

use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_i64};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
/// Font combo box widget for font selection.
///
/// Holds a list of font family names and an index into it. The index uses `-1`
/// as an explicit "nothing selected" sentinel, which is why selection is an
/// `i32` rather than a `usize`.
///
/// The widget does not enumerate installed fonts: the caller populates the list
/// with [`FontComboBox::add_font`].
///
pub struct FontComboBox {
    base: BaseWidget,
    current_font: Font,
    fonts: Vec<String>,
    current_index: i32,
    editable: bool,
    /// In-progress typed text while the box is editable.
    ///
    /// `None` means "not editing": the field shows the current font instead. It is
    /// distinct from `Some(String::new())`, which is an edit in progress whose text
    /// the user has erased — the two render differently, and collapsing them would
    /// make a cleared field look like an idle one.
    edit_buffer: Option<String>,
    max_visible_items: i32,
    expanded: bool,
    /// Emitted with the new font whenever [`FontComboBox::set_current_font`]
    /// actually changes it — including as a side effect of changing the index.
    pub current_font_changed: Signal1<Font>,
    /// Emitted with the new index (possibly `-1`) whenever
    /// [`FontComboBox::set_current_index`] actually changes it.
    pub current_index_changed: Signal1<i32>,
    /// Emitted with the index that was activated by the user. Currently only
    /// fired from the mouse-release path, with the same index that was just
    /// passed to `set_current_index`.
    pub activated: Signal1<i32>,
    /// Emitted when the user commits typed text while the combo box is editable.
    ///
    /// The payload is the committed family name. "Committed" rather than "every
    /// keystroke": a font family is only meaningful once the caller acts on it, and
    /// firing per keystroke would make a handler that loads a font reload it on
    /// every character. Enter and the popup's own selection both commit.
    pub text_edited: Signal1<String>,
    /// Emitted when the popup list is opened.
    pub popup_shown: GenericSignal,
    /// Emitted when the popup list is closed.
    pub popup_hidden: GenericSignal,
}
impl FontComboBox {
    /// Creates an empty, non-editable combo box with no selection
    /// ([`FontComboBox::current_index`] `-1`), the default font, and a maximum
    /// of 10 visible popup items (the popup starts closed).
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 200x28.
    pub fn new(geometry: Rect) -> Self {
        let default_font = Font::default();
        Self {
            base: BaseWidget::new(WidgetKind::FontComboBox, geometry, "FontComboBox"),
            current_font: default_font.clone(),
            fonts: Vec::new(),
            current_index: -1,
            editable: false,
            edit_buffer: None,
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
    /// Returns the font family and size currently in effect. This is a
    /// [`Font`] value, which carries the size; note that selecting an index
    /// keeps the size but resets the bold/italic flags to `false`.
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    /// Returns the list of selectable family names, in display order.
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }
    /// Returns the selected index, or `-1` when nothing is selected.
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    /// Returns whether the combo box is marked editable. Defaults to `false`.
    ///
    /// When editable, typed characters accumulate in [`Self::edit_buffer`], Enter or
    /// [`Self::commit_edit`] emits [`Self::text_edited`], and Escape discards.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Returns how many popup entries are shown before the list is truncated.
    /// Always at least `1`; defaults to `10`.
    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }
    /// Returns the number of fonts in the list; equivalent to `fonts().len()`
    /// narrowed to `i32`.
    pub fn count(&self) -> i32 {
        self.fonts.len() as i32
    }
    /// Replaces the effective font.
    ///
    /// A no-op when the value is unchanged: no signal, no redraw. This does
    /// **not** update [`FontComboBox::current_index`], so the index and the font
    /// can disagree until the index is set. Emits `current_font_changed`.
    pub fn set_current_font(&mut self, font: Font) {
        if self.current_font != font {
            self.current_font = font.clone();
            self.current_font_changed.emit(font);
            self.base.request_redraw();
        }
    }
    /// Selects an entry, clamped to `-1 ..= count()-1`.
    ///
    /// A no-op when the clamped index is unchanged. Otherwise emits
    /// `current_index_changed`, and for a non-negative index also derives the
    /// font from the selected name — **keeping the current point size but
    /// clearing the bold and italic flags** — which may in turn emit
    /// `current_font_changed`. Index `-1`, or an empty list (where the clamp is
    /// also `-1`), leaves the font untouched.
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
    /// Sets the editable flag. See [`FontComboBox::is_editable`] — it currently
    /// has no behavioural effect beyond being reported. Requests a redraw.
    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
        if !editable {
            // Turning the feature off abandons any in-progress edit, so the field
            // cannot keep showing text a non-editable box would not accept.
            self.edit_buffer = None;
        }
        self.base.request_redraw();
    }
    /// The in-progress typed text, or `None` when no edit is under way.
    ///
    /// `Some("")` is a real state: an edit the user has erased. See the field's own
    /// comment for why the two are not collapsed.
    pub fn edit_buffer(&self) -> Option<&str> {
        self.edit_buffer.as_deref()
    }
    /// Commits the typed text, emitting `text_edited`.
    ///
    /// Committing an empty edit is refused rather than emitting an empty family
    /// name: an empty string is not a font, and a caller that loads what the signal
    /// names would have nothing to load. Returns `true` when a commit happened.
    pub fn commit_edit(&mut self) -> bool {
        let Some(text) = self.edit_buffer.take() else {
            return false;
        };
        self.base.request_redraw();
        if text.trim().is_empty() {
            return false;
        }
        self.text_edited.emit(text.clone());
        // A typed family that matches a listed font also moves the selection, so the
        // typed path and the picker path cannot end up describing different fonts.
        if let Some(index) = self.fonts.iter().position(|family| family == &text) {
            self.set_current_index(index as i32);
        }
        true
    }
    /// Sets how many popup entries are shown, floored at `1` so the popup is
    /// never zero-height. Does not request a redraw (unlike the other setters),
    /// so an already-open popup may not repaint until the next redraw.
    pub fn set_max_visible_items(&mut self, max_items: i32) {
        self.max_visible_items = max_items.max(1);
    }
    /// Appends a family name to the end of the list and requests a redraw.
    ///
    /// Duplicates are not filtered: adding the same name twice yields two
    /// identical, independently selectable entries.
    pub fn add_font(&mut self, font_name: String) {
        self.fonts.push(font_name);
        self.base.request_redraw();
    }
    /// Removes the font at `index`; out-of-range indices are ignored.
    ///
    /// If the removed entry was selected, the selection is cleared to `-1`;
    /// otherwise an index after the removal point is shifted down by one so it
    /// keeps referring to the same entry. Note that only the raw index field is
    /// adjusted in that case — `current_index_changed` is **not** emitted —
    /// though the selection still points at the same font.
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
    /// Empties the font list and clears the selection via
    /// [`FontComboBox::set_current_index`], so `current_index_changed` fires if
    /// an entry had been selected.
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.set_current_index(-1);
        self.base.request_redraw();
    }
    /// Opens the popup list, emits `popup_shown`, and requests a redraw.
    ///
    /// Not idempotent: calling it while already open emits again. The widget
    /// does not close the popup on outside clicks.
    pub fn show_popup(&mut self) {
        self.expanded = true;
        self.popup_shown.emit();
        self.base.request_redraw();
    }
    /// Closes the popup list, emits `popup_hidden`, and requests a redraw.
    /// As with [`FontComboBox::show_popup`], calling it while already closed
    /// still emits.
    pub fn hide_popup(&mut self) {
        self.expanded = false;
        self.popup_hidden.emit();
        self.base.request_redraw();
    }
    /// Returns the family name of the selected entry, or an empty string when
    /// nothing is selected ([`FontComboBox::current_index`] `-1`).
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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `FontComboBox`'s property contract.
///
/// This control uses `i32` indices (the `-1` sentinel means "no font"), which is
/// why its index properties are `Int` rather than `UInt` — the split
/// `FONT_COMBO_BOX_PROPERTIES` records and the old dispatch preserved.
impl WidgetProperties for FontComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_font_family" => Ok(CapabilityValue::String(self.current_text())),
            "item_count" => Ok(CapabilityValue::Int(self.count() as i64)),
            "current_index" => Ok(CapabilityValue::Int(self.current_index() as i64)),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "max_visible_items" => Ok(CapabilityValue::Int(self.max_visible_items() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_i64(value)? as i32);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_i64(value)? as i32);
                Ok(())
            }
            // Derived from the font list.
            "current_font_family" | "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `FONT_COMBO_BOX_PROPERTIES`.
        property_names_of![
            "current_font_family",
            "item_count",
            "current_index",
            "editable",
            "max_visible_items",
            BASE_PROPERTY_NAMES
        ]
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
            Event::KeyPress { key, modifiers: _ } if self.editable => {
                // Mirrors `EditableComboBox`'s key convention: 8 is backspace, 13 is
                // Enter, 27 is Escape, and any other printable code is a character.
                match *key {
                    8 => {
                        let mut buffer = self
                            .edit_buffer
                            .take()
                            .unwrap_or_else(|| self.current_font().family().to_string());
                        buffer.pop();
                        self.edit_buffer = Some(buffer);
                        self.base.request_redraw();
                    }
                    13 => {
                        self.commit_edit();
                    }
                    27 => {
                        // Escape discards the edit rather than committing it, which is
                        // the only difference between the two exit paths and therefore
                        // the whole reason both exist.
                        self.edit_buffer = None;
                        self.base.request_redraw();
                    }
                    _ => {
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                let mut buffer = self.edit_buffer.take().unwrap_or_default();
                                buffer.push(ch);
                                self.edit_buffer = Some(buffer);
                                self.base.request_redraw();
                            }
                        }
                    }
                }
            }
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
