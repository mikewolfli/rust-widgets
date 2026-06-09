//! QRCode widget — displays a simple QR code pattern generated from a data string.
//!
//! This widget renders a deterministic black/white matrix by hashing the input
//! data string to produce a 21×21 QR-code-like minimatrix. Each cell is drawn
//! as a small filled rectangle.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Size of the QR code matrix (rows × columns).
const MATRIX_SIZE: u32 = 21;

/// QRCode widget that renders a deterministic QR-like pattern.
pub struct QRCode {
    base: BaseWidget,
    data: String,
    /// Size of each module (cell) in logical pixels.
    module_size: u32,
    /// Quiet zone (white border) around the matrix in modules.
    quiet_zone: u32,
}

impl QRCode {
    /// Creates a new QRCode widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::QRCode, geometry, "QRCode"),
            data: String::new(),
            module_size: 4,
            quiet_zone: 2,
        }
    }

    /// Returns a reference to the current data string.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Sets the data string used to generate the QR code pattern.
    pub fn set_data(&mut self, data: &str) {
        self.data = data.to_string();
        self.base.request_redraw();
    }

    /// Returns the current module size in pixels.
    pub fn module_size(&self) -> u32 {
        self.module_size
    }

    /// Sets the size of each module (cell) in logical pixels.
    pub fn set_module_size(&mut self, size: u32) {
        self.module_size = size.max(1);
        self.base.request_redraw();
    }

    /// Generate a deterministic matrix where `true` = black, `false` = white.
    fn generate_matrix(&self) -> [[bool; MATRIX_SIZE as usize]; MATRIX_SIZE as usize] {
        let mut matrix = [[false; MATRIX_SIZE as usize]; MATRIX_SIZE as usize];

        // Use the data string to seed a deterministic hash.
        let mut hasher = DefaultHasher::new();
        self.data.hash(&mut hasher);
        let seed = hasher.finish();

        // Fill the matrix using a simple LCG (linear congruential generator)
        // seeded from the data hash so the same data always produces the same pattern.
        let mut state = seed;
        for row in 0..MATRIX_SIZE {
            for col in 0..MATRIX_SIZE {
                // Skip finder patterns (top-left, top-right, bottom-left corners)
                let in_finder = (row < 7 && col < 7)
                    || (row < 7 && col >= MATRIX_SIZE - 7)
                    || (row >= MATRIX_SIZE - 7 && col < 7);

                if in_finder {
                    // Draw finder pattern: 7x7 with a 3x3 inner black square
                    let is_outer = row == 0 || row == 6 || col == 0 || col == 6;
                    let is_inner = row >= 2 && row <= 4 && col >= 2 && col <= 4;
                    let _is_sep = row == 7
                        || col == 7
                        || (row < 7 && col == MATRIX_SIZE - 8)
                        || (row == MATRIX_SIZE - 8 && col < 7);

                    matrix[row as usize][col as usize] = is_outer || is_inner;
                    continue;
                }

                // Fill the remaining area with deterministic pseudo-random bits
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                matrix[row as usize][col as usize] = (state >> (col % 8) * 8) & 1 == 1;
            }
        }

        matrix
    }
}

impl Widget for QRCode {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for QRCode {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let module = self.module_size.max(1);
        let total_modules = MATRIX_SIZE + self.quiet_zone * 2;
        let total_pixels = total_modules * module;

        // Center the QR code in the available geometry.
        let offset_x = rect.x + (rect.width.saturating_sub(total_pixels) / 2) as i32;
        let offset_y = rect.y + (rect.height.saturating_sub(total_pixels) / 2) as i32;

        // Draw white background for the entire QR code area.
        let bg_rect = Rect::new(
            offset_x.max(rect.x),
            offset_y.max(rect.y),
            total_pixels.min(rect.width),
            total_pixels.min(rect.height),
        );
        context.fill_rect(bg_rect, Color::WHITE);

        let matrix = self.generate_matrix();

        // Draw each module.
        for row in 0..MATRIX_SIZE {
            for col in 0..MATRIX_SIZE {
                let x = offset_x + (self.quiet_zone + col) as i32 * module as i32;
                let y = offset_y + (self.quiet_zone + row) as i32 * module as i32;

                // Clamp to widget bounds.
                if x + module as i32 <= rect.x
                    || x >= rect.x + rect.width as i32
                    || y + module as i32 <= rect.y
                    || y >= rect.y + rect.height as i32
                {
                    continue;
                }

                let cell_w = module.min((rect.x + rect.width as i32 - x).max(0) as u32);
                let cell_h = module.min((rect.y + rect.height as i32 - y).max(0) as u32);
                if cell_w == 0 || cell_h == 0 {
                    continue;
                }

                if matrix[row as usize][col as usize] {
                    let cell_rect = Rect::new(x, y, cell_w, cell_h);
                    context.fill_rect(cell_rect, Color::BLACK);
                }
            }
        }
    }
}

impl EventHandler for QRCode {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn qr_code_creation() {
        let qr = QRCode::new(Rect::new(0, 0, 120, 120));
        assert!(qr.data().is_empty());
        assert_eq!(qr.module_size(), 4);
        assert_eq!(qr.kind(), WidgetKind::QRCode);
    }

    #[test]
    fn qr_code_set_data() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("Hello, World!");
        assert_eq!(qr.data(), "Hello, World!");
    }

    #[test]
    fn qr_code_set_module_size() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        assert_eq!(qr.module_size(), 4);
        qr.set_module_size(8);
        assert_eq!(qr.module_size(), 8);
        qr.set_module_size(0); // should clamp to 1
        assert_eq!(qr.module_size(), 1);
    }

    #[test]
    fn qr_code_deterministic_output() {
        let mut qr1 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr1.set_data("test-data");
        let mut qr2 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr2.set_data("test-data");
        // Two widgets with the same data should produce identical matrices.
        let m1 = qr1.generate_matrix();
        let m2 = qr2.generate_matrix();
        assert_eq!(m1, m2);
    }

    #[test]
    fn qr_code_different_data_different_output() {
        let mut qr1 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr1.set_data("data-a");
        let mut qr2 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr2.set_data("data-b");
        let m1 = qr1.generate_matrix();
        let m2 = qr2.generate_matrix();
        // Different data should produce different matrices (extremely likely).
        assert_ne!(m1, m2, "different data should produce different matrices");
    }

    #[test]
    fn qr_code_svg_output() {
        let mut qr = QRCode::new(Rect::new(0, 0, 100, 100));
        qr.set_data("QR SVG Test");
        let svg = crate::widget::svg::render_to_svg(&mut qr);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Should contain fill operations (white background + black modules).
        assert!(svg.contains("fill="));
    }

    #[test]
    fn qr_code_event_handler_delegates() {
        let mut qr = QRCode::new(Rect::new(0, 0, 100, 100));
        // Should not panic.
        qr.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        qr.handle_event(&Event::KeyDown((65, 0)));
    }
}
