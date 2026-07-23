//! PDF export module.
//!
//! Provides [`PdfExporter`] and [`export_to_pdf`] to render widget trees
//! into PDF documents by leveraging the SVG rendering pipeline.

use crate::core::{Rect, Size};
#[cfg(not(feature = "mini"))]
use crate::widget::svg::render_widget_to_svg;
use crate::widget::Draw;

/// Standard page sizes in points (1/72 inch).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PageSize {
    /// A4: 595.28 x 841.89 pt
    #[default]
    A4,
    /// US Letter: 612.0 x 792.0 pt
    Letter,
    /// Custom width/height in points.
    Custom { width: f32, height: f32 },
}

impl PageSize {
    /// Return the dimensions in points.
    pub fn dimensions(&self) -> (f32, f32) {
        match self {
            PageSize::A4 => (595.28, 841.89),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Custom { width, height } => (*width, *height),
        }
    }

    /// Return the width in points.
    pub fn width(&self) -> f32 {
        self.dimensions().0
    }

    /// Return the height in points.
    pub fn height(&self) -> f32 {
        self.dimensions().1
    }

    /// Convert to a `Size` using the given DPI to map from points to pixels.
    pub fn to_size(&self, dpi: u32) -> Size {
        let (w, h) = self.dimensions();
        let scale = dpi as f32 / 72.0;
        Size { width: (w * scale).round() as u32, height: (h * scale).round() as u32 }
    }
}

/// Export orientation for the PDF page.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PdfOrientation {
    /// Portrait (tall).
    #[default]
    Portrait,
    /// Landscape (wide).
    Landscape,
}

impl PdfOrientation {
    /// Apply the orientation to a page size, swapping dimensions for landscape.
    pub fn apply(&self, (w, h): (f32, f32)) -> (f32, f32) {
        match self {
            PdfOrientation::Portrait => (w, h),
            PdfOrientation::Landscape => (h, w),
        }
    }
}

/// Settings controlling PDF export.
#[derive(Debug, Clone)]
pub struct PdfExportSettings {
    /// Page size (A4, Letter, or Custom).
    pub page_size: PageSize,
    /// Page orientation.
    pub orientation: PdfOrientation,
    /// Margins in points (top, right, bottom, left).
    pub margins: [f32; 4],
    /// Output DPI for mapping between page-space and pixel-space.
    pub dpi: u32,
}

impl Default for PdfExportSettings {
    fn default() -> Self {
        Self {
            page_size: PageSize::A4,
            orientation: PdfOrientation::Portrait,
            margins: [56.0, 56.0, 56.0, 56.0], // ~20 mm
            dpi: 72,
        }
    }
}

impl PdfExportSettings {
    /// Create new settings with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the effective page dimensions after applying orientation.
    pub fn effective_dimensions(&self) -> (f32, f32) {
        self.orientation.apply(self.page_size.dimensions())
    }

    /// Return the width available for content after subtracting margins.
    pub fn content_width(&self) -> f32 {
        let (w, _) = self.effective_dimensions();
        w - self.margins[1] - self.margins[3]
    }

    /// Return the height available for content after subtracting margins.
    pub fn content_height(&self) -> f32 {
        let (_, h) = self.effective_dimensions();
        h - self.margins[0] - self.margins[2]
    }

    /// Convert the effective page dimensions to pixel `Size` using DPI.
    pub fn pixel_size(&self) -> Size {
        let (w, h) = self.effective_dimensions();
        let scale = self.dpi as f32 / 72.0;
        Size { width: (w * scale).round() as u32, height: (h * scale).round() as u32 }
    }
}

/// A single page in a PDF export, storing its SVG content and dimensions.
#[derive(Debug, Clone)]
pub struct ExportPage {
    /// Page index (0-based).
    pub index: u32,
    /// SVG string representing the page content.
    pub svg_content: String,
    /// Page width in points.
    pub width_pt: f32,
    /// Page height in points.
    pub height_pt: f32,
    /// Page width in pixels (at the configured DPI).
    pub width_px: u32,
    /// Page height in pixels (at the configured DPI).
    pub height_px: u32,
}

impl ExportPage {
    /// Create a new PDF page with the given SVG content and dimensions.
    pub fn new(
        index: u32,
        svg_content: String,
        width_pt: f32,
        height_pt: f32,
        width_px: u32,
        height_px: u32,
    ) -> Self {
        Self { index, svg_content, width_pt, height_pt, width_px, height_px }
    }
}

/// PDF exporter that converts widget renderings into PDF documents.
///
/// Uses the existing SVG rendering pipeline to capture widget content,
/// then wraps the SVG output in a minimal PDF structure.
#[derive(Debug)]
pub struct PdfExporter {
    /// Export settings controlling page layout.
    pub settings: PdfExportSettings,
}

impl PdfExporter {
    /// Create a new `PdfExporter` with default settings.
    pub fn new() -> Self {
        Self { settings: PdfExportSettings::new() }
    }

    /// Create a new `PdfExporter` with the given settings.
    pub fn with_settings(settings: PdfExportSettings) -> Self {
        Self { settings }
    }

    /// Export a slice of drawable widgets to a PDF file.
    ///
    /// Each widget is rendered via the SVG pipeline and placed on its own page.
    /// The resulting PDF is a minimal valid PDF-1.4 file with the SVG content
    /// embedded directly in the content streams.
    #[cfg(not(feature = "mini"))]
    pub fn export(&self, widgets: &mut [&mut dyn Draw], path: &str) -> Result<(), String> {
        let pages = self.render_pages(widgets)?;
        let pdf_bytes = build_svg_pdf(&pages, &self.settings)?;
        std::fs::write(path, &pdf_bytes)
            .map_err(|err| format!("failed to write PDF file '{path}': {err}"))?;
        Ok(())
    }

    /// Export requires the SVG pipeline (not available in mini mode).
    #[cfg(feature = "mini")]
    pub fn export(&self, _widgets: &mut [&mut dyn Draw], _path: &str) -> Result<(), String> {
        Err("PDF export requires the SVG pipeline which is not available in mini mode".to_string())
    }

    /// Render each widget into a [`ExportPage`] using the SVG pipeline.
    #[cfg(not(feature = "mini"))]
    pub fn render_pages(&self, widgets: &mut [&mut dyn Draw]) -> Result<Vec<ExportPage>, String> {
        let pixel_size = self.settings.pixel_size();
        let (page_w_pt, page_h_pt) = self.settings.effective_dimensions();
        let mut pages = Vec::with_capacity(widgets.len());

        for (idx, widget) in widgets.iter_mut().enumerate() {
            // Render the widget to SVG at the target pixel size
            let svg =
                render_widget_to_svg(*widget, Rect::new(0, 0, pixel_size.width, pixel_size.height));
            pages.push(ExportPage::new(
                idx as u32,
                svg,
                page_w_pt,
                page_h_pt,
                pixel_size.width,
                pixel_size.height,
            ));
        }

        Ok(pages)
    }

    /// Render pages requires the SVG pipeline (not available in mini mode).
    #[cfg(feature = "mini")]
    pub fn render_pages(&self, _widgets: &mut [&mut dyn Draw]) -> Result<Vec<ExportPage>, String> {
        Err("PDF export requires the SVG pipeline which is not available in mini mode".to_string())
    }
}

impl Default for PdfExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a minimal PDF byte stream embedding SVG content on each page.
///
/// This creates a valid PDF-1.4 file where each page's content stream contains
/// the SVG markup wrapped in a `q`/`Q` pair. The SVG is embedded directly,
/// making the output suitable for further processing or viewer consumption.
fn build_svg_pdf(pages: &[ExportPage], settings: &PdfExportSettings) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("at least one page is required".to_string());
    }

    let mut objects: Vec<Vec<u8>> = Vec::new();
    // Reserve slots for catalog (1) and pages tree (2)
    objects.push(Vec::new());
    objects.push(Vec::new());

    // Track object IDs for page references
    let mut page_obj_ids: Vec<u32> = Vec::new();

    for page in pages {
        // Page content stream object
        let content_stream = build_content_stream(page, settings);
        let content_obj_id = (objects.len() + 1) as u32;
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{}\nendstream",
                content_stream.len(),
                content_stream,
            )
            .into_bytes(),
        );

        // Page object
        let page_obj_id = (objects.len() + 1) as u32;
        let page_obj = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Contents {} 0 R >>",
            page.width_pt, page.height_pt, content_obj_id,
        );
        objects.push(page_obj.into_bytes());
        page_obj_ids.push(page_obj_id);
    }

    // Info object
    let info_obj_id = (objects.len() + 1) as u32;
    objects.push(
        b"<< /Title (Exported Document) /Creator (rust-widgets PdfExporter) /Producer (rust-widgets) >>"
            .to_vec(),
    );

    // Catalog (object 1)
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();

    // Pages tree (object 2)
    let kids = page_obj_ids.iter().map(|id| format!("{id} 0 R")).collect::<Vec<_>>().join(" ");
    objects[1] =
        format!("<< /Type /Pages /Count {} /Kids [{}] >>", page_obj_ids.len(), kids,).into_bytes();

    // Assemble the final PDF
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets: Vec<usize> = Vec::new();
    for (idx, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let obj_id = idx + 1;
        out.extend_from_slice(format!("{obj_id} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            info_obj_id,
            xref_offset,
        )
        .as_bytes(),
    );

    Ok(out)
}

/// Build the content stream for a single PDF page, converting SVG content
/// into real PDF content operators so viewers render the content visually.
fn build_content_stream(page: &ExportPage, _settings: &PdfExportSettings) -> String {
    let mut stream = String::new();

    // Save graphics state and apply a scaling transform so the SVG (rendered
    // at the pixel size) maps linearly onto the page's point dimensions.
    stream.push_str(&format!(
        "q\n{:.4} 0 0 {:.4} 0 0 cm\n",
        page.width_pt / page.width_px as f32,
        page.height_pt / page.height_px as f32,
    ));

    // Convert SVG content to real PDF drawing operators
    stream.push_str(&svg_to_pdf_operators(&page.svg_content));

    // Restore graphics state
    stream.push_str("Q\n");

    stream
}

/// Convert SVG content to PDF content-stream operators.
///
/// Parses basic SVG primitives (`rect`, `circle`, `path`) and emits
/// the corresponding PDF operators so the content renders visually.
fn svg_to_pdf_operators(svg: &str) -> String {
    let mut pdf = String::new();

    // Scan for SVG elements anywhere in the content (may be nested or on same line).
    let mut pos = 0;
    while let Some(tag_start) = svg[pos..].find('<') {
        let abs_start = pos + tag_start;
        if abs_start + 1 >= svg.len() {
            break;
        }
        // Find the end of this element (> or />)
        let tag_end = match svg[abs_start..].find('>') {
            Some(e) => abs_start + e + 1,
            None => break,
        };
        let element = &svg[abs_start..tag_end];
        pos = tag_end;

        if element.starts_with("<rect") {
            let mut x = 0f32;
            let mut y = 0f32;
            let mut w = 0f32;
            let mut h = 0f32;
            let mut fill: Option<(f32, f32, f32)> = None;
            let mut stroke: Option<(f32, f32, f32)> = None;
            let mut has_fill = false;
            let mut has_stroke = false;

            if let Some(val) = extract_attr(element, "x") {
                x = val.parse().unwrap_or(0.0);
            }
            if let Some(val) = extract_attr(element, "y") {
                y = val.parse().unwrap_or(0.0);
            }
            if let Some(val) = extract_attr(element, "width") {
                w = val.parse().unwrap_or(0.0);
            }
            if let Some(val) = extract_attr(element, "height") {
                h = val.parse().unwrap_or(0.0);
            }
            if let Some(val) = extract_attr(element, "fill") {
                has_fill = true;
                fill = parse_svg_color(&val);
            }
            if let Some(val) = extract_attr(element, "stroke") {
                has_stroke = true;
                stroke = parse_svg_color(&val);
            }

            if w > 0.0 && h > 0.0 {
                if let Some((r, g, b)) = fill {
                    pdf.push_str(&format!("{r:.4} {g:.4} {b:.4} rg\n"));
                }
                if let Some((r, g, b)) = stroke {
                    pdf.push_str(&format!("{r:.4} {g:.4} {b:.4} RG\n"));
                }
                pdf.push_str(&format!("{x:.2} {y:.2} {w:.2} {h:.2} re "));
                if has_fill && has_stroke {
                    pdf.push_str("B\n");
                } else if has_fill {
                    pdf.push_str("f\n");
                } else if has_stroke {
                    pdf.push_str("S\n");
                } else {
                    pdf.push_str("f\n");
                }
            }
        } else if element.starts_with("<circle") {
            let cx = extract_attr(element, "cx").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let cy = extract_attr(element, "cy").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            let r = extract_attr(element, "r").and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
            if r > 0.0 {
                if let Some(val) = extract_attr(element, "fill") {
                    if let Some((rg, g, b)) = parse_svg_color(&val) {
                        pdf.push_str(&format!("{rg:.4} {g:.4} {b:.4} rg\n"));
                    }
                }
                // Approximate circle with 4 cubic b\u00e9zier curves
                let k = r * 0.552_284_8; // 4/3 * (sqrt(2)-1)
                pdf.push_str(&format!(
                    "{} {} m {} {} {} {} {} {} c \
                     {} {} {} {} {} {} c \
                     {} {} {} {} {} {} c \
                     {} {} {} {} {} {} c f\n",
                    cx,
                    cy - r,
                    cx + k,
                    cy - r,
                    cx + r,
                    cy - k,
                    cx + r,
                    cy,
                    cx + r,
                    cy + k,
                    cx + k,
                    cy + r,
                    cx,
                    cy + r,
                    cx - k,
                    cy + r,
                    cx - r,
                    cy + k,
                    cx - r,
                    cy,
                    cx - r,
                    cy - k,
                    cx - k,
                    cy - r,
                    cx,
                    cy - r,
                ));
            }
        } else if element.starts_with("<path") {
            if let Some(d) = extract_attr(element, "d") {
                if let Some(val) = extract_attr(element, "fill") {
                    if let Some((rg, g, b)) = parse_svg_color(&val) {
                        pdf.push_str(&format!("{rg:.4} {g:.4} {b:.4} rg\n"));
                    }
                }
                pdf.push_str(&svg_path_to_pdf(&d));
            }
        }
    }

    pdf
}

/// Extract the value of an XML attribute by name using simple string search.
fn extract_attr(s: &str, name: &str) -> Option<String> {
    let pattern = format!("{name}=\"");
    if let Some(start) = s.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = s[val_start..].find('"') {
            return Some(s[val_start..val_start + end].to_string());
        }
    }
    None
}

/// Parse an SVG color string (#RRGGBB) into normalized RGB floats (0.0-1.0).
fn parse_svg_color(color: &str) -> Option<(f32, f32, f32)> {
    if color.starts_with('#') && color.len() == 7 {
        let r = u8::from_str_radix(&color[1..3], 16).ok()?;
        let g = u8::from_str_radix(&color[3..5], 16).ok()?;
        let b = u8::from_str_radix(&color[5..7], 16).ok()?;
        Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
    } else {
        None
    }
}

/// Convert a simplified SVG path `d` string to PDF path operators.
fn svg_path_to_pdf(d: &str) -> String {
    let mut pdf = String::new();
    let parts: Vec<&str> = d.split_whitespace().collect();
    let mut i = 0;
    while i < parts.len() {
        match parts[i] {
            "M" => {
                if i + 2 < parts.len() {
                    pdf.push_str(&format!("{} {} m\n", parts[i + 1], parts[i + 2]));
                    i += 3;
                } else {
                    i += 1;
                }
            }
            "L" => {
                if i + 2 < parts.len() {
                    pdf.push_str(&format!("{} {} l\n", parts[i + 1], parts[i + 2]));
                    i += 3;
                } else {
                    i += 1;
                }
            }
            "C" => {
                if i + 6 < parts.len() {
                    pdf.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        parts[i + 1],
                        parts[i + 2],
                        parts[i + 3],
                        parts[i + 4],
                        parts[i + 5],
                        parts[i + 6]
                    ));
                    i += 7;
                } else {
                    i += 1;
                }
            }
            "Z" | "z" => {
                pdf.push_str("h f\n");
                i += 1;
            }
            _ => i += 1,
        }
    }
    if !pdf.is_empty() && !pdf.ends_with("h f\n") {
        pdf.push_str("f\n");
    }
    pdf
}

/// Export a slice of drawable widgets to a PDF file.
///
/// This is a convenience function that creates a [`PdfExporter`] with default
/// settings, renders each widget via the SVG pipeline, and writes a minimal
/// PDF file with one page per widget.
///
/// # Errors
/// Returns `Err` if no widgets are provided, or if the file cannot be written.
#[cfg(not(feature = "mini"))]
pub fn export_to_pdf(widgets: &mut [&mut dyn Draw], path: &str) -> Result<(), String> {
    let exporter = PdfExporter::new();
    exporter.export(widgets, path)
}

/// PDF export requires the SVG pipeline (not available in mini mode).
#[cfg(feature = "mini")]
pub fn export_to_pdf(_widgets: &mut [&mut dyn Draw], _path: &str) -> Result<(), String> {
    Err("PDF export requires the SVG pipeline which is not available in mini mode".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use std::sync::{Arc, Mutex};

    /// A simple test widget that records draw calls.
    struct TestWidget {
        _geometry: Rect,
        draw_count: Arc<Mutex<u32>>,
    }

    impl TestWidget {
        fn new(width: u32, height: u32) -> Self {
            Self { _geometry: Rect::new(0, 0, width, height), draw_count: Arc::new(Mutex::new(0)) }
        }
    }

    impl Draw for TestWidget {
        fn draw(&mut self, _context: &mut crate::render::RenderContext) {
            *self.draw_count.lock().unwrap() += 1;
        }

        fn uses_custom_drawing(&self) -> bool {
            true
        }
    }

    #[test]
    fn pdf_exporter_default_settings() {
        let exporter = PdfExporter::new();
        assert_eq!(exporter.settings.page_size, PageSize::A4);
        assert_eq!(exporter.settings.orientation, PdfOrientation::Portrait);
        assert_eq!(exporter.settings.dpi, 72);
    }

    #[test]
    fn pdf_exporter_with_custom_settings() {
        let settings = PdfExportSettings {
            page_size: PageSize::Letter,
            orientation: PdfOrientation::Landscape,
            margins: [72.0, 72.0, 72.0, 72.0],
            dpi: 150,
        };
        let exporter = PdfExporter::with_settings(settings.clone());
        assert_eq!(exporter.settings.page_size, PageSize::Letter);
        assert_eq!(exporter.settings.orientation, PdfOrientation::Landscape);
        assert_eq!(exporter.settings.dpi, 150);
    }

    #[test]
    fn page_size_dimensions() {
        let (aw, ah) = PageSize::A4.dimensions();
        assert!((aw - 595.28).abs() < 0.01);
        assert!((ah - 841.89).abs() < 0.01);

        let (lw, lh) = PageSize::Letter.dimensions();
        assert!((lw - 612.0).abs() < 0.01);
        assert!((lh - 792.0).abs() < 0.01);

        let (cw, ch) = PageSize::Custom { width: 300.0, height: 400.0 }.dimensions();
        assert!((cw - 300.0).abs() < 0.01);
        assert!((ch - 400.0).abs() < 0.01);
    }

    #[test]
    fn orientation_applies_correctly() {
        let (w, h) = PdfOrientation::Portrait.apply((612.0, 792.0));
        assert!((w - 612.0).abs() < 0.01);
        assert!((h - 792.0).abs() < 0.01);

        let (w, h) = PdfOrientation::Landscape.apply((612.0, 792.0));
        assert!((w - 792.0).abs() < 0.01);
        assert!((h - 612.0).abs() < 0.01);
    }

    #[test]
    fn export_settings_content_area() {
        let settings = PdfExportSettings {
            page_size: PageSize::A4,
            orientation: PdfOrientation::Portrait,
            margins: [72.0, 72.0, 72.0, 72.0],
            dpi: 72,
        };
        let cw = settings.content_width();
        let ch = settings.content_height();
        // A4 width = 595.28, subtract left+right margins (72+72=144)
        assert!((cw - (595.28 - 144.0)).abs() < 0.01);
        // A4 height = 841.89, subtract top+bottom margins (72+72=144)
        assert!((ch - (841.89 - 144.0)).abs() < 0.01);
    }

    #[test]
    fn pdf_exporter_render_pages() {
        let mut widget = TestWidget::new(100, 50);
        let mut widgets: [&mut dyn Draw; 1] = [&mut widget];
        let exporter = PdfExporter::new();
        let pages = exporter.render_pages(&mut widgets).expect("render pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].index, 0);
        // SVG content should be valid
        assert!(pages[0].svg_content.contains("<svg"));
    }

    #[test]
    fn export_to_pdf_empty_widgets() {
        let mut empty: [&mut dyn Draw; 0] = [];
        let result = export_to_pdf(&mut empty, "/tmp/nonexistent.pdf");
        assert!(result.is_err());
    }

    #[test]
    fn build_svg_pdf_produces_valid_pdf_header() {
        let pages = vec![ExportPage::new(
            0,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100\" height=\"50\"><rect width=\"100\" height=\"50\" fill=\"red\"/></svg>".to_string(),
            595.28,
            841.89,
            100,
            50,
        )];
        let settings = PdfExportSettings::new();
        let pdf = build_svg_pdf(&pages, &settings).expect("build pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("/Type /Pages"));
        assert!(text.contains("/Type /Page"));
        assert!(text.contains("/MediaBox [0 0 595.28 841.89]"));
        // SVG content should be rendered as real PDF operators, not comments
        assert!(text.contains("re f"));
        assert!(!text.contains("% BEGIN SVG CONTENT"));
        assert!(text.contains("startxref"));
        assert!(text.contains("%%EOF"));
    }

    #[test]
    fn build_svg_pdf_empty_pages_returns_error() {
        let pages = vec![];
        let settings = PdfExportSettings::new();
        let result = build_svg_pdf(&pages, &settings);
        assert!(result.is_err());
    }
}
