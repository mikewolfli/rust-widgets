//! EmptyState widget — a placeholder shown when a view has no content.
//!
//! The EmptyState widget displays a large icon (emoji/symbol), title, descriptive
//! message, and an optional action button. It is commonly used in list views,
//! search results, inboxes, and dashboards to communicate "no data" states
//! rather than showing a blank screen.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Fallback icon used when no icon has been explicitly set.
const DEFAULT_EMPTY_ICON: &str = "📭";
/// Default empty title text.
const DEFAULT_TITLE: &str = "Nothing here";
/// Default empty message text.
const DEFAULT_MESSAGE: &str = "There are no items to display yet.";
/// Height of bottom action button area.
const ACTION_BUTTON_HEIGHT: u32 = 36;
/// Padding between layout sections.
const SECTION_GAP: i32 = 12;

/// EmptyState widget — a placeholder shown when a view has no content.
///
/// Displays a centered layout consisting of a large icon, a title label,
/// a descriptive message, and an optional action button. The action button
/// is only drawn when `action_text` is non-empty, and emits `action_pressed`
/// when clicked.
pub struct EmptyState {
    base: BaseWidget,
    icon: String,
    title: String,
    message: String,
    action_text: String,
    /// Emitted when the action button is clicked.
    pub action_pressed: GenericSignal,
}

impl EmptyState {
    /// Creates a new EmptyState widget with the given geometry.
    ///
    /// The widget is initialized with default placeholder values:
    /// - Icon: 📭 (empty mailbox)
    /// - Title: "Nothing here"
    /// - Message: "There are no items to display yet."
    /// - Action text: empty (no action button shown)
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::EmptyState, geometry, "EmptyState"),
            icon: DEFAULT_EMPTY_ICON.to_string(),
            title: DEFAULT_TITLE.to_string(),
            message: DEFAULT_MESSAGE.to_string(),
            action_text: String::new(),
            action_pressed: GenericSignal::new(),
        }
    }

    /// Sets the icon displayed at the top of the empty state.
    ///
    /// This is typically an emoji or a short symbol string (1–2 characters).
    pub fn set_icon(&mut self, icon: &str) {
        self.icon = icon.to_string();
        self.base.request_redraw();
    }

    /// Returns the current icon string.
    pub fn icon(&self) -> &str {
        &self.icon
    }

    /// Sets the title text displayed below the icon.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
        self.base.request_redraw();
    }

    /// Returns the current title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the descriptive message displayed below the title.
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.base.request_redraw();
    }

    /// Returns the current message text.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Sets the action button label text.
    ///
    /// When set to a non-empty string, an action button is rendered at the
    /// bottom of the empty state. Clicking the button emits `action_pressed`.
    /// When set to an empty string, the action button is hidden.
    pub fn set_action_text(&mut self, action_text: &str) {
        self.action_text = action_text.to_string();
        self.base.request_redraw();
    }

    /// Returns the current action button label text.
    pub fn action_text(&self) -> &str {
        &self.action_text
    }

    /// Returns `true` if the action button is visible (non-empty text).
    pub fn has_action(&self) -> bool {
        !self.action_text.is_empty()
    }

    /// Computes the rectangle for the action button, if it is visible.
    fn action_button_rect(&self) -> Option<Rect> {
        if !self.has_action() {
            return None;
        }
        let rect = self.geometry();
        let btn_width = rect.width.clamp(80, 200);
        let btn_x = rect.x + (rect.width as i32 - btn_width as i32) / 2;
        let btn_y = rect.y + rect.height as i32 - ACTION_BUTTON_HEIGHT as i32 - SECTION_GAP;
        Some(Rect::new(btn_x, btn_y, btn_width, ACTION_BUTTON_HEIGHT))
    }
}

impl Widget for EmptyState {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for EmptyState {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // ── Background ──
        let bg_color = Color::rgba(245, 245, 250, 200);
        context.fill_rect(rect, bg_color);

        let center_x = rect.x + rect.width as i32 / 2;

        // Helper to draw text centered horizontally at a given Y baseline.
        let draw_centered =
            |ctx: &mut RenderContext, y: i32, text: &str, font: &Font, color: Color| {
                let metrics = ctx.measure_text(text, font);
                let origin = Point::new(center_x - (metrics.width as i32 / 2), y);
                ctx.draw_text(origin, text, font, color, HorizontalAlignment::Left);
            };

        // ── Icon ──
        let icon_size = 48;
        let icon_y = rect.y + SECTION_GAP;
        let icon_color = if is_enabled {
            Color::rgba(120, 120, 140, 220)
        } else {
            Color::rgba(180, 180, 180, 120)
        };
        let icon_font = Font::with_weight("Sans", icon_size as f32, 400, false);
        draw_centered(context, icon_y + icon_size, &self.icon, &icon_font, icon_color);

        // ── Title ──
        let title_font_size = 20;
        let title_y = icon_y + icon_size + SECTION_GAP;
        let title_color =
            if is_enabled { Color::rgb(50, 50, 60) } else { Color::rgba(160, 160, 160, 180) };
        let title_font = Font::with_weight("Sans", title_font_size as f32, 600, false);
        let title_metrics = context.measure_text(&self.title, &title_font);
        let title_origin = Point::new(center_x - (title_metrics.width as i32 / 2), title_y);
        context.draw_text(
            title_origin,
            &self.title,
            &title_font,
            title_color,
            HorizontalAlignment::Left,
        );

        // ── Message ──
        let message_font_size = 14;
        let message_y = title_y + title_font_size + 6;
        let message_color = if is_enabled {
            Color::rgba(110, 110, 120, 220)
        } else {
            Color::rgba(170, 170, 170, 150)
        };
        let message_font = Font::with_weight("Sans", message_font_size as f32, 400, false);

        // Wrap message text if it's wider than the available width
        let available_width = rect.width.max(50) - 20;
        let wrapped_lines =
            wrap_text(context, &self.message, &message_font, available_width as usize);
        for (i, line) in wrapped_lines.iter().enumerate() {
            let line_y = message_y + i as i32 * (message_font_size + 4);
            let line_metrics = context.measure_text(line, &message_font);
            let line_origin = Point::new(center_x - (line_metrics.width as i32 / 2), line_y);
            context.draw_text(
                line_origin,
                line,
                &message_font,
                message_color,
                HorizontalAlignment::Left,
            );
        }

        // ── Action Button ──
        if let Some(btn_rect) = self.action_button_rect() {
            let corner_radius = btn_rect.height / 2;

            // Button background
            let btn_bg = if !is_enabled {
                Color::rgba(200, 200, 200, 120)
            } else {
                Color::rgb(59, 130, 246) // Blue action color
            };
            context.fill_rounded_rect(btn_rect, corner_radius, btn_bg);

            // Button text
            let btn_text_color =
                if !is_enabled { Color::rgba(160, 160, 160, 180) } else { Color::WHITE };
            let btn_font = Font::with_weight("Sans", 14.0, 600, false);
            let btn_metrics = context.measure_text(&self.action_text, &btn_font);
            let btn_origin = Point::new(
                btn_rect.x + (btn_rect.width as i32 - btn_metrics.width as i32) / 2,
                btn_rect.y + btn_rect.height as i32 / 2,
            );
            context.draw_text(
                btn_origin,
                &self.action_text,
                &btn_font,
                btn_text_color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for EmptyState {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Check if the click is on the action button
                    if let Some(btn_rect) = self.action_button_rect() {
                        if btn_rect.contains_point(*pos) {
                            self.action_pressed.emit();
                            return;
                        }
                    }
                    // Click on empty state body (outside action button)
                    if self.geometry().contains_point(*pos) {
                        self.base.clicked.emit();
                    }
                }
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 {
                    // No special handling needed
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// Wraps a text string into multiple lines at word boundaries to fit within
/// `max_width` pixels. Returns a vector of wrapped line strings.
fn wrap_text(context: &RenderContext, text: &str, font: &Font, max_width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    let words: Vec<&str> = text.split(' ').collect();
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();

    for word in words {
        let candidate = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current_line, word)
        };
        let metrics = context.measure_text(&candidate, font);
        if metrics.width as usize <= max_width || current_line.is_empty() {
            current_line = candidate;
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn empty_state_default_creation() {
        let es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.icon(), "📭");
        assert_eq!(es.title(), "Nothing here");
        assert_eq!(es.message(), "There are no items to display yet.");
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());
        assert_eq!(es.kind(), WidgetKind::EmptyState);
    }

    #[test]
    fn empty_state_set_icon() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_icon("🔍");
        assert_eq!(es.icon(), "🔍");
        es.set_icon("📦");
        assert_eq!(es.icon(), "📦");
    }

    #[test]
    fn empty_state_set_title() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.title(), "Nothing here");
        es.set_title("No results found");
        assert_eq!(es.title(), "No results found");
        es.set_title("");
        assert_eq!(es.title(), "");
    }

    #[test]
    fn empty_state_set_message() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_message("Your search did not match any items.");
        assert_eq!(es.message(), "Your search did not match any items.");
    }

    #[test]
    fn empty_state_set_action_text() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());

        es.set_action_text("Retry");
        assert_eq!(es.action_text(), "Retry");
        assert!(es.has_action());

        es.set_action_text("");
        assert_eq!(es.action_text(), "");
        assert!(!es.has_action());
    }

    #[test]
    fn empty_state_action_button_rect_none_when_no_action() {
        let es = EmptyState::new(Rect::new(0, 0, 300, 200));
        assert!(es.action_button_rect().is_none());
    }

    #[test]
    fn empty_state_action_button_rect_some_when_action() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Refresh");
        let btn_rect = es.action_button_rect();
        assert!(btn_rect.is_some());
        let rect = btn_rect.unwrap();
        // Button should be within the widget bounds
        assert!(rect.width <= 200);
        assert_eq!(rect.height, 36);
    }

    #[test]
    fn empty_state_action_pressed_signal() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Retry");
        let fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        // Compute action button rect and click on it
        let btn_rect = es.action_button_rect().unwrap();
        let click_x = btn_rect.x + btn_rect.width as i32 / 2;
        let click_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(click_x, click_y, 1));
        assert!(*fired.lock().unwrap());
    }

    #[test]
    fn empty_state_action_pressed_not_emitted_when_no_action() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        // No action text set
        let fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let fired = Arc::clone(&fired);
            move || {
                *fired.lock().unwrap() = true;
            }
        });

        // Click somewhere in the middle of the widget
        es.handle_event(&Event::mouse_press(150, 100, 1));
        assert!(!*fired.lock().unwrap());
    }

    #[test]
    fn empty_state_click_outside_action_emits_base_clicked() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Go");

        let base_clicked = Arc::new(Mutex::new(false));
        es.base.clicked.connect({
            let base_clicked = Arc::clone(&base_clicked);
            move || {
                *base_clicked.lock().unwrap() = true;
            }
        });

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        // Click in the upper icon area (not on action button)
        es.handle_event(&Event::mouse_press(150, 30, 1));
        assert!(!*action_fired.lock().unwrap());
        assert!(*base_clicked.lock().unwrap());
    }

    #[test]
    fn empty_state_disabled_blocks_events() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Press Me");
        es.set_enabled(false);

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        let btn_rect = es.action_button_rect().unwrap();
        let click_x = btn_rect.x + btn_rect.width as i32 / 2;
        let click_y = btn_rect.y + btn_rect.height as i32 / 2;
        es.handle_event(&Event::mouse_press(click_x, click_y, 1));
        assert!(!*action_fired.lock().unwrap());
    }

    #[test]
    fn empty_state_svg_output() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_icon("📦");
        es.set_title("No items");
        es.set_message("Your list is empty.");
        es.set_action_text("Add Item");
        let svg = crate::widget::svg::render_to_svg(&mut es);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"300\""));
        assert!(svg.contains("height=\"200\""));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn empty_state_setters_chain() {
        let mut es = EmptyState::new(Rect::new(0, 0, 400, 250));
        es.set_icon("⭐");
        es.set_title("Not found");
        es.set_message("Try adjusting your filters.");
        es.set_action_text("Clear Filters");
        assert_eq!(es.icon(), "⭐");
        assert_eq!(es.title(), "Not found");
        assert_eq!(es.message(), "Try adjusting your filters.");
        assert_eq!(es.action_text(), "Clear Filters");
    }

    #[test]
    fn empty_state_right_mouse_button_ignored() {
        let mut es = EmptyState::new(Rect::new(0, 0, 300, 200));
        es.set_action_text("Click");

        let action_fired = Arc::new(Mutex::new(false));
        es.action_pressed.connect({
            let action_fired = Arc::clone(&action_fired);
            move || {
                *action_fired.lock().unwrap() = true;
            }
        });

        // Right click should be ignored
        let btn_rect = es.action_button_rect().unwrap();
        es.handle_event(&Event::mouse_press(
            btn_rect.x + btn_rect.width as i32 / 2,
            btn_rect.y + btn_rect.height as i32 / 2,
            2,
        ));
        assert!(!*action_fired.lock().unwrap());
    }

    #[test]
    fn empty_state_kind() {
        let es = EmptyState::new(Rect::new(0, 0, 200, 150));
        assert_eq!(es.kind(), WidgetKind::EmptyState);
    }
}
