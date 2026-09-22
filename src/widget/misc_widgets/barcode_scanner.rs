// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! BarcodeScanner widget — simulated barcode/QR scanner viewfinder.
//!
//! Self-drawn simulation: renders a viewfinder with corner brackets, a sweeping
//! scan line and a result overlay. It does not connect to a camera and does not
//! decode barcode images. Scan results are injected externally via
//! [`BarcodeScanner::detect_barcode`]; without such an injection the widget
//! never reports a detection.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::ControlMetrics;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// The height of the scan-result overlay band pinned to the viewfinder's bottom: 50.
///
/// Two 13 px text lines plus the air around them; a fixed band rather than a fraction of the
/// control, so the readout does not stretch with a taller viewfinder.
const BARCODE_OVERLAY_HEIGHT: u32 = 50;

/// The inset of the viewfinder's own chrome (the status dot, the overlay text) from its edges: 10.
const BARCODE_PADDING: i32 = 10;
use crate::{impl_widget_property_hooks, property_names_of};

/// Barcode format types supported by the scanner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarcodeFormat {
    /// QR Code (ISO/IEC 18004).
    QRCode,
    /// Code 128 (ISO/IEC 15417).
    Code128,
    /// Code 39 (ISO/IEC 16388).
    Code39,
    /// EAN-13 (European Article Number, 13 digits).
    EAN13,
    /// EAN-8 (European Article Number, 8 digits).
    EAN8,
    /// UPC-A (Universal Product Code, 12 digits).
    UPCA,
    /// Data Matrix (ISO/IEC 16022).
    DataMatrix,
    /// PDF417 (ISO/IEC 15438).
    PDF417,
}

impl BarcodeFormat {
    /// Returns a human-readable name for the format.
    pub fn name(&self) -> &'static str {
        match self {
            BarcodeFormat::QRCode => "QR Code",
            BarcodeFormat::Code128 => "Code 128",
            BarcodeFormat::Code39 => "Code 39",
            BarcodeFormat::EAN13 => "EAN-13",
            BarcodeFormat::EAN8 => "EAN-8",
            BarcodeFormat::UPCA => "UPC-A",
            BarcodeFormat::DataMatrix => "Data Matrix",
            BarcodeFormat::PDF417 => "PDF417",
        }
    }
}

/// Result of a barcode scan operation.
#[derive(Debug, Clone)]
pub struct BarcodeResult {
    /// Decoded data string from the barcode.
    pub data: String,
    /// Format of the scanned barcode.
    pub format: BarcodeFormat,
    /// Timestamp (ms since epoch) when the barcode was scanned.
    pub timestamp: u64,
}

/// Barcode/QR code scanner widget (simulated).
///
/// Draws a viewfinder with corner brackets and an animated scan line while
/// scanning. No image decoding or device capture is performed: `last_result`
/// is populated only by `detect_barcode`, so the widget acts as a display and
/// control surface for an external decoding integration.
pub struct BarcodeScanner {
    base: BaseWidget,
    /// Whether the scanner is actively scanning.
    is_scanning: bool,
    /// Last barcode result injected via `detect_barcode`.
    last_result: Option<BarcodeResult>,
    /// Interval in milliseconds between simulated scan-line sweeps; drives the
    /// viewfinder animation speed only (no real scanning is performed).
    scan_interval: u64,
    /// Whether to show the viewfinder corner brackets.
    show_viewfinder: bool,
    /// Emitted when a barcode is detected.
    pub barcode_detected: Signal1<BarcodeResult>,
}

impl BarcodeScanner {
    /// Creates a new BarcodeScanner widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::BarcodeScanner, geometry, "BarcodeScanner"),
            is_scanning: false,
            last_result: None,
            scan_interval: 100,
            show_viewfinder: true,
            barcode_detected: Signal1::new(),
        }
    }

    /// Starts the simulated scanning animation (local state and redraw only).
    ///
    /// No camera is opened and no decoding begins.
    pub fn start_scanning(&mut self) {
        self.is_scanning = true;
        self.base.request_redraw();
    }

    /// Stops the simulated scanning animation (local state and redraw only).
    pub fn stop_scanning(&mut self) {
        self.is_scanning = false;
        self.base.request_redraw();
    }

    /// Returns whether the scanner is currently scanning.
    pub fn is_scanning(&self) -> bool {
        self.is_scanning
    }

    /// Toggles scanning on/off.
    pub fn toggle_scanning(&mut self) {
        if self.is_scanning {
            self.stop_scanning();
        } else {
            self.start_scanning();
        }
    }

    /// Sets the scan-sweep interval in milliseconds for the animation.
    pub fn set_scan_interval(&mut self, ms: u64) {
        self.scan_interval = ms.max(10);
    }

    /// Returns the current scan-sweep interval in milliseconds.
    pub fn scan_interval(&self) -> u64 {
        self.scan_interval
    }

    /// Returns the last injected barcode result, if any.
    ///
    /// This widget performs no real decoding, so this is only ever populated by
    /// [`Self::detect_barcode`].
    pub fn last_result(&self) -> Option<&BarcodeResult> {
        self.last_result.as_ref()
    }

    /// Clears the last barcode result.
    pub fn clear_result(&mut self) {
        self.last_result = None;
        self.base.request_redraw();
    }

    /// Injects a simulated barcode detection with the given data and format.
    ///
    /// This widget cannot decode images on its own; `detect_barcode` is the
    /// only way a result enters the widget (an external decoder/integration is
    /// expected to call it). The result is stored, emitted on
    /// `barcode_detected`, and shown in the overlay.
    pub fn detect_barcode(&mut self, data: String, format: BarcodeFormat) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        let result = BarcodeResult { data, format, timestamp };
        self.last_result = Some(result.clone());
        self.barcode_detected.emit(result);
        self.base.request_redraw();
    }

    /// Shows or hides the viewfinder overlay.
    pub fn set_show_viewfinder(&mut self, show: bool) {
        self.show_viewfinder = show;
        self.base.request_redraw();
    }

    /// Returns whether the viewfinder overlay is shown.
    pub fn show_viewfinder(&self) -> bool {
        self.show_viewfinder
    }
}

impl Widget for BarcodeScanner {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 100)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `BarcodeScanner`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch: the bool
/// drives `start_scanning()` / `stop_scanning()` rather than a field write.
impl WidgetProperties for BarcodeScanner {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "is_scanning" => Ok(CapabilityValue::Bool(self.is_scanning())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "is_scanning" => {
                if expect_bool(value)? {
                    self.start_scanning();
                } else {
                    self.stop_scanning();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["is_scanning", BASE_PROPERTY_NAMES]
    }
}

impl Draw for BarcodeScanner {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let w = rect.width as i32;
        let h = rect.height as i32;

        if w <= 0 || h <= 0 {
            return;
        }

        // Create fonts
        let small_font = Font::new("sans-serif", 11.0, false, false);
        let normal_font = Font::new("sans-serif", 13.0, false, false);

        // Every colour below is chrome, not content: this widget is a *simulated*
        // viewfinder that connects to no camera (see the type's own docs), so the
        // dark backdrop, the brackets, the sweep line, the overlay and the status
        // dot are all the control's own decoration. They resolve explicit style
        // first, then the theme's resolved style for this control, and only then a
        // literal. Previously each was hardcoded, so light and dark were identical.
        //
        // `resolved_theme_style` and `semantic_color` each take and release the
        // global manager's lock internally, so no guard is held across the draw (the
        // mutex is not re-entrant).
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("barcode_scanner");
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::WHITE);
        // The viewfinder surface is the control's backdrop. In light it is a light
        // surface and in dark a dark one, so the whole widget moves with the
        // appearance instead of being pinned to one hardcoded near-black.
        let surface = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        // The viewfinder well is a recessed area of the surface: the text colour
        // nudged toward it, so it reads as inset in either appearance.
        let viewfinder_fill = surface.blend(&text_color, 0.06);
        // The letterbox around the viewfinder is the surface dimmed, not a fixed
        // black at half opacity.
        let letterbox = surface.blend(&Color::BLACK, 0.45);
        let vf_border = surface.blend(&text_color, 0.35);
        // The brackets read as the scanner's "ready" accent, and the sweep line as
        // the active state, so each reads its own semantic token.
        let bracket_color = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .unwrap_or(Color::GREEN);
        let scan_line_color = crate::style::semantic_color(crate::style::SemanticColor::Success)
            .unwrap_or(Color::GREEN);

        // Background
        context.fill_rect(rect, surface);

        // Viewfinder area (centered, 80% of widget size)
        let vf_margin_x = (w as f32 * 0.1) as i32;
        let vf_margin_y = (h as f32 * 0.15) as i32;
        let vf_rect = Rect::new(
            rect.x + vf_margin_x,
            rect.y + vf_margin_y,
            (w - vf_margin_x * 2) as u32,
            (h - vf_margin_y * 2) as u32,
        );

        // Darken area outside viewfinder
        if vf_margin_y > 0 {
            context.fill_rect(Rect::new(rect.x, rect.y, w as u32, vf_margin_y as u32), letterbox);
        }
        let vf_bottom = rect.y + vf_margin_y + vf_rect.height as i32;
        if vf_bottom < rect.y + h {
            context.fill_rect(
                Rect::new(rect.x, vf_bottom, w as u32, (rect.y + h - vf_bottom) as u32),
                letterbox,
            );
        }
        if vf_margin_x > 0 {
            context.fill_rect(
                Rect::new(rect.x, rect.y + vf_margin_y, vf_margin_x as u32, vf_rect.height),
                letterbox,
            );
        }
        let vf_right = rect.x + vf_margin_x + vf_rect.width as i32;
        if vf_right < rect.x + w {
            context.fill_rect(
                Rect::new(
                    vf_right,
                    rect.y + vf_margin_y,
                    (rect.x + w - vf_right) as u32,
                    vf_rect.height,
                ),
                letterbox,
            );
        }

        // Viewfinder inner area
        context.fill_rect(vf_rect, viewfinder_fill);
        context.draw_rect_stroke(vf_rect, vf_border, 1);

        // Corner brackets
        if self.show_viewfinder {
            let bracket_len = 20;

            // Top-left corner
            context.draw_line(
                Point::new(vf_rect.x, vf_rect.y + bracket_len),
                Point::new(vf_rect.x, vf_rect.y),
                bracket_color,
            );
            context.draw_line(
                Point::new(vf_rect.x, vf_rect.y),
                Point::new(vf_rect.x + bracket_len, vf_rect.y),
                bracket_color,
            );

            // Top-right corner
            context.draw_line(
                Point::new(vf_rect.x + vf_rect.width as i32, vf_rect.y),
                Point::new(vf_rect.x + vf_rect.width as i32 - bracket_len, vf_rect.y),
                bracket_color,
            );
            context.draw_line(
                Point::new(vf_rect.x + vf_rect.width as i32, vf_rect.y),
                Point::new(vf_rect.x + vf_rect.width as i32, vf_rect.y + bracket_len),
                bracket_color,
            );

            // Bottom-left corner
            context.draw_line(
                Point::new(vf_rect.x, vf_rect.y + vf_rect.height as i32),
                Point::new(vf_rect.x, vf_rect.y + vf_rect.height as i32 - bracket_len),
                bracket_color,
            );
            context.draw_line(
                Point::new(vf_rect.x, vf_rect.y + vf_rect.height as i32),
                Point::new(vf_rect.x + bracket_len, vf_rect.y + vf_rect.height as i32),
                bracket_color,
            );

            // Bottom-right corner
            context.draw_line(
                Point::new(vf_rect.x + vf_rect.width as i32, vf_rect.y + vf_rect.height as i32),
                Point::new(
                    vf_rect.x + vf_rect.width as i32 - bracket_len,
                    vf_rect.y + vf_rect.height as i32,
                ),
                bracket_color,
            );
            context.draw_line(
                Point::new(vf_rect.x + vf_rect.width as i32, vf_rect.y + vf_rect.height as i32),
                Point::new(
                    vf_rect.x + vf_rect.width as i32,
                    vf_rect.y + vf_rect.height as i32 - bracket_len,
                ),
                bracket_color,
            );
        }

        // Scanning animation line (when active)
        if self.is_scanning {
            // Animation speed is proportional to scan_interval:
            // shorter interval = faster sweep, longer interval = slower sweep.
            // Default scan_interval (100) / 5 = 20, matching the previous hardcoded value.
            let speed_divisor = (self.scan_interval / 5).max(1);
            let sweep_span = (vf_rect.height as u64).saturating_sub(20).max(1);
            let scan_line_y = vf_rect.y
                + 10
                + ((std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
                    / speed_divisor)
                    % sweep_span) as i32;
            context.draw_line_stroke(
                Point::new(vf_rect.x + 4, scan_line_y),
                Point::new(vf_rect.x + vf_rect.width as i32 - 4, scan_line_y),
                scan_line_color.with_alpha(200),
                2,
            );
        }

        // Draw detected result overlay
        if let Some(ref result) = self.last_result {
            // The result overlay is a fixed-height band pinned to the bottom of the viewfinder,
            // not a fraction of it: its two text lines and their insets are the control's own
            // chrome, so a taller viewfinder does not stretch the readout. The band is clamped
            // to the control, so a very short viewfinder keeps it inside its own rectangle.
            let overlay_height = BARCODE_OVERLAY_HEIGHT.min(h as u32);
            let overlay_rect = ControlMetrics::bottom_band(rect, overlay_height);
            // The overlay is a scrim over whatever is behind it, so it is the
            // surface dimmed rather than a fixed black.
            context.fill_rect(overlay_rect, surface.blend(&Color::BLACK, 0.78));

            let format_text = format!("[{}]", result.format.name());
            let format_line = context.text_line(overlay_rect, &small_font);
            context.draw_text(
                Point::new(rect.x + BARCODE_PADDING, format_line.y),
                &format_text,
                &small_font,
                bracket_color,
                HorizontalAlignment::Left,
            );

            let display_data = if result.data.len() > 30 {
                format!("{}...", &result.data[..30])
            } else {
                result.data.clone()
            };
            // The second line occupies the overlay's lower half, so the two lines cannot
            // overlap whatever the overlay was clamped to.
            let data_band = Rect::new(
                overlay_rect.x,
                overlay_rect.y + (overlay_rect.height / 2) as i32,
                overlay_rect.width,
                overlay_rect.height / 2,
            );
            let data_line = context.text_line(data_band, &normal_font);
            context.draw_text(
                Point::new(rect.x + BARCODE_PADDING, data_line.y),
                &display_data,
                &normal_font,
                text_color,
                HorizontalAlignment::Left,
            );
        }

        // Status indicator: "scanning" is the success state, "idle" the error one,
        // so each reads its own semantic token instead of a fixed green/red.
        let status_color = if self.is_scanning {
            crate::style::semantic_color(crate::style::SemanticColor::Success)
                .unwrap_or(Color::GREEN)
        } else {
            crate::style::semantic_color(crate::style::SemanticColor::Error).unwrap_or(Color::RED)
        };
        let dot_rect = Rect::new(rect.x + 6, rect.y + 6, 8, 8);
        context.fill_rect(dot_rect, status_color);
    }
}

impl EventHandler for BarcodeScanner {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                // The press must land on the scanner. Discarding the position meant a
                // click anywhere in the window started or stopped scanning.
                if *button == 1 && self.geometry().contains_point(*pos) {
                    self.toggle_scanning();
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn barcode_scanner_default_state() {
        let bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        assert!(!bs.is_scanning());
        assert!(bs.last_result().is_none());
        assert_eq!(bs.scan_interval(), 100);
        assert!(bs.show_viewfinder());
        assert_eq!(bs.kind(), WidgetKind::BarcodeScanner);
    }

    #[test]
    fn barcode_scanner_toggle_scanning() {
        let mut bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        assert!(!bs.is_scanning());
        bs.start_scanning();
        assert!(bs.is_scanning());
        bs.stop_scanning();
        assert!(!bs.is_scanning());
        bs.toggle_scanning();
        assert!(bs.is_scanning());
        bs.toggle_scanning();
        assert!(!bs.is_scanning());
    }

    #[test]
    fn barcode_scanner_detect_and_clear() {
        let mut bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        assert!(bs.last_result().is_none());

        bs.detect_barcode("HelloWorld".to_string(), BarcodeFormat::QRCode);
        let result = bs.last_result().unwrap();
        assert_eq!(result.data, "HelloWorld");
        assert_eq!(result.format, BarcodeFormat::QRCode);

        bs.clear_result();
        assert!(bs.last_result().is_none());
    }

    #[test]
    fn barcode_scanner_detect_triggers_signal() {
        let mut bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        let detected = Arc::new(AtomicBool::new(false));
        let d = detected.clone();
        bs.barcode_detected.connect(move |_result| {
            d.store(true, Ordering::SeqCst);
        });

        bs.detect_barcode("TestData".to_string(), BarcodeFormat::Code128);
        assert!(detected.load(Ordering::SeqCst));
    }

    #[test]
    fn barcode_scanner_set_scan_interval() {
        let mut bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        bs.set_scan_interval(500);
        assert_eq!(bs.scan_interval(), 500);
        bs.set_scan_interval(5); // Should clamp to 10
        assert_eq!(bs.scan_interval(), 10);
    }

    #[test]
    fn barcode_scanner_viewfinder_visibility() {
        let mut bs = BarcodeScanner::new(Rect::new(0, 0, 300, 200));
        assert!(bs.show_viewfinder());
        bs.set_show_viewfinder(false);
        assert!(!bs.show_viewfinder());
        bs.set_show_viewfinder(true);
        assert!(bs.show_viewfinder());
    }

    #[test]
    fn barcode_format_names() {
        assert_eq!(BarcodeFormat::QRCode.name(), "QR Code");
        assert_eq!(BarcodeFormat::Code128.name(), "Code 128");
        assert_eq!(BarcodeFormat::Code39.name(), "Code 39");
        assert_eq!(BarcodeFormat::EAN13.name(), "EAN-13");
        assert_eq!(BarcodeFormat::EAN8.name(), "EAN-8");
        assert_eq!(BarcodeFormat::UPCA.name(), "UPC-A");
        assert_eq!(BarcodeFormat::DataMatrix.name(), "Data Matrix");
        assert_eq!(BarcodeFormat::PDF417.name(), "PDF417");
    }
}
