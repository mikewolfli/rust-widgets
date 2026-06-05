//! Chart widget.
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Chart type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartType {
    #[default]
    Bar,
    Line,
    Pie,
    Scatter,
}

/// Chart widget for data visualization.
pub struct ChartWidget {
    base: BaseWidget,
    chart_type: ChartType,
    data: Vec<f64>,
    labels: Vec<String>,
    hovered_index: Option<usize>,
    /// Emitted when a data point is clicked.
    pub data_point_clicked: Signal1<usize>,
    /// Emitted when pointer hover enters a data point bucket.
    pub data_point_hovered: Signal1<usize>,
}
impl ChartWidget {
    /// Creates a new chart widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "ChartWidget"),
            chart_type: ChartType::default(),
            data: Vec::new(),
            labels: Vec::new(),
            hovered_index: None,
            data_point_clicked: Signal1::new(),
            data_point_hovered: Signal1::new(),
        }
    }

    /// Returns the chart type.
    pub fn chart_type(&self) -> ChartType {
        self.chart_type
    }

    /// Returns the chart data values.
    pub fn data(&self) -> &[f64] {
        &self.data
    }

    /// Returns the chart data labels.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Sets the chart type.
    pub fn set_chart_type(&mut self, chart_type: ChartType) {
        self.chart_type = chart_type;
        self.base.request_redraw();
    }

    /// Sets the chart data values.
    pub fn set_data(&mut self, data: Vec<f64>) {
        self.data = data;
        self.base.request_redraw();
    }

    /// Sets the chart data labels.
    pub fn set_labels(&mut self, labels: Vec<String>) {
        self.labels = labels;
        self.base.request_redraw();
    }

    fn data_index_at(&self, pos: Point) -> Option<usize> {
        if self.data.is_empty() {
            return None;
        }
        let rect = self.base.geometry();
        if !rect.contains_point(pos) || rect.width == 0 {
            return None;
        }
        let local_x = (pos.x - rect.x).max(0) as u32;
        let width = rect.width.max(1);
        let mut idx = ((local_x as u64) * (self.data.len() as u64) / (width as u64)) as usize;
        if idx >= self.data.len() {
            idx = self.data.len() - 1;
        }
        Some(idx)
    }
}
impl Widget for ChartWidget {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for ChartWidget {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.base.geometry();
        use crate::core::Color;
        use crate::core::Font;
        // Draw chart background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border to make chart area visible
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));

        if self.data.is_empty() {
            // No data — draw placeholder text
            let text_origin = crate::core::Point {
                x: rect.x + 4,
                y: rect.y + rect.height as i32 / 2,
            };
            let font = Font::simple("Sans", 12.0);
            context.draw_text(
                text_origin,
                "No data",
                &font,
                Color::from_rgb(180, 180, 180),
            );
            return;
        }

        match self.chart_type {
            ChartType::Bar => self.draw_bar_chart(context, rect),
            ChartType::Line => self.draw_line_chart(context, rect),
            ChartType::Pie => self.draw_pie_chart(context, rect),
            ChartType::Scatter => self.draw_scatter_chart(context, rect),
        }
    }
}

impl ChartWidget {
    fn draw_bar_chart(&self, context: &mut RenderContext, rect: Rect) {
        use crate::core::Color;
        use crate::core::Font;
        let max_val = self.data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if max_val <= 0.0 {
            return;
        }
        let n = self.data.len();
        if n == 0 {
            return;
        }
        let padding: i32 = 8;
        let bottom_margin: i32 = 20;
        let chart_w = (rect.width as i32).saturating_sub(padding * 2);
        let bar_area_x = rect.x.saturating_add(padding);
        let baseline_y = rect
            .y
            .saturating_add(rect.height as i32)
            .saturating_sub(bottom_margin);
        let top_y = rect.y.saturating_add(padding);
        let bar_width = (chart_w / n as i32).max(4).saturating_sub(2);
        let gap = 2;
        let bar_colors = [
            Color::from_rgb(66, 133, 244),
            Color::from_rgb(219, 68, 55),
            Color::from_rgb(244, 180, 0),
            Color::from_rgb(15, 157, 88),
            Color::from_rgb(171, 71, 188),
            Color::from_rgb(0, 172, 193),
        ];
        let label_font = Font::simple("Sans", 10.0);
        for (i, &val) in self.data.iter().enumerate() {
            let i32_i = i as i32;
            let x = bar_area_x.saturating_add(i32_i * (bar_width + gap));
            let bar_h = ((val / max_val) * (baseline_y.saturating_sub(top_y)) as f64) as i32;
            let bar_y = baseline_y.saturating_sub(bar_h).max(top_y);
            let color = bar_colors[i % bar_colors.len()];
            context.fill_rect(
                crate::core::Rect {
                    x,
                    y: bar_y,
                    width: bar_width.max(1) as u32,
                    height: (baseline_y - bar_y).max(1) as u32,
                },
                color,
            );
            if i < self.labels.len() {
                let label = &self.labels[i];
                let label_text = if label.len() > 6 {
                    format!("{}..", &label[..4])
                } else {
                    label.clone()
                };
                let label_origin = crate::core::Point {
                    x: x + 1,
                    y: baseline_y + 12,
                };
                context.draw_text(
                    label_origin,
                    &label_text,
                    &label_font,
                    Color::from_rgb(80, 80, 80),
                );
            }
        }
    }

    fn draw_line_chart(&self, context: &mut RenderContext, rect: Rect) {
        use crate::core::Color;
        use crate::core::Font;
        let max_val = self.data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if max_val <= 0.0 || self.data.len() < 2 {
            return;
        }
        let n = self.data.len();
        let padding: i32 = 8;
        let bottom_margin: i32 = 20;
        let chart_w = (rect.width as i32).saturating_sub(padding * 2);
        let bar_area_x = rect.x.saturating_add(padding);
        let baseline_y = rect
            .y
            .saturating_add(rect.height as i32)
            .saturating_sub(bottom_margin);
        let top_y = rect.y.saturating_add(padding);
        let step_x = chart_w / (n.saturating_sub(1) as i32).max(1);
        let height_range = (baseline_y.saturating_sub(top_y)).max(1) as f64;
        let line_color = Color::from_rgb(66, 133, 244);
        let points: Vec<crate::core::Point> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, &val)| {
                let x = bar_area_x.saturating_add(i as i32 * step_x);
                let y_ratio = val / max_val;
                let y = baseline_y.saturating_sub((y_ratio * height_range) as i32);
                crate::core::Point { x, y }
            })
            .collect();
        for i in 1..points.len() {
            let from = &points[i - 1];
            let to = &points[i];
            context.draw_line_stroke(*from, *to, line_color, 2);
        }
        let label_font = Font::simple("Sans", 10.0);
        for (i, pt) in points.iter().enumerate() {
            context.fill_circle(*pt, 3, line_color);
            if i < self.labels.len() {
                let label = &self.labels[i];
                let label_text = if label.len() > 6 {
                    format!("{}..", &label[..4])
                } else {
                    label.clone()
                };
                let label_origin = crate::core::Point {
                    x: pt.x.saturating_sub(6),
                    y: baseline_y + 12,
                };
                context.draw_text(
                    label_origin,
                    &label_text,
                    &label_font,
                    Color::from_rgb(80, 80, 80),
                );
            }
        }
    }

    fn draw_pie_chart(&self, context: &mut RenderContext, rect: Rect) {
        use crate::core::Color;
        use crate::core::Font;
        let total: f64 = self.data.iter().sum();
        if total <= 0.0 {
            return;
        }
        let cx = rect.x + rect.width as i32 / 2;
        let cy = rect.y + rect.height as i32 / 2;
        let radius = (rect.width.min(rect.height) as i32 / 2)
            .saturating_sub(10)
            .max(10);
        let pie_colors = [
            Color::from_rgb(66, 133, 244),
            Color::from_rgb(219, 68, 55),
            Color::from_rgb(244, 180, 0),
            Color::from_rgb(15, 157, 88),
            Color::from_rgb(171, 71, 188),
            Color::from_rgb(0, 172, 193),
        ];
        let mut start_angle = -std::f64::consts::FRAC_PI_2;
        for (i, &val) in self.data.iter().enumerate() {
            let slice_angle = 2.0 * std::f64::consts::PI * (val / total);
            let mid_angle = start_angle + slice_angle / 2.0;
            let end_angle = start_angle + slice_angle;
            let color = pie_colors[i % pie_colors.len()];
            let num_lines = (radius as f64 * slice_angle * 0.4).ceil() as i32;
            let segments = num_lines.clamp(1, 120);
            for s in 0..segments {
                let t = start_angle + slice_angle * (s as f64 + 0.5) / segments as f64;
                let ex = cx + (radius as f64 * t.cos()) as i32;
                let ey = cy + (radius as f64 * t.sin()) as i32;
                context.draw_line(
                    crate::core::Point { x: cx, y: cy },
                    crate::core::Point { x: ex, y: ey },
                    color,
                );
            }
            let label_radius = radius.saturating_add(14) as f64;
            let lx = cx + (label_radius * mid_angle.cos()) as i32;
            let ly = cy + (label_radius * mid_angle.sin()) as i32;
            let label_font = Font::simple("Sans", 9.0);
            if i < self.labels.len() {
                let pct = val / total * 100.0;
                let label_text = if pct >= 1.0 {
                    format!("{}:{:.0}%", self.labels[i], pct)
                } else {
                    format!("{}:{:.1}%", self.labels[i], pct)
                };
                let label_origin = crate::core::Point { x: lx, y: ly };
                context.draw_text(
                    label_origin,
                    &label_text,
                    &label_font,
                    Color::from_rgb(60, 60, 60),
                );
            }
            start_angle = end_angle;
        }
    }

    fn draw_scatter_chart(&self, context: &mut RenderContext, rect: Rect) {
        use crate::core::Color;
        use crate::core::Font;
        let max_val = self.data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if max_val <= 0.0 || self.data.is_empty() {
            return;
        }
        let n = self.data.len();
        let padding: i32 = 8;
        let bottom_margin: i32 = 20;
        let chart_w = (rect.width as i32).saturating_sub(padding * 2);
        let bar_area_x = rect.x.saturating_add(padding);
        let baseline_y = rect
            .y
            .saturating_add(rect.height as i32)
            .saturating_sub(bottom_margin);
        let top_y = rect.y.saturating_add(padding);
        let step_x = chart_w / (n as i32).max(1);
        let height_range = (baseline_y.saturating_sub(top_y)).max(1) as f64;
        let dot_color = Color::from_rgb(66, 133, 244);
        let label_font = Font::simple("Sans", 10.0);
        for (i, &val) in self.data.iter().enumerate() {
            let x = bar_area_x.saturating_add(i as i32 * step_x);
            let y_ratio = val / max_val;
            let y = baseline_y.saturating_sub((y_ratio * height_range) as i32);
            context.fill_circle(crate::core::Point { x, y }, 3, dot_color);
            if i < self.labels.len() {
                let label = &self.labels[i];
                let label_text = if label.len() > 6 {
                    format!("{}..", &label[..4])
                } else {
                    label.clone()
                };
                let label_origin = crate::core::Point {
                    x: x.saturating_sub(6),
                    y: baseline_y + 12,
                };
                context.draw_text(
                    label_origin,
                    &label_text,
                    &label_font,
                    Color::from_rgb(80, 80, 80),
                );
            }
        }
    }
}
impl EventHandler for ChartWidget {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                if let Some(index) = self.data_index_at(*pos) {
                    if self.hovered_index != Some(index) {
                        self.hovered_index = Some(index);
                        self.data_point_hovered.emit(index);
                    }
                }
            }
            Event::MousePress { pos, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
                if let Some(index) = self.data_index_at(*pos) {
                    self.base.clicked.emit();
                    self.data_point_clicked.emit(index);
                }
            }
            Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn chart_mouse_interaction_emits_data_index_signals() {
        let mut chart = ChartWidget::new(Rect::new(0, 0, 200, 100));
        chart.set_data(vec![10.0, 20.0, 30.0, 40.0]);

        let clicked = Arc::new(Mutex::new(Vec::<usize>::new()));
        let hovered = Arc::new(Mutex::new(Vec::<usize>::new()));

        let clicked_sink = clicked.clone();
        chart.data_point_clicked.connect(move |index| {
            if let Ok(mut guard) = clicked_sink.lock() {
                guard.push(*index);
            }
        });

        let hovered_sink = hovered.clone();
        chart.data_point_hovered.connect(move |index| {
            if let Ok(mut guard) = hovered_sink.lock() {
                guard.push(*index);
            }
        });

        chart.handle_event(&Event::mouse_move(120, 50));
        chart.handle_event(&Event::mouse_press(120, 50, 1));

        let clicked_values = clicked.lock().expect("clicked lock poisoned").clone();
        let hovered_values = hovered.lock().expect("hovered lock poisoned").clone();

        assert_eq!(hovered_values, vec![2]);
        assert_eq!(clicked_values, vec![2]);
    }
}
