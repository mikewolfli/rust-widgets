//! Snackbar widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Snackbar with optional action and progress display.
pub struct Snackbar {
    base: BaseWidget,
    message: String,
    action_label: Option<String>,
    visible: bool,
    progress: Option<f32>,
    /// Emitted when action is triggered.
    pub action_triggered: Signal1<String>,
    /// Emitted when snackbar is dismissed.
    pub dismissed: Signal1<()>,
}

impl Snackbar {
    /// Creates hidden snackbar.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::StatusBar, geometry, "Snackbar"),
            message: String::new(),
            action_label: None,
            visible: false,
            progress: None,
            action_triggered: Signal1::new(),
            dismissed: Signal1::new(),
        }
    }

    /// Returns visibility.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Returns current message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Shows snackbar without action.
    pub fn show(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.action_label = None;
        self.visible = true;
        self.base.request_redraw();
    }

    /// Shows snackbar with action button.
    pub fn show_with_action(
        &mut self,
        message: impl Into<String>,
        action_label: impl Into<String>,
    ) {
        self.message = message.into();
        self.action_label = Some(action_label.into());
        self.visible = true;
        self.base.request_redraw();
    }

    /// Dismisses snackbar.
    pub fn dismiss(&mut self) {
        if self.visible {
            self.visible = false;
            self.dismissed.emit(());
            self.base.request_redraw();
        }
    }

    /// Sets optional progress indicator.
    pub fn set_progress(&mut self, progress: Option<f32>) {
        self.progress = progress.map(|value| value.clamp(0.0, 1.0));
        self.base.request_redraw();
    }

    /// Returns action label.
    pub fn action_label(&self) -> Option<&str> {
        self.action_label.as_deref()
    }

    fn trigger_action(&mut self) -> bool {
        if !self.visible {
            return false;
        }
        let Some(label) = self.action_label.clone() else {
            return false;
        };
        self.action_triggered.emit(label);
        true
    }

    fn action_rect(&self) -> Option<Rect> {
        self.action_label.as_ref()?;
        let rect = self.geometry();
        Some(Rect::new(rect.x + rect.width as i32 - 90, rect.y + rect.height as i32 - 28, 76, 20))
    }

    fn point_in_rect(pos: Point, rect: Rect) -> bool {
        pos.x >= rect.x
            && pos.x < rect.x + rect.width as i32
            && pos.y >= rect.y
            && pos.y < rect.y + rect.height as i32
    }
}

impl Widget for Snackbar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 48)
    }
}

impl EventHandler for Snackbar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() || !self.visible {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(rect) = self.action_rect() {
                    if Self::point_in_rect(*pos, rect) {
                        let _ = self.trigger_action();
                    }
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                13 => {
                    let _ = self.trigger_action();
                }
                27 => self.dismiss(),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Snackbar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(248, 250, 253));
        context.draw_rect(rect, Color::rgb(204, 210, 220));

        if !self.visible {
            return;
        }

        let bar = Rect::new(
            rect.x + 8,
            rect.y + rect.height as i32 - 34,
            rect.width.saturating_sub(16),
            26,
        );
        context.fill_rect(bar, Color::rgb(43, 51, 64));
        context.draw_rect(bar, Color::rgb(75, 87, 105));

        context.draw_text(
            Point::new(bar.x + 10, bar.y + 16),
            &self.message,
            &Font::default(),
            Color::rgb(236, 240, 246),
            HorizontalAlignment::Left,
        );

        if let Some(action_rect) = self.action_rect() {
            context.fill_rect(action_rect, Color::rgb(69, 108, 171));
            context.draw_rect(action_rect, Color::rgb(105, 143, 204));
            if let Some(label) = &self.action_label {
                context.draw_text(
                    Point::new(action_rect.x + 10, action_rect.y + 13),
                    label,
                    &Font::default(),
                    Color::rgb(238, 244, 252),
                    HorizontalAlignment::Left,
                );
            }
        }

        if let Some(progress) = self.progress {
            let progress_bar = Rect::new(bar.x, bar.y + bar.height as i32 - 3, bar.width, 3);
            context.fill_rect(progress_bar, Color::rgb(77, 88, 103));
            let fill = (progress_bar.width as f32 * progress).round() as u32;
            if fill > 0 {
                context.fill_rect(
                    Rect::new(progress_bar.x, progress_bar.y, fill, progress_bar.height),
                    Color::rgb(93, 179, 125),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn show_and_dismiss_change_visibility() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        assert!(!bar.is_visible());

        bar.show("Saved successfully");
        assert!(bar.is_visible());
        assert_eq!(bar.message(), "Saved successfully");

        bar.dismiss();
        assert!(!bar.is_visible());
    }

    #[test]
    fn enter_key_triggers_action_signal() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Retry deployment", "Retry");

        let actions = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = actions.clone();
        bar.action_triggered.connect(move |label| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(label.as_ref().clone());
            }
        });

        bar.handle_event(&Event::key_press(13, 0));

        let got = actions.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["Retry".to_string()]);
    }

    #[test]
    fn esc_key_dismisses() {
        let mut bar = Snackbar::new(Rect::new(0, 0, 420, 120));
        bar.show_with_action("Sync pending", "Open");
        assert!(bar.is_visible());

        bar.handle_event(&Event::key_press(27, 0));
        assert!(!bar.is_visible());
    }
}
