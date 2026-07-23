//! SVG paint backend — converts `RenderCommand`s into SVG elements.

use crate::core::{Color, Font, Size};
use crate::render::core::command::RenderCommand;
use crate::render::core::types::{ShapedText, TextMetrics};
use crate::render::{PaintBackend, SoftwareRenderConfig};
use crate::style::gradient::GradientType;
use super::convert::{color_to_rgba, escape_xml, point_attrs, rect_attrs};

/// PaintBackend implementation that generates SVG markup from render commands.
///
/// Every [`RenderCommand`] is converted into an equivalent SVG element.
/// The resulting SVG document is produced when [`finish()`](SvgPaintBackend::finish) is called.
pub struct SvgPaintBackend {
    pub(crate) size: Size,
    dpi_scale: f32,
    pub(crate) elements: Vec<String>,
    clip_depth: u32,
    clip_path_counter: u32,
    svg_output: Option<String>,
    gradient_counter: u32,
}

impl SvgPaintBackend {
    /// Create a new SVG backend with the given canvas size.
    pub fn new(size: Size) -> Self {
        Self {
            size,
            dpi_scale: 1.0,
            elements: Vec::new(),
            clip_depth: 0,
            clip_path_counter: 0,
            svg_output: None,
            gradient_counter: 0,
        }
    }

    /// Finalize and retrieve the full SVG document string.
    ///
    /// Once called, the backend is consumed and no further commands can be added.
    pub fn finish(&mut self) -> String {
        if let Some(svg) = self.svg_output.take() {
            return svg;
        }
        self.build_svg()
    }

    /// Build the SVG document from the collected elements.
    fn build_svg(&self) -> String {
        let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg""#);
        svg.push_str(&format!(r#" width="{}" height="{}""#, self.size.width, self.size.height));
        svg.push_str(&format!(r#" viewBox="0 0 {} {}">"#, self.size.width, self.size.height));
        for element in &self.elements {
            svg.push('\n');
            svg.push_str("  ");
            svg.push_str(element);
        }
        svg.push_str("\n</svg>");
        svg
    }

    /// Add a raw SVG element string to the internal list.
    fn push_element(&mut self, element: String) {
        self.elements.push(element);
    }
}

// ─── Helper: RGBA→BMP conversion ──────────────────────────────────────────

/// Convert raw RGBA pixel data into an in-memory BMP file (32-bit BGRA).
fn rgba_to_bmp(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row_size = width * 4; // 4 bytes/pixel, already 4-byte aligned
    let pixel_data_size = row_size * height;
    let file_size: usize = 14 + 40 + pixel_data_size as usize;
    let mut bmp = Vec::with_capacity(file_size);

    // BITMAPFILEHEADER (14 bytes)
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]); // reserved
    bmp.extend_from_slice(&54u32.to_le_bytes()); // offset to pixel array

    // BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes()); // header size
    bmp.extend_from_slice(&width.to_le_bytes());
    bmp.extend_from_slice(&height.to_le_bytes());
    bmp.extend_from_slice(&1u16.to_le_bytes()); // color planes
    bmp.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    bmp.extend_from_slice(&0u32.to_le_bytes()); // compression (BI_RGB)
    bmp.extend_from_slice(&pixel_data_size.to_le_bytes()); // image size
    bmp.extend_from_slice(&0i32.to_le_bytes()); // x pixels-per-meter
    bmp.extend_from_slice(&0i32.to_le_bytes()); // y pixels-per-meter
    bmp.extend_from_slice(&0u32.to_le_bytes()); // colors used
    bmp.extend_from_slice(&0u32.to_le_bytes()); // important colors

    // Pixel data: RGBA → BGRA, stored bottom-up
    for y in (0..height).rev() {
        let row_off = (y * row_size) as usize;
        for x in 0..width {
            let idx = row_off + (x * 4) as usize;
            bmp.push(rgba[idx + 2]); // B
            bmp.push(rgba[idx + 1]); // G
            bmp.push(rgba[idx]); // R
            bmp.push(rgba[idx + 3]); // A
        }
    }

    bmp
}

/// Minimal base64 encoder (RFC 4648) — no dependencies needed.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";
    let cap = data.len().div_ceil(3) * 4;
    let mut out = String::with_capacity(cap);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ─── PaintBackend implementation ─────────────────────────────────────────────

impl PaintBackend for SvgPaintBackend {
    fn begin_frame(&mut self, clear: Color) {
        self.elements.clear();
        self.clip_depth = 0;
        // Background fill rect matching clear color
        if clear.a > 0 {
            let fill = color_to_rgba(&clear);
            let bg_rect = crate::core::Rect::new(0, 0, self.size.width, self.size.height);
            self.push_element(format!(r#"<rect {} fill="{}" />"#, rect_attrs(&bg_rect), fill));
        }
    }

    fn end_frame(&mut self) {
        // Close any remaining clip groups
        for _ in 0..self.clip_depth {
            self.push_element("</g>".to_string());
        }
        self.clip_depth = 0;
        self.svg_output = Some(self.build_svg());
    }

    fn execute_command(&mut self, command: &RenderCommand) {
        match command {
            // ── Filled rectangles ──────────────────────────────────────
            RenderCommand::FillRect { rect, color } => {
                self.push_element(format!(
                    r#"<rect {} fill="{}" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color)
                ));
            }

            RenderCommand::FillRoundedRect { rect, radius, color }
            | RenderCommand::FillRoundedRectAA { rect, radius, color } => {
                self.push_element(format!(
                    r#"<rect {} rx="{}" ry="{}" fill="{}" />"#,
                    rect_attrs(rect),
                    radius,
                    radius,
                    color_to_rgba(color)
                ));
            }

            // ── Rectangle outlines ─────────────────────────────────────
            RenderCommand::DrawRect { rect, color } => {
                self.push_element(format!(
                    r#"<rect {} fill="none" stroke="{}" stroke-width="1" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawRectStroke { rect, color, width } => {
                self.push_element(format!(
                    r#"<rect {} fill="none" stroke="{}" stroke-width="{}" />"#,
                    rect_attrs(rect),
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Rounded rectangle outlines ─────────────────────────────
            RenderCommand::DrawRoundedRectStroke { rect, radius, color, width }
            | RenderCommand::DrawRoundedRectStrokeAA { rect, radius, color, width } => {
                self.push_element(format!(
                    r#"<rect {} rx="{}" ry="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
                    rect_attrs(rect),
                    radius,
                    radius,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Lines ──────────────────────────────────────────────────
            RenderCommand::DrawLine { from, to, color }
            | RenderCommand::DrawLineAA { from, to, color } => {
                self.push_element(format!(
                    r#"<line {} x2="{}" y2="{}" stroke="{}" stroke-width="1" />"#,
                    point_attrs(from),
                    to.x,
                    to.y,
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawLineStroke { from, to, color, width }
            | RenderCommand::DrawLineStrokeAA { from, to, color, width } => {
                self.push_element(format!(
                    r#"<line {} x2="{}" y2="{}" stroke="{}" stroke-width="{}" />"#,
                    point_attrs(from),
                    to.x,
                    to.y,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Filled circles ─────────────────────────────────────────
            RenderCommand::FillCircle { center, radius, color }
            | RenderCommand::FillCircleAA { center, radius, color } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{}" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color)
                ));
            }

            // ── Circle outlines ────────────────────────────────────────
            RenderCommand::DrawCircle { center, radius, color } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="1" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color)
                ));
            }

            RenderCommand::DrawCircleStroke { center, radius, color, width } => {
                self.push_element(format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
                    center.x,
                    center.y,
                    radius,
                    color_to_rgba(color),
                    width
                ));
            }

            // ── Text ───────────────────────────────────────────────────
            RenderCommand::DrawText { origin, text, font, color, .. } => {
                self.push_element(format!(
                    r#"<text x="{}" y="{}" font-family="{}" font-size="{}" font-style="{}" font-weight="{}" fill="{}">{}</text>"#,
                    origin.x,
                    origin.y,
                    escape_xml(font.family()),
                    font.size(),
                    font.style_css(),
                    font.weight_css(),
                    color_to_rgba(color),
                    escape_xml(text)
                ));
            }

            // ── Image ──────────────────────────────────────────────────
            RenderCommand::DrawImage { x, y, width, height, data } => {
                if !data.is_empty() && *width > 0 && *height > 0 {
                    // Convert RGBA pixel data to BMP and base64-encode for embedding.
                    let bmp = rgba_to_bmp(*width, *height, data);
                    let b64 = base64_encode(&bmp);
                    self.push_element(format!(
                        r##"<image x="{x}" y="{y}" width="{width}" height="{height}" href="data:image/bmp;base64,{b64}" />"##
                    ));
                } else {
                    // No pixel data: render an error placeholder rectangle.
                    self.push_element(format!(
                        r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="#fee" stroke="#c00" stroke-width="2" />"##
                    ));
                    let cx = x + (*width as i32) / 2;
                    let cy = y + (*height as i32) / 2;
                    self.push_element(format!(
                        r##"<text x="{cx}" y="{cy}" text-anchor="middle" dominant-baseline="central" font-family="sans-serif" font-size="11" fill="#c00">Image (no data)</text>"##
                    ));
                }
            }

            // ── Clipping ───────────────────────────────────────────────
            RenderCommand::PushClip { x, y, width, height } => {
                let clip_id = format!("clip_{}", self.clip_depth);
                // Build a minimal <defs> clipPath — SVG renderers need the
                // definition available before the referencing <g> element.
                let clip_def = format!(
                    r#"<clipPath id="{clip_id}"><rect x="{x}" y="{y}" width="{width}" height="{height}" /></clipPath>"#
                );
                self.push_element(clip_def);
                self.push_element(format!(r#"<g clip-path="url(#{clip_id})">"#));
                self.clip_depth += 1;
            }

            RenderCommand::PopClip => {
                if self.clip_depth > 0 {
                    self.push_element("</g>".to_string());
                    self.clip_depth -= 1;
                }
            }

            // ── Gradient ────────────────────────────────────────────────
            RenderCommand::DrawGradient { rect, gradient } => {
                self.gradient_counter += 1;
                let gid = format!("g{}", self.gradient_counter);
                let mut def = String::new();
                match gradient.gradient_type {
                    GradientType::Linear => {
                        def.push_str(&format!(
                            r##"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}">"##,
                            gid,
                            gradient.start_point.x,
                            gradient.start_point.y,
                            gradient.end_point.x,
                            gradient.end_point.y
                        ));
                    }
                    GradientType::Radial => {
                        def.push_str(&format!(
                            r##"<radialGradient id="{}" cx="{}" cy="{}" r="{}">"##,
                            gid, gradient.center.x, gradient.center.y, gradient.radius
                        ));
                    }
                    GradientType::Conic => {
                        // SVG does not natively support conic gradients; approximate with linear.
                        def.push_str(&format!(
                            r##"<linearGradient id="{}" x1="{}" y1="{}" x2="{}" y2="{}">"##,
                            gid,
                            rect.x as f32,
                            rect.y as f32,
                            (rect.x + rect.width as i32) as f32,
                            (rect.y + rect.height as i32) as f32
                        ));
                    }
                }
                for stop in &gradient.stops {
                    let hex =
                        format!("#{:02x}{:02x}{:02x}", stop.color.r, stop.color.g, stop.color.b);
                    let alpha = stop.color.a as f32 / 255.0;
                    def.push_str(&format!(
                        r##"<stop offset="{:.3}" stop-color="{}" stop-opacity="{:.3}"/>"##,
                        stop.position, hex, alpha
                    ));
                }
                match gradient.gradient_type {
                    GradientType::Linear | GradientType::Conic => {
                        def.push_str("</linearGradient>");
                    }
                    GradientType::Radial => {
                        def.push_str("</radialGradient>");
                    }
                }
                self.push_element(format!("<defs>{def}</defs>"));
                self.push_element(format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" fill="url(#{})" />"##,
                    rect.x, rect.y, rect.width, rect.height, gid
                ));
            }

            // ── Arc ─────────────────────────────────────────────────────
            RenderCommand::DrawArc { center, radius, start_angle, end_angle, color, filled } => {
                // Convert arc to SVG path element.
                let large_arc =
                    if (end_angle - start_angle).abs() > std::f32::consts::PI { 1 } else { 0 };
                let start_x = center.x + (*radius as f32 * start_angle.cos()) as i32;
                let start_y = center.y + (*radius as f32 * start_angle.sin()) as i32;
                let end_x = center.x + (*radius as f32 * end_angle.cos()) as i32;
                let end_y = center.y + (*radius as f32 * end_angle.sin()) as i32;
                let fill = if *filled { color_to_rgba(color) } else { "none".to_string() };
                let stroke = if *filled { "none".to_string() } else { color_to_rgba(color) };
                if *filled {
                    // Pie/wedge shape: center → arc start → arc → arc end → close to center
                    self.push_element(format!(
                        r##"<path d="M {} {} L {} {} A {} {} 0 {} 1 {} {} Z" fill="{}" stroke="{}" />"##,
                        center.x, center.y,
                        start_x, start_y,
                        radius, radius, large_arc, end_x, end_y,
                        fill, stroke
                    ));
                } else {
                    self.push_element(format!(
                        r##"<path d="M {start_x} {start_y} A {radius} {radius} 0 {large_arc} 1 {end_x} {end_y}" fill="{fill}" stroke="{stroke}" />"##
                    ));
                }
            }

            // ── Path ────────────────────────────────────────────────────
            RenderCommand::DrawPath { points, closed, color, filled, width } => {
                if points.is_empty() {
                    return;
                }
                let mut d = format!("M {} {}", points[0].x, points[0].y);
                for pt in &points[1..] {
                    d.push_str(&format!(" L {} {}", pt.x, pt.y));
                }
                if *closed {
                    d.push_str(" Z");
                }
                let fill = if *filled { color_to_rgba(color) } else { "none".to_string() };
                let stroke = if *filled { "none".to_string() } else { color_to_rgba(color) };
                self.push_element(format!(
                    r##"<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{width}" />"##
                ));
            }
            RenderCommand::BoxShadow { rect, color, offset_x, offset_y, blur_radius, spread } => {
                let spread_w = (rect.width as i32 + *spread * 2).max(0) as u32;
                let spread_h = (rect.height as i32 + *spread * 2).max(0) as u32;
                let x = rect.x + offset_x - *spread;
                let y = rect.y + offset_y - *spread;
                let filter_attr = if *blur_radius > 0 {
                    let filter_id = format!("shadow_blur_{blur_radius}");
                    self.push_element(format!(
                        r##"<filter id=\"{filter_id}\"><feGaussianBlur stdDeviation=\"{blur_radius}\" /></filter>"##
                    ));
                    format!(r##" filter=\"url(#{filter_id})\""##)
                } else {
                    String::new()
                };
                self.push_element(format!(
                    r##"<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"{} rx=\"4\" />"##,
                    x, y, spread_w, spread_h, color_to_rgba(color), filter_attr
                ));
            }
            RenderCommand::Blur { radius } => {
                self.push_element(format!(
                    r##"<filter id="blur_{radius}"><feGaussianBlur stdDeviation="{radius}" /></filter>"##
                ));
            }
            RenderCommand::ClipPath { points } => {
                if !points.is_empty() {
                    let clip_id = format!("cp_{}", self.clip_path_counter);
                    self.clip_path_counter += 1;
                    let mut d = format!("M {} {}", points[0].x, points[0].y);
                    for pt in &points[1..] {
                        d.push_str(&format!(" L {} {}", pt.x, pt.y));
                    }
                    d.push_str(" Z");
                    self.push_element(format!(
                        r##"<clipPath id=\"{clip_id}\"><path d=\"{d}\" /></clipPath>"##
                    ));
                    self.push_element(format!(r##"<g clip-path=\"url(#{clip_id})\">"##));
                    self.clip_depth += 1;
                }
            }
            RenderCommand::SetBlendMode { mode: _ } => {
                // SVG backend: blend mode is not directly supported; skip
            }
            RenderCommand::DrawConicGradient { center: _, start_angle: _, stops: _ } => {
                // SVG backend: conic gradient is not natively supported; skip
            }
        }
    }

    fn size(&self) -> Size {
        self.size
    }

    fn set_size(&mut self, size: Size) {
        self.size = size;
    }

    fn dpi_scale(&self) -> f32 {
        self.dpi_scale
    }

    fn set_dpi_scale(&mut self, dpi_scale: f32) {
        self.dpi_scale = dpi_scale;
    }

    fn measure_text(&self, text: &str, _font: &Font) -> TextMetrics {
        // Approximate measurement: 8px per character width, 16px height.
        TextMetrics { width: text.len() as u32 * 8, height: 16, ascent: 12, descent: 4 }
    }

    fn shape_text(&self, text: &str, _font: &Font) -> ShapedText {
        ShapedText { clusters: vec![], advance: text.len() as f32 * 8.0 }
    }

    fn frame_rgba(&self) -> &[u8] {
        &[]
    }

    fn apply_render_config(&mut self, _config: SoftwareRenderConfig) {}

    fn render_config(&self) -> SoftwareRenderConfig {
        SoftwareRenderConfig::default()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};

    #[test]
    fn svg_backend_creates_valid_document() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(10, 10, 30, 20),
            color: Color::rgb(255, 0, 0),
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<svg"));
        assert!(result.contains("</svg>"));
        assert!(result.contains("fill=\"rgba(255,0,0,1.00)\""));
    }

    #[test]
    fn svg_backend_text_escaping() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 20),
            text: "<hello> & world".to_string(),
            font: Font::default_ui(),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("&lt;hello&gt; &amp; world"));
    }

    #[test]
    fn svg_backend_all_command_types() {
        let mut svg = SvgPaintBackend::new(Size::new(200, 200));
        svg.begin_frame(Color::WHITE);

        // Test all command types compile and produce output
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 50, 50),
            color: Color::RED,
        });
        svg.execute_command(&RenderCommand::FillRoundedRect {
            rect: Rect::new(50, 0, 50, 50),
            radius: 5,
            color: Color::GREEN,
        });
        svg.execute_command(&RenderCommand::DrawRect {
            rect: Rect::new(0, 50, 50, 50),
            color: Color::BLUE,
        });
        svg.execute_command(&RenderCommand::DrawRectStroke {
            rect: Rect::new(50, 50, 50, 50),
            color: Color::BLACK,
            width: 2,
        });
        svg.execute_command(&RenderCommand::DrawLine {
            from: Point::new(0, 100),
            to: Point::new(50, 150),
            color: Color::RED,
        });
        svg.execute_command(&RenderCommand::DrawLineStroke {
            from: Point::new(50, 100),
            to: Point::new(100, 150),
            color: Color::GREEN,
            width: 3,
        });
        svg.execute_command(&RenderCommand::FillCircle {
            center: Point::new(30, 130),
            radius: 15,
            color: Color::BLUE,
        });
        svg.execute_command(&RenderCommand::DrawCircle {
            center: Point::new(80, 130),
            radius: 15,
            color: Color::BLACK,
        });
        svg.execute_command(&RenderCommand::DrawCircleStroke {
            center: Point::new(130, 130),
            radius: 15,
            color: Color::RED,
            width: 2,
        });
        svg.execute_command(&RenderCommand::DrawText {
            origin: Point::new(10, 180),
            text: "Test".to_string(),
            font: Font::default_ui(),
            color: Color::BLACK,
            alignment: HorizontalAlignment::Left,
        });
        svg.execute_command(&RenderCommand::PushClip { x: 0, y: 0, width: 100, height: 100 });
        svg.execute_command(&RenderCommand::PopClip);
        svg.execute_command(&RenderCommand::DrawImage {
            x: 100,
            y: 0,
            width: 50,
            height: 50,
            data: vec![],
        });

        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<svg"));
        assert!(result.contains("</svg>"));
        assert!(result.contains("stroke"));
        assert!(result.contains("fill"));
        assert!(result.contains("Image (no data)"));
        assert!(result.contains("#fee"));
        assert!(result.contains("#c00"));
    }

    #[test]
    fn svg_backend_begin_frame_clears() {
        let mut svg = SvgPaintBackend::new(Size::new(10, 10));
        svg.begin_frame(Color::rgb(200, 200, 200));
        assert!(svg.elements.len() == 1);
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("200"));
    }

    #[test]
    fn svg_backend_draw_rounded_rect_stroke() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::DrawRoundedRectStroke {
            rect: Rect::new(10, 10, 80, 80),
            radius: 8,
            color: Color::BLUE,
            width: 2,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("rx=\"8\""));
        assert!(result.contains("ry=\"8\""));
        assert!(result.contains("stroke-width=\"2\""));
    }

    #[test]
    fn svg_backend_draw_line_aa() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::TRANSPARENT);
        svg.execute_command(&RenderCommand::DrawLineAA {
            from: Point::new(5, 5),
            to: Point::new(95, 95),
            color: Color::RED,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("x1=\"5\""));
        assert!(result.contains("y1=\"5\""));
        assert!(result.contains("x2=\"95\""));
        assert!(result.contains("y2=\"95\""));
    }

    #[test]
    fn svg_backend_fill_circle_aa() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillCircleAA {
            center: Point::new(50, 50),
            radius: 25,
            color: Color::GREEN,
        });
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("<circle"));
        assert!(result.contains("cx=\"50\""));
        assert!(result.contains("r=\"25\""));
    }

    #[test]
    fn svg_backend_multiple_frames() {
        let mut svg = SvgPaintBackend::new(Size::new(50, 50));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::FillRect {
            rect: Rect::new(0, 0, 25, 25),
            color: Color::RED,
        });
        svg.end_frame();

        let frame1 = svg.finish();
        assert!(frame1.contains("rgba(255,0,0"));

        // Start second frame
        svg.begin_frame(Color::BLACK);
        // Background fill rect is pushed (BLACK has alpha > 0)
        assert!(svg.elements.len() == 1);
        svg.end_frame();
        let frame2 = svg.finish();
        assert!(frame2.contains("rgba(0,0,0"));
    }

    #[test]
    fn svg_backend_clip_nesting() {
        let mut svg = SvgPaintBackend::new(Size::new(100, 100));
        svg.begin_frame(Color::WHITE);
        svg.execute_command(&RenderCommand::PushClip { x: 10, y: 10, width: 50, height: 50 });
        svg.execute_command(&RenderCommand::PushClip { x: 20, y: 20, width: 30, height: 30 });
        svg.execute_command(&RenderCommand::PopClip);
        svg.execute_command(&RenderCommand::PopClip);
        svg.end_frame();
        let result = svg.finish();
        assert!(result.contains("clip_0"));
        assert!(result.contains("clip_1"));
    }
}
