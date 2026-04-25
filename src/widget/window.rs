//! Window widget and platform integration.
use crate::core::{Color, Font, ObjectId, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
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
    }
    /// Returns the title bar height.
    pub fn get_title_bar_height(&self) -> u32 {
        self.title_bar_height
    }

    /// Sets the title bar height and requests redraw.
    pub fn set_title_bar_height(&mut self, height: u32) {
        self.title_bar_height = height;
        self.base.request_redraw();
    }

    /// Returns the close button size.
    pub fn get_close_button_size(&self) -> u32 {
        self.close_button_size
    }

    /// Sets the close button size and requests redraw.
    pub fn set_close_button_size(&mut self, size: u32) {
        self.close_button_size = size;
        self.base.request_redraw();
    }

    /// Returns the button spacing.
    pub fn get_button_spacing(&self) -> u32 {
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
    fn min_size(&self) -> Option<crate::core::Size> {
        self.base.min_size()
    }
    fn max_size(&self) -> Option<crate::core::Size> {
        self.base.max_size()
    }
    fn set_min_size(&mut self, min_size: Option<crate::core::Size>) {
        self.base.set_min_size(min_size);
    }
    fn set_max_size(&mut self, max_size: Option<crate::core::Size>) {
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
    fn hover_signal(&self) -> &Signal1<crate::core::Point> {
        self.base.hover_signal()
    }
    fn mouse_down_signal(&self) -> &Signal1<(crate::core::Point, u32)> {
        self.base.mouse_down_signal()
    }
    fn mouse_up_signal(&self) -> &Signal1<(crate::core::Point, u32)> {
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
        let border_width = style.border_width;
        // Draw window background
        context.fill_rect(rect, bg_color);
        // Draw title bar
        let title_bar_height = self.title_bar_height;
        let title_bar_rect = Rect::new(rect.x, rect.y, rect.width, title_bar_height);
        context.fill_rect(title_bar_rect, title_bar_color);
        // Draw title text
        let title_font = Font::new("Arial", 12.0, false, false);
        let title_x = rect.x + 10;
        let title_y = rect.y + title_bar_height as i32 / 2;
        context.draw_text(
            Point::new(title_x, title_y),
            &self.title,
            &title_font,
            title_text_color,
        );
        // Draw window border
        if border_width > 0 {
            context.draw_rect_stroke(rect, border_color, border_width);
        }
        // Draw window controls (close button)
        let close_button_size = self.close_button_size;
        let close_button_rect = Rect::new(
            rect.x + rect.width as f32 as i32 - close_button_size as i32 - 10,
            rect.y + (title_bar_height as i32 - close_button_size as i32) / 2,
            close_button_size,
            close_button_size,
        );
        // Draw close button background
        context.fill_rect(close_button_rect, Color::rgba(232, 17, 35, 255));
        // Draw close button X
        let padding = 3;
        let x1 = Point::new(
            close_button_rect.x + padding as i32,
            close_button_rect.y + padding as i32,
        );
        let x2 = Point::new(
            close_button_rect.x + close_button_rect.width as i32 - padding as i32,
            close_button_rect.y + close_button_rect.height as i32 - padding as i32,
        );
        let x3 = Point::new(
            close_button_rect.x + close_button_rect.width as i32 - padding as i32,
            close_button_rect.y + padding as i32,
        );
        let x4 = Point::new(
            close_button_rect.x + padding as i32,
            close_button_rect.y + close_button_rect.height as i32 - padding as i32,
        );
        context.draw_line(x1, x2, Color::WHITE);
        context.draw_line(x3, x4, Color::WHITE);
        // Draw minimize button
        let minimize_button_rect = Rect::new(
            rect.x + rect.width as i32
                - close_button_size as i32
                - (self.button_spacing * 2 + self.close_button_size * 2) as i32,
            rect.y + (title_bar_height as i32 - close_button_size as i32) / 2,
            close_button_size,
            close_button_size,
        );
        context.fill_rect(minimize_button_rect, Color::rgba(255, 255, 255, 50));
        // Draw minimize line
        let minimize_y = minimize_button_rect.y + minimize_button_rect.height as i32 / 2;
        context.draw_line(
            Point::new(minimize_button_rect.x + 2, minimize_y),
            Point::new(
                minimize_button_rect.x + minimize_button_rect.width as i32 - 2,
                minimize_y,
            ),
            Color::WHITE,
        );
        // Draw maximize button
        let maximize_button_rect = Rect::new(
            rect.x + rect.width as f32 as i32
                - close_button_size as i32
                - (self.button_spacing + self.close_button_size) as i32,
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
