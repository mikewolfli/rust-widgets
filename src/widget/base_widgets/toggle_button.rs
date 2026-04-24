//! Toggle button widget.
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
    pub fn set_text(&mut self, text: String) {
        if self.text != text {
            self.text = text;
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
        use crate::core::Color;
        // Draw background based on state
        let bg_color = if !self.base.is_enabled() {
            Color::from_rgb(220, 220, 220)
        } else if self.checked {
            Color::from_rgb(200, 220, 255)
        } else {
            Color::from_rgb(240, 240, 240)
        };
        context.fill_rect(rect, bg_color);
        // Draw border
        let border_color = if self.checked {
            Color::from_rgb(80, 120, 200)
        } else {
            Color::from_rgb(180, 180, 180)
        };
        context.draw_rect(rect, border_color);
        // Draw text centered
        let text_color = if !self.base.is_enabled() {
            Color::from_rgb(150, 150, 150)
        } else {
            Color::from_rgb(0, 0, 0)
        };
        context.draw_text(
            crate::core::Point::new(
                rect.x + rect.width as i32 / 2,
                rect.y + rect.height as i32 / 2,
            ),
            &self.text,
            &crate::core::Font::default(),
            text_color,
        );
    }
}
impl crate::event::EventHandler for ToggleButton {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } => {
                if *button == 1 {
                    self.set_pressed(true);
                }
            }
            crate::event::Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    if self.pressed {
                        self.toggle();
                    }
                    self.set_pressed(false);
                }
            }
            _ => {}
        }
    }
}
