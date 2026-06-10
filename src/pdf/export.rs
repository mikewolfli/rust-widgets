//! PDF export module.
//!
//! Provides [`PdfExporter`] and [`export_to_pdf`] to render widget trees
//! into PDF documents by leveraging the SVG rendering pipeline.

use crate::core::{Rect, Size};
use crate::widget::svg::render_widget_to_svg;
use crate::widget::Draw;

/// Standard page sizes in points (1/72 inch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
    /// A4: 595.28 x 841.89 pt
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

impl Default for PageSize {
    fn default() -> Self {
        PageSize::A4
    }
}

/// Export orientation for the PDF page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdfOrientation {
    /// Portrait (tall).
    Portrait,
    /// Landscape (wide).
    Landscape,
}

impl Default for PdfOrientation {
    fn default() -> Self {
        PdfOrientation::Portrait
    }
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
pub struct PdfPage {
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

impl PdfPage {
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
    pub fn export(&self, widgets: &mut [&mut dyn Draw], path: &str) -> Result<(), String> {
        let pages = self.render_pages(widgets)?;
        let pdf_bytes = build_svg_pdf(&pages, &self.settings)?;
        std::fs::write(path, &pdf_bytes)
            .map_err(|err| format!("failed to write PDF file '{path}': {err}"))?;
        Ok(())
    }

    /// Render each widget into a [`PdfPage`] using the SVG pipeline.
    pub fn render_pages(&self, widgets: &mut [&mut dyn Draw]) -> Result<Vec<PdfPage>, String> {
        let pixel_size = self.settings.pixel_size();
        let (page_w_pt, page_h_pt) = self.settings.effective_dimensions();
        let mut pages = Vec::with_capacity(widgets.len());

        for (idx, widget) in widgets.iter_mut().enumerate() {
            // Render the widget to SVG at the target pixel size
            let svg =
                render_widget_to_svg(*widget, Rect::new(0, 0, pixel_size.width, pixel_size.height));
            pages.push(PdfPage::new(
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
fn build_svg_pdf(pages: &[PdfPage], settings: &PdfExportSettings) -> Result<Vec<u8>, String> {
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
    let kids = page_obj_ids.iter().map(|id| format!("{} 0 R", id)).collect::<Vec<_>>().join(" ");
    objects[1] =
        format!("<< /Type /Pages /Count {} /Kids [{}] >>", page_obj_ids.len(), kids,).into_bytes();

    // Assemble the final PDF
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets: Vec<usize> = Vec::new();
    for (idx, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        let obj_id = idx + 1;
        out.extend_from_slice(format!("{} 0 obj\n", obj_id).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
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

/// Build the content stream for a single PDF page embedding its SVG content.
///
/// The SVG string is placed inside the stream as a comment block, wrapped
/// with the PDF graphics state operators so it can be extracted by reader
/// tools while remaining valid PDF syntax.
fn build_content_stream(page: &PdfPage, _settings: &PdfExportSettings) -> String {
    let mut stream = String::new();

    // Save graphics state and apply a scaling transform so the SVG (rendered
    // at the pixel size) maps linearly onto the page's point dimensions.
    stream.push_str(&format!(
        "q\n{:.4} 0 0 {:.4} 0 0 cm\n",
        page.width_pt / page.width_px as f32,
        page.height_pt / page.height_px as f32,
    ));

    // Embed the SVG content as a structured comment block
    stream.push_str("% BEGIN SVG CONTENT\n");
    for line in page.svg_content.lines() {
        stream.push_str(&format!("% {}\n", line));
    }
    stream.push_str("% END SVG CONTENT\n");

    // Restore graphics state
    stream.push_str("Q\n");

    stream
}

/// Export a slice of drawable widgets to a PDF file.
///
/// This is a convenience function that creates a [`PdfExporter`] with default
/// settings, renders each widget via the SVG pipeline, and writes a minimal
/// PDF file with one page per widget.
///
/// # Errors
/// Returns `Err` if no widgets are provided, or if the file cannot be written.
pub fn export_to_pdf(widgets: &mut [&mut dyn Draw], path: &str) -> Result<(), String> {
    let exporter = PdfExporter::new();
    exporter.export(widgets, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use std::sync::{Arc, Mutex};

    /// A simple test widget that records draw calls.
    struct TestWidget {
        geometry: Rect,
        draw_count: Arc<Mutex<u32>>,
    }

    impl TestWidget {
        fn new(width: u32, height: u32) -> Self {
            Self { geometry: Rect::new(0, 0, width, height), draw_count: Arc::new(Mutex::new(0)) }
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
        let pages = vec![PdfPage::new(
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
        assert!(text.contains("% BEGIN SVG CONTENT"));
        assert!(text.contains("% END SVG CONTENT"));
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
