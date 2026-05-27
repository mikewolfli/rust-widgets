//! Tool button widget.
use crate::core::{Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use std::path::{Path, PathBuf};
/// Tool button popup mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolButtonPopupMode {
    DelayedPopup,
    MenuButtonPopup,
    InstantPopup,
}
/// Tool button style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolButtonStyle {
    IconOnly,
    TextOnly,
    TextBesideIcon,
    TextUnderIcon,
    FollowStyle,
}
/// Tool button widget.
pub struct ToolButton {
    base: BaseWidget,
    text: String,
    icon: Option<PathBuf>,
    checkable: bool,
    checked: bool,
    popup_mode: ToolButtonPopupMode,
    button_style: ToolButtonStyle,
    auto_raise: bool,
    pressed: bool,
    hovered: bool,
    pub clicked: Signal1<bool>,
    pub toggled: Signal1<bool>,
    pub triggered: GenericSignal,
}
impl ToolButton {
    pub fn new(text: impl Into<String>, geometry: Rect) -> Self {
        let text = text.into();
        Self {
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, "ToolButton"),
            text,
            icon: None,
            checkable: false,
            checked: false,
            popup_mode: ToolButtonPopupMode::DelayedPopup,
            button_style: ToolButtonStyle::IconOnly,
            auto_raise: false,
            pressed: false,
            hovered: false,
            clicked: Signal1::new(),
            toggled: Signal1::new(),
            triggered: GenericSignal::new(),
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn icon(&self) -> Option<&Path> {
        self.icon.as_deref()
    }
    pub fn is_checkable(&self) -> bool {
        self.checkable
    }
    pub fn is_checked(&self) -> bool {
        self.checked
    }
    pub fn popup_mode(&self) -> ToolButtonPopupMode {
        self.popup_mode
    }
    pub fn button_style(&self) -> ToolButtonStyle {
        self.button_style
    }
    pub fn auto_raise(&self) -> bool {
        self.auto_raise
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
    pub fn set_icon(&mut self, icon: Option<PathBuf>) {
        self.icon = icon;
    }
    pub fn set_checkable(&mut self, v: bool) {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
    }
    pub fn set_popup_mode(&mut self, mode: ToolButtonPopupMode) {
        self.popup_mode = mode;
    }
    pub fn set_button_style(&mut self, style: ToolButtonStyle) {
        self.button_style = style;
    }
    pub fn set_auto_raise(&mut self, v: bool) {
        self.auto_raise = v;
    }
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable && self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
        }
    }
    pub fn click(&mut self) {
        if self.checkable {
            self.set_checked(!self.checked);
        }
        self.clicked.emit(self.checked);
        self.triggered.emit();
    }
}
impl Widget for ToolButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}
impl EventHandler for ToolButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseEnter { pos: _ } => {
                self.hovered = true;
            }
            Event::MouseLeave { pos: _ } => {
                self.hovered = false;
                self.pressed = false;
            }
            Event::MousePress { button: 1, .. } => {
                self.pressed = true;
            }
            Event::MouseRelease { button: 1, .. } if self.pressed => {
                self.pressed = false;
                self.click();
            }
            Event::KeyPress { key: 13, .. } | Event::KeyPress { key: 32, .. } => {
                self.click();
            }
            _ => {}
        }
    }
}
impl Draw for ToolButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let bg = if self.pressed {
            Color::from_rgb(180, 210, 255)
        } else if self.checked {
            Color::from_rgb(200, 225, 255)
        } else if self.hovered && !self.auto_raise {
            Color::from_rgb(220, 238, 255)
        } else if self.auto_raise && !self.hovered {
            Color::from_rgba(0, 0, 0, 0) // transparent
        } else {
            Color::from_rgb(240, 240, 240)
        };
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        if self.hovered || self.pressed || self.checked {
            context.draw_rect(
                Rect::new(rect.x, rect.y, rect.width, rect.height),
                Color::from_rgb(0, 120, 215),
            );
        }
        let fg = if !self.base.is_enabled() {
            Color::from_rgb(150, 150, 150)
        } else {
            Color::from_rgb(0, 0, 0)
        };
        let label = match self.button_style {
            ToolButtonStyle::TextOnly
            | ToolButtonStyle::TextBesideIcon
            | ToolButtonStyle::TextUnderIcon
            | ToolButtonStyle::FollowStyle => &self.text,
            ToolButtonStyle::IconOnly => &self.text,
        };
        // Popup arrow indicator
        let has_popup = self.popup_mode == ToolButtonPopupMode::MenuButtonPopup
            || self.popup_mode == ToolButtonPopupMode::InstantPopup;
        let text_right = if has_popup {
            rect.x as f32 + rect.width as f32 - 12.0
        } else {
            rect.x as f32 + rect.width as f32
        };
        context.draw_text(
            Point::from_f32(
                rect.x as f32 + (text_right - rect.x as f32) / 2.0,
                rect.y as f32 + rect.height as f32 / 2.0,
            ),
            label,
            &Font::default(),
            fg,
        );
        if has_popup {
            context.draw_text(
                Point::from_f32(
                    rect.x as f32 + rect.width as f32 - 8.0,
                    rect.y as f32 + rect.height as f32 - 6.0,
                ),
                "▾",
                &Font::default(),
                fg,
            );
        }
    }
}
