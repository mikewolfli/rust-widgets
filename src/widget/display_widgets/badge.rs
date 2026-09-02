//! Badge widget — a notification count/indicator dot.
//!
//! The Badge widget displays a small notification count or status indicator,
//! rendered as a colored circle or pill shape. It supports different severity
//! levels (Info, Success, Warning, Error) and a dot mode that shows only a
//! colored dot without text. When the count is set to 0, the badge hides itself.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Severity level for a badge, determining its color scheme.
/// Badge severity/notification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeLevel {
    /// Informational — blue.
    #[default]
    Info,
    /// Success/green — green.
    Success,
    /// Warning — yellow/orange.
    Warning,
    /// Error/danger — red.
    Error,
}

impl BadgeLevel {
    /// Returns the background color associated with this severity level.
    pub fn color(&self) -> Color {
        match self {
            BadgeLevel::Info => Color::INFO,
            BadgeLevel::Success => Color::SUCCESS,
            BadgeLevel::Warning => Color::WARNING,
            BadgeLevel::Error => Color::ERROR,
        }
    }
}

/// Badge widget for notification counts and status indicators.
///
/// Renders as a filled circle or pill with an optional text overlay.
/// Supports dot mode (colored dot with no text) and automatic hiding
/// when count is 0 (unless overridden with custom text).
pub struct Badge {
    base: BaseWidget,
    count: u32,
    text: String,
    level: BadgeLevel,
    dot_mode: bool,
}

impl Badge {
    /// Creates a new Badge widget with the given geometry.
    ///
    /// Initial state has no count, no text, and an Info level.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Badge, geometry, "Badge"),
            count: 0,
            text: String::new(),
            level: BadgeLevel::Info,
            dot_mode: false,
        }
    }

    /// Sets the notification count. A value of 0 hides the badge (unless dot mode is enabled
    /// or custom text is set). Values above 999 are displayed as "999+".
    pub fn set_count(&mut self, count: u32) {
        self.count = count;
        self.base.request_redraw();
    }

    /// Returns the current notification count.
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Sets the display text shown inside the badge. When non-empty, this takes
    /// precedence over the numeric count.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.base.request_redraw();
    }

    /// Returns the current display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the severity level, which changes the badge's background color.
    pub fn set_level(&mut self, level: BadgeLevel) {
        self.level = level;
        self.base.request_redraw();
    }

    /// Returns the current severity level.
    pub fn level(&self) -> BadgeLevel {
        self.level
    }

    /// Enables or disables dot mode. When enabled, the badge renders as a small
    /// colored dot without any text overlay, regardless of count or text.
    pub fn set_dot_mode(&mut self, enabled: bool) {
        self.dot_mode = enabled;
        self.base.request_redraw();
    }

    /// Returns whether dot mode is currently enabled.
    pub fn is_dot_mode(&self) -> bool {
        self.dot_mode
    }

    /// Returns the display string to render inside the badge.
    fn display_text(&self) -> String {
        if !self.text.is_empty() {
            return self.text.clone();
        }
        if self.count > 999 {
            "999+".to_string()
        } else if self.count > 0 {
            self.count.to_string()
        } else {
            String::new()
        }
    }

    /// Returns whether the badge should be drawn at all.
    fn should_draw(&self) -> bool {
        if self.dot_mode {
            return true;
        }
        if !self.text.is_empty() {
            return true;
        }
        self.count > 0
    }
}

impl Widget for Badge {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(24, 24)
    }
}

impl Draw for Badge {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.should_draw() {
            return;
        }

        let rect = self.geometry();
        let bg_color = self.level.color();
        let text_str = self.display_text();

        if self.dot_mode {
            // Draw a small colored dot centered in the geometry
            let dot_radius = (rect.height.min(rect.width) / 2).max(4);
            let center =
                Point::new(rect.x + (rect.width as i32) / 2, rect.y + (rect.height as i32) / 2);
            context.fill_circle(center, dot_radius, bg_color);
            return;
        }

        if text_str.is_empty() {
            return;
        }

        // Determine pill dimensions based on content
        let font = Font::simple("sans-serif", 11.0);
        let metrics = context.measure_text(&text_str, &font);

        let text_width = metrics.width;
        let glyph_height = metrics.height;
        let padding_x = 6u32;
        let padding_y = 2u32;

        let pill_height = (glyph_height + padding_y * 2).max(rect.height.min(18));
        let pill_width = (text_width + padding_x * 2).max(pill_height);

        // Center the pill within the widget geometry
        let pill_x = rect.x + (rect.width as i32 - pill_width as i32) / 2;
        let pill_y = rect.y + (rect.height as i32 - pill_height as i32) / 2;

        let pill_rect = Rect::new(pill_x.max(rect.x), pill_y.max(rect.y), pill_width, pill_height);
        let corner_radius = pill_height / 2;

        // Draw pill background
        context.fill_rounded_rect(pill_rect, corner_radius, bg_color);

        // Draw text centered on the pill
        let text_x = pill_rect.x + ((pill_rect.width as i32 - text_width as i32) / 2).max(0);
        let text_y = pill_rect.y
            + ((pill_rect.height as i32 - glyph_height as i32) / 2).max(0)
            + metrics.ascent as i32;

        context.draw_text(
            Point::new(text_x, text_y),
            &text_str,
            &font,
            Color::WHITE,
            HorizontalAlignment::Left,
        );
    }
}

impl EventHandler for Badge {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Size;
    use crate::widget::svg::render_to_svg;

    #[test]
    fn badge_default_creation() {
        let badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert_eq!(badge.kind(), WidgetKind::Badge);
        assert_eq!(badge.count(), 0);
        assert!(badge.text().is_empty());
        assert_eq!(badge.level(), BadgeLevel::Info);
        assert!(!badge.is_dot_mode());
        assert_eq!(badge.geometry(), Rect::new(0, 0, 40, 24));
    }

    #[test]
    fn badge_count_display() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert!(!badge.should_draw());

        badge.set_count(5);
        assert_eq!(badge.count(), 5);
        assert!(badge.should_draw());
        assert_eq!(badge.display_text(), "5");

        badge.set_count(123);
        assert_eq!(badge.display_text(), "123");

        badge.set_count(1000);
        assert_eq!(badge.display_text(), "999+");
    }

    #[test]
    fn badge_level_changes_color() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));

        assert_eq!(badge.level(), BadgeLevel::Info);
        assert_eq!(badge.level().color(), Color::INFO);

        badge.set_level(BadgeLevel::Success);
        assert_eq!(badge.level(), BadgeLevel::Success);
        assert_eq!(badge.level().color(), Color::SUCCESS);

        badge.set_level(BadgeLevel::Warning);
        assert_eq!(badge.level(), BadgeLevel::Warning);
        assert_eq!(badge.level().color(), Color::WARNING);

        badge.set_level(BadgeLevel::Error);
        assert_eq!(badge.level(), BadgeLevel::Error);
        assert_eq!(badge.level().color(), Color::ERROR);
    }

    #[test]
    fn badge_zero_count_hides() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        assert!(!badge.should_draw());

        badge.set_count(0);
        assert!(!badge.should_draw());

        badge.set_text("!".to_string());
        assert!(badge.should_draw());

        badge.set_text(String::new());
        badge.set_count(1);
        assert!(badge.should_draw());

        badge.set_count(0);
        assert!(!badge.should_draw());
    }

    #[test]
    fn badge_dot_mode() {
        let mut badge = Badge::new(Rect::new(0, 0, 20, 20));
        assert!(!badge.is_dot_mode());

        badge.set_dot_mode(true);
        assert!(badge.is_dot_mode());
        assert!(badge.should_draw());

        badge.set_count(0);
        badge.set_text(String::new());
        assert!(badge.should_draw()); // dot mode always draws

        badge.set_dot_mode(false);
        assert!(!badge.should_draw()); // no count + no text + not dot mode = hidden
    }

    #[test]
    fn badge_text_override() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        badge.set_count(5);
        assert_eq!(badge.display_text(), "5");

        badge.set_text("New".to_string());
        assert_eq!(badge.display_text(), "New");
        assert_eq!(badge.text(), "New");

        badge.set_text(String::new());
        assert_eq!(badge.display_text(), "5");
    }

    #[test]
    fn badge_event_forwarding() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        // Should not panic
        badge.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        badge.handle_event(&Event::MousePress { pos: Point::new(10, 10), button: 1 });
        badge.handle_event(&Event::Resize { size: Size::new(50, 30) });
    }

    #[test]
    fn badge_svg_output() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        badge.set_count(3);

        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"), "SVG should start with <svg, got: {svg:.60}");
        assert!(svg.ends_with("</svg>"), "SVG should end with </svg>");
    }

    #[test]
    fn badge_svg_output_dot_mode() {
        let mut badge = Badge::new(Rect::new(0, 0, 20, 20));
        badge.set_dot_mode(true);

        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn badge_svg_hidden_when_zero() {
        let mut badge = Badge::new(Rect::new(0, 0, 40, 24));
        // count is 0, no text, not dot mode → should be hidden → only background fill
        let svg = render_to_svg(&mut badge);
        assert!(svg.starts_with("<svg"));
        // The SVG backend adds one background fill rect; there should be no
        // additional fill elements from badge drawing since nothing was drawn.
        // A common test: SVG ends right after the background, no badge elements.
        let fill_count = svg.matches("fill=").count();
        assert_eq!(fill_count, 1, "expected only background fill, got {fill_count}: {svg}");
    }
}
