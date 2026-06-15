//! ModalBottomSheet widget — Material-style modal bottom sheet with drag-to-dismiss.
//!
//! Displays a semi-transparent overlay behind a rounded top sheet containing a
//! drag handle, title, and optional content. Supports show/hide, drag-to-dismiss,
//! and overlay-click-to-dismiss. Emits a `dismissed` signal when closed.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Material-style modal bottom sheet widget.
///
/// When visible, a semi-transparent overlay covers the widget geometry and a
/// rounded sheet appears at the bottom with a drag handle, title, and optional
/// child content. The user can tap the overlay or drag downward to dismiss.
pub struct ModalBottomSheet {
    base: BaseWidget,
    title: String,
    content: Option<Box<dyn Widget>>,
    is_visible: bool,
    drag_offset: f32,
    is_dragging: bool,
    /// Emitted when the sheet is dismissed by user interaction.
    pub dismissed: GenericSignal,
}

impl ModalBottomSheet {
    /// Creates a new ModalBottomSheet widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::ModalBottomSheet, geometry, "ModalBottomSheet"),
            title: String::new(),
            content: None,
            is_visible: false,
            drag_offset: 0.0,
            is_dragging: false,
            dismissed: GenericSignal::new(),
        }
    }

    /// Shows the bottom sheet.
    pub fn show(&mut self) {
        if !self.is_visible {
            self.is_visible = true;
            self.drag_offset = 0.0;
            self.is_dragging = false;
            self.base.request_redraw();
        }
    }

    /// Hides the bottom sheet without emitting the dismissed signal.
    pub fn hide(&mut self) {
        if self.is_visible {
            self.is_visible = false;
            self.drag_offset = 0.0;
            self.is_dragging = false;
            self.base.request_redraw();
        }
    }

    /// Returns whether the sheet is currently visible.
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    /// Sets the sheet title text.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_redraw();
    }

    /// Returns the sheet title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the child content widget displayed in the sheet body.
    pub fn set_content(&mut self, widget: Box<dyn Widget>) {
        self.content = Some(widget);
        self.base.request_redraw();
    }

    /// Returns a reference to the child content, if any.
    pub fn content(&self) -> Option<&dyn Widget> {
        self.content.as_deref()
    }

    /// Returns a mutable reference to the child content, if any.
    pub fn content_mut(&mut self) -> Option<&mut dyn Widget> {
        self.content.as_deref_mut()
    }

    /// Returns the current drag offset.
    pub fn drag_offset(&self) -> f32 {
        self.drag_offset
    }

    /// Returns whether the user is currently dragging the sheet.
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    /// Called when the user starts dragging the sheet.
    pub fn start_drag(&mut self) {
        if self.is_visible {
            self.is_dragging = true;
            self.base.request_redraw();
        }
    }

    /// Called to update the drag offset.
    /// Positive values indicate dragging downward.
    pub fn update_drag(&mut self, delta: f32) {
        if self.is_dragging {
            self.drag_offset = (self.drag_offset + delta).max(0.0);
            self.base.request_redraw();
        }
    }

    /// Called when the user ends the drag gesture.
    /// Dismisses the sheet if drag offset exceeds one third of the sheet height.
    pub fn end_drag(&mut self) {
        if self.is_dragging {
            self.is_dragging = false;
            let sheet_height = self.compute_sheet_height();
            if self.drag_offset > sheet_height as f32 / 3.0 {
                self.is_visible = false;
                self.dismissed.emit();
            }
            self.drag_offset = 0.0;
            self.base.request_redraw();
        }
    }

    /// Computes the approximate sheet panel height based on geometry and content.
    fn compute_sheet_height(&self) -> u32 {
        let rect = self.geometry();
        let title_height: u32 = 40;
        let handle_area: u32 = 24;
        let min_sheet = 120;
        let max_sheet = rect.height.saturating_sub(40);
        (title_height + handle_area + 80).min(max_sheet).max(min_sheet)
    }
}

impl Widget for ModalBottomSheet {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ModalBottomSheet {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.is_visible {
            return;
        }

        let rect = self.geometry();
        let sheet_height = self.compute_sheet_height();
        let drag_offset_px = self.drag_offset as i32;

        // 1. Semi-transparent overlay
        let overlay_rect = Rect::new(rect.x, rect.y, rect.width, rect.height);
        context.fill_rect(overlay_rect, Color::rgba(0, 0, 0, 80));

        // 2. Sheet panel at the bottom, shifted by drag offset
        let sheet_y = rect.y + rect.height as i32 - sheet_height as i32 + drag_offset_px;
        let sheet_rect_panel = Rect::new(rect.x, sheet_y, rect.width, sheet_height);
        let corner_radius: u32 = 16;

        context.fill_rounded_rect(sheet_rect_panel, corner_radius, Color::rgba(248, 248, 248, 255));
        context.draw_rounded_rect_stroke(
            sheet_rect_panel,
            corner_radius,
            Color::rgba(220, 220, 220, 255),
            1,
        );

        // 3. Drag handle
        let handle_width: u32 = 36;
        let handle_height: u32 = 5;
        let handle_x = rect.x + (rect.width as i32 - handle_width as i32) / 2;
        let handle_y = sheet_y + 10;
        let handle_rect = Rect::new(handle_x, handle_y, handle_width, handle_height);
        context.fill_rounded_rect(handle_rect, handle_height / 2, Color::rgba(180, 180, 180, 200));

        // 4. Title
        let title_y = handle_y + handle_height as i32 + 12;
        let title_font = Font::simple("sans-serif", 16.0);
        let title_metrics = context.measure_text(&self.title, &title_font);
        if !self.title.is_empty() {
            let title_x = rect.x + (rect.width as i32 - title_metrics.width as i32) / 2;
            context.draw_text(
                Point::new(title_x.max(rect.x), title_y + title_metrics.ascent as i32),
                &self.title,
                &title_font,
                Color::rgba(30, 30, 30, 255),
                HorizontalAlignment::Left,
            );
        }

        // 5. Content area (child widget rendering is delegated)
        let content_top = title_y + title_metrics.height as i32 + 8;
        let content_bottom = rect.y + rect.height as i32 + drag_offset_px;
        let content_available = (content_bottom - content_top) as u32;

        if content_available > 20 {
            let content_rect = Rect::new(
                rect.x + 8,
                content_top,
                rect.width.saturating_sub(16),
                content_available,
            );
            context.fill_rect(content_rect, Color::WHITE);
        }
    }
}

impl EventHandler for ModalBottomSheet {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() || !self.is_visible {
            return;
        }

        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    let sheet_height = self.compute_sheet_height();
                    let drag_offset_px = self.drag_offset as i32;
                    let sheet_y =
                        rect.y + rect.height as i32 - sheet_height as i32 + drag_offset_px;

                    // Check if click is in the sheet area (including drag handle area)
                    let in_sheet = pos.y >= sheet_y;

                    if in_sheet {
                        // Start potential drag
                        self.start_drag();
                    } else {
                        // Click on overlay — dismiss
                        self.is_visible = false;
                        self.dismissed.emit();
                        self.base.request_redraw();
                    }
                }
            }
            Event::MouseMove { pos: _ } => {
                // In a real integration, delta_y would be tracked from MousePress origin
            }
            Event::MouseRelease { pos: _, button } => {
                if *button == 1 && self.is_dragging {
                    self.end_drag();
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
    use crate::widget::svg::render_to_svg;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn make_sheet() -> ModalBottomSheet {
        ModalBottomSheet::new(Rect::new(0, 0, 400, 600))
    }

    #[test]
    fn modal_bottom_sheet_default_state() {
        let sheet = make_sheet();
        assert!(!sheet.is_visible());
        assert_eq!(sheet.title(), "");
        assert_eq!(sheet.kind(), WidgetKind::ModalBottomSheet);
    }

    #[test]
    fn modal_bottom_sheet_show_and_hide() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_visible());

        sheet.show();
        assert!(sheet.is_visible());

        sheet.hide();
        assert!(!sheet.is_visible());
    }

    #[test]
    fn modal_bottom_sheet_dismiss_signal_on_overlay_click() {
        let mut sheet = make_sheet();
        sheet.show();
        assert!(sheet.is_visible());

        let dismissed = Arc::new(AtomicBool::new(false));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.store(true, Ordering::SeqCst);
            }
        });

        // Click above the sheet area (overlay region)
        // Sheet height is computed, but geometry is 600 tall so overlay is top portion
        sheet.handle_event(&Event::MousePress { pos: Point::new(200, 50), button: 1 });
        assert!(!sheet.is_visible());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn modal_bottom_sheet_set_title() {
        let mut sheet = make_sheet();
        assert_eq!(sheet.title(), "");
        sheet.set_title("Options");
        assert_eq!(sheet.title(), "Options");
    }

    #[test]
    fn modal_bottom_sheet_drag_end_dismisses() {
        let mut sheet = make_sheet();
        sheet.show();
        assert!(sheet.is_visible());

        let dismissed = Arc::new(AtomicBool::new(false));
        sheet.dismissed.connect({
            let d = Arc::clone(&dismissed);
            move || {
                d.store(true, Ordering::SeqCst);
            }
        });

        // Start drag, push it past threshold, then release
        sheet.start_drag();
        assert!(sheet.is_dragging());

        // Push past 1/3 of sheet height (sheet is at least 120, so > 40)
        sheet.update_drag(80.0);
        assert_eq!(sheet.drag_offset(), 80.0);

        sheet.end_drag();
        assert!(!sheet.is_visible());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn modal_bottom_sheet_drag_small_offset_no_dismiss() {
        let mut sheet = make_sheet();
        sheet.show();

        sheet.start_drag();
        sheet.update_drag(20.0);
        sheet.end_drag();

        // Small drag should not dismiss
        assert!(sheet.is_visible());
    }

    #[test]
    fn modal_bottom_sheet_svg_output_visible() {
        let mut sheet = make_sheet();
        sheet.set_title("Example");
        sheet.show();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn modal_bottom_sheet_svg_output_hidden() {
        let mut sheet = make_sheet();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
