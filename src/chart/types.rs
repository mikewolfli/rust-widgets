//! Chart data types, series, chart types, and core traits.

use crate::core::{Color, Point, Rect};

/// Chart data point
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// Data-domain x coordinate.
    pub x: f64,
    /// Data-domain y coordinate.
    pub y: f64,
    /// Optional point label for legends/tooltips.
    pub label: Option<String>,
}
/// Chart series
#[derive(Debug, Clone)]
pub struct ChartSeries {
    /// Series display name.
    pub name: String,
    /// Ordered data points.
    pub data: Vec<DataPoint>,
    /// Series draw color.
    pub color: Color,
    /// Visibility flag for filtering/toggling.
    pub visible: bool,
}
/// Chart type
pub enum ChartType {
    /// Polyline chart.
    Line,
    /// Vertical bar chart.
    Bar,
    /// Pie chart.
    Pie,
}
/// Chart
pub trait Chart {
    /// Add a series
    fn add_series(&mut self, series: ChartSeries);
    /// Remove a series
    fn remove_series(&mut self, name: &str);
    /// Clear all series
    fn clear_series(&mut self);
    /// Set chart title
    fn set_title(&mut self, title: String);
    /// Set x-axis label
    fn set_x_axis_label(&mut self, label: String);
    /// Set y-axis label
    fn set_y_axis_label(&mut self, label: String);
    /// Draw the chart
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext);
}
/// Chart context
pub trait ChartContext {
    /// Draw line
    fn draw_line(&mut self, from: Point, to: Point, width: f32, color: Color);
    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, color: Color);
    /// Draw text
    fn draw_text(&mut self, text: &str, pos: Point, font_size: f32, color: Color);
    /// Draw circle
    fn draw_circle(&mut self, center: Point, radius: f32, color: Color);
}

