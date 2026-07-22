//! Window widget and platform integration.
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Main application window.
pub struct Window {
    base: BaseWidget,
    title: String,
    title_bar_height: u32,
    close_button_size: u32,
    button_spacing: u32,
    /// Emitted when the window is closed.
    pub closed: GenericSignal,
}
impl Window {
    /// Creates a new window with title and geometry.
    pub fn new(title: String, geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Window, geometry, "Window"),
            title,
            title_bar_height: 32,
            close_button_size: 14,
            button_spacing: 40,
            closed: GenericSignal::new(),
        }
    }
    /// Adds a child widget to the window.
    pub fn add_child(&mut self, child: ObjectId) {
        self.base.add_child(child);
    }
    /// Returns window title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Updates window title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.base.request_redraw();
    }
    /// Returns the title bar height.
    pub fn title_bar_height(&self) -> u32 {
        self.title_bar_height
    }

    /// Sets the title bar height and requests redraw.
    pub fn set_title_bar_height(&mut self, height: u32) {
        self.title_bar_height = height;
        self.base.request_redraw();
    }

    /// Returns the close button size.
    pub fn close_button_size(&self) -> u32 {
        self.close_button_size
    }

    /// Sets the close button size and requests redraw.
    pub fn set_close_button_size(&mut self, size: u32) {
        self.close_button_size = size;
        self.base.request_redraw();
    }

    /// Returns the button spacing.
    pub fn button_spacing(&self) -> u32 {
        self.button_spacing
    }

    /// Sets the button spacing and requests redraw.
    pub fn set_button_spacing(&mut self, spacing: u32) {
        self.button_spacing = spacing;
        self.base.request_redraw();
    }

    /// Emits the window closed signal.
    pub fn close(&mut self) {
        self.closed.emit();
    }
}
impl Widget for Window {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(640, 480)
    }
}
impl EventHandler for Window {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if matches!(event, Event::Quit) {
            self.closed.emit();
        }
    }
}

impl Draw for Window {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();
        let bg_color = style.background_color.unwrap_or(Color::rgb(240, 240, 240));
        let border_color = style.border_color.unwrap_or(Color::GRAY);
        let title_bar_color = Color::rgb(53, 53, 53);
        let title_text_color = Color::WHITE;
        let border_width = style.border_width.unwrap_or(0);
        // Draw window background
        context.fill_rect(rect, bg_color);
        // Draw title bar
        let title_bar_height = self.title_bar_height;
        let title_bar_rect = Rect::new(rect.x, rect.y, rect.width, title_bar_height);
        context.fill_rect(title_bar_rect, title_bar_color);
        // Draw title text
        let title_font =
            self.font().cloned().unwrap_or_else(|| Font::new("Arial", 12.0, false, false));
        let title_x = rect.x + 10;
        let title_y = rect.y + title_bar_height as i32 / 2;
        context.draw_text(
            Point::new(title_x, title_y),
            &self.title,
            &title_font,
            title_text_color,
            HorizontalAlignment::Left,
        );
        // Draw window border
        if border_width > 0 {
            context.draw_rect_stroke(rect, border_color, border_width);
        }
        // Draw window controls (close button)
        let close_button_size = self.close_button_size;
        let close_button_rect = Rect::new(
            rect.right() - close_button_size as i32 - 10,
            rect.y + (title_bar_height as i32 - close_button_size as i32) / 2,
            close_button_size,
            close_button_size,
        );
        // Draw close button background
        context.fill_rect(close_button_rect, Color::rgba(232, 17, 35, 255));
        // Draw close button X
        let padding = 3;
        let x1 = Point::new(close_button_rect.x + padding, close_button_rect.y + padding);
        let x2 = Point::new(
            close_button_rect.x + close_button_rect.width as i32 - padding,
            close_button_rect.y + close_button_rect.height as i32 - padding,
        );
        let x3 = Point::new(
            close_button_rect.x + close_button_rect.width as i32 - padding,
            close_button_rect.y + padding,
        );
        let x4 = Point::new(
            close_button_rect.x + padding,
            close_button_rect.y + close_button_rect.height as i32 - padding,
        );
        context.draw_line(x1, x2, Color::WHITE);
        context.draw_line(x3, x4, Color::WHITE);
        // Draw minimize button
        let minimize_button_rect = Rect::new(
            rect.right()
                - close_button_size as i32
                - (self.button_spacing * 2 + self.close_button_size * 2) as i32
                - 10,
            rect.y + (title_bar_height as i32 - close_button_size as i32) / 2,
            close_button_size,
            close_button_size,
        );
        context.fill_rect(minimize_button_rect, Color::rgba(255, 255, 255, 50));
        // Draw minimize line
        let minimize_y = minimize_button_rect.y + minimize_button_rect.height as i32 / 2;
        context.draw_line(
            Point::new(minimize_button_rect.x + 2, minimize_y),
            Point::new(minimize_button_rect.x + minimize_button_rect.width as i32 - 2, minimize_y),
            Color::WHITE,
        );
        // Draw maximize button
        let maximize_button_rect = Rect::new(
            rect.right()
                - close_button_size as i32
                - (self.button_spacing + self.close_button_size) as i32
                - 10,
            rect.y + (title_bar_height as i32 - close_button_size as i32) / 2,
            close_button_size,
            close_button_size,
        );
        context.fill_rect(maximize_button_rect, Color::rgba(255, 255, 255, 50));
        // Draw maximize square
        let max_padding = 3;
        context.draw_rect_stroke(
            Rect::new(
                maximize_button_rect.x + max_padding as i32,
                maximize_button_rect.y + max_padding as i32,
                maximize_button_rect.width - max_padding * 2,
                maximize_button_rect.height - max_padding * 2,
            ),
            Color::WHITE,
            1,
        );
    }
}
// NOTE: The show() method is now handled by platform backend.
// For full application integration, use the platform event loop via crate::run().
// The platform backend (macOS: NSApp().run(), Windows: message loop, etc.)
// handles all event dispatch and rendering coordination.
