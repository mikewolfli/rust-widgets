// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Group box widget.
use crate::core::{Alignment, Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{
    alignment_to_str, expect_alignment, expect_bool, expect_string,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;
/// Group box widget.
pub struct GroupBox {
    base: BaseWidget,
    title: String,
    alignment: Alignment,
    checkable: bool,
    checked: bool,
    /// Emitted when the group box is toggled, with the new checked state.
    /// `checked` starts as `true` even when `checkable` is false; the signal is
    /// only emitted for a real state change.
    pub toggled: Signal1<bool>,
    /// Cached title width computed in draw() via RenderContext::measure_text().
    cached_title_width: Option<u32>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
}
impl GroupBox {
    /// Creates a group box.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::GroupBox, geometry, "GroupBox"),
            title: String::new(),
            alignment: Alignment::Left,
            checkable: false,
            checked: true,
            toggled: Signal1::new(),
            cached_title_width: None,
            registry: None,
        }
    }
    /// Returns title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Sets title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    /// Returns alignment.
    pub fn alignment(&self) -> Alignment {
        self.alignment
    }
    /// Sets alignment.
    pub fn set_alignment(&mut self, alignment: Alignment) {
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns whether group box is checkable.
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    /// Sets checkable state.
    pub fn set_checkable(&mut self, checkable: bool) {
        self.checkable = checkable;
    }
    /// Returns whether group box is checked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    /// Sets checked state.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.toggled.emit(checked);
    }
    /// Toggles checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }

    /// Sets the shared widget registry for child forwarding.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }
    /// Returns title rectangle.
    fn title_rect(&self) -> Rect {
        let rect = self.geometry();
        let text_width = self.cached_title_width.unwrap_or_else(|| {
            // Fallback approximate measurement if draw() hasn't run yet.
            self.title.len() as u32 * 8
        });
        let text_height = 16i32;
        let x = match self.alignment {
            Alignment::Left => rect.x + 10,
            Alignment::Center => rect.x + ((rect.width - text_width) / 2) as i32,
            Alignment::Right => rect.x + rect.width as i32 - text_width as i32 - 10,
            Alignment::Top | Alignment::Bottom => rect.x + 10,
        };
        Rect::new(x, rect.y - text_height / 2, text_width, text_height as u32)
    }
    /// Returns checkbox rectangle if checkable.
    fn checkbox_rect(&self) -> Option<Rect> {
        if !self.checkable {
            return None;
        }
        let title_rect = self.title_rect();
        let checkbox_size: i32 = 12;
        Some(Rect::new(
            title_rect.x - checkbox_size - 5,
            title_rect.y + (title_rect.height as i32 - checkbox_size) / 2,
            checkbox_size as u32,
            checkbox_size as u32,
        ))
    }

    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let top = 14u32;
        let inset = 4u32;
        Rect::new(
            rect.x + inset as i32,
            rect.y + top as i32,
            rect.width.saturating_sub(inset * 2),
            rect.height.saturating_sub(top + inset),
        )
    }
}
// Implement Widget trait
impl Widget for GroupBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(200, 150)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `GroupBox`'s property contract.
///
/// `Panel` is a type alias for this type (`pub type Panel = GroupBox`), so this
/// single impl serves both `WidgetKind::GroupBox` and every `WidgetKind::Panel`
/// value that is actually a group box. The alias is deliberately not given an
/// impl of its own — it would be a second, duplicate impl of the same type.
impl WidgetProperties for GroupBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "alignment" => {
                Ok(CapabilityValue::String(alignment_to_str(self.alignment()).to_string()))
            }
            "checkable" => Ok(CapabilityValue::Bool(self.is_checkable())),
            "checked" => Ok(CapabilityValue::Bool(self.is_checked())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_alignment(value)?);
                Ok(())
            }
            "checkable" => {
                self.set_checkable(expect_bool(value)?);
                Ok(())
            }
            "checked" => {
                self.set_checked(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "alignment", "checkable", "checked", BASE_PROPERTY_NAMES]
    }
}

impl EventHandler for GroupBox {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        // Handle checkbox toggle
        if self.checkable {
            if let Event::MousePress { pos, button } = event {
                if *button == 1 {
                    if let Some(checkbox_rect) = self.checkbox_rect() {
                        if checkbox_rect.contains(*pos) {
                            self.toggle();
                        }
                    }
                }
            }
        }
        let content = self.content_rect();
        if let Some(ref reg) = self.registry {
            let target = match event {
                Event::MousePress { pos, .. }
                | Event::MouseRelease { pos, .. }
                | Event::MouseMove { pos } => {
                    content.contains(*pos).then(|| self.base.children.first().copied()).flatten()
                }
                _ => self.base.children.first().copied(),
            };
            if let Some(child_id) = target {
                let _ = reg.borrow_mut().forward_event(child_id, event);
            }
        }
    }
}
impl Draw for GroupBox {
    fn draw(&mut self, context: &mut RenderContext) {
        // Cache actual title width from render context.
        if !self.title.is_empty() {
            let metrics = context.measure_text(&self.title, &Font::default());
            self.cached_title_width = Some(metrics.width);
        }
        // Draw base widget
        let rect = self.geometry();
        let content = self.content_rect();
        let title_rect = self.title_rect();
        let style = self.style();
        // Draw border
        context.draw_rect(rect, style.border_color.unwrap_or(Color::rgb(200, 200, 200)));
        // Draw title background to hide border
        let title_bg_width = title_rect.width + 20;
        let title_bg_x = title_rect.x - 10;
        context.fill_rect(
            Rect::new(title_bg_x, rect.y, title_bg_width, 2),
            style.background_color.unwrap_or(Color::rgb(255, 255, 255)),
        );
        // Draw checkbox if checkable
        if self.checkable {
            if let Some(checkbox_rect) = self.checkbox_rect() {
                // Draw checkbox border
                context.draw_rect(checkbox_rect, Color::rgb(100, 100, 100));
                // Draw checkmark if checked
                if self.checked {
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + 2.0,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 * 0.5,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        Color::rgb(0, 0, 0),
                    );
                    context.draw_line(
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 * 0.5,
                            checkbox_rect.y as f32 + checkbox_rect.height as f32 - 2.0,
                        ),
                        Point::from_f32(
                            checkbox_rect.x as f32 + checkbox_rect.width as f32 - 2.0,
                            checkbox_rect.y as f32 + 2.0,
                        ),
                        Color::rgb(0, 0, 0),
                    );
                }
            }
        }
        // Draw title text
        if !self.title.is_empty() {
            let text_color = if self.base.is_enabled() {
                style.text_color.unwrap_or(Color::rgb(0, 0, 0))
            } else {
                Color::rgb(150, 150, 150)
            };
            context.draw_text(
                Point::from_f32(title_rect.x as f32, title_rect.y as f32),
                &self.title,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );
        }
        if let Some(ref reg) = self.registry {
            context.push_clip(content.x, content.y, content.width, content.height);
            for child_id in &self.base.children {
                reg.borrow_mut().draw_widget(*child_id, context);
            }
            context.pop_clip();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ObjectId, Rect};

    #[test]
    fn groupbox_creation_defaults() {
        let gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        assert_eq!(gb.geometry(), Rect::new(0, 0, 200, 100));
        assert!(gb.title().is_empty());
        assert!(gb.is_checked());
        assert!(!gb.is_checkable());
    }

    #[test]
    fn groupbox_title_and_toggle() {
        let mut gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        gb.set_title("Options".to_string());
        assert_eq!(gb.title(), "Options");
        gb.set_checkable(true);
        assert!(gb.is_checkable());
        gb.set_checked(false);
        assert!(!gb.is_checked());
        gb.toggle();
        assert!(gb.is_checked());
    }

    // ── Panel / child management tests ─────────────────────────────────
    #[test]
    fn test_panel_add_remove_children() {
        let mut gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        let child1: ObjectId = 100;
        let child2: ObjectId = 200;
        let child3: ObjectId = 300;

        assert!(gb.children().is_empty());

        gb.add_child(child1);
        assert_eq!(gb.children().len(), 1);
        assert_eq!(gb.children()[0], child1);

        gb.add_child(child2);
        assert_eq!(gb.children().len(), 2);

        gb.add_child(child3);
        assert_eq!(gb.children().len(), 3);

        // Remove middle child
        gb.remove_child(child2);
        assert_eq!(gb.children().len(), 2);
        assert_eq!(gb.children()[0], child1);
        assert_eq!(gb.children()[1], child3);

        // Remove remaining children
        gb.remove_child(child1);
        assert_eq!(gb.children().len(), 1);

        gb.remove_child(child3);
        assert!(gb.children().is_empty());
    }

    #[test]
    fn test_panel_empty_has_no_children() {
        let gb = GroupBox::new(Rect::new(0, 0, 200, 100));
        assert!(gb.children().is_empty());
        assert_eq!(gb.children().len(), 0);
    }
}
