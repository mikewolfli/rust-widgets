//! BottomSheet widget — a modal panel that slides up from the bottom edge of the
//! screen, similar to Android/Material Design bottom sheets.
//!
//! The BottomSheet displays a semi-transparent overlay behind a rounded-rect panel
//! at the bottom of its geometry. It supports open/close state, a drag handle at
//! the top of the sheet, and emits a `dismissed` signal when the user taps outside
//! or directly on the sheet area.

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// BottomSheet widget — a modal panel that slides up from the bottom edge.
///
/// The sheet occupies the bottom `content_height` pixels of its geometry rect.
/// When `open` is true, a semi-transparent gray overlay fills the area above the
/// sheet and a rounded panel is drawn at the bottom with a small drag handle.
/// Any click dismisses the sheet and fires the `dismissed` signal.
pub struct BottomSheet {
    base: BaseWidget,
    open: bool,
    content_height: u32,
    /// Emitted when the sheet is dismissed by user interaction.
    pub dismissed: GenericSignal,
}

impl BottomSheet {
    /// Creates a new BottomSheet widget with the given geometry.
    ///
    /// The sheet starts closed. Default content height is half the geometry height.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BottomSheet, geometry, "BottomSheet"),
            open: false,
            content_height: geometry.height / 2,
            dismissed: GenericSignal::new(),
        }
    }

    /// Opens the bottom sheet, making it visible.
    pub fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.base.request_redraw();
        }
    }

    /// Dismisses (closes) the bottom sheet without emitting the dismissed signal.
    pub fn dismiss(&mut self) {
        if self.open {
            self.open = false;
            self.base.request_redraw();
        }
    }

    /// Sets the content height of the sheet panel (in pixels).
    ///
    /// The sheet is drawn at the bottom of the geometry rect using this height.
    /// Clamped to the geometry height.
    pub fn set_content_height(&mut self, height: u32) {
        self.content_height = height.min(self.base.geometry.height);
        self.base.request_redraw();
    }

    /// Returns whether the bottom sheet is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }
}

impl Widget for BottomSheet {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for BottomSheet {
    fn draw(&mut self, context: &mut RenderContext) {
        if !self.open {
            return;
        }

        let rect = self.geometry();
        let sheet_height = self.content_height.min(rect.height);

        // Sheet panel rect at the bottom of the geometry
        let sheet_y = rect.y + rect.height as i32 - sheet_height as i32;
        let sheet_rect = Rect::new(rect.x, sheet_y, rect.width, sheet_height);

        // 1. Draw semi-transparent overlay covering the area above the sheet
        let overlay_height = rect.height - sheet_height;
        if overlay_height > 0 {
            let overlay_rect = Rect::new(rect.x, rect.y, rect.width, overlay_height);
            let overlay_color = Color::rgba(0, 0, 0, 100);
            context.fill_rect(overlay_rect, overlay_color);
        }

        // 2. Draw the sheet panel with rounded top corners
        let sheet_radius = 16;
        let sheet_color = Color::rgba(248, 248, 248, 255);
        context.fill_rounded_rect(sheet_rect, sheet_radius, sheet_color);

        // 3. Draw a thin stroke at the rounded top edge for definition
        context.draw_rounded_rect_stroke(
            sheet_rect,
            sheet_radius,
            Color::rgba(220, 220, 220, 255),
            1,
        );

        // 4. Draw the drag handle at the top center of the sheet
        let handle_width = 32;
        let handle_height = 5;
        let handle_x = rect.x + (rect.width as i32 - handle_width as i32) / 2;
        let handle_y = sheet_y + 8;
        let handle_rect = Rect::new(handle_x, handle_y, handle_width, handle_height);
        let handle_color = Color::rgba(180, 180, 180, 200);
        context.fill_rounded_rect(handle_rect, handle_height / 2, handle_color);
    }
}

impl EventHandler for BottomSheet {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos: _, button } => {
                if *button == 1 && self.open {
                    self.open = false;
                    self.dismissed.emit();
                    self.base.request_redraw();
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

    fn make_sheet() -> BottomSheet {
        BottomSheet::new(Rect::new(0, 0, 400, 600))
    }

    #[test]
    fn bottom_sheet_default_is_closed() {
        let sheet = make_sheet();
        assert!(!sheet.is_open());
        assert_eq!(sheet.kind(), WidgetKind::BottomSheet);
        assert_eq!(sheet.base.geometry, Rect::new(0, 0, 400, 600));
    }

    #[test]
    fn bottom_sheet_open_and_close() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_open());

        sheet.open();
        assert!(sheet.is_open());

        sheet.dismiss();
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_set_content_height() {
        let mut sheet = make_sheet();
        assert_eq!(sheet.content_height, 300); // half of 600

        sheet.set_content_height(200);
        assert_eq!(sheet.content_height, 200);

        // Clamp to geometry height
        sheet.set_content_height(999);
        assert_eq!(sheet.content_height, 600);
    }

    #[test]
    fn bottom_sheet_dismiss_signal_emits() {
        let mut sheet = make_sheet();
        sheet.open();
        assert!(sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(200, 300), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_mouse_press_dismisses_when_open() {
        let mut sheet = make_sheet();
        sheet.open();
        assert!(sheet.is_open());

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_mouse_press_noop_when_closed() {
        let mut sheet = make_sheet();
        assert!(!sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_other_button_noop() {
        let mut sheet = make_sheet();
        sheet.open();

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 2 });
        assert!(sheet.is_open());
    }

    #[test]
    fn bottom_sheet_disabled_blocks_events() {
        let mut sheet = make_sheet();
        sheet.set_enabled(false);
        sheet.open();
        assert!(sheet.is_open());

        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1 });
        assert!(sheet.is_open());
        assert!(!dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_open_twice_noop() {
        let mut sheet = make_sheet();
        sheet.open();
        sheet.open(); // should not change anything
        assert!(sheet.is_open());
    }

    #[test]
    fn bottom_sheet_dismiss_twice_noop() {
        let mut sheet = make_sheet();
        sheet.dismiss(); // no-op when already closed
        assert!(!sheet.is_open());

        sheet.open();
        sheet.dismiss();
        sheet.dismiss(); // no-op when already closed
        assert!(!sheet.is_open());
    }

    #[test]
    fn bottom_sheet_svg_output_open() {
        let mut sheet = make_sheet();
        sheet.open();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("width=\"400\""));
        assert!(svg.contains("height=\"600\""));
    }

    #[test]
    fn bottom_sheet_svg_output_closed() {
        let mut sheet = make_sheet();
        let svg = render_to_svg(&mut sheet);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn bottom_sheet_content_height_default_is_half_geometry() {
        let sheet = BottomSheet::new(Rect::new(0, 0, 400, 800));
        assert_eq!(sheet.content_height, 400);
    }

    #[test]
    fn bottom_sheet_dismiss_via_overlay_click() {
        let mut sheet = make_sheet();
        sheet.open();

        // Click in the overlay area (above the sheet panel, in the top half)
        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(100, 50), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }

    #[test]
    fn bottom_sheet_dismiss_via_sheet_click() {
        let mut sheet = make_sheet();
        sheet.open();

        // Click in the sheet area (bottom half)
        let dismissed = Arc::new(AtomicBool::new(false));
        let d = dismissed.clone();
        sheet.dismissed.connect(move || {
            d.store(true, Ordering::SeqCst);
        });

        sheet.handle_event(&Event::MousePress { pos: Point::new(200, 450), button: 1 });
        assert!(!sheet.is_open());
        assert!(dismissed.load(Ordering::SeqCst));
    }
}
