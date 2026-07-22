//! Tool button widget.
use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
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
        self.base.request_redraw();
    }
    pub fn set_icon(&mut self, icon: Option<PathBuf>) {
        self.icon = icon;
        self.base.request_redraw();
    }
    pub fn set_checkable(&mut self, v: bool) {
        self.checkable = v;
        if !v {
            self.checked = false;
        }
        self.base.request_redraw();
    }
    pub fn set_popup_mode(&mut self, mode: ToolButtonPopupMode) {
        self.popup_mode = mode;
        self.base.request_redraw();
    }
    pub fn set_button_style(&mut self, style: ToolButtonStyle) {
        self.button_style = style;
        self.base.request_redraw();
    }
    pub fn set_auto_raise(&mut self, v: bool) {
        self.auto_raise = v;
        self.base.request_redraw();
    }
    pub fn set_checked(&mut self, checked: bool) {
        if self.checkable && self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
            self.base.request_redraw();
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

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(28, 28)
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
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl Draw for ToolButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let bg = if self.pressed {
            Color::rgb(180, 210, 255)
        } else if self.checked {
            Color::rgb(200, 225, 255)
        } else if self.hovered && !self.auto_raise {
            Color::rgb(220, 238, 255)
        } else if self.auto_raise && !self.hovered {
            Color::rgba(0, 0, 0, 0) // transparent
        } else {
            Color::rgb(240, 240, 240)
        };
        context.fill_rect(Rect::new(rect.x, rect.y, rect.width, rect.height), bg);
        if self.hovered || self.pressed || self.checked {
            context.draw_rect(
                Rect::new(rect.x, rect.y, rect.width, rect.height),
                Color::rgb(0, 120, 215),
            );
        }
        let fg =
            if !self.base.is_enabled() { Color::rgb(150, 150, 150) } else { Color::rgb(0, 0, 0) };
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
            HorizontalAlignment::Left,
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
                HorizontalAlignment::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolButton, ToolButtonPopupMode, ToolButtonStyle};
    use crate::core::{Color, Point, Rect, Size};
    use crate::event::Event;
    use crate::event::EventHandler;
    use crate::render::svg::SvgPaintBackend;
    use crate::render::PaintBackend;
    use crate::render::RenderContext;
    use crate::widget::{Draw, Widget, WidgetKind};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    fn rect() -> Rect {
        Rect::new(10, 20, 100, 30)
    }

    // ── 1. Creating default ToolButton ──
    #[test]
    fn tool_button_default_creation() {
        let btn = ToolButton::new("Open", rect());
        assert_eq!(btn.text(), "Open");
        assert!(btn.icon().is_none());
        assert!(!btn.is_checkable());
        assert!(!btn.is_checked());
        assert!(btn.is_enabled());
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);
        assert_eq!(btn.button_style(), ToolButtonStyle::IconOnly);
        assert!(!btn.auto_raise());
        assert_eq!(btn.geometry(), rect());
    }

    // ── 2. Setting/getting text ──
    #[test]
    fn tool_button_set_get_text() {
        let mut btn = ToolButton::new("Old", rect());
        btn.set_text("New Label");
        assert_eq!(btn.text(), "New Label");
    }

    // ── 3. Setting/getting icon ──
    #[test]
    fn tool_button_set_get_icon() {
        let mut btn = ToolButton::new("Open", rect());
        assert!(btn.icon().is_none());
        btn.set_icon(Some(PathBuf::from("icons/open.png")));
        assert_eq!(btn.icon(), Some(PathBuf::from("icons/open.png").as_path()));
        btn.set_icon(None);
        assert!(btn.icon().is_none());
    }

    // ── 4. Enabled/disabled state ──
    #[test]
    fn tool_button_enabled_disabled_state() {
        let mut btn = ToolButton::new("X", rect());
        assert!(btn.is_enabled());
        btn.set_enabled(false);
        assert!(!btn.is_enabled());
        btn.set_enabled(true);
        assert!(btn.is_enabled());
    }

    // ── 5. Button style configuration ──
    #[test]
    fn tool_button_button_style_configuration() {
        let mut btn = ToolButton::new("Btn", rect());
        assert_eq!(btn.button_style(), ToolButtonStyle::IconOnly);

        btn.set_button_style(ToolButtonStyle::TextOnly);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextOnly);

        btn.set_button_style(ToolButtonStyle::TextBesideIcon);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextBesideIcon);

        btn.set_button_style(ToolButtonStyle::TextUnderIcon);
        assert_eq!(btn.button_style(), ToolButtonStyle::TextUnderIcon);

        btn.set_button_style(ToolButtonStyle::FollowStyle);
        assert_eq!(btn.button_style(), ToolButtonStyle::FollowStyle);
    }

    // ── 6. Popup mode (for menu buttons) ──
    #[test]
    fn tool_button_popup_mode() {
        let mut btn = ToolButton::new("Menu", rect());
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);

        btn.set_popup_mode(ToolButtonPopupMode::InstantPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::InstantPopup);

        btn.set_popup_mode(ToolButtonPopupMode::MenuButtonPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::MenuButtonPopup);

        btn.set_popup_mode(ToolButtonPopupMode::DelayedPopup);
        assert_eq!(btn.popup_mode(), ToolButtonPopupMode::DelayedPopup);
    }

    // ── 7. Signal accessor (clicked) ──
    #[test]
    fn tool_button_clicked_signal() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked_count = Arc::new(AtomicUsize::new(0));
        let c = clicked_count.clone();
        btn.clicked.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        btn.click();
        assert_eq!(clicked_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn tool_button_toggled_signal() {
        let mut btn = ToolButton::new("Tog", rect());
        btn.set_checkable(true);
        let last = Arc::new(AtomicBool::new(false));
        let l = last.clone();
        btn.toggled.connect(move |v| {
            l.store(*v, Ordering::SeqCst);
        });
        btn.set_checked(true);
        assert!(last.load(Ordering::SeqCst));
    }

    #[test]
    fn tool_button_triggered_signal() {
        let mut btn = ToolButton::new("Trg", rect());
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        btn.triggered.connect(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });
        btn.click();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    // ── 8. Mouse click triggers signal ──
    #[test]
    fn tool_button_mouse_click_triggers_signal() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Press then release
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1 });
        btn.handle_event(&Event::MouseRelease { pos: Point::new(15, 25), button: 1 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    #[test]
    fn tool_button_keyboard_enter_triggers() {
        let mut btn = ToolButton::new("Btn", rect());
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Enter (key 13) and Space (key 32) trigger
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(clicked.load(Ordering::SeqCst));
    }

    // ── 9. Geometry delegation ──
    #[test]
    fn tool_button_geometry_delegation() {
        let mut btn = ToolButton::new("X", rect());
        assert_eq!(btn.geometry(), rect());
        let new_rect = Rect::new(0, 0, 200, 50);
        btn.set_geometry(new_rect);
        assert_eq!(btn.geometry(), new_rect);
    }

    // ── 10. Widget ID and kind ──
    #[test]
    fn tool_button_widget_id_and_kind() {
        let btn = ToolButton::new("A", rect());
        assert_eq!(btn.kind(), WidgetKind::ToolButton);
        let btn2 = ToolButton::new("B", rect());
        assert_ne!(btn.id(), btn2.id());
    }

    // ── 11. SVG output ──
    #[test]
    fn tool_button_svg_output() {
        let mut btn = ToolButton::new("Btn", rect());
        let mut svg = SvgPaintBackend::new(Size::new(200, 60));
        svg.begin_frame(Color::WHITE);
        let mut rc = RenderContext::new(&mut svg);
        btn.draw(&mut rc);
        svg.end_frame();
        let output = svg.finish();
        assert!(output.contains("<svg"), "SVG output should contain svg tag");
        // Should contain a rect (fill) for the background
        assert!(
            output.contains("rect") || output.contains("text"),
            "ToolButton SVG should contain visual elements"
        );
    }

    #[test]
    fn tool_button_svg_disabled_draw() {
        let mut btn = ToolButton::new("Disabled", rect());
        btn.set_enabled(false);
        let mut svg = SvgPaintBackend::new(Size::new(200, 60));
        svg.begin_frame(Color::WHITE);
        let mut rc = RenderContext::new(&mut svg);
        btn.draw(&mut rc);
        svg.end_frame();
        let output = svg.finish();
        assert!(output.contains("<svg"));
    }

    // ── 12. Disabled state blocks events ──
    #[test]
    fn tool_button_disabled_state_blocks_events() {
        let mut btn = ToolButton::new("X", rect());
        btn.set_enabled(false);
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Mouse press+release should be blocked
        btn.handle_event(&Event::MousePress { pos: Point::new(15, 25), button: 1 });
        btn.handle_event(&Event::MouseRelease { pos: Point::new(15, 25), button: 1 });
        assert!(
            !clicked.load(Ordering::SeqCst),
            "Disabled button should not emit clicked on mouse events"
        );
    }

    #[test]
    fn tool_button_disabled_blocks_keyboard() {
        let mut btn = ToolButton::new("X", rect());
        btn.set_enabled(false);
        let clicked = Arc::new(AtomicBool::new(false));
        let c = clicked.clone();
        btn.clicked.connect(move |_| {
            c.store(true, Ordering::SeqCst);
        });
        // Keyboard Enter should be blocked
        btn.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(
            !clicked.load(Ordering::SeqCst),
            "Disabled button should not emit clicked on key events"
        );
    }

    // ── 13. Checkable toggle ──
    #[test]
    fn tool_button_checkable_toggle() {
        let mut btn = ToolButton::new("Tog", rect());
        btn.set_checkable(true);
        assert!(!btn.is_checked());
        btn.click();
        assert!(btn.is_checked());
        btn.click();
        assert!(!btn.is_checked());
    }

    // ── 14. Non-checkable click does not change checked ──
    #[test]
    fn tool_button_non_checkable_ignores_checked() {
        let mut btn = ToolButton::new("X", rect());
        assert!(!btn.is_checkable());
        btn.click();
        // Non-checkable: click() calls set_checked which checks checkable
        // Since checkable is false, the click won't toggle
        assert!(!btn.is_checked(), "Non-checkable button should not toggle via click");
    }

    // ── 15. Auto raise ──
    #[test]
    fn tool_button_auto_raise() {
        let mut btn = ToolButton::new("X", rect());
        assert!(!btn.auto_raise());
        btn.set_auto_raise(true);
        assert!(btn.auto_raise());
        btn.set_auto_raise(false);
        assert!(!btn.auto_raise());
    }
}
