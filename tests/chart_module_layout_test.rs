// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Structural guard for the chart engine/widget merge.
//!
//! The chart *engine* (layout, axes, ticks, SVG, adapter) and the chart
//! *widgets* used to live in two disconnected trees: a top-level `chart/` module
//! and `widget/chart_widgets/`. That split invited the widgets to hand-roll the
//! engine's math (they did, three times) and made the engine read as an orphan
//! dependency. They now live together under `widget::chart_widgets`.
//!
//! This test pins the merged shape so a future refactor cannot silently
//! re-introduce the split — the failure mode being an empty/absent engine rather
//! than a compile error, which review would not catch on its own.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::core::{Color, Rect};
use rust_widgets::layout::Layout;
use rust_widgets::widget::chart_widgets::charts::{
    compute_cartesian_layout, draw_cartesian_axes, draw_legend, draw_x_ticks, draw_y_ticks,
};
use rust_widgets::widget::chart_widgets::layout::ChartLayout;
use rust_widgets::widget::chart_widgets::svg::SvgChartContext;
use rust_widgets::widget::chart_widgets::types::ChartSeries;
use rust_widgets::widget::chart_widgets::{BarChart, BarEntry, LineChart, PieChart, Sparkline};
use rust_widgets::Widget;

/// The engine's cartesian layout must be reachable from the same module tree as
/// the widgets — this is the whole point of the merge.
#[test]
fn chart_engine_lives_beside_chart_widgets() {
    let layout = compute_cartesian_layout(Rect::new(0, 0, 320, 200), true, true, 1);
    assert!(layout.plot_w() > 0.0, "engine must shrink the plot area for axes");
    assert!(layout.plot_h() > 0.0, "engine must shrink the plot area for a title");
}

/// The full engine pipeline must render through the SVG context without a widget
/// in sight: this is the engine layer being independently usable. Before the
/// merge this path was unreachable from the widget tree.
#[test]
fn chart_engine_renders_through_svg_context_without_widgets() {
    let rect = Rect::new(0, 0, 320, 200);
    let series = ChartSeries {
        name: "revenue".to_string(),
        data: vec![],
        color: Color::BLUE,
        visible: true,
    };
    let layout = compute_cartesian_layout(rect, true, true, 1);
    let mut context = SvgChartContext::new(320, 200);
    draw_cartesian_axes(&mut context, &layout);
    draw_y_ticks(&mut context, &layout, 0.0, 100.0, 5, true);
    draw_x_ticks(&mut context, &layout, 0.0, 10.0, 5, false);
    draw_legend(&mut context, &layout, &[&series]);
    let svg = context.to_svg_string();
    assert!(svg.starts_with("<svg"), "engine must emit real SVG output");
    assert!(svg.contains("revenue"), "engine legend must reach the output");
}

/// `ChartLayout` is part of the engine layer and must remain wired to the
/// `Layout` trait from the same tree.
#[test]
fn chart_layout_stays_a_layout_implementation() {
    let mut layout = ChartLayout::new(7);
    assert!(layout.has_child(7));
    layout.clear();
    assert!(!layout.has_child(7));
}

/// The control layer must remain in the same module tree as the engine, and the
/// bare names must keep meaning the widgets (not the engine's renderers), which
/// is why the engine's same-named types are only reachable via `charts::`.
#[test]
fn chart_widgets_remain_the_control_layer() {
    let mut bar = BarChart::new(Rect::new(0, 0, 200, 120));
    bar.add_bar(BarEntry::new("a", 3.0));
    bar.add_bar(BarEntry::new("b", 5.0));

    let mut line = LineChart::new(Rect::new(0, 0, 200, 120));
    line.add_point(1.0, 2.0);
    line.add_point(3.0, 4.0);

    let mut pie = PieChart::new(Rect::new(0, 0, 200, 120));
    pie.add_slice(rust_widgets::widget::chart_widgets::pie_chart::PieSlice::new(
        "x",
        1.0,
        Color::BLUE,
    ));

    let mut spark = Sparkline::new(Rect::new(0, 0, 200, 40));
    spark.set_data(vec![1.0, 3.0, 2.0]);

    // Touch each so the control-layer constructors are genuinely exercised.
    assert_eq!(bar.geometry().width, 200);
    assert_eq!(bar.bar_count(), 2);
    assert_eq!(line.geometry().width, 200);
    assert_eq!(pie.geometry().width, 200);
    assert_eq!(spark.data().len(), 3);
}
