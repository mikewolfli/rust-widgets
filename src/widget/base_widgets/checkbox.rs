//! Checkbox widget implementation.

use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Checkbox state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    PartiallyChecked,
    Checked,
}

/// Checkbox widget for boolean or tristate selection.
pub struct CheckBox {
    base: BaseWidget,
    state: CheckState,
    tristate_enabled: bool,
    pub toggled: Signal1<bool>,
    pub state_changed: Signal1<CheckState>,
}

impl CheckBox {
    /// Creates an unchecked checkbox with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::CheckBox, geometry, "CheckBox"),
            state: CheckState::Unchecked,
            tristate_enabled: false,
            toggled: Signal1::new(),
            state_changed: Signal1::new(),
        }
    }

    /// Returns current check state.
    pub fn state(&self) -> CheckState {
        self.state
    }

    /// Returns true when the checkbox is fully checked.
    pub fn is_checked(&self) -> bool {
        self.state == CheckState::Checked
    }

    /// Returns true when the checkbox is partially checked (tristate).
    pub fn is_partially_checked(&self) -> bool {
        self.state == CheckState::PartiallyChecked
    }

    /// Returns true when tristate behavior is enabled.
    pub fn is_tristate_enabled(&self) -> bool {
        self.tristate_enabled
    }

    /// Sets check state and emits signals when changed.
    pub fn set_state(&mut self, state: CheckState) {
        if self.state == state {
            return;
        }

        let previous = self.state;
        self.state = state;
        self.state_changed.emit(state);

        // Emit toggled signal for boolean transitions
        match (previous, state) {
            (CheckState::Unchecked, CheckState::Checked) => self.toggled.emit(true),
            (CheckState::Checked, CheckState::Unchecked) => self.toggled.emit(false),
            _ => {}
        }

        self.base.request_redraw();
    }

    /// Sets checked state (true = checked, false = unchecked).
    pub fn set_checked(&mut self, checked: bool) {
        self.set_state(if checked {
            CheckState::Checked
        } else {
            CheckState::Unchecked
        });
    }

    /// Enables or disables tristate behavior.
    pub fn set_tristate_enabled(&mut self, enabled: bool) {
        self.tristate_enabled = enabled;
        if !enabled && self.state == CheckState::PartiallyChecked {
            self.set_state(CheckState::Unchecked);
        }
    }

    /// Toggles between checked states.
    pub fn toggle(&mut self) {
        let next_state = match self.state {
            CheckState::Unchecked => CheckState::Checked,
            CheckState::Checked => {
                if self.tristate_enabled {
                    CheckState::PartiallyChecked
                } else {
                    CheckState::Unchecked
                }
            }
            CheckState::PartiallyChecked => CheckState::Unchecked,
        };
        self.set_state(next_state);
    }
}

impl Widget for CheckBox {
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
        self.base.set_enabled(enabled);
        self.base.request_redraw();
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

impl EventHandler for CheckBox {
    fn handle_event(&mut self, event: &Event) -> bool {
        let handled = self.base.handle_event(event);

        match event {
            Event::MouseDown {
                position: _,
                button: _,
                ..
            } => {
                if self.base.is_enabled() {
                    self.toggle();
                    return true;
                }
            }
            Event::KeyDown {
                key, modifiers: _, ..
            } => {
                // Space key toggles checkbox
                if *key == 32 && self.base.is_enabled() {
                    // Space key
                    self.toggle();
                    return true;
                }
            }
            _ => {}
        }

        handled
    }
}

impl Draw for CheckBox {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let checkbox_size = 16; // Standard checkbox size

        // Calculate checkbox rectangle
        let checkbox_rect = Rect::new(
            rect.x,
            rect.y + (rect.height - checkbox_size) / 2,
            checkbox_size as u32,
            checkbox_size as u32,
        );

        // Draw checkbox background
        let bg_color = if !self.base.is_enabled() {
            Color::from_rgb(240, 240, 240)
        } else {
            Color::from_rgb(255, 255, 255)
        };
        context.fill_rect(checkbox_rect, bg_color);

        // Draw checkbox border
        let border_color = if !self.base.is_enabled() {
            Color::from_rgb(180, 180, 180)
        } else {
            Color::from_rgb(100, 100, 100)
        };
        context.draw_rect(checkbox_rect, border_color, 1);

        // Draw checkmark or partial check
        if self.state != CheckState::Unchecked {
            let check_color = if !self.base.is_enabled() {
                Color::from_rgb(150, 150, 150)
            } else {
                Color::from_rgb(0, 120, 215) // Blue checkmark
            };

            match self.state {
                CheckState::Checked => {
                    // Draw checkmark
                    let check_rect = Rect::new(
                        checkbox_rect.x + 3,
                        checkbox_rect.y + 6,
                        checkbox_rect.width - 6,
                        checkbox_rect.height - 12,
                    );
                    context.draw_checkmark(check_rect, check_color);
                }
                CheckState::PartiallyChecked => {
                    // Draw partial check (minus sign)
                    let partial_rect = Rect::new(
                        checkbox_rect.x + 4,
                        checkbox_rect.y + checkbox_rect.height as i32 / 2 - 1,
                        checkbox_rect.width - 8,
                        2,
                    );
                    context.fill_rect(partial_rect, check_color);
                }
                _ => {}
            }
        }

        // Draw label if there's text in the style
        if let Some(text) = &self.base.style().text {
            let text_rect = Rect::new(
                checkbox_rect.right() + 4,
                rect.y,
                rect.width - checkbox_rect.width as u32 - 4,
                rect.height,
            );
            let text_color = if !self.base.is_enabled() {
                Color::from_rgb(150, 150, 150)
            } else {
                self.style().text_color.unwrap_or(Color::from_rgb(0, 0, 0))
            };
            context.draw_text(text_rect, text, text_color);
        }
    }
}
