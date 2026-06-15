//! Toggle button widget.
use crate::core::HorizontalAlignment;
use crate::core::Rect;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Toggle button state enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleButtonState {
    Normal,
    Checked,
    Disabled,
}
pub struct ToggleButton {
    base: BaseWidget,
    text: String,
    checked: bool,
    auto_exclusive: bool,
    group_id: Option<String>,
    pressed: bool,
    pub toggled: Signal1<bool>,
    pub checked_changed: Signal1<bool>,
    pub pressed_signal: GenericSignal,
    pub released_signal: GenericSignal,
    pub state_changed: Signal1<ToggleButtonState>,
}
impl ToggleButton {
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ToggleButton, geometry, "ToggleButton"),
            text,
            checked: false,
            auto_exclusive: false,
            group_id: None,
            pressed: false,
            toggled: Signal1::new(),
            checked_changed: Signal1::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text != text {
            self.text = text;
            self.base.request_redraw();
        }
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.base.request_redraw();
        self.checked_changed.emit(checked);
        self.toggled.emit(checked);
        self.state_changed.emit(self.state());
    }
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }
    pub fn is_auto_exclusive(&self) -> bool {
        self.auto_exclusive
    }
    pub fn set_auto_exclusive(&mut self, exclusive: bool) {
        self.auto_exclusive = exclusive;
    }
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }
    pub fn set_group_id(&mut self, group_id: Option<String>) {
        self.group_id = group_id;
    }
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }
    pub fn set_pressed(&mut self, pressed: bool) {
        if self.pressed == pressed {
            return;
        }
        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
    }
    pub fn state(&self) -> ToggleButtonState {
        if !self.base.enabled {
            ToggleButtonState::Disabled
        } else if self.checked {
            ToggleButtonState::Checked
        } else {
            ToggleButtonState::Normal
        }
    }
}
impl Widget for ToggleButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl Draw for ToggleButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        let state = self.state();
        let style = self.style();
        use crate::core::Color;

        // ── Background ──
        let bg_color = style.background_color.unwrap_or_else(|| match state {
            ToggleButtonState::Disabled => Color::rgb(220, 220, 220),
            ToggleButtonState::Checked => Color::rgb(200, 220, 255),
            ToggleButtonState::Normal => Color::rgb(240, 240, 240),
        });
        context.fill_rect(rect, bg_color);

        // ── Border ──
        let border_color = style.border_color.unwrap_or_else(|| {
            if self.checked {
                Color::rgb(80, 120, 200)
            } else {
                Color::rgb(180, 180, 180)
            }
        });
        let bw = style.border_width.unwrap_or(0);
        let border_width = if bw > 0 { bw } else { 1 };
        context.draw_rect_stroke(rect, border_color, border_width);

        // ── Text ──
        if !self.text.is_empty() {
            let text_color = style.text_color.unwrap_or_else(|| {
                if state == ToggleButtonState::Disabled {
                    Color::rgb(150, 150, 150)
                } else {
                    Color::rgb(0, 0, 0)
                }
            });
            let font = style.font.clone().unwrap_or_default();
            context.draw_text(
                crate::core::Point::new(
                    rect.x + rect.width as i32 / 2,
                    rect.y + rect.height as i32 / 2,
                ),
                &self.text,
                &font,
                text_color,
                HorizontalAlignment::Center,
            );
        }
    }
}
impl crate::event::EventHandler for ToggleButton {
    fn handle_event(&mut self, event: &crate::event::Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.set_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                if self.pressed {
                    self.toggle();
                }
                self.set_pressed(false);
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, ObjectId, Rect};
    use crate::style::WidgetStyle;

    #[test]
    fn toggle_creation_defaults() {
        let tb = ToggleButton::new("Toggle".to_string(), Rect::new(0, 0, 100, 30));
        assert!(!tb.is_checked());
        assert_eq!(tb.text(), "Toggle");
        assert!(!tb.is_auto_exclusive());
        assert!(tb.group_id().is_none());
        assert!(!tb.is_pressed());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_set_checked() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        tb.set_checked(true);
        assert!(tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Checked);
        tb.set_checked(false);
        assert!(!tb.is_checked());
        assert_eq!(tb.state(), ToggleButtonState::Normal);
    }

    #[test]
    fn toggle_toggle_method() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_checked());
        tb.toggle();
        assert!(tb.is_checked());
        tb.toggle();
        assert!(!tb.is_checked());
    }

    #[test]
    fn toggle_set_text() {
        let mut tb = ToggleButton::new("Old".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(tb.text(), "Old");
        tb.set_text("New".to_string());
        assert_eq!(tb.text(), "New");
    }

    #[test]
    fn toggle_auto_exclusive() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_auto_exclusive());
        tb.set_auto_exclusive(true);
        assert!(tb.is_auto_exclusive());
        tb.set_auto_exclusive(false);
        assert!(!tb.is_auto_exclusive());
    }

    #[test]
    fn toggle_group_id() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(tb.group_id().is_none());
        tb.set_group_id(Some("group1".to_string()));
        assert_eq!(tb.group_id(), Some("group1"));
        tb.set_group_id(None);
        assert!(tb.group_id().is_none());
    }

    #[test]
    fn toggle_pressed_state() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        assert!(!tb.is_pressed());
        tb.set_pressed(true);
        assert!(tb.is_pressed());
        tb.set_pressed(false);
        assert!(!tb.is_pressed());
    }

    #[test]
    fn toggle_geometry_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        tb.set_geometry(Rect::new(10, 10, 200, 50));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 200, 50));
    }

    #[test]
    fn toggle_visibility_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn toggle_enabled_delegation() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.is_enabled());
        tb.set_enabled(false);
        assert!(!tb.is_enabled());
        assert_eq!(tb.state(), ToggleButtonState::Disabled);
        tb.set_enabled(true);
        assert!(tb.is_enabled());
    }

    #[test]
    fn toggle_parent_children() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.parent().is_none());
        let pid: ObjectId = 42;
        tb.set_parent(Some(pid));
        assert_eq!(tb.parent(), Some(pid));
        tb.set_parent(None);
        assert!(tb.parent().is_none());

        let cid: ObjectId = 100;
        tb.add_child(cid);
        assert_eq!(tb.children().len(), 1);
        assert_eq!(tb.children()[0], cid);
        tb.remove_child(cid);
        assert!(tb.children().is_empty());
    }

    #[test]
    fn toggle_tooltip_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert!(tb.tooltip().is_empty());
        tb.set_tooltip("Helpful tip".to_string());
        assert_eq!(tb.tooltip(), "Helpful tip");
        tb.set_tooltip(String::new());
        assert!(tb.tooltip().is_empty());
    }

    #[test]
    fn toggle_style_roundtrip() {
        let mut tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 100, 30));
        assert_eq!(*tb.style(), WidgetStyle::default());
        let custom = WidgetStyle::default().with_background(Color::rgb(200, 200, 200));
        tb.set_style(custom.clone());
        assert_eq!(*tb.style(), custom);
    }

    #[test]
    fn toggle_id_kind() {
        let tb_a = ToggleButton::new("A".to_string(), Rect::new(0, 0, 50, 30));
        let tb_b = ToggleButton::new("B".to_string(), Rect::new(0, 0, 50, 30));
        assert_ne!(tb_a.id(), tb_b.id());
        assert_eq!(tb_a.kind(), WidgetKind::ToggleButton);
        assert_eq!(tb_b.kind(), WidgetKind::ToggleButton);
    }

    #[test]
    fn toggle_signal_accessors() {
        let tb = ToggleButton::new("T".to_string(), Rect::new(0, 0, 50, 30));
        // Signal1<bool>
        let _toggled = &tb.toggled;
        let _checked = &tb.checked_changed;
        let _state = &tb.state_changed;
        // GenericSignal
        let _pressed = &tb.pressed_signal;
        let _released = &tb.released_signal;
    }
}
