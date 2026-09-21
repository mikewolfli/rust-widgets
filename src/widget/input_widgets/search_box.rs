// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SearchBox widget — a text input with search icon, placeholder, and clear button.
//!
//! The SearchBox presents a text field with a magnifying glass icon on the left,
//! placeholder text when empty, and a clear (X) button on the right when the
//! text is non-empty. It emits `text_changed` with the current text value on
//! every edit.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// SearchBox widget with search icon, placeholder, and clear button.
pub struct SearchBox {
    base: BaseWidget,
    text: String,
    placeholder: String,
    focused: bool,
    /// Emitted when the text changes, providing the new text value.
    pub text_changed: Signal1<String>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl SearchBox {
    /// Creates a new SearchBox widget with the given geometry.
    /// Uses "Search…" as the default placeholder text.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SearchBox, geometry, "SearchBox"),
            text: String::new(),
            placeholder: "Search\u{2026}".to_string(),
            focused: false,
            text_changed: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the current text content (alias for `text()`).
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Sets the text content. Emits `text_changed` if the text actually changes.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            let before = self.text.clone();
            self.text = text.clone();
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                    self.history_target.clone(),
                    before,
                    self.text.clone(),
                    "search_box_text",
                )));
            }
            self.text_changed.emit(text);
            self.base.request_redraw();
        }
    }

    /// Returns the current placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text shown when the search box is empty.
    pub fn set_placeholder(&mut self, placeholder: &str) {
        self.placeholder = placeholder.to_string();
        self.base.request_redraw();
    }

    /// Returns whether this search box currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets the focused state.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            self.base.request_redraw();
            if focused {
                self.base.focus_gained.emit();
            } else {
                self.base.focus_lost.emit();
            }
        }
    }

    /// Clears the text content. Emits `text_changed` if the text was non-empty.
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.set_text(String::new());
        }
    }

    /// Reverts the most recent edit in this search box.
    ///
    /// Returns `false` and changes nothing when there is nothing to undo;
    /// otherwise emits `text_changed` with the restored text.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Reapplies the most recently undone edit.
    ///
    /// Returns `false` and changes nothing when there is nothing to redo;
    /// otherwise emits `text_changed` with the restored text.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns whether there is an undone edit to reapply.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
}

impl Widget for SearchBox {
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

/// `SearchBox`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before.
impl WidgetProperties for SearchBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                self.set_placeholder(&expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["text", "placeholder", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `search_box` publishes.
    ///
    /// `clear` is the zero-argument action and goes through the control's own method, so
    /// the `text_changed` signal and the empty-text short circuit stay in one place.
    /// `set_text` and `set_placeholder` assign state and need a payload, so they are
    /// answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "set_text" | "set_placeholder" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for SearchBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let icon_size = 14;
        let icon_margin = 10;
        let clear_btn_size = 16;
        let clear_margin = 8;
        let text_x = rect.x + icon_margin + icon_size + 6;
        let text_width = if self.text.is_empty() {
            rect.width as i32 - (icon_margin + icon_size + 6 + clear_margin)
        } else {
            rect.width as i32
                - (icon_margin + icon_size + 6 + clear_margin + clear_btn_size + clear_margin)
        };
        let text_rect = Rect::new(text_x, rect.y, text_width.max(1) as u32, rect.height);
        let center_y = rect.y + rect.height as i32 / 2;

        // — Background —
        //
        // From the style, not a literal. This painted a fixed light grey in every state and
        // so stayed light in a dark theme: the theme resolved a search box's colour, handed
        // it to the widget, and the widget ignored it. The three-state ladder is kept, but
        // it is now *derived* from the resolved colour — a focused box is the resolved
        // colour blended toward the accent, a disabled one is faded — so the states stay
        // distinguishable *and* follow the theme.
        let style = self.style().clone();
        // Resolved once, because the three branches below all need it and re-resolving would
        // take the theme lock three times inside one draw.
        let themed = crate::style::resolved_theme_style("search_box");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
        // Precedence: an explicit style wins, then the theme, then the original literal as a
        // last resort so an inactive theme still gives the widget a defined appearance.
        let accent = style.border_color.or(themed_border);
        let base_bg =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(235, 235, 235, 200));
        let bg_color = if !is_enabled {
            // Faded rather than a separate literal: "disabled" is the same colour with the
            // energy taken out, which is what blending toward white expresses.
            base_bg.blend(&Color::WHITE, 0.25)
        } else if self.focused {
            base_bg.blend(&accent.unwrap_or(Color::rgba(60, 140, 255, 200)), 0.18)
        } else {
            base_bg
        };
        context.fill_rounded_rect(rect, 6, bg_color);

        // — Focus border —
        if self.focused && is_enabled {
            context.draw_rounded_rect_stroke(
                rect,
                6,
                accent.unwrap_or(Color::rgba(60, 140, 255, 200)),
                2,
            );
        } else {
            context.draw_rounded_rect_stroke(
                rect,
                6,
                style.border_color.unwrap_or(Color::rgba(200, 200, 200, 160)),
                1,
            );
        }

        // — Search icon (magnifying glass) —
        let icon_cx = rect.x + icon_margin + icon_size / 2;
        let icon_cy = center_y;
        let icon_color = if is_enabled {
            Color::rgba(140, 140, 140, 220)
        } else {
            Color::rgba(180, 180, 180, 120)
        };

        // Draw circle part of magnifying glass
        let circle_radius = icon_size as u32 * 5 / 14; // ~5 pixel radius
        context.draw_circle(Point::new(icon_cx - 1, icon_cy - 1), circle_radius, icon_color);
        // Draw handle (angled line)
        let handle_start_x = icon_cx - 1 + circle_radius as i32;
        let handle_start_y = icon_cy - 1 + circle_radius as i32;
        let handle_end_x = handle_start_x + 4;
        let handle_end_y = handle_start_y + 4;
        context.draw_line(
            Point::new(handle_start_x, handle_start_y),
            Point::new(handle_end_x, handle_end_y),
            icon_color,
        );

        // — Text / Placeholder —
        let default_font = crate::core::Font::default();
        let font = self.font().unwrap_or(&default_font);
        let text_color = if !is_enabled {
            Color::rgba(160, 160, 160, 180)
        } else if !self.text.is_empty() {
            Color::rgba(30, 30, 30, 230)
        } else {
            Color::rgba(160, 160, 160, 200)
        };
        let display_text = if self.text.is_empty() { &self.placeholder } else { &self.text };
        let text_origin =
            Point::new(text_rect.x + 2, text_rect.y + text_rect.height as i32 / 2 + 4);
        context.draw_text(text_origin, display_text, font, text_color, HorizontalAlignment::Left);

        // — Clear button (X circle) —
        if !self.text.is_empty() && is_enabled {
            let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
            let clear_cy = center_y;
            let clear_radius = clear_btn_size as u32 / 2;
            // Circle background
            context.fill_circle(
                Point::new(clear_cx, clear_cy),
                clear_radius,
                Color::rgba(180, 180, 180, 200),
            );
            // X mark (two diagonal lines)
            let offset = 3;
            context.draw_line(
                Point::new(clear_cx - offset, clear_cy - offset),
                Point::new(clear_cx + offset, clear_cy + offset),
                Color::WHITE,
            );
            context.draw_line(
                Point::new(clear_cx + offset, clear_cy - offset),
                Point::new(clear_cx - offset, clear_cy + offset),
                Color::WHITE,
            );
        }
    }
}

impl EventHandler for SearchBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } if *button == 1 => {
                // Check if clear button was clicked
                if !self.text.is_empty() {
                    let rect = self.geometry();
                    let clear_margin = 8;
                    let clear_btn_size = 16;
                    let center_y = rect.y + rect.height as i32 / 2;
                    let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
                    let clear_cy = center_y;
                    let dx = pos.x - clear_cx;
                    let dy = pos.y - clear_cy;
                    let clear_radius = clear_btn_size as u32 / 2;
                    // Hit-test within the clear button circle
                    if dx * dx + dy * dy <= (clear_radius as i32 * clear_radius as i32) {
                        self.clear();
                        return;
                    }
                }
                // Click on the search box → gain focus
                self.set_focused(true);
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                // No special release handling needed
            }
            Event::FocusGained => {
                self.set_focused(true);
            }
            Event::FocusLost => {
                self.set_focused(false);
            }
            Event::KeyPress { key, modifiers } => {
                if !self.focused {
                    return;
                }
                if *modifiers == 2 && *key == 90 {
                    let _ = self.undo();
                    return;
                }
                if *modifiers == 2 && *key == 89 {
                    let _ = self.redo();
                    return;
                }
                match *key {
                    8 => {
                        // Backspace — remove last character
                        if !self.text.is_empty() {
                            let mut next = self.text.clone();
                            next.pop();
                            self.set_text(next);
                        }
                    }
                    127 => {
                        // Delete — remove last character
                        if !self.text.is_empty() {
                            let mut next = self.text.clone();
                            next.pop();
                            self.set_text(next);
                        }
                    }
                    13 | 27 => {
                        // Enter / Escape — lose focus
                        self.set_focused(false);
                    }
                    _ => {
                        // Character input
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                let mut next = self.text.clone();
                                next.push(ch);
                                self.set_text(next);
                            }
                        }
                    }
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn search_box_default_creation() {
        let sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        assert_eq!(sb.text(), "");
        assert_eq!(sb.get_text(), "");
        assert_eq!(sb.placeholder(), "Search\u{2026}");
        assert!(!sb.is_focused());
        assert_eq!(sb.kind(), WidgetKind::SearchBox);
        assert!(sb.is_enabled());
    }

    #[test]
    fn search_box_set_text_get_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("hello");
        assert_eq!(sb.text(), "hello");
        assert_eq!(sb.get_text(), "hello");
    }

    #[test]
    fn search_box_set_text_emits_signal() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(None::<String>));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        sb.set_text("world");
        assert_eq!(*captured.lock().unwrap(), Some("world".to_string()));
    }

    #[test]
    fn search_box_text_changed_signal_on_type() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let last = Arc::new(Mutex::new(String::new()));
        sb.text_changed.connect({
            let last = Arc::clone(&last);
            move |val: Arc<String>| {
                *last.lock().unwrap() = val.to_string();
            }
        });

        // Simulate typing 'a', 'b', 'c'
        sb.set_focused(true);
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        sb.handle_event(&Event::KeyPress { key: 98, modifiers: 0 }); // 'b'
        sb.handle_event(&Event::KeyPress { key: 99, modifiers: 0 }); // 'c'
        assert_eq!(sb.text(), "abc");
        assert_eq!(*last.lock().unwrap(), "abc");
    }

    #[test]
    fn search_box_placeholder_display() {
        let sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        // Default placeholder
        assert_eq!(sb.placeholder(), "Search\u{2026}");

        // Custom placeholder
        let mut sb_custom = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb_custom.set_placeholder("Find...");
        assert_eq!(sb_custom.placeholder(), "Find...");
    }

    #[test]
    fn search_box_clear_button_click() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("test text");
        assert_eq!(sb.text(), "test text");

        // Calculate clear button center position
        let clear_margin = 8;
        let clear_btn_size = 16;
        let rect = sb.geometry();
        let center_y = rect.y + rect.height as i32 / 2;
        let clear_cx = rect.x + rect.width as i32 - clear_margin - clear_btn_size / 2;
        let clear_cy = center_y;

        // Click the clear button
        sb.handle_event(&Event::MousePress { pos: Point::new(clear_cx, clear_cy), button: 1 });

        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_clear_button_emits_signal() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(None::<String>));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        sb.set_text("hello");

        // Clear via method
        sb.clear();
        assert_eq!(sb.text(), "");
        assert_eq!(*captured.lock().unwrap(), Some("".to_string()));
    }

    #[test]
    fn search_box_backspace() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("hello");
        sb.set_focused(true);

        sb.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(sb.text(), "hell");

        sb.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // Backspace
        assert_eq!(sb.text(), "hel");
    }

    #[test]
    fn search_box_set_placeholder_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_placeholder("Type to search...");
        assert_eq!(sb.placeholder(), "Type to search...");
    }

    #[test]
    fn search_box_focus_on_click() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        assert!(!sb.is_focused());

        sb.handle_event(&Event::MousePress { pos: Point::new(50, 16), button: 1 });
        assert!(sb.is_focused());
    }

    #[test]
    fn search_box_focus_lost_on_escape() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_focused(true);
        assert!(sb.is_focused());

        sb.handle_event(&Event::KeyPress { key: 27, modifiers: 0 }); // Escape
        assert!(!sb.is_focused());
    }

    #[test]
    fn search_box_disabled_blocks_events() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_enabled(false);

        sb.handle_event(&Event::MousePress { pos: Point::new(50, 16), button: 1 });
        assert!(!sb.is_focused());
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_disabled_blocks_keyboard() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_enabled(false);
        sb.set_focused(true);

        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 }); // 'a'
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_svg_output() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"200\""));
        assert!(svg.contains("height=\"32\""));
    }

    #[test]
    fn search_box_svg_with_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        sb.set_text("testing");
        let svg = crate::widget::svg::render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
    }

    #[test]
    fn search_box_clear_non_empty_only() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let captured = Arc::new(Mutex::new(0));
        let c = captured.clone();
        sb.text_changed.connect(move |_: Arc<String>| {
            *c.lock().unwrap() += 1;
        });

        // Clear when already empty — should not emit
        sb.clear();
        assert_eq!(*captured.lock().unwrap(), 0);

        // Set text and clear
        sb.set_text("data");
        assert_eq!(*captured.lock().unwrap(), 1);
        sb.clear();
        assert_eq!(*captured.lock().unwrap(), 2);
        assert_eq!(sb.text(), "");
    }

    #[test]
    fn search_box_no_duplicate_emit_on_set_same_text() {
        let mut sb = SearchBox::new(Rect::new(0, 0, 200, 32));
        let count = Arc::new(Mutex::new(0));
        let c = count.clone();
        sb.text_changed.connect(move |_: Arc<String>| {
            *c.lock().unwrap() += 1;
        });

        sb.set_text("hello");
        sb.set_text("hello"); // same text — should not emit
        assert_eq!(*count.lock().unwrap(), 1);
    }
}
