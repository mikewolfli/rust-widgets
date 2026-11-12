// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! R5-02: a repainted `AreaChart` must allocate nothing once its scratch is warm.
//!
//! # What this measures
//!
//! `AreaChart::draw` used to build three buffers on every paint — two `Vec<f64>` (stacked
//! totals for the bounds pass, then a running accumulation) and one `Vec<Point>` per visible
//! series for the polygon. The first two are `2` allocations per frame; the third is `S`
//! allocations per frame, one per series, sized `2N + 2` points. They are now taken from
//! thread-local pools and returned when their guard drops.
//!
//! The measurement is an **allocation count**, not a wall-clock time: the count is deterministic
//! and platform-independent, and it is the property the pooling exists to guarantee. A timing
//! benchmark would mostly measure the no-op context below.
//!
//! # Why the window starts after a warm-up
//!
//! The pools start empty and grow to the chart's size on first use; the axis labels also keep a
//! thread-local `String` whose first write allocates. Those are one-time costs, exactly like the
//! rasteriser's warm-up scratch the sibling `path_allocations` harness documents. Counting them
//! would make the gate assert "this is the first draw" rather than "steady state is flat".
//!
//! # Why one test rather than one per mode
//!
//! The allocator counts every thread while the flag is set, and the test harness runs tests in
//! parallel. Two counting windows in one binary could overlap and charge each other's
//! allocations. One test keeps the window exclusive by construction.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: Every operation delegates to `System` with the original layout and pointer.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `layout` is supplied by the allocator caller and passed through unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` are supplied by the allocator caller and passed through.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The old pointer/layout and requested size are passed through unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Counts the allocations made while `draw` runs `steps` times.
fn allocations_for(steps: usize, mut draw: impl FnMut(usize)) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for step in 0..steps {
        draw(step);
    }
    COUNTING.store(false, Ordering::Relaxed);
    ALLOCATIONS.load(Ordering::Relaxed)
}

/// A chart context that records nothing, so the harness measures the chart's own allocations.
///
/// Every method is a no-op by design: a recording context (`MemoryChartContext`) would build a
/// `String` per command and swamp the numbers this test is about.
struct NoopContext;

impl rust_widgets::widget::chart_widgets::types::ChartContext for NoopContext {
    fn draw_line(
        &mut self,
        _from: rust_widgets::core::Point,
        _to: rust_widgets::core::Point,
        _width: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_rect(&mut self, _rect: rust_widgets::core::Rect, _color: rust_widgets::core::Color) {}
    fn draw_text(
        &mut self,
        _text: &str,
        _pos: rust_widgets::core::Point,
        _font_size: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn text_line(
        &self,
        band: rust_widgets::core::Rect,
        _font_size: f32,
    ) -> rust_widgets::core::Rect {
        band
    }
    fn draw_circle(
        &mut self,
        _center: rust_widgets::core::Point,
        _radius: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_polygon(
        &mut self,
        _points: &[rust_widgets::core::Point],
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_path_segment(
        &mut self,
        _start: rust_widgets::core::Point,
        _end: rust_widgets::core::Point,
        _width: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_arc(
        &mut self,
        _center: rust_widgets::core::Point,
        _radius: f32,
        _start_angle: f64,
        _end_angle: f64,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_path(
        &mut self,
        _points: &[rust_widgets::core::Point],
        _width: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn draw_ellipse(
        &mut self,
        _center: rust_widgets::core::Point,
        _radius_x: f32,
        _radius_y: f32,
        _color: rust_widgets::core::Color,
    ) {
    }
    fn set_fill_color(&mut self, _color: rust_widgets::core::Color) {}
    fn set_stroke_color(&mut self, _color: rust_widgets::core::Color) {}
}

#[test]
fn a_repainted_area_chart_allocates_nothing_once_its_scratch_is_warm() {
    use rust_widgets::core::{Color, Rect};
    use rust_widgets::widget::chart_widgets::charts::AreaChart;
    use rust_widgets::widget::chart_widgets::types::{Chart, ChartSeries, DataPoint};

    const POINTS: usize = 366;
    const WARM_UP: usize = 3;
    const STEPS: usize = 8;
    let rect = Rect::new(0, 0, 320, 200);

    fn series(name: &str, color: Color, offset: f64) -> ChartSeries {
        ChartSeries {
            name: name.to_string(),
            data: (0..POINTS)
                .map(|index| DataPoint {
                    x: index as f64,
                    y: offset + (index as f64 * 0.1).sin(),
                    label: None,
                })
                .collect(),
            color,
            visible: true,
        }
    }

    let mut chart = AreaChart::new();
    chart.add_series(series("Alpha", Color::RED, 10.0));
    chart.add_series(series("Bravo", Color::GREEN, 20.0));
    chart.add_series(series("Charlie", Color::BLUE, 30.0));
    chart.set_stacked(true);
    let mut context = NoopContext;

    // Warm-up is outside every counting window: first draws grow the pooled buffers and the
    // thread-local axis-label strings to their steady-state capacity.
    for _ in 0..WARM_UP {
        chart.draw(rect, &mut context);
    }
    let stacked = allocations_for(STEPS, |_| chart.draw(rect, &mut context));
    assert_eq!(
        stacked, 0,
        "a stacked area chart must reuse its totals and polygon buffers instead of allocating \
         {stacked} times across {STEPS} repaints"
    );

    // The unstacked path draws no totals at all, but it still builds one polygon per series.
    // It must be flat for the same reason: the pooled vertex buffer is reused across series.
    chart.set_stacked(false);
    for _ in 0..WARM_UP {
        chart.draw(rect, &mut context);
    }
    let unstacked = allocations_for(STEPS, |_| chart.draw(rect, &mut context));
    assert_eq!(
        unstacked, 0,
        "an unstacked area chart must reuse its polygon buffer instead of allocating \
         {unstacked} times across {STEPS} repaints"
    );
}
