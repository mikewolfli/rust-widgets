//! Chart rendering tests

use rust_widgets::chart::{
    AreaChart, BarChart, Chart, ChartContext, ChartSeries, DataPoint, LineChart,
    MemoryChartContext, PieChart, ScatterChart,
};
use rust_widgets::core::{Color, Point, Rect};

#[test]
fn test_data_point() {
    let point = DataPoint {
        x: 10.0,
        y: 20.0,
        label: Some("Test".to_string()),
    };

    assert_eq!(point.x, 10.0);
    assert_eq!(point.y, 20.0);
    assert_eq!(point.label, Some("Test".to_string()));
}

#[test]
fn test_chart_series() {
    let series = ChartSeries {
        name: "Series 1".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 1.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 2.0,
                label: None,
            },
            DataPoint {
                x: 2.0,
                y: 3.0,
                label: None,
            },
        ],
        color: Color::rgba(0, 0, 255, 255),
        visible: true,
    };

    assert_eq!(series.name, "Series 1");
    assert_eq!(series.data.len(), 3);
    assert!(series.visible);
}

#[test]
fn test_line_chart_creation() {
    let chart = LineChart::new();
    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_bar_chart_creation() {
    let chart = BarChart::new();
    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_pie_chart_creation() {
    let chart = PieChart::new();
    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_scatter_chart_creation() {
    let chart = ScatterChart::new();
    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_area_chart_creation() {
    let chart = AreaChart::new();
    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_chart_series_data() {
    let series = ChartSeries {
        name: "Temperature".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 20.5,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 22.3,
                label: None,
            },
            DataPoint {
                x: 2.0,
                y: 25.1,
                label: None,
            },
        ],
        color: Color::rgba(255, 0, 0, 255),
        visible: true,
    };

    let sum: f64 = series.data.iter().map(|p| p.y).sum();
    assert!((sum - 67.9).abs() < 0.01);
}

#[test]
fn test_chart_series_visibility() {
    let series = ChartSeries {
        name: "Hidden Series".to_string(),
        data: vec![],
        color: Color::rgba(128, 128, 128, 255),
        visible: false,
    };

    assert!(!series.visible);
}

#[test]
fn test_memory_chart_context() {
    let mut context = MemoryChartContext::default();

    context.draw_line(0.0, 0.0, 100.0, 100.0, 2.0, Color::rgba(0, 0, 0, 255));
    context.draw_rect(Rect::new(10, 10, 50, 50), Color::rgba(255, 0, 0, 255));
    context.draw_text("Test", 20.0, 30.0, 12.0, Color::rgba(0, 0, 0, 255));
    context.draw_circle(Point { x: 50, y: 50 }, 10.0, Color::rgba(0, 0, 255, 255));

    assert_eq!(context.commands.len(), 4);
    assert!(context.commands[0].starts_with("line:"));
    assert!(context.commands[1].starts_with("rect:"));
    assert!(context.commands[2].starts_with("text:"));
    assert!(context.commands[3].starts_with("circle:"));
}

#[test]
fn test_chart_add_series() {
    let mut chart = LineChart::new();

    let series = ChartSeries {
        name: "Series 1".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 1.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 2.0,
                label: None,
            },
        ],
        color: Color::rgba(0, 0, 255, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_chart_remove_series() {
    let mut chart = LineChart::new();

    let series = ChartSeries {
        name: "Series 1".to_string(),
        data: vec![DataPoint {
            x: 0.0,
            y: 1.0,
            label: None,
        }],
        color: Color::rgba(0, 0, 255, 255),
        visible: true,
    };

    chart.add_series(series);
    chart.remove_series("Series 1");

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_chart_clear_series() {
    let mut chart = LineChart::new();

    let series1 = ChartSeries {
        name: "Series 1".to_string(),
        data: vec![DataPoint {
            x: 0.0,
            y: 1.0,
            label: None,
        }],
        color: Color::rgba(0, 0, 255, 255),
        visible: true,
    };

    let series2 = ChartSeries {
        name: "Series 2".to_string(),
        data: vec![DataPoint {
            x: 0.0,
            y: 2.0,
            label: None,
        }],
        color: Color::rgba(255, 0, 0, 255),
        visible: true,
    };

    chart.add_series(series1);
    chart.add_series(series2);
    chart.clear_series();

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);
    chart.draw(rect, &mut context);
}

#[test]
fn test_chart_draw() {
    let mut chart = LineChart::new();
    chart.set_title("Test Chart".to_string());

    let series = ChartSeries {
        name: "Data".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 10.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 20.0,
                label: None,
            },
            DataPoint {
                x: 2.0,
                y: 15.0,
                label: None,
            },
        ],
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_bar_chart_draw() {
    let mut chart = BarChart::new();

    let series = ChartSeries {
        name: "Sales".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 100.0,
                label: Some("Q1".to_string()),
            },
            DataPoint {
                x: 1.0,
                y: 150.0,
                label: Some("Q2".to_string()),
            },
            DataPoint {
                x: 2.0,
                y: 120.0,
                label: Some("Q3".to_string()),
            },
        ],
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_pie_chart_draw() {
    let mut chart = PieChart::new();

    let series = ChartSeries {
        name: "Market Share".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 40.0,
                label: Some("Product A".to_string()),
            },
            DataPoint {
                x: 1.0,
                y: 30.0,
                label: Some("Product B".to_string()),
            },
            DataPoint {
                x: 2.0,
                y: 30.0,
                label: Some("Product C".to_string()),
            },
        ],
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_scatter_chart_draw() {
    let mut chart = ScatterChart::new();

    let series = ChartSeries {
        name: "Points".to_string(),
        data: vec![
            DataPoint {
                x: 1.0,
                y: 2.0,
                label: None,
            },
            DataPoint {
                x: 2.0,
                y: 4.0,
                label: None,
            },
            DataPoint {
                x: 3.0,
                y: 3.0,
                label: None,
            },
            DataPoint {
                x: 4.0,
                y: 5.0,
                label: None,
            },
        ],
        color: Color::rgba(255, 128, 0, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_area_chart_draw() {
    let mut chart = AreaChart::new();

    let series = ChartSeries {
        name: "Growth".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 10.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 25.0,
                label: None,
            },
            DataPoint {
                x: 2.0,
                y: 40.0,
                label: None,
            },
            DataPoint {
                x: 3.0,
                y: 35.0,
                label: None,
            },
        ],
        color: Color::rgba(0, 200, 100, 255),
        visible: true,
    };

    chart.add_series(series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_multiple_series_chart() {
    let mut chart = LineChart::new();

    let series1 = ChartSeries {
        name: "Series A".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 10.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 20.0,
                label: None,
            },
        ],
        color: Color::rgba(255, 0, 0, 255),
        visible: true,
    };

    let series2 = ChartSeries {
        name: "Series B".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 15.0,
                label: None,
            },
            DataPoint {
                x: 1.0,
                y: 25.0,
                label: None,
            },
        ],
        color: Color::rgba(0, 0, 255, 255),
        visible: true,
    };

    chart.add_series(series1);
    chart.add_series(series2);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_chart_with_hidden_series() {
    let mut chart = LineChart::new();

    let visible_series = ChartSeries {
        name: "Visible".to_string(),
        data: vec![DataPoint {
            x: 0.0,
            y: 10.0,
            label: None,
        }],
        color: Color::rgba(0, 255, 0, 255),
        visible: true,
    };

    let hidden_series = ChartSeries {
        name: "Hidden".to_string(),
        data: vec![DataPoint {
            x: 0.0,
            y: 20.0,
            label: None,
        }],
        color: Color::rgba(255, 0, 0, 255),
        visible: false,
    };

    chart.add_series(visible_series);
    chart.add_series(hidden_series);

    let mut context = MemoryChartContext::default();
    let rect = Rect::new(0, 0, 400, 300);

    chart.draw(rect, &mut context);

    assert!(!context.commands.is_empty());
}

#[test]
fn test_chart_series_with_labels() {
    let series = ChartSeries {
        name: "Labeled Data".to_string(),
        data: vec![
            DataPoint {
                x: 0.0,
                y: 10.0,
                label: Some("Point A".to_string()),
            },
            DataPoint {
                x: 1.0,
                y: 20.0,
                label: Some("Point B".to_string()),
            },
            DataPoint {
                x: 2.0,
                y: 15.0,
                label: Some("Point C".to_string()),
            },
        ],
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };

    assert_eq!(series.data[0].label, Some("Point A".to_string()));
    assert_eq!(series.data[1].label, Some("Point B".to_string()));
    assert_eq!(series.data[2].label, Some("Point C".to_string()));
}

#[test]
fn test_chart_context_operations() {
    let mut context = MemoryChartContext::default();

    context.draw_line(10.0, 20.0, 30.0, 40.0, 1.5, Color::rgba(255, 0, 0, 255));
    assert_eq!(context.commands.len(), 1);

    context.draw_rect(Rect::new(0, 0, 100, 100), Color::rgba(0, 255, 0, 255));
    assert_eq!(context.commands.len(), 2);

    context.draw_text("Hello", 50.0, 50.0, 14.0, Color::rgba(0, 0, 0, 255));
    assert_eq!(context.commands.len(), 3);

    context.draw_circle(Point { x: 75, y: 75 }, 5.0, Color::rgba(0, 0, 255, 255));
    assert_eq!(context.commands.len(), 4);
}
