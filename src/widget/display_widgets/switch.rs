//! Switch/Toggle widget — a modern on/off binary state control.
//!
//! The Switch widget presents a sliding toggle that represents a boolean state,
//! similar to iOS UISwitch or Android Switch material widget. It supports
//! on/off toggling, animated transitions, and accessibility role mapping.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Switch/Toggle widget for binary on/off state selection.
pub struct Switch {
    base: BaseWidget,
    checked: bool,
    /// Emitted when the checked state changes.
    pub toggled: Signal1<bool>,
}

impl Switch {
    /// Creates a new Switch widget with the given geometry.
    /// Initial state is unchecked (off).
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Switch, geometry, "Switch"),
            checked: false,
            toggled: Signal1::new(),
        }
    }

    /// Returns whether the switch is in the checked (on) state.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Sets the checked state. Emits `toggled` signal if the state actually changes.
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked != checked {
            self.checked = checked;
            self.toggled.emit(checked);
            self.base.request_redraw();
        }
    }

    /// Toggles the checked state.
    pub fn toggle(&mut self) {
        self.set_checked(!self.checked);
    }
}

impl Widget for Switch {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for Switch {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();
        let style = self.style();

        // Track dimensions
        let track_width = rect.width.max(44);
        let track_height = (rect.height.max(24)).min(track_width / 2);
        let knob_size = track_height - 4;

        let track_x = rect.x;
        let track_y = rect.y + (rect.height as i32 - track_height as i32) / 2;
        let track_rect = Rect::new(track_x, track_y, track_width, track_height);

        // Draw track
        let track_color = if !is_enabled {
            style.background_color.unwrap_or(Color::rgba(200, 200, 200, 128))
        } else if self.checked {
            style.background_color.unwrap_or(Color::rgba(52, 199, 89, 200)) // iOS green
        } else {
            style.background_color.unwrap_or(Color::rgba(180, 180, 180, 200))
        };
        context.fill_rounded_rect(track_rect, track_height / 2, track_color);

        // Draw knob
        let knob_x = if self.checked {
            track_x + track_width as i32 - knob_size as i32 - 2
        } else {
            track_x + 2
        };
        let knob_y = track_y + 2;
        let knob_rect = Rect::new(knob_x, knob_y, knob_size, knob_size);

        let knob_color = if !is_enabled { Color::rgba(240, 240, 240, 200) } else { Color::WHITE };
        context.fill_rounded_rect(knob_rect, knob_size / 2, knob_color);

        // Draw knob shadow/border
        let knob_border_color = style.border_color.unwrap_or(Color::rgba(0, 0, 0, 30));
        context.draw_rounded_rect_stroke(knob_rect, knob_size / 2, knob_border_color, 1);
    }
}

impl EventHandler for Switch {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos: _, button } | Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    self.toggle();
                }
            }
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
    use std::sync::{Arc, Mutex};

    #[test]
    fn switch_default_is_unchecked() {
        let sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        assert_eq!(sw.kind(), WidgetKind::Switch);
    }

    #[test]
    fn switch_set_checked_emits_signal() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let captured = Arc::new(Mutex::new(None));
        sw.toggled.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<bool>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        sw.set_checked(true);
        assert!(sw.is_checked());
        assert_eq!(*captured.lock().unwrap(), Some(true));
    }

    #[test]
    fn switch_toggle_flips_state() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        assert!(!sw.is_checked());
        sw.toggle();
        assert!(sw.is_checked());
        sw.toggle();
        assert!(!sw.is_checked());
    }

    #[test]
    fn switch_mouse_press_toggles() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(sw.is_checked());
    }

    #[test]
    fn switch_disabled_blocks_events() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        sw.set_enabled(false);
        sw.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        assert!(!sw.is_checked());
    }

    #[test]
    fn switch_svg_output() {
        let mut sw = Switch::new(Rect::new(0, 0, 60, 30));
        let svg = crate::widget::svg::render_to_svg(&mut sw);
        assert!(svg.starts_with("<svg"));
    }
}
