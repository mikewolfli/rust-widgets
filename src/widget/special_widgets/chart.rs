//! Chart widget.
use crate::core::Rect;
use crate::render::RenderContext;
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
}
impl ChartWidget {
    /// Creates a new chart widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chart, geometry, "ChartWidget"),
            chart_type: ChartType::default(),
            data: Vec::new(),
            labels: Vec::new(),
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
        // Draw chart background
        context.fill_rect(rect, Color::from_rgb(255, 255, 255));
        // Draw border to make chart area visible
        context.draw_rect(rect, Color::from_rgb(200, 200, 200));
    }
}
impl crate::event::EventHandler for ChartWidget {
    fn handle_event(&mut self, event: &crate::event::Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            crate::event::Event::MousePress { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(true);
            }
            crate::event::Event::MouseRelease { pos: _, button } if *button == 1 => {
                self.base.set_mouse_pressed(false);
            }
            _ => {}
        }
    }
}
