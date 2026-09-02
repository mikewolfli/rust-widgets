//! MiniChart widget — simplified line/bar chart for mini builds (BLUE13 R2.10).

use crate::core::{Color, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Chart rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    /// Connected line chart.
    Line,
    /// Vertical bar chart.
    Bar,
}

/// A single data point with a label and numeric value.
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// Data point label.
    pub label: String,
    /// Numeric value.
    pub value: u32,
}

impl DataPoint {
    /// Creates a new data point.
    pub fn new(label: impl Into<String>, value: u32) -> Self {
        Self { label: label.into(), value }
    }
}

/// Simplified line/bar chart widget for mini builds.
///
/// Provides a compact chart suitable for mini UI builds where the full
/// [`ChartWidget`](crate::widget::special_widgets::chart::ChartWidget) is too heavy.
pub struct MiniChart {
    base: BaseWidget,
    chart_type: ChartType,
    data: Vec<DataPoint>,
    min_value: u32,
    max_value: u32,
}

impl MiniChart {
    /// Creates a new MiniChart with default Line chart type and 0–100 range.
    pub fn new(rect: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::MiniChart, rect, "MiniChart"),
            chart_type: ChartType::Line,
            data: Vec::new(),
            min_value: 0,
            max_value: 100,
        }
    }

    /// Sets the chart type (Line or Bar).
    pub fn set_chart_type(&mut self, t: ChartType) {
        self.chart_type = t;
        self.base.request_redraw();
    }

    /// Returns the current chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// Sets the chart data points.
    pub fn set_data(&mut self, data: Vec<DataPoint>) {
        self.data = data;
        self.base.request_redraw();
    }

    /// Returns the current data points.
    pub fn data(&self) -> &[DataPoint] {
        &self.data
    }

    /// Sets the value range (min, max) for the chart Y axis.
    pub fn set_range(&mut self, min: u32, max: u32) {
        let (a, b) = if min <= max { (min, max) } else { (max, min) };
        self.min_value = a;
        self.max_value = b;
        self.base.request_redraw();
    }
}

impl Widget for MiniChart {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(200, 150)
    }
}

impl EventHandler for MiniChart {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for MiniChart {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let style = self.style();

        let bg_color = style.background_color.unwrap_or(Color::rgb(255, 255, 255));
        let line_color = style.text_color.unwrap_or(Color::rgb(0, 0, 0));
        let axis_color = style.border_color.unwrap_or(Color::rgb(80, 80, 80));

        // Draw background
        context.fill_rect(rect, bg_color);

        // Chart area margins (leave room for axes labels)
        let margin_left = 40i32;
        let margin_right = 10i32;
        let margin_top = 10i32;
        let margin_bottom = 30i32;

        let chart_area_w = (rect.width as i32 - margin_left - margin_right).max(1) as u32;
        let chart_area_h = (rect.height as i32 - margin_top - margin_bottom).max(1) as u32;
        let chart_origin_x = rect.x + margin_left;
        let chart_origin_y = rect.y + margin_top;

        // Draw horizontal grid lines
        let grid_count = 4;
        let grid_color = Color::rgb(220, 220, 220);
        for i in 0..=grid_count {
            let gy = chart_origin_y + (chart_area_h as i32 * i / grid_count);
            context.draw_line(
                Point::new(chart_origin_x, gy),
                Point::new(chart_origin_x + chart_area_w as i32 - 1, gy),
                grid_color,
            );
        }

        // Draw Y axis (vertical line on the left)
        context.draw_line(
            Point::new(chart_origin_x, chart_origin_y),
            Point::new(chart_origin_x, chart_origin_y + chart_area_h as i32),
            axis_color,
        );

        // Draw X axis (horizontal line at the bottom)
        context.draw_line(
            Point::new(chart_origin_x, chart_origin_y + chart_area_h as i32),
            Point::new(
                chart_origin_x + chart_area_w as i32 - 1,
                chart_origin_y + chart_area_h as i32,
            ),
            axis_color,
        );

        if self.data.is_empty() {
            return;
        }

        // Compute effective Y range
        let effective_max = if self.max_value > self.min_value {
            self.max_value
        } else {
            let max_val = self.data.iter().map(|dp| dp.value).max().unwrap_or(100);
            if max_val < 1 {
                100
            } else {
                max_val
            }
        };
        let effective_min = self.min_value;
        let range = (effective_max - effective_min) as f32;

        match self.chart_type {
            ChartType::Line => {
                let count = self.data.len();
                if count == 1 {
                    // Single data point: draw a small cross marker at the center
                    let cx = chart_origin_x + chart_area_w as i32 / 2;
                    let y_ratio = if range > 0.0 {
                        (self.data[0].value - effective_min) as f32 / range
                    } else {
                        0.5
                    };
                    let cy = chart_origin_y + chart_area_h as i32
                        - (chart_area_h as f32 * y_ratio) as i32;
                    // Cross marker arms
                    let arm = 3i32;
                    context.draw_line_stroke(
                        Point::new(cx - arm, cy),
                        Point::new(cx + arm, cy),
                        line_color,
                        2,
                    );
                    context.draw_line_stroke(
                        Point::new(cx, cy - arm),
                        Point::new(cx, cy + arm),
                        line_color,
                        2,
                    );
                } else if count >= 2 {
                    for i in 0..(count - 1) {
                        let x1 =
                            chart_origin_x + (chart_area_w as i32 * i as i32 / (count - 1) as i32);
                        let x2 = chart_origin_x
                            + (chart_area_w as i32 * (i + 1) as i32 / (count - 1) as i32);

                        let y1_ratio = if range > 0.0 {
                            (self.data[i].value - effective_min) as f32 / range
                        } else {
                            0.5
                        };
                        let y2_ratio = if range > 0.0 {
                            (self.data[i + 1].value - effective_min) as f32 / range
                        } else {
                            0.5
                        };

                        let y1 = chart_origin_y + chart_area_h as i32
                            - (chart_area_h as f32 * y1_ratio) as i32;
                        let y2 = chart_origin_y + chart_area_h as i32
                            - (chart_area_h as f32 * y2_ratio) as i32;

                        context.draw_line_stroke(
                            Point::new(x1, y1),
                            Point::new(x2, y2),
                            line_color,
                            2,
                        );
                    }
                }
            }
            ChartType::Bar => {
                let count = self.data.len();
                if count == 0 {
                    return;
                }

                // Compute gaps and bar widths evenly across the chart area
                let bar_count = count as u32;
                // Reserve one gap before first bar and one after last bar
                let total_gaps = bar_count + 1;
                // Try to use a reasonable bar width, shrinking if needed
                let max_bar_width = 40u32;
                let ideal_bar_width = (chart_area_w / bar_count).min(max_bar_width).max(2);
                let used_width = ideal_bar_width * bar_count;
                let remaining = chart_area_w.saturating_sub(used_width);
                let gap = (remaining / total_gaps).max(1);
                let bar_width = ideal_bar_width;

                for (i, dp) in self.data.iter().enumerate() {
                    let bx =
                        chart_origin_x + gap as i32 * (i + 1) as i32 + bar_width as i32 * i as i32;

                    let bar_h_ratio =
                        if range > 0.0 { (dp.value - effective_min) as f32 / range } else { 0.5 };
                    let bar_h = (chart_area_h as f32 * bar_h_ratio) as u32;

                    if bar_h == 0 {
                        continue;
                    }

                    let by = chart_origin_y + chart_area_h as i32 - bar_h as i32;

                    // Proportionally color the bar: low values → blue, high values → red
                    let t = bar_h_ratio.clamp(0.0, 1.0);
                    let r = (60.0 + t * 195.0) as u8;
                    let g = (120.0 + t * 50.0) as u8;
                    let b = (200.0 - t * 180.0) as u8;

                    context.fill_rect(Rect::new(bx, by, bar_width, bar_h), Color::rgb(r, g, b));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

    #[test]
    fn mini_chart_creation() {
        let chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        assert_eq!(chart.chart_type(), ChartType::Line);
        assert!(chart.data().is_empty());
        assert_eq!(chart.size_hint(), Size::new(200, 150));
    }

    #[test]
    fn mini_chart_set_data() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        let data = vec![DataPoint::new("A", 10), DataPoint::new("B", 50), DataPoint::new("C", 30)];
        chart.set_data(data);
        assert_eq!(chart.data().len(), 3);
        assert_eq!(chart.data()[0].label, "A");
        assert_eq!(chart.data()[0].value, 10);
        assert_eq!(chart.data()[1].value, 50);
        assert_eq!(chart.data()[2].value, 30);
    }

    #[test]
    fn mini_chart_set_chart_type() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        assert_eq!(chart.chart_type(), ChartType::Line);
        chart.set_chart_type(ChartType::Bar);
        assert_eq!(chart.chart_type(), ChartType::Bar);
        chart.set_chart_type(ChartType::Line);
        assert_eq!(chart.chart_type(), ChartType::Line);
    }

    #[test]
    fn mini_chart_set_range() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_range(0, 200);
        chart.set_range(500, 100); // inverted — should swap
                                   // Does not panic; range is stored internally
    }

    #[test]
    fn mini_chart_draw_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        let data = vec![
            DataPoint::new("Jan", 10),
            DataPoint::new("Feb", 50),
            DataPoint::new("Mar", 80),
            DataPoint::new("Apr", 30),
        ];
        chart.set_data(data);
        chart.set_chart_type(ChartType::Line);

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }

    #[test]
    fn mini_chart_draw_empty_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }

    #[test]
    fn mini_chart_draw_bar_no_panic() {
        let mut chart = MiniChart::new(Rect::new(0, 0, 200, 150));
        chart.set_data(vec![
            DataPoint::new("X", 25),
            DataPoint::new("Y", 70),
            DataPoint::new("Z", 45),
        ]);
        chart.set_chart_type(ChartType::Bar);

        let size = Size::new(200, 150);
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(Color::WHITE);
        let mut ctx = RenderContext::new(&mut backend);
        chart.draw(&mut ctx);
    }
}
