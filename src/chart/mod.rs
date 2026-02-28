//! Chart widgets and drawing contracts.

use crate::core::{Point, Rect, Color};

/// Chart data point
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    pub label: Option<String>,
}

/// Chart series
#[derive(Debug, Clone)]
pub struct ChartSeries {
    pub name: String,
    pub data: Vec<DataPoint>,
    pub color: Color,
    pub visible: bool,
}

/// Chart type
pub enum ChartType {
    Line,
    Bar,
    Pie,
}

#[derive(Default)]
pub struct MemoryChartContext {
    pub commands: Vec<String>,
}

impl ChartContext for MemoryChartContext {
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, _color: Color) {
        self.commands.push(format!("line:{x1},{y1}->{x2},{y2}:{width}"));
    }

    fn draw_rect(&mut self, rect: Rect, _color: Color) {
        self.commands.push(format!("rect:{},{},{},{}", rect.x, rect.y, rect.width, rect.height));
    }

    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, _color: Color) {
        self.commands.push(format!("text:{text}@{x},{y}:{font_size}"));
    }

    fn draw_circle(&mut self, center: Point, radius: f32, _color: Color) {
        self.commands.push(format!("circle:{},{}:{radius}", center.x, center.y));
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
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, width: f32, color: Color);
    
    /// Draw rectangle
    fn draw_rect(&mut self, rect: Rect, color: Color);
    
    /// Draw text
    fn draw_text(&mut self, text: &str, x: f32, y: f32, font_size: f32, color: Color);
    
    /// Draw circle
    fn draw_circle(&mut self, center: Point, radius: f32, color: Color);
}

/// Line chart
pub struct LineChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
}

impl LineChart {
    /// Create a new line chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
        }
    }
}

impl Chart for LineChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    
    fn clear_series(&mut self) {
        self.series.clear();
    }
    
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 230, g: 230, b: 230, a: 255 });
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });

        let plot_x = rect.x as f32 + 32.0;
        let plot_y = rect.y as f32 + 32.0;
        let plot_w = (rect.width as f32 - 48.0).max(1.0);
        let plot_h = (rect.height as f32 - 56.0).max(1.0);

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for series in &self.series {
            for point in &series.data {
                min_x = min_x.min(point.x);
                max_x = max_x.max(point.x);
                min_y = min_y.min(point.y);
                max_y = max_y.max(point.y);
            }
        }
        if min_x == f64::MAX || min_y == f64::MAX {
            return;
        }
        let span_x = (max_x - min_x).max(1.0);
        let span_y = (max_y - min_y).max(1.0);

        for series in &self.series {
            if !series.visible || series.data.len() < 2 {
                continue;
            }
            for i in 1..series.data.len() {
                let p1 = &series.data[i - 1];
                let p2 = &series.data[i];
                let x1 = plot_x + (((p1.x - min_x) / span_x) as f32) * plot_w;
                let y1 = plot_y + plot_h - (((p1.y - min_y) / span_y) as f32) * plot_h;
                let x2 = plot_x + (((p2.x - min_x) / span_x) as f32) * plot_w;
                let y2 = plot_y + plot_h - (((p2.y - min_y) / span_y) as f32) * plot_h;
                context.draw_line(
                    x1,
                    y1,
                    x2,
                    y2,
                    2.0,
                    series.color,
                );
            }
        }
    }
}

/// Bar chart
pub struct BarChart {
    title: String,
    x_axis_label: String,
    y_axis_label: String,
    series: Vec<ChartSeries>,
}

impl BarChart {
    /// Create a new bar chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            x_axis_label: String::new(),
            y_axis_label: String::new(),
            series: Vec::new(),
        }
    }
}

impl Chart for BarChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    
    fn clear_series(&mut self) {
        self.series.clear();
    }
    
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    
    fn set_x_axis_label(&mut self, label: String) {
        self.x_axis_label = label;
    }
    fn set_y_axis_label(&mut self, label: String) {
        self.y_axis_label = label;
    }
    
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_rect(rect, Color { r: 240, g: 240, b: 240, a: 255 });
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });
        let plot_x = rect.x + 24;
        let plot_y = rect.y + 32;
        let plot_h = rect.height.saturating_sub(48).max(1);

        let mut max_y = 1.0f64;
        for series in &self.series {
            for point in &series.data {
                max_y = max_y.max(point.y.max(1.0));
            }
        }

        for series in &self.series {
            if !series.visible {
                continue;
            }
            for point in &series.data {
                let bar_h = ((point.y / max_y) * plot_h as f64) as u32;
                let bar = Rect {
                    x: plot_x + point.x as i32,
                    y: plot_y + plot_h as i32 - bar_h as i32,
                    width: 18,
                    height: bar_h,
                };
                context.draw_rect(bar, series.color);
            }
        }
    }
}

/// Pie chart
pub struct PieChart {
    title: String,
    series: Vec<ChartSeries>,
}

impl PieChart {
    /// Create a new pie chart
    pub fn new() -> Self {
        Self {
            title: String::new(),
            series: Vec::new(),
        }
    }
}

impl Chart for PieChart {
    fn add_series(&mut self, series: ChartSeries) {
        self.series.push(series);
    }
    
    fn remove_series(&mut self, name: &str) {
        self.series.retain(|s| s.name != name);
    }
    
    fn clear_series(&mut self) {
        self.series.clear();
    }
    
    fn set_title(&mut self, title: String) {
        self.title = title;
    }
    
    fn set_x_axis_label(&mut self, _label: String) {
        // Not used in pie chart
    }
    
    fn set_y_axis_label(&mut self, _label: String) {
        // Not used in pie chart
    }
    
    fn draw(&self, rect: Rect, context: &mut dyn ChartContext) {
        context.draw_text(&self.title, rect.x as f32 + 8.0, rect.y as f32 + 16.0, 14.0, Color { r: 20, g: 20, b: 20, a: 255 });
        let center = Point {
            x: rect.x + rect.width as i32 / 2,
            y: rect.y + rect.height as i32 / 2,
        };
        let radius = (rect.width.min(rect.height) / 3) as f32;
        for series in &self.series {
            if !series.visible {
                continue;
            }
            context.draw_circle(center, radius, series.color);
        }
    }
}
