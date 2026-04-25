//! Chart tests.

use crate::chart::*;
use crate::chart::{
    BarChart, ChartSeries, DataPoint, LineChart, MemoryChartContext, SvgChartContext,
};
use crate::core::{Color, Rect};
fn stable_hash64(input: &str) -> u64 {
    // FNV-1a 64-bit stable hash for deterministic snapshot checks.
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
fn count_numeric_text_labels(commands: &[String]) -> usize {
    commands
        .iter()
        .filter_map(|cmd| cmd.strip_prefix("text:"))
        .filter_map(|payload| payload.split_once('@').map(|(text, _)| text))
        .filter(|text| text.parse::<f64>().is_ok())
        .count()
}
fn sample_series(name: &str, color: Color, points: &[(f64, f64)]) -> ChartSeries {
    ChartSeries {
        name: name.to_string(),
        data: points
            .iter()
            .map(|(x, y)| DataPoint {
                x: *x,
                y: *y,
                label: None,
            })
            .collect(),
        color,
        visible: true,
    }
}
#[test]
fn line_chart_draws_axis_labels_and_legend() {
    let mut chart = LineChart::new();
    chart.set_title("Latency".to_string());
    chart.set_x_axis_label("Time".to_string());
    chart.set_y_axis_label("ms".to_string());
    chart.add_series(sample_series(
        "p50",
        Color {
            r: 0,
            g: 120,
            b: 255,
            a: 255,
        },
        &[(0.0, 10.0), (1.0, 20.0), (2.0, 16.0)],
    ));
    chart.add_series(sample_series(
        "p95",
        Color {
            r: 255,
            g: 128,
            b: 0,
            a: 255,
        },
        &[(0.0, 15.0), (1.0, 30.0), (2.0, 24.0)],
    ));
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        },
        &mut context,
    );
    assert!(context
        .commands
        .iter()
        .any(|cmd| cmd.contains("text:Time@")));
    assert!(context.commands.iter().any(|cmd| cmd.contains("text:ms@")));
    assert!(context.commands.iter().any(|cmd| cmd.contains("text:p50@")));
    assert!(context.commands.iter().any(|cmd| cmd.contains("text:p95@")));
}
#[test]
fn bar_chart_draws_legend_and_axis_ticks() {
    let mut chart = BarChart::new();
    chart.set_title("Throughput".to_string());
    chart.set_x_axis_label("Bucket".to_string());
    chart.set_y_axis_label("req/s".to_string());
    chart.add_series(sample_series(
        "region-a",
        Color {
            r: 40,
            g: 180,
            b: 99,
            a: 255,
        },
        &[(0.0, 20.0), (1.0, 40.0), (2.0, 30.0)],
    ));
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 500,
            height: 280,
        },
        &mut context,
    );
    assert!(context
        .commands
        .iter()
        .any(|cmd| cmd.contains("text:Bucket@")));
    assert!(context
        .commands
        .iter()
        .any(|cmd| cmd.contains("text:req/s@")));
    assert!(context
        .commands
        .iter()
        .any(|cmd| cmd.contains("text:region-a@")));
    assert!(context.commands.iter().any(|cmd| cmd.starts_with("line:")));
}
#[test]
fn legend_truncates_long_labels() {
    let mut chart = LineChart::new();
    chart.set_title("Legend".to_string());
    chart.add_series(sample_series(
        "this-is-a-very-long-legend-label-that-should-truncate",
        Color {
            r: 0,
            g: 100,
            b: 220,
            a: 255,
        },
        &[(0.0, 1.0), (1.0, 2.0)],
    ));
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 220,
        },
        &mut context,
    );
    let truncated_entry = context
        .commands
        .iter()
        .find(|cmd| cmd.starts_with("text:this-is-a-very-") && cmd.contains("...@"));
    assert!(truncated_entry.is_some());
    assert!(!context
        .commands
        .iter()
        .any(|cmd| cmd.contains("text:this-is-a-very-long-legend-label-that-should-truncate@")));
}
#[test]
fn legend_shows_overflow_summary() {
    let mut chart = LineChart::new();
    chart.set_title("Overflow".to_string());
    for index in 0..10 {
        chart.add_series(sample_series(
            &format!("s{index}"),
            Color {
                r: 20,
                g: 120,
                b: 200,
                a: 255,
            },
            &[(0.0, index as f64 + 1.0), (1.0, index as f64 + 2.0)],
        ));
    }
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 110,
        },
        &mut context,
    );
    assert!(context.commands.iter().any(|cmd| cmd.contains("text:+")));
    assert!(context.commands.iter().any(|cmd| cmd.contains("more@")));
}
#[test]
fn line_chart_respects_tick_density_configuration() {
    let mut chart = LineChart::new();
    chart.set_title("Ticks".to_string());
    chart.set_x_tick_count(3);
    chart.set_y_tick_count(4);
    chart.add_series(sample_series(
        "s1",
        Color {
            r: 0,
            g: 120,
            b: 255,
            a: 255,
        },
        &[(0.0, 0.0), (1.0, 10.0), (2.0, 20.0)],
    ));
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 320,
        },
        &mut context,
    );
    // x ticks: 4 labels (0..=3), y ticks: 5 labels (0..=4)
    assert_eq!(count_numeric_text_labels(&context.commands), 9);
}
#[test]
fn bar_chart_respects_tick_density_configuration() {
    let mut chart = BarChart::new();
    chart.set_title("Bars".to_string());
    chart.set_x_tick_count(6);
    chart.set_y_tick_count(2);
    chart.add_series(sample_series(
        "s1",
        Color {
            r: 20,
            g: 160,
            b: 100,
            a: 255,
        },
        &[(0.0, 5.0), (1.0, 10.0), (2.0, 8.0)],
    ));
    let mut context = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 520,
            height: 280,
        },
        &mut context,
    );
    // x ticks: 7 labels (0..=6), y ticks: 3 labels (0..=2)
    assert_eq!(count_numeric_text_labels(&context.commands), 10);
}
#[test]
fn line_chart_gridline_toggle_changes_line_count() {
    let mut chart = LineChart::new();
    chart.set_title("Grid".to_string());
    chart.set_x_tick_count(3);
    chart.set_y_tick_count(3);
    chart.add_series(sample_series(
        "s1",
        Color {
            r: 0,
            g: 120,
            b: 255,
            a: 255,
        },
        &[(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)],
    ));
    let mut without_grid = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 320,
        },
        &mut without_grid,
    );
    let without_count = without_grid
        .commands
        .iter()
        .filter(|cmd| cmd.starts_with("line:"))
        .count();
    chart.set_grid_enabled(true);
    let mut with_grid = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 320,
        },
        &mut with_grid,
    );
    let with_count = with_grid
        .commands
        .iter()
        .filter(|cmd| cmd.starts_with("line:"))
        .count();
    assert!(with_count > without_count);
}
#[test]
fn bar_chart_gridline_toggle_changes_line_count() {
    let mut chart = BarChart::new();
    chart.set_title("GridBar".to_string());
    chart.set_x_tick_count(4);
    chart.set_y_tick_count(2);
    chart.add_series(sample_series(
        "s1",
        Color {
            r: 20,
            g: 160,
            b: 100,
            a: 255,
        },
        &[(0.0, 5.0), (1.0, 10.0), (2.0, 8.0)],
    ));
    let mut without_grid = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 520,
            height: 280,
        },
        &mut without_grid,
    );
    let without_count = without_grid
        .commands
        .iter()
        .filter(|cmd| cmd.starts_with("line:"))
        .count();
    chart.set_grid_enabled(true);
    let mut with_grid = MemoryChartContext::default();
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 520,
            height: 280,
        },
        &mut with_grid,
    );
    let with_count = with_grid
        .commands
        .iter()
        .filter(|cmd| cmd.starts_with("line:"))
        .count();
    assert!(with_count > without_count);
}
#[test]
fn svg_snapshot_line_chart_stable() {
    let mut chart = LineChart::new();
    chart.set_title("SnapshotLine".to_string());
    chart.set_x_axis_label("X".to_string());
    chart.set_y_axis_label("Y".to_string());
    chart.set_grid_enabled(true);
    chart.set_x_tick_count(4);
    chart.set_y_tick_count(3);
    chart.add_series(sample_series(
        "line-a",
        Color {
            r: 15,
            g: 120,
            b: 240,
            a: 255,
        },
        &[(0.0, 1.0), (1.0, 4.0), (2.0, 2.0), (3.0, 5.0)],
    ));
    let mut context = SvgChartContext::new(640, 360);
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        },
        &mut context,
    );
    let svg = context.to_svg_string();
    let got = stable_hash64(&svg);
    let expected = 17974278823255601663u64;
    assert_eq!(got, expected, "line snapshot hash changed: {got}");
}
#[test]
fn svg_snapshot_bar_chart_stable() {
    let mut chart = BarChart::new();
    chart.set_title("SnapshotBar".to_string());
    chart.set_x_axis_label("Bucket".to_string());
    chart.set_y_axis_label("Value".to_string());
    chart.set_grid_enabled(true);
    chart.set_x_tick_count(5);
    chart.set_y_tick_count(4);
    chart.add_series(sample_series(
        "bar-a",
        Color {
            r: 20,
            g: 170,
            b: 100,
            a: 255,
        },
        &[(0.0, 2.0), (1.0, 5.0), (2.0, 3.0)],
    ));
    let mut context = SvgChartContext::new(640, 360);
    chart.draw(
        Rect {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        },
        &mut context,
    );
    let svg = context.to_svg_string();
    let got = stable_hash64(&svg);
    let expected = 18105083242857139820u64;
    assert_eq!(got, expected, "bar snapshot hash changed: {got}");
}
