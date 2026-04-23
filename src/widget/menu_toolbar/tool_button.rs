//! Tool button widget.
use crate::core::{Alignment, Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::object::Object;
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
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
    checkable: bool,
    checked: bool,
    popup_mode: ToolButtonPopupMode,
    button_style: ToolButtonStyle,
    auto_raise: bool,
    arrow_type: Option<u8>, // 0=up 1=down 2=left 3=right 4=no
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
            base: BaseWidget::new(WidgetKind::ToolButton, geometry, &text),
            text,
            checkable: false,
            checked: false,
            popup_mode: ToolButtonPopupMode::DelayedPopup,
            button_style: ToolButtonStyle::IconOnly,
            auto_raise: false,
            arrow_type: None,
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
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, g: Rect) {
        self.base.set_geometry(g);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, s: Option<Size>) {
        self.base.set_min_size(s);
    }
    fn set_max_size(&mut self, s: Option<Size>) {
        self.base.set_max_size(s);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, p: Option<ObjectId>) {
        self.base.set_parent(p);
    }
    fn add_child(&mut self, c: ObjectId) {
        self.base.add_child(c);
    }
    fn remove_child(&mut self, c: ObjectId) {
        self.base.remove_child(c);
    }
    fn children(&self) -> &[ObjectId] {
        self.base.children()
    }
    fn show(&mut self) {
        self.base.show();
    }
    fn hide(&mut self) {
        self.base.hide();
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn set_enabled(&mut self, e: bool) {
        self.base.set_enabled(e);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, t: String) {
        self.base.set_tooltip(t);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, s: WidgetStyle) {
        self.base.set_style(s);
    }
    fn connection_scope(&self) -> &ConnectionScope {
        self.base.connection_scope()
    }
    fn hover_signal(&self) -> &Signal1<Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(Point, u32)> {
        self.base.mouse_up_signal()
    }
    fn key_down_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_down_signal()
    }
    fn key_up_signal(&self) -> &Signal1<(u32, u32)> {
        self.base.key_up_signal()
    }
    fn focus_gained_signal(&self) -> &GenericSignal {
        self.base.focus_gained_signal()
    }
    fn focus_lost_signal(&self) -> &GenericSignal {
        self.base.focus_lost_signal()
    }
    fn redraw_requested_signal(&self) -> &GenericSignal {
        self.base.redraw_requested_signal()
    }
    fn layout_requested_signal(&self) -> &GenericSignal {
        self.base.layout_requested_signal()
    }
}
impl EventHandler for ToolButton {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseEnter => {
                self.hovered = true;
            }
            Event::MouseLeave => {
                self.hovered = false;
                self.pressed = false;
            }
            Event::MousePress { button: 1, .. } => {
                self.pressed = true;
            }
            Event::MouseRelease { button: 1, .. } => {
                if self.pressed {
                    self.pressed = false;
                    self.click();
                }
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
                rect.x,
                rect.y,
                rect.width,
                rect.height,
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
            rect.x + rect.width as i32 - 12
        } else {
            rect.x + rect.width as i32
        };
        context.draw_text(
            rect.x + (text_right - rect.x) / 2,
            rect.y + rect.height as i32 / 2,
            label,
            &Font::default(),
            fg,
            Alignment::Center,
        );
        if has_popup {
            context.draw_text(
                rect.x + rect.width as i32 - 8,
                rect.y + rect.height as i32 - 6,
                "▾",
                &Font::default(),
                fg,
                Alignment::Center,
            );
        }
    }
}
