// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Popup window widget.
use crate::core::{ObjectId, Rect, Size};
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Popup window widget.
pub struct PopupWindow {
    base: BaseWidget,
    content_widget: Option<ObjectId>,
    /// Title shown in the popup's own chrome.
    ///
    /// Kept on the control rather than in a host-side map: the title is part of
    /// what the popup *is*, and a host that held it separately could not paint it.
    title: String,
    /// Emitted when the popup is opened.
    pub opened: GenericSignal,
    /// Emitted when the popup is closed.
    pub closed: GenericSignal,
}
impl PopupWindow {
    /// Creates a popup window with geometry and no title.
    pub fn new(geometry: Rect) -> Self {
        Self::with_title(String::new(), geometry)
    }

    /// Creates a popup window with a title and geometry.
    pub fn with_title(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::PopupWindow, geometry, "PopupWindow"),
            content_widget: None,
            title,
            opened: GenericSignal::new(),
            closed: GenericSignal::new(),
        }
    }

    /// Returns the popup's title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the popup's title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }

    /// Returns the content widget ID, if any.
    pub fn content_widget(&self) -> Option<ObjectId> {
        self.content_widget
    }

    /// Sets the content widget for this popup.
    pub fn set_content_widget(&mut self, widget: Option<ObjectId>) {
        if let Some(old) = self.content_widget {
            self.base.remove_child(old);
        }
        self.content_widget = widget;
        if let Some(id) = widget {
            self.base.add_child(id);
        }
        self.base.request_redraw();
    }

    /// Shows the popup and emits `opened`.
    pub fn open(&mut self) {
        self.show();
        self.opened.emit();
    }

    /// Hides the popup and emits `closed`.
    pub fn close(&mut self) {
        self.hide();
        self.closed.emit();
    }
}
impl Widget for PopupWindow {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 200)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `PopupWindow` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    /// Returns this widget as its property contract.
    fn properties_dyn(
        &self,
    ) -> Option<&dyn crate::widget::capability::properties_trait::WidgetProperties> {
        Some(self)
    }

    /// Mutable counterpart to `properties_dyn`.
    fn properties_dyn_mut(
        &mut self,
    ) -> Option<&mut dyn crate::widget::capability::properties_trait::WidgetProperties> {
        Some(self)
    }
}

/// `PopupWindow`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. `has_content` is read-only: a popup's
/// content is supplied by its owner, not through the property layer.
impl WidgetProperties for PopupWindow {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "has_content" => Ok(CapabilityValue::Bool(self.content_widget().is_some())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            // A popup's content is supplied by its owner via `set_content_widget`,
            // not through the property layer: the property reports presence only.
            "has_content" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["title", "has_content", BASE_PROPERTY_NAMES]
    }
}
impl Draw for PopupWindow {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        // Draw popup background with semi-transparent effect
        context.fill_rect(rect, Color::rgb(255, 255, 255));
        // Draw border
        context.draw_rect(rect, Color::rgb(120, 120, 120));
    }
}
impl crate::event::EventHandler for PopupWindow {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::Object;
    use std::sync::{Arc, Mutex};

    #[test]
    fn popup_open_close_emits_lifecycle_signals() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let opened = Arc::new(Mutex::new(0usize));
        let closed = Arc::new(Mutex::new(0usize));

        let opened_sink = opened.clone();
        popup.opened.connect(move || {
            if let Ok(mut count) = opened_sink.lock() {
                *count += 1;
            }
        });

        let closed_sink = closed.clone();
        popup.closed.connect(move || {
            if let Ok(mut count) = closed_sink.lock() {
                *count += 1;
            }
        });

        popup.open();
        popup.close();

        assert!(!popup.is_visible());
        assert_eq!(*opened.lock().expect("opened lock poisoned"), 1);
        assert_eq!(*closed.lock().expect("closed lock poisoned"), 1);
    }

    #[test]
    fn popup_replaces_content_widget_child_binding() {
        let mut popup = PopupWindow::new(Rect::new(0, 0, 120, 80));
        let old_id = Object::new("OldContent").id();
        let new_id = Object::new("NewContent").id();

        popup.set_content_widget(Some(old_id));
        assert_eq!(popup.content_widget(), Some(old_id));
        assert_eq!(popup.children(), &[old_id]);

        popup.set_content_widget(Some(new_id));
        assert_eq!(popup.content_widget(), Some(new_id));
        assert_eq!(popup.children(), &[new_id]);
    }
}
