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
    /// Scatter chart.
    Scatter,
    /// Area chart.
    Area,
}

impl ChartType {
    /// Create a boxed chart instance from this variant.
    pub fn create_chart(&self) -> Box<dyn Chart> {
        match self {
            ChartType::Line => Box::new(crate::chart::charts::LineChart::new()),
            ChartType::Bar => Box::new(crate::chart::charts::BarChart::new()),
            ChartType::Pie => Box::new(crate::chart::charts::PieChart::new()),
            ChartType::Scatter => Box::new(crate::chart::charts::ScatterChart::new()),
            ChartType::Area => Box::new(crate::chart::charts::AreaChart::new()),
        }
    }
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartType::Line => write!(f, "Line"),
            ChartType::Bar => write!(f, "Bar"),
            ChartType::Pie => write!(f, "Pie"),
            ChartType::Scatter => write!(f, "Scatter"),
            ChartType::Area => write!(f, "Area"),
        }
    }
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
    /// Draw filled polygon from a list of points.
    fn draw_polygon(&mut self, points: &[Point], color: Color);
    /// Draw a path segment (line) with specified width.
    fn draw_path_segment(&mut self, start: Point, end: Point, width: f32, color: Color);
    /// Draw an arc approximated by a series of line segments.
    fn draw_arc(
        &mut self,
        center: Point,
        radius: f32,
        start_angle: f64,
        end_angle: f64,
        color: Color,
    );
    /// Draw a multi-segment path from a list of points.
    fn draw_path(&mut self, points: &[Point], width: f32, color: Color);
    /// Draw a filled ellipse.
    fn draw_ellipse(&mut self, center: Point, radius_x: f32, radius_y: f32, color: Color);
    /// Set the fill color for subsequent draw operations.
    fn set_fill_color(&mut self, color: Color);
    /// Set the stroke color for subsequent draw operations.
    fn set_stroke_color(&mut self, color: Color);
}
