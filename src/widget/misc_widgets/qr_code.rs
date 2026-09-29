// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! QRCode widget — displays a QR-code-*like* pattern generated from a data string.
//!
//! # What this control is, precisely
//!
//! It renders a **deterministic decorative matrix**: the data string is hashed, the hash seeds an
//! LCG, and the bits fill a 21×21 grid around three finder patterns. The same input always gives
//! the same picture, which is what makes it useful for a mock-up, a placeholder or a visual test
//! fixture.
//!
//! **It is not a QR encoder.** The matrix carries no Reed–Solomon codewords, no format or version
//! information and no mode encoding, so a camera cannot decode it — the symbol is a picture *of* a
//! QR code, not a QR code. Two consequences a caller must know:
//!
//! * There is **no error-correction level to configure**, because there is no error correction. A
//!   property for one would be a control that accepts a setting and ignores it, which is the
//!   "reported success for something that did not happen" this crate refuses. A real EC level is a
//!   property of an *encoder*, and adding one means adding an encoder (see below), not a field.
//! * A host that needs a **scannable** symbol must encode it itself and hand the control the
//!   module grid — or use a QR crate — because no amount of drawing here can make this matrix
//!   decodable. That is stated here rather than discovered by a user pointing a phone at it.
//!
//! The plan (`blue23.md` A.8.4) lists "error-correction level" as an M10 gap for this control. It is
//! **withdrawn** for the reason above, not deferred: implementing it would mean writing a QR
//! encoder, which is a new capability rather than a thicker contract on an existing one.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::{Color, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Size of the QR code matrix (rows × columns).
const MATRIX_SIZE: u32 = 21;

/// Side of a finder pattern (and of its separator ring) in modules.
const FINDER_SIZE: u32 = 7;

/// Returns the top-left module of the finder block a cell belongs to, together with
/// whether the corner-local coordinates must be mirrored on each axis.
///
/// Each finder pattern owns an 8×8 block: the 7×7 finder itself plus a one-module light
/// separator on its inner edges. The three blocks are the top-left corner, the top-right
/// corner (growing leftward) and the bottom-left corner (growing upward). A cell outside
/// every block is data and returns `None`.
fn finder_corner(row: u32, col: u32) -> Option<(u32, u32, bool, bool)> {
    let far = MATRIX_SIZE - FINDER_SIZE - 1;
    let outer = FINDER_SIZE + 1;
    if row < outer && col < outer {
        Some((0, 0, false, false))
    } else if row < outer && col >= far {
        Some((0, far, false, true))
    } else if row >= far && col < outer {
        Some((far, 0, true, false))
    } else {
        None
    }
}

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

    /// Returns the size of the **quiet zone** — the light margin around the symbol — in
    /// modules.
    ///
    /// The quiet zone is a first-class parameter of a QR symbol, not decoration: the
    /// specification requires a margin of at least four modules for a decoder to locate the
    /// finder patterns, and both `qrencode`'s `margin` and every other QR library expose it. This
    /// control had the field and drew with it, but published no way to read or set it, so a
    /// caller could not satisfy a scanner that needed a wider margin.
    pub fn quiet_zone(&self) -> u32 {
        self.quiet_zone
    }

    /// Sets the quiet-zone width in modules.
    ///
    /// Zero is accepted and means "no margin" — legitimate when the caller is composing the
    /// symbol into a layout that supplies its own padding. The value is stored as given rather
    /// than clamped to the specification's minimum four: this control draws a symbol, and
    /// refusing a margin a caller explicitly asked for would be a different kind of wrong. The
    /// default is two, unchanged, so existing renders are unaffected.
    pub fn set_quiet_zone(&mut self, modules: u32) {
        self.quiet_zone = modules;
        self.base.request_redraw();
    }

    /// Returns the rendered side length of the whole symbol in pixels,
    /// including the quiet zone on both edges.
    pub fn size(&self) -> u32 {
        (MATRIX_SIZE + self.quiet_zone * 2) * self.module_size
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
                // The three finder patterns are 7x7 squares in the top-left, top-right
                // and bottom-left corners, each separated from the data area by a
                // one-module light separator (row/column 7 from its own corner).
                //
                // The shape test has to run in **corner-local** coordinates: testing the
                // absolute `row`/`col` only ever matches the top-left corner, so the other
                // two corners used to be filled with data modules instead of a finder
                // pattern -- three visually different corners in a widget whose whole
                // purpose is to look like a QR symbol. `local` therefore reports the
                // distance from whichever corner the cell belongs to.
                let corner = finder_corner(row, col);
                if let Some((base_row, base_col, flip_row, flip_col)) = corner {
                    // Corner-local coordinates, measured from the **inner** corner of the
                    // block: a mirrored finder is congruent to the top-left one, which is
                    // what makes all three corners the same shape. Without the mirror the
                    // other two would be reflections, not copies.
                    let mut local_row = (row - base_row) as usize;
                    let mut local_col = (col - base_col) as usize;
                    if flip_row {
                        local_row = FINDER_SIZE as usize - local_row;
                    }
                    if flip_col {
                        local_col = FINDER_SIZE as usize - local_col;
                    }
                    // The separator is the 8th row/column of the corner block. It is
                    // painted light (it is a background cell), and the data generator must
                    // not be advanced for it either -- otherwise the data area's bit
                    // stream would depend on where the finder patterns happen to be.
                    if local_row == 7 || local_col == 7 {
                        matrix[row as usize][col as usize] = false;
                        continue;
                    }
                    // Draw finder pattern: 7x7 ring with a 3x3 solid centre.
                    let is_outer =
                        local_row == 0 || local_row == 6 || local_col == 0 || local_col == 6;
                    let is_inner = (2..=4).contains(&local_row) && (2..=4).contains(&local_col);
                    matrix[row as usize][col as usize] = is_outer || is_inner;
                    continue;
                }

                // Fill the remaining area with deterministic pseudo-random bits
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                matrix[row as usize][col as usize] = (state >> ((col % 8) * 8)) & 1 == 1;
            }
        }

        matrix
    }
}

impl Widget for QRCode {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        let side = crate::widget::metrics::dimensions::QR_CODE_SIZE;
        crate::core::Size::new(side, side)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `QRCode`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `size` reports the rendered
/// symbol extent in pixels; a write is carried to the module size, which is the
/// only knob the matrix has, and the value then reads back rounded to a whole
/// number of modules. Negative writes are clamped to one pixel per module rather
/// than rejected, matching `set_module_size`.
impl WidgetProperties for QRCode {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "data" => Ok(CapabilityValue::String(self.data().to_string())),
            "size" => Ok(CapabilityValue::UInt(self.size() as u64)),
            "quiet_zone" => Ok(CapabilityValue::UInt(self.quiet_zone() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "data" => {
                self.set_data(&expect_string(value)?);
                Ok(())
            }
            "size" => {
                let requested = expect_usize(value)? as u32;
                let module = (requested / (MATRIX_SIZE + self.quiet_zone * 2)).max(1);
                self.set_module_size(module);
                Ok(())
            }
            "quiet_zone" => {
                self.set_quiet_zone(expect_usize(value)? as u32);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["data", "size", "quiet_zone", BASE_PROPERTY_NAMES]
    }
}

impl Draw for QRCode {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let module = self.module_size.max(1);
        let total_modules = MATRIX_SIZE + self.quiet_zone * 2;
        let total_pixels = total_modules * module;

        // The symbol is a contrasting module/surface pair, and *both* members come
        // from the theme: modules are the resolved text colour, the quiet zone is the
        // resolved background. That keeps the symbol scannable in either appearance
        // while making it respond to a theme switch; hardcoding white/black meant the
        // quiet zone stayed white on a dark window.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("qr_code");
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let module_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);

        // Center the symbol in the available geometry, then **clamp the origin once** and use
        // that single value for both the quiet-zone face and every module.
        //
        // The face was clamped (`offset_x.max(rect.x)`) while the module loop used the raw
        // `offset_x`, so a symbol larger than its control painted its modules outside the light
        // margin it had just drawn. The two must not be derived separately: the margin only
        // reads as a margin if the modules sit inside the same rectangle.
        let available_x = (rect.width.saturating_sub(total_pixels) / 2) as i32;
        let available_y = (rect.height.saturating_sub(total_pixels) / 2) as i32;
        let symbol_x = (rect.x + available_x).max(rect.x);
        let symbol_y = (rect.y + available_y).max(rect.y);
        let symbol_side = total_pixels.min(rect.width).min(rect.height);

        // Draw the quiet-zone surface for the entire symbol area.
        context.fill_rect(Rect::new(symbol_x, symbol_y, symbol_side, symbol_side), surface);

        let matrix = self.generate_matrix();

        // Draw each module.
        for row in 0..MATRIX_SIZE {
            for col in 0..MATRIX_SIZE {
                let x = symbol_x + (self.quiet_zone + col) as i32 * module as i32;
                let y = symbol_y + (self.quiet_zone + row) as i32 * module as i32;

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
                    context.fill_rect(cell_rect, module_color);
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

    /// All three finder patterns must be the **same shape**. Only the top-left corner
    /// used to be drawn; the other two were filled with data modules, so the symbol
    /// had one recognisable corner and two that looked like noise.
    #[test]
    fn all_three_finder_patterns_are_identical() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("finder-shape");
        let matrix = qr.generate_matrix();

        let n = MATRIX_SIZE as usize;
        let top_left: Vec<bool> =
            (0..7).flat_map(|r| (0..7).map(move |c| (r, c))).map(|(r, c)| matrix[r][c]).collect();
        let top_right: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| matrix[r][n - 1 - c])
            .collect();
        let bottom_left: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| matrix[n - 1 - r][c])
            .collect();

        assert_eq!(top_left, top_right, "the top-right finder mirrors the top-left");
        assert_eq!(top_left, bottom_left, "the bottom-left finder mirrors the top-left");

        // And the canonical 7x7 shape: a dark ring, a light gap, a 3x3 dark core.
        let expected: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| {
                let outer = r == 0 || r == 6 || c == 0 || c == 6;
                let inner = (2..=4).contains(&r) && (2..=4).contains(&c);
                outer || inner
            })
            .collect();
        assert_eq!(top_left, expected, "the finder is a ring plus a solid centre");
    }

    /// The separator ring between a finder pattern and the data area is **light**, so a
    /// decoder (and a reader) can tell where the 7x7 block ends. It used to be computed
    /// and thrown away.
    #[test]
    fn finder_patterns_are_ringed_by_a_light_separator() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("separator");
        let matrix = qr.generate_matrix();
        let n = MATRIX_SIZE as usize;

        for (r, c) in (0..8).flat_map(|r| (0..8).map(move |c| (r, c))) {
            if r == 7 || c == 7 {
                assert!(!matrix[r][c], "top-left separator at ({r},{c}) must be light");
            }
        }
        for c in 0..8 {
            assert!(!matrix[7][c]);
            assert!(!matrix[n - 1 - c][7], "bottom-left separator is light");
            assert!(!matrix[7][n - 1 - c], "top-right separator is light");
        }
    }
}
