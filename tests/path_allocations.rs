// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! GV4-10: the allocation cost of the immediate three-vertex path.
//!
//! # What this measures, and why it is the evidence the issue asked for
//!
//! GV4-10 was gated on showing that the `Arc<[Point]>` copy in `DrawPath` is an observable share
//! of a frame. The existing frame benchmarks are built from buttons and rounded rectangles, so
//! they never take this path and cannot answer the question. This harness measures the two shapes
//! directly in the same binary:
//!
//! * a **three-vertex** path, which the renderer now records as the inline `DrawPath3` variant;
//! * a **four-vertex** control with the same fill, which still goes through `translated_path_points`.
//!
//! A `draw_path` miss in the translation cache allocates twice — the `Arc<[Point]>` the command
//! carries, and the key vector the cache keeps for an exact comparison. The control varies its
//! geometry on every draw so it keeps taking those misses; the three-vertex case must allocate
//! **nothing** once the rasteriser's intersection scratch is warm. If the inline branch is removed
//! the counts converge and this test fails, which is what makes it a gate rather than a note.
//!
//! The window covers draws only, not `begin_frame`/`end_frame`: those are per-frame costs that both
//! shapes pay equally, and including them would hide the per-path difference this measures.

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

/// Runs `draw` once per step inside the counting window and returns the allocation count.
fn allocations_for(steps: usize, mut draw: impl FnMut(usize)) -> usize {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    for step in 0..steps {
        draw(step);
    }
    COUNTING.store(false, Ordering::Relaxed);
    ALLOCATIONS.load(Ordering::Relaxed)
}

#[test]
fn a_three_vertex_path_allocates_nothing_while_the_translated_control_still_does() {
    use rust_widgets::core::{Color, Point, Size};
    use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};

    const STEPS: usize = 8;
    let mut backend = SoftwarePaintBackend::new(Size::new(64, 64), 1.0);
    backend.begin_frame(Color::WHITE);

    // Warm up outside the window: the first fill grows the reusable intersection scratch, which is
    // a per-surface cost both shapes pay, not a per-path one.
    {
        let mut context = RenderContext::new(&mut backend);
        context.draw_path(
            &[Point::new(2, 2), Point::new(60, 2), Point::new(2, 60)],
            true,
            Color::BLACK,
            true,
            1,
        );
        context.draw_path(
            &[Point::new(2, 30), Point::new(60, 30), Point::new(60, 60), Point::new(2, 60)],
            true,
            Color::BLACK,
            true,
            1,
        );
    }

    let triangle = allocations_for(STEPS, |step| {
        let inset = step as i32;
        let mut context = RenderContext::new(&mut backend);
        context.draw_path(
            &[Point::new(2 + inset, 2), Point::new(60, 2), Point::new(2, 60)],
            true,
            Color::BLACK,
            true,
            1,
        );
    });

    let quad = allocations_for(STEPS, |step| {
        // A distinct polygon per step, so the translation cache misses every time: this is the
        // shape the inline variant removes, and it must stay visible in the same measurement.
        let inset = step as i32;
        let mut context = RenderContext::new(&mut backend);
        context.draw_path(
            &[Point::new(2 + inset, 30), Point::new(60, 30), Point::new(60, 60), Point::new(2, 60)],
            true,
            Color::BLACK,
            true,
            1,
        );
    });

    backend.end_frame();

    assert_eq!(
        triangle, 0,
        "an inline three-vertex path must not allocate once the rasteriser is warm"
    );
    assert!(
        quad >= STEPS,
        "the four-vertex control must keep allocating, or the window measured nothing (got {quad})"
    );
}
