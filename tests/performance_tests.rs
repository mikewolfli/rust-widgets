//! Performance baseline tests

use rust_widgets::chart::{
    BarChart, Chart, ChartContext, ChartSeries, DataPoint, LineChart, MemoryChartContext, PieChart,
};
use rust_widgets::core::{Color, Point, Rect, Size};
use std::time::Instant;

const ITERATIONS: usize = 1000;

fn measure_time<F>(f: F) -> u128
where
    F: Fn(),
{
    let start = Instant::now();
    f();
    start.elapsed().as_micros()
}

fn average_time<F>(iterations: usize, f: F) -> f64
where
    F: Fn(),
{
    let mut total = 0u128;
    for _ in 0..iterations {
        total += measure_time(&f);
    }
    total as f64 / iterations as f64
}

#[test]
fn benchmark_rect_creation() {
    let avg_time = average_time(ITERATIONS, || {
        let rect = Rect::new(0, 0, 100, 100);
        std::hint::black_box(rect);
    });

    println!("Rect creation: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Rect creation should be under 10µs");
}

#[test]
fn benchmark_rect_operations() {
    let avg_time = average_time(ITERATIONS, || {
        let rect = Rect::new(0, 0, 100, 100);
        let _x = rect.x;
        let _y = rect.y;
        let _w = rect.width;
        let _h = rect.height;
        let _area = rect.width * rect.height;
    });

    println!("Rect operations: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Rect operations should be under 10µs");
}

#[test]
fn benchmark_color_creation() {
    let avg_time = average_time(ITERATIONS, || {
        let color = Color::rgba(128, 128, 128, 255);
        std::hint::black_box(color);
    });

    println!("Color creation: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Color creation should be under 10µs");
}

#[test]
fn benchmark_color_operations() {
    let avg_time = average_time(ITERATIONS, || {
        let color = Color::rgba(128, 128, 128, 255);
        let _r = color.r;
        let _g = color.g;
        let _b = color.b;
        let _a = color.a;
    });

    println!("Color operations: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Color operations should be under 10µs");
}

#[test]
fn benchmark_point_creation() {
    let avg_time = average_time(ITERATIONS, || {
        let point = Point { x: 10, y: 20 };
        std::hint::black_box(point);
    });

    println!("Point creation: {:.2} µs", avg_time);
    assert!(avg_time < 5.0, "Point creation should be under 5µs");
}

#[test]
fn benchmark_point_operations() {
    let avg_time = average_time(ITERATIONS, || {
        let p1 = Point { x: 10, y: 20 };
        let p2 = Point { x: 30, y: 40 };
        let _dx = (p2.x - p1.x) as f64;
        let _dy = (p2.y - p1.y) as f64;
        let _dist = (_dx * _dx + _dy * _dy).sqrt();
        let _mid_x = (p1.x + p2.x) / 2;
        let _mid_y = (p1.y + p2.y) / 2;
    });

    println!("Point operations: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Point operations should be under 10µs");
}

#[test]
fn benchmark_size_creation() {
    let avg_time = average_time(ITERATIONS, || {
        let size = Size::new(100, 200);
        std::hint::black_box(size);
    });

    println!("Size creation: {:.2} µs", avg_time);
    assert!(avg_time < 5.0, "Size creation should be under 5µs");
}

#[test]
fn benchmark_size_operations() {
    let avg_time = average_time(ITERATIONS, || {
        let size = Size::new(100, 200);
        let _w = size.width;
        let _h = size.height;
        let _area = size.width * size.height;
    });

    println!("Size operations: {:.2} µs", avg_time);
    assert!(avg_time < 10.0, "Size operations should be under 10µs");
}

#[test]
fn benchmark_chart_series_creation() {
    let avg_time = average_time(ITERATIONS, || {
        let series = ChartSeries {
            name: "Test Series".to_string(),
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
        std::hint::black_box(series);
    });

    println!("Chart series creation: {:.2} µs", avg_time);
    assert!(
        avg_time < 50.0,
        "Chart series creation should be under 50µs"
    );
}

#[test]
fn benchmark_chart_series_large() {
    let avg_time = average_time(100, || {
        let series = ChartSeries {
            name: "Large Series".to_string(),
            data: (0..1000)
                .map(|i| DataPoint {
                    x: i as f64,
                    y: (i as f64).sin() * 50.0,
                    label: None,
                })
                .collect(),
            color: Color::rgba(0, 128, 255, 255),
            visible: true,
        };
        std::hint::black_box(series);
    });

    println!("Chart series creation (1000 points): {:.2} µs", avg_time);
    assert!(
        avg_time < 500.0,
        "Large chart series creation should be under 500µs"
    );
}

#[test]
fn benchmark_line_chart_draw() {
    let mut chart = LineChart::new();
    chart.set_title("Performance Test".to_string());

    let series = ChartSeries {
        name: "Data".to_string(),
        data: (0..100)
            .map(|i| DataPoint {
                x: i as f64,
                y: (i as f64).sin() * 50.0 + 50.0,
                label: None,
            })
            .collect(),
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };
    chart.add_series(series);

    let avg_time = average_time(100, || {
        let mut context = MemoryChartContext::default();
        let rect = Rect::new(0, 0, 800, 600);
        chart.draw(rect, &mut context);
        std::hint::black_box(context);
    });

    println!("Line chart draw (100 points): {:.2} µs", avg_time);
    assert!(avg_time < 1000.0, "Line chart draw should be under 1ms");
}

#[test]
fn benchmark_bar_chart_draw() {
    let mut chart = BarChart::new();

    let series = ChartSeries {
        name: "Data".to_string(),
        data: (0..50)
            .map(|i| DataPoint {
                x: i as f64,
                y: (i * i) as f64,
                label: Some(format!("Bar {}", i)),
            })
            .collect(),
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };
    chart.add_series(series);

    let avg_time = average_time(100, || {
        let mut context = MemoryChartContext::default();
        let rect = Rect::new(0, 0, 800, 600);
        chart.draw(rect, &mut context);
        std::hint::black_box(context);
    });

    println!("Bar chart draw (50 bars): {:.2} µs", avg_time);
    assert!(avg_time < 1000.0, "Bar chart draw should be under 1ms");
}

#[test]
fn benchmark_pie_chart_draw() {
    let mut chart = PieChart::new();

    let series = ChartSeries {
        name: "Data".to_string(),
        data: (0..10)
            .map(|i| DataPoint {
                x: i as f64,
                y: (i + 1) as f64 * 10.0,
                label: Some(format!("Slice {}", i)),
            })
            .collect(),
        color: Color::rgba(0, 128, 255, 255),
        visible: true,
    };
    chart.add_series(series);

    let avg_time = average_time(100, || {
        let mut context = MemoryChartContext::default();
        let rect = Rect::new(0, 0, 400, 400);
        chart.draw(rect, &mut context);
        std::hint::black_box(context);
    });

    println!("Pie chart draw (10 slices): {:.2} µs", avg_time);
    assert!(avg_time < 500.0, "Pie chart draw should be under 500µs");
}

#[test]
fn benchmark_vector_operations() {
    let data: Vec<f64> = (0..1000).map(|i| i as f64).collect();

    let avg_time = average_time(ITERATIONS, || {
        let sum: f64 = data.iter().sum();
        let avg = sum / data.len() as f64;
        let _variance: f64 =
            data.iter().map(|x| (x - avg).powi(2)).sum::<f64>() / data.len() as f64;
    });

    println!("Vector operations (1000 elements): {:.2} µs", avg_time);
    assert!(avg_time < 100.0, "Vector operations should be under 100µs");
}

#[test]
fn benchmark_string_operations() {
    let avg_time = average_time(ITERATIONS, || {
        let s = format!("Widget_{}_{}_{}", 1, 2, 3);
        let _len = s.len();
        let _contains = s.contains('_');
        let _upper = s.to_uppercase();
    });

    println!("String operations: {:.2} µs", avg_time);
    assert!(avg_time < 50.0, "String operations should be under 50µs");
}

#[test]
fn benchmark_hash_map_operations() {
    use std::collections::HashMap;

    let avg_time = average_time(ITERATIONS, || {
        let mut map: HashMap<i32, String> = HashMap::new();
        for i in 0..100 {
            map.insert(i, format!("value_{}", i));
        }
        for i in 0..100 {
            let _ = map.get(&i);
        }
    });

    println!("HashMap operations (100 entries): {:.2} µs", avg_time);
    assert!(avg_time < 200.0, "HashMap operations should be under 200µs");
}

#[test]
fn benchmark_memory_chart_context() {
    let avg_time = average_time(ITERATIONS, || {
        let mut context = MemoryChartContext::default();
        for i in 0..100 {
            context.draw_line(
                i as f32,
                0.0,
                i as f32,
                100.0,
                1.0,
                Color::rgba(0, 0, 0, 255),
            );
        }
        std::hint::black_box(context);
    });

    println!("MemoryChartContext (100 lines): {:.2} µs", avg_time);
    assert!(avg_time < 100.0, "MemoryChartContext should be under 100µs");
}

#[test]
fn benchmark_embedded_config() {
    use rust_widgets::core::Size;
    use rust_widgets::embedded::EmbeddedConfig;

    let avg_time = average_time(ITERATIONS, || {
        let config = EmbeddedConfig::new(Size::new(800, 600))
            .with_fixed_dpi(96)
            .low_memory()
            .with_touch(true);
        std::hint::black_box(config);
    });

    println!("EmbeddedConfig creation: {:.2} µs", avg_time);
    assert!(
        avg_time < 50.0,
        "EmbeddedConfig creation should be under 50µs"
    );
}

#[test]
fn benchmark_lightweight_config() {
    use rust_widgets::embedded::LightweightConfig;

    let avg_time = average_time(ITERATIONS, || {
        let config = LightweightConfig::minimal();
        std::hint::black_box(config);
    });

    println!("LightweightConfig creation: {:.2} µs", avg_time);
    assert!(
        avg_time < 20.0,
        "LightweightConfig creation should be under 20µs"
    );
}

#[test]
fn performance_summary() {
    println!("\n=== Performance Baseline Summary ===");
    println!("All benchmarks should complete within their time limits.");
    println!("These baselines can be used to detect performance regressions.");
}
