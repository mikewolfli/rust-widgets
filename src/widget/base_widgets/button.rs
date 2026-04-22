//! Button widget implementation.

use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Button interaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Pressed,
    Disabled,
}

/// Button widget for clickable actions.
pub struct Button {
    base: BaseWidget,
    text: String,
    pressed: bool,
    pub activated: GenericSignal,
    pub pressed_signal: GenericSignal,
    pub released_signal: GenericSignal,
    pub state_changed: Signal1<ButtonState>,
}

impl Button {
    /// Creates a button with initial text and geometry.
    pub fn new(text: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Button, geometry, "Button"),
            text,
            pressed: false,
            activated: GenericSignal::new(),
            pressed_signal: GenericSignal::new(),
            released_signal: GenericSignal::new(),
            state_changed: Signal1::new(),
        }
    }

    /// Returns button text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns current button interaction state.
    pub fn state(&self) -> ButtonState {
        if !self.base.is_enabled() {
            ButtonState::Disabled
        } else if self.pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Normal
        }
    }

    /// Returns whether button is in pressed state.
    pub fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Sets pressed state and emits transition signals when changed.
    pub fn set_pressed(&mut self, pressed: bool) {
        if !self.base.is_enabled() {
            return;
        }
        if self.pressed == pressed {
            return;
        }

        self.pressed = pressed;
        if pressed {
            self.pressed_signal.emit();
        } else {
            self.released_signal.emit();
        }
        self.state_changed.emit(self.state());
    }

    pub fn press(&mut self) {
        self.set_pressed(true);
    }

    pub fn release(&mut self) {
        self.set_pressed(false);
    }

    /// Enables/disables button while preserving deterministic state transitions.
    pub fn set_enabled_state(&mut self, enabled: bool) {
        let previous = self.state();
        self.base.set_enabled(enabled);
        if !enabled {
            self.pressed = false;
        }
        let current = self.state();
        if previous != current {
            self.state_changed.emit(current);
        }
    }

    /// Sets button text.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.base.request_redraw();
    }
}

impl Widget for Button {
    fn id(&self) -> ObjectId {
        self.base.id()
    }
    fn kind(&self) -> WidgetKind {
        self.base.kind()
    }
    fn geometry(&self) -> Rect {
        self.base.geometry()
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
    }
    fn min_size(&self) -> Option<Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<Size>) {
        self.base.set_max_size(max_size);
    }
    fn parent(&self) -> Option<ObjectId> {
        self.base.parent()
    }
    fn set_parent(&mut self, parent: Option<ObjectId>) {
        self.base.set_parent(parent);
    }
    fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    fn remove_child(&mut self, child: ObjectId) {
        self.base.remove_child(child);
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
    fn set_enabled(&mut self, enabled: bool) {
        self.set_enabled_state(enabled);
    }
    fn is_enabled(&self) -> bool {
        self.base.is_enabled()
    }
    fn set_tooltip(&mut self, tooltip: String) {
        self.base.set_tooltip(tooltip);
    }
    fn tooltip(&self) -> &str {
        self.base.tooltip()
    }
    fn style(&self) -> &WidgetStyle {
        self.base.style()
    }
    fn set_style(&mut self, style: WidgetStyle) {
        self.base.set_style(style);
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

impl EventHandler for Button {
    fn handle_event(&mut self, event: &Event) -> bool {
        let handled = self.base.handle_event(event);

        match event {
            Event::MouseDown {
                position: _,
                button: _,
                ..
            } => {
                if self.base.is_enabled() {
                    self.press();
                    return true;
                }
            }
            Event::MouseUp {
                position: _,
                button: _,
                ..
            } => {
                if self.pressed {
                    self.release();
                    self.activated.emit();
                    return true;
                }
            }
            _ => {}
        }

        handled
    }
}

impl Draw for Button {
    fn draw(&mut self, context: &mut RenderContext) {
        // Button rendering logic will be implemented in the render module
        // For now, just draw a simple rectangle
        let rect = self.geometry();
        let state = self.state();

        match state {
            ButtonState::Normal => {
                context.fill_rect(rect, Color::from_rgb(240, 240, 240));
                context.draw_rect(rect, Color::from_rgb(200, 200, 200), 1);
            }
            ButtonState::Pressed => {
                context.fill_rect(rect, Color::from_rgb(200, 200, 200));
                context.draw_rect(rect, Color::from_rgb(150, 150, 150), 1);
            }
            ButtonState::Disabled => {
                context.fill_rect(rect, Color::from_rgb(220, 220, 220));
                context.draw_rect(rect, Color::from_rgb(180, 180, 180), 1);
            }
        }

        // Draw text
        if !self.text.is_empty() {
            let text_color = if state == ButtonState::Disabled {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 0, 0)
            };

            context.draw_text(rect, &self.text, text_color);
        }
    }
}
