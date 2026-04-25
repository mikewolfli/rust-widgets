//! CollapsiblePane — a container widget that can be collapsed/expanded.
use crate::core::{Color, Font, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::style::WidgetStyle;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// A container widget that can be collapsed/expanded by clicking its header bar.
///
/// The header displays a title and an arrow indicator (▶ when collapsed, ▼ when expanded).
/// When collapsed, the content area is hidden; when expanded, the content area is visible
/// below the header bar.
pub struct CollapsiblePane {
    base: BaseWidget,
    title: String,
    collapsed: bool,
    animation_progress: f32,
    content_child: Option<ObjectId>,
    header_height: u32,
    /// Emitted when the collapsed state changes (parameter: new collapsed state).
    pub toggled: Signal1<bool>,
}

impl CollapsiblePane {
    /// Creates a new collapsible pane with the specified geometry and title.
    pub fn new(geometry: Rect, title: String) -> Self {
        let mut base = BaseWidget::new(WidgetKind::CollapsiblePane, geometry, "CollapsiblePane");
        // Collapsible pane starts expanded by default.
        base.visible = true;
        Self {
            base,
            title,
            collapsed: false,
            animation_progress: 1.0,
            content_child: None,
            header_height: 24,
            toggled: Signal1::new(),
        }
    }

    /// Returns the title text.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title text.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns whether the pane is currently collapsed.
    pub fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Sets the collapsed state, emits the `toggled` signal, and requests a redraw.
    pub fn set_collapsed(&mut self, collapsed: bool) {
        if self.collapsed == collapsed {
            return;
        }
        self.collapsed = collapsed;
        self.animation_progress = if collapsed { 0.0 } else { 1.0 };
        self.toggled.emit(collapsed);
        self.base.request_redraw();
    }

    /// Toggles the collapsed state.
    pub fn toggle(&mut self) {
        self.set_collapsed(!self.collapsed);
    }

    /// Sets the child widget to display in the content area.
    pub fn set_content(&mut self, child: ObjectId) {
        // Remove existing content child from children list if present.
        if let Some(existing) = self.content_child {
            self.base.remove_child(existing);
        }
        self.content_child = Some(child);
        self.base.add_child(child);
    }

    /// Returns the content child widget ID, if any.
    pub fn content(&self) -> Option<ObjectId> {
        self.content_child
    }

    /// Returns the height of the header bar in pixels.
    pub fn header_height(&self) -> u32 {
        self.header_height
    }

    /// Sets the height of the header bar in pixels.
    pub fn set_header_height(&mut self, height: u32) {
        self.header_height = height;
    }

    /// Returns the geometry of the header area.
    fn header_rect(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x, rect.y, rect.width, self.header_height)
    }

    /// Returns the geometry of the content area (below the header).
    fn content_rect(&self) -> Rect {
        let rect = self.geometry();
        let y_offset = rect.y + self.header_height as i32;
        let height = if rect.height > self.header_height {
            rect.height - self.header_height
        } else {
            0
        };
        Rect::new(rect.x, y_offset, rect.width, height)
    }
}

// Implement Widget trait
impl Widget for CollapsiblePane {
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
        if self.content_child == Some(child) {
            self.content_child = None;
        }
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

impl EventHandler for CollapsiblePane {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    // Check if click is within the header area.
                    let hdr = self.header_rect();
                    if hdr.contains(*pos) {
                        self.toggle();
                    }
                }
            }
            _ => {}
        }
    }
}

impl Draw for CollapsiblePane {
    fn draw(&mut self, context: &mut RenderContext) {
        let hdr = self.header_rect();

        // --- Draw header background ---
        let header_bg = if self.base.is_enabled() {
            Color::from_rgb(220, 220, 220)
        } else {
            Color::from_rgb(240, 240, 240)
        };
        context.fill_rect(hdr, header_bg);

        // --- Draw header bottom border ---
        let border_color = Color::from_rgb(180, 180, 180);
        context.draw_line(
            Point::from_f32(hdr.x as f32, (hdr.y + hdr.height as i32) as f32),
            Point::from_f32(
                (hdr.x + hdr.width as i32) as f32,
                (hdr.y + hdr.height as i32) as f32,
            ),
            border_color,
        );

        // --- Draw expand/collapse arrow (▶ collapsed, ▼ expanded) ---
        let arrow_x = hdr.x + 6;
        let arrow_y = hdr.y + (hdr.height as i32 / 2) - 4;
        let arrow_color = if self.base.is_enabled() {
            Color::from_rgb(80, 80, 80)
        } else {
            Color::from_rgb(180, 180, 180)
        };
        let arrow_char = if self.collapsed { "▶" } else { "▼" };
        context.draw_text(
            Point::from_f32(arrow_x as f32, arrow_y as f32),
            arrow_char,
            &Font::default(),
            arrow_color,
        );

        // --- Draw title text ---
        if !self.title.is_empty() {
            let text_x = hdr.x + 20;
            let text_y = hdr.y + (hdr.height as i32 / 2) - 6;
            let text_color = if self.base.is_enabled() {
                Color::from_rgb(0, 0, 0)
            } else {
                Color::from_rgb(150, 150, 150)
            };
            context.draw_text(
                Point::from_f32(text_x as f32, text_y as f32),
                &self.title,
                &Font::default(),
                text_color,
            );
        }

        // --- Draw content area (only when expanded) ---
        if !self.collapsed {
            let content_rect = self.content_rect();
            // Draw a subtle inner background for the content area.
            context.fill_rect(content_rect, Color::from_rgb(248, 248, 248));
            // Draw border around the content area (left, right, bottom).
            context.draw_line(
                Point::from_f32(content_rect.x as f32, content_rect.y as f32),
                Point::from_f32(
                    content_rect.x as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );
            context.draw_line(
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    content_rect.y as f32,
                ),
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );
            context.draw_line(
                Point::from_f32(
                    content_rect.x as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                Point::from_f32(
                    (content_rect.x + content_rect.width as i32) as f32,
                    (content_rect.y + content_rect.height as i32) as f32,
                ),
                border_color,
            );
        }
    }
}
