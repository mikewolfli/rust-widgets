// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SearchBar widget — iOS-style search bar with search icon, clear button,
//! and optional cancel button when active.
//!
//! Unlike the existing SearchBox, this widget is designed for mobile-style
//! search UX with active state management and a cancel button.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{TextSnapshotCommand, UndoStack};
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// iOS-style search bar widget.
///
/// Displays a rounded search field with a search icon on the left, optional
/// clear button when text is present, and a cancel button on the right when
/// the search bar is active.
pub struct SearchBar {
    base: BaseWidget,
    text: String,
    placeholder: String,
    is_active: bool,
    cancel_button_visible: bool,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when the user submits a search (Enter key).
    pub search_submitted: Signal1<String>,
    /// Emitted when the user taps the cancel button.
    pub canceled: GenericSignal,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
}

impl SearchBar {
    /// Creates a new SearchBar widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SearchBar, geometry, "SearchBar"),
            text: String::new(),
            placeholder: "Search".to_string(),
            is_active: false,
            cancel_button_visible: true,
            text_changed: Signal1::new(),
            search_submitted: Signal1::new(),
            canceled: GenericSignal::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
        }
    }

    /// Returns the current search text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the search text. Emits `text_changed` signal.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let new_text = text.into();
        if self.text != new_text {
            let before = self.text.clone();
            self.text = new_text.clone();
            if !self.restoring_history {
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(TextSnapshotCommand::new(
                    self.history_target.clone(),
                    before,
                    self.text.clone(),
                    "search_bar_text",
                )));
            }
            self.text_changed.emit(new_text);
            self.base.request_redraw();
        }
    }

    /// Returns the placeholder text.
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the placeholder text.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
        self.base.request_redraw();
    }

    /// Sets whether the search bar is active (showing cancel button).
    pub fn set_active(&mut self, active: bool) {
        if self.is_active != active {
            self.is_active = active;
            self.base.request_redraw();
        }
    }

    /// Returns whether the search bar is active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Sets whether the cancel button is visible when active.
    pub fn set_cancel_button_visible(&mut self, visible: bool) {
        self.cancel_button_visible = visible;
        self.base.request_redraw();
    }

    /// Returns whether the cancel button is visible when active.
    pub fn cancel_button_visible(&self) -> bool {
        self.cancel_button_visible
    }

    /// Clears the search text. Emits `text_changed` signal.
    pub fn clear(&mut self) {
        if !self.text.is_empty() {
            self.set_text(String::new());
        }
    }

    /// Reverts the most recent edit in this search bar.
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

    /// Submits the current search text.
    fn submit(&mut self) {
        let text = self.text.clone();
        if !text.is_empty() {
            self.search_submitted.emit(text);
        }
    }
}

impl Widget for SearchBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 36)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SearchBar`'s property contract.
impl WidgetProperties for SearchBar {
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
                self.set_placeholder(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `SEARCH_BAR_PROPERTIES`.
        property_names_of!["text", "placeholder", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `search_bar` publishes.
    ///
    /// Both names assign state and need a payload, so both are answered through the
    /// property route. The capability publishes no zero-argument action for this
    /// control, which is why there is no `Ok(())` arm here.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" | "set_placeholder" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for SearchBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        // Resolved once for the whole draw: the field, the icon and the cancel label all read
        // it, and re-resolving would take the theme lock once per read. Precedence is the
        // crate-wide one — an explicit style wins, then the theme, then the literal.
        let style = self.style().clone();
        let themed = crate::style::resolved_theme_style("search_bar");
        let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
        let accent = style
            .border_color
            .or_else(|| themed.as_ref().and_then(|resolved| resolved.border_color));
        let cancel_width: u32 = if self.is_active && self.cancel_button_visible { 60 } else { 0 };
        let field_width = rect.width.saturating_sub(cancel_width + 4);

        // Cancel button area
        let cancel_x = rect.x + rect.width as i32 - cancel_width as i32;
        if self.is_active && self.cancel_button_visible {
            let _cancel_rect = Rect::new(cancel_x, rect.y, cancel_width, rect.height);
            let cancel_font = Font::simple("sans-serif", 14.0);
            let cancel_text = "Cancel";
            let metrics = context.measure_text(cancel_text, &cancel_font);
            let text_x = cancel_x + (cancel_width as i32 - metrics.width as i32) / 2;
            let text_y =
                rect.y + (rect.height as i32 - metrics.height as i32) / 2 + metrics.ascent as i32;
            context.draw_text(
                Point::new(text_x, text_y),
                cancel_text,
                &cancel_font,
                // The cancel label is the control's accent — an actionable affordance — so it
                // follows the resolved border colour rather than a fixed blue.
                accent.unwrap_or(Color::rgba(52, 120, 246, 255)),
                HorizontalAlignment::Left,
            );
        }

        // Search field background
        //
        // Resolved from the style so a theme switch reaches this control; the two states are
        // *derived* from the resolved colour, so disabled reads as the same field with the
        // energy taken out and the control follows the theme.
        let field_rect = Rect::new(rect.x, rect.y, field_width, rect.height);
        let base_field =
            style.background_color.or(themed_bg).unwrap_or(Color::rgba(200, 200, 205, 200));
        let field_color =
            if !is_enabled { base_field.blend(&Color::WHITE, 0.35) } else { base_field };
        let corner_radius = rect.height / 2;
        context.fill_rounded_rect(field_rect, corner_radius, field_color);

        // Search icon (magnifying glass)
        let icon_size: u32 = 14;
        let icon_left = rect.x + 10;
        let icon_top = rect.y + (rect.height as i32 - icon_size as i32) / 2;
        // The magnifier is a glyph, so it reads the control's text colour.
        let icon_color = style.text_color.unwrap_or(Color::rgba(140, 140, 140, 255));
        context.draw_circle_stroke(
            Point::new(icon_left + (icon_size / 2) as i32, icon_top + (icon_size / 2) as i32),
            icon_size / 2,
            icon_color,
            2,
        );
        // Search icon handle (diagonal line from bottom-right of circle)
        let handle_offset = icon_size as i32;
        context.draw_line(
            Point::new(icon_left + handle_offset - 2, icon_top + handle_offset - 2),
            Point::new(icon_left + handle_offset + 3, icon_top + handle_offset + 3),
            icon_color,
        );

        // Text or placeholder
        let text_left = icon_left + icon_size as i32 + 6;
        let text_width = if self.text.is_empty() {
            field_width.saturating_sub((text_left - rect.x) as u32)
        } else {
            field_width.saturating_sub((text_left - rect.x) as u32 + 24)
        };
        let font = Font::simple("sans-serif", 14.0);

        if self.text.is_empty() {
            // Draw placeholder text
            let metrics = context.measure_text(&self.placeholder, &font);
            if text_width >= metrics.width {
                let text_y = rect.y
                    + (rect.height as i32 - metrics.height as i32) / 2
                    + metrics.ascent as i32;
                context.draw_text(
                    Point::new(text_left, text_y),
                    &self.placeholder,
                    &font,
                    Color::rgba(160, 160, 160, 255),
                    HorizontalAlignment::Left,
                );
            }
        } else {
            // Draw text
            let metrics = context.measure_text(&self.text, &font);
            if text_width >= metrics.width {
                let text_y = rect.y
                    + (rect.height as i32 - metrics.height as i32) / 2
                    + metrics.ascent as i32;
                context.draw_text(
                    Point::new(text_left, text_y),
                    &self.text,
                    &font,
                    Color::rgba(40, 40, 40, 255),
                    HorizontalAlignment::Left,
                );
            }

            // Draw clear button (X)
            let clear_x = rect.x + field_width as i32 - 22;
            let clear_y = rect.y + (rect.height as i32 - 16) / 2;
            let clear_center = Point::new(clear_x + 8, clear_y + 8);
            context.fill_circle(clear_center, 8, Color::rgba(180, 180, 180, 200));
            // X lines
            context.draw_line(
                Point::new(clear_center.x - 3, clear_center.y - 3),
                Point::new(clear_center.x + 3, clear_center.y + 3),
                Color::WHITE,
            );
            context.draw_line(
                Point::new(clear_center.x + 3, clear_center.y - 3),
                Point::new(clear_center.x - 3, clear_center.y + 3),
                Color::WHITE,
            );
        }
    }
}

impl EventHandler for SearchBar {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    let cancel_width: u32 =
                        if self.is_active && self.cancel_button_visible { 60 } else { 0 };
                    let cancel_x = rect.x + rect.width as i32 - cancel_width as i32;

                    // Cancel button click
                    if self.is_active && self.cancel_button_visible && pos.x >= cancel_x {
                        self.is_active = false;
                        self.canceled.emit();
                        self.base.request_redraw();
                        return;
                    }

                    // Clear button click (when text is non-empty)
                    let field_width = rect.width.saturating_sub(cancel_width + 4);
                    let clear_x = rect.x + field_width as i32 - 22;
                    if !self.text.is_empty()
                        && pos.x >= clear_x
                        && pos.x < clear_x + 16
                        && pos.y >= rect.y
                        && pos.y < rect.y + rect.height as i32
                    {
                        self.clear();
                        return;
                    }

                    // Focus the search bar
                    if !self.is_active {
                        self.set_active(true);
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                if *key == 90 && *modifiers == 2 {
                    let _ = self.undo();
                    return;
                }
                if *key == 89 && *modifiers == 2 {
                    let _ = self.redo();
                    return;
                }
                if *key == 13 || *key == 10 {
                    // Enter key — submit search
                    self.submit();
                } else if *key == 8 {
                    // Backspace — remove last character
                    if !self.text.is_empty() {
                        let mut chars: Vec<char> = self.text.chars().collect();
                        chars.pop();
                        self.set_text(chars.into_iter().collect::<String>());
                    }
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append character
                    if let Some(c) = char::from_u32(*key) {
                        let mut new_text = self.text.clone();
                        new_text.push(c);
                        self.set_text(new_text);
                    }
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                if !text.is_empty() {
                    let mut new_text = self.text.clone();
                    new_text.push_str(text);
                    self.set_text(new_text);
                }
            }
            Event::ImePreedit { .. } => {}
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    fn make_search_bar() -> SearchBar {
        SearchBar::new(Rect::new(0, 0, 300, 40))
    }

    #[test]
    fn search_bar_default_state() {
        let sb = make_search_bar();
        assert_eq!(sb.text(), "");
        assert_eq!(sb.placeholder(), "Search");
        assert!(!sb.is_active());
        assert!(sb.cancel_button_visible());
        assert_eq!(sb.kind(), WidgetKind::SearchBar);
    }

    #[test]
    fn search_bar_set_text() {
        let mut sb = make_search_bar();
        sb.set_text("hello");
        assert_eq!(sb.text(), "hello");
    }

    #[test]
    fn search_bar_text_changed_signal() {
        let mut sb = make_search_bar();
        let captured = Arc::new(Mutex::new(None));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        sb.set_text("test");
        assert_eq!(captured.lock().unwrap().as_deref(), Some("test"));
    }

    #[test]
    fn search_bar_submit_signal() {
        let mut sb = make_search_bar();
        sb.set_text("query");
        let captured = Arc::new(Mutex::new(None));
        sb.search_submitted.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        // Simulate Enter key press
        sb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(captured.lock().unwrap().as_deref(), Some("query"));
    }

    #[test]
    fn search_bar_clear_button_and_signal() {
        let mut sb = make_search_bar();
        sb.set_text("hello");
        let captured = Arc::new(Mutex::new(None));
        sb.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some((*val).clone());
            }
        });

        sb.clear();
        assert_eq!(sb.text(), "");
        assert_eq!(captured.lock().unwrap().as_deref(), Some(""));
    }

    #[test]
    fn search_bar_cancel_signal() {
        let mut sb = make_search_bar();
        sb.set_active(true);
        let captured = Arc::new(AtomicBool::new(false));
        sb.canceled.connect({
            let captured = Arc::clone(&captured);
            move || {
                captured.store(true, Ordering::SeqCst);
            }
        });

        // Click cancel button (at x=240+, since width=300, cancel starts at 300-60=240)
        sb.handle_event(&Event::MousePress { pos: Point::new(260, 20), button: 1 });
        assert!(!sb.is_active());
        assert!(captured.load(Ordering::SeqCst));
    }

    #[test]
    fn search_bar_set_placeholder() {
        let mut sb = make_search_bar();
        sb.set_placeholder("Find...");
        assert_eq!(sb.placeholder(), "Find...");
    }

    #[test]
    fn search_bar_svg_output() {
        let mut sb = make_search_bar();
        sb.set_text("foo");
        let svg = render_to_svg(&mut sb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn search_bar_undo_redo_restores_text() {
        let mut sb = make_search_bar();
        sb.set_text("alpha");
        sb.set_text("beta");

        assert!(sb.can_undo());
        assert!(sb.undo());
        assert_eq!(sb.text(), "alpha");
        assert!(sb.can_redo());
        assert!(sb.redo());
        assert_eq!(sb.text(), "beta");
    }

    #[test]
    fn search_bar_key_shortcuts_drive_history() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::KeyPress { key: 97, modifiers: 0 });
        sb.handle_event(&Event::KeyPress { key: 98, modifiers: 0 });
        assert_eq!(sb.text(), "ab");

        sb.handle_event(&Event::KeyPress { key: 90, modifiers: 2 });
        assert_eq!(sb.text(), "a");
        sb.handle_event(&Event::KeyPress { key: 89, modifiers: 2 });
        assert_eq!(sb.text(), "ab");
    }

    #[test]
    fn search_bar_accepts_unicode_text_input_events() {
        let mut sb = make_search_bar();
        sb.handle_event(&Event::text_input("你好"));
        sb.handle_event(&Event::ime_commit("世界"));

        assert_eq!(sb.text(), "你好世界");
        assert!(sb.undo());
        assert_eq!(sb.text(), "你好");
    }
}
