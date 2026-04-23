//! SVG chart context and in-memory chart context implementations.

use crate::chart::types::*;
use crate::core::{Color, Point, Rect};
use std::fs;

#[derive(Default)]
/// In-memory chart draw-command collector used by tests/demos.
pub struct MemoryChartContext {
    /// Recorded draw commands for tests/demos.
    pub commands: Vec<String>,
}
/// SVG chart context that emits real vector drawing output.
pub struct SvgChartContext {
    width: u32,
    height: u32,
    elements: Vec<String>,
}
impl SvgChartContext {
    /// Create SVG context with explicit viewport size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            elements: Vec::new(),
        }
    }
    /// Return SVG XML text.
    pub fn to_svg_string(&self) -> String {
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
            self.width, self.height, self.width, self.height
        ));
        for element in &self.elements {
            svg.push_str(element);
            svg.push('\n');
        }
        svg.push_str("</svg>\n");
        svg
    }
    /// Save SVG XML text to a file path.
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        fs::write(path, self.to_svg_string())
    }
}
impl ChartContext for SvgChartContext {
    fn draw_line(&mut self, from: Point, to: Point, width: f32, color: Color) {
        self.elements.push(format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{:.2}\" />",
            from.x,
            from.y,
            to.x,
            to.y,
            svg_color_hex(color),
            svg_alpha(color),
            width.max(0.1)
        ));
    }
    fn draw_rect(&mut self, rect: Rect, color: Color) {
        self.elements.push(format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"1\" />",
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            svg_color_hex(color),
            svg_alpha(color)
        ));
    }
    fn draw_text(&mut self, text: &str, pos: Point, font_size: f32, color: Color) {
        self.elements.push(format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" fill=\"{}\" fill-opacity=\"{:.3}\" font-size=\"{:.2}\" font-family=\"sans-serif\">{}</text>",
            pos.x,
            pos.y,
            svg_color_hex(color),
            svg_alpha(color),
            font_size.max(1.0),
            svg_escape_text(text)
        ));
    }
    fn draw_circle(&mut self, center: Point, radius: f32, color: Color) {
        self.elements.push(format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{:.2}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"2\" />",
            center.x,
            center.y,
            radius.max(0.1),
            svg_color_hex(color),
            svg_alpha(color)
        ));
    }
}
/// Render chart into SVG content and write to file.
pub fn render_chart_to_svg_file(
    chart: &dyn Chart,
    rect: Rect,
    path: &str,
) -> Result<(), std::io::Error> {
    let width = (rect.x.max(0) as u32).saturating_add(rect.width);
    let height = (rect.y.max(0) as u32).saturating_add(rect.height);
    let mut context = SvgChartContext::new(width.max(1), height.max(1));
    chart.draw(rect, &mut context);
    context.save(path)
}
impl ChartContext for MemoryChartContext {
    fn draw_line(&mut self, from: Point, to: Point, width: f32, _color: Color) {
        self.commands
            .push(format!("line:{},{}->{},{}:{width}", from.x, from.y, to.x, to.y));
    }
    fn draw_rect(&mut self, rect: Rect, _color: Color) {
        self.commands.push(format!(
            "rect:{},{},{},{}",
            rect.x, rect.y, rect.width, rect.height
        ));
    }
    fn draw_text(&mut self, text: &str, pos: Point, font_size: f32, _color: Color) {
        self.commands
            .push(format!("text:{text}@{},{}:{font_size}", pos.x, pos.y));
    }
    fn draw_circle(&mut self, center: Point, radius: f32, _color: Color) {
        self.commands
            .push(format!("circle:{},{}:{radius}", center.x, center.y));
    }
}
fn svg_color_hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}
fn svg_alpha(color: Color) -> f32 {
    color.a as f32 / 255.0
}
fn svg_escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

