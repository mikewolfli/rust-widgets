// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! blue27 §19.13 case I17: the per-frame allocation budget for icon painting.
//!
//! # The defect this measures
//!
//! Before the unified content contract, `Icon` submitted **one `DrawPath` per contour** and the
//! software backend filled each polygon with its own even-odd scanline pass — which cloned a
//! `Vec<Point>` per call and allocated an intersection vector per scanline. blue27 §19.13 recorded
//! the cost: 22-279 allocations for **one icon draw**, so a 30-item toolbar paid ~2.6k-4.2k
//! allocations every frame, and the figure grew with the icon size and the DPI.
//!
//! # What the gate asserts, and why both halves
//!
//! 1. A steady-state draw allocates **zero** times. This is the strong form, and it is the one that
//!    can actually fail: it is what caught the per-contour clone when the fill rule was unified.
//! 2. The measured figure is **not above the recorded U0 baseline**
//!    (`tools/icon_allocations_baseline.txt`, which also states where each baseline number came
//!    from). This is case I17's own wording, and it survives a future change that legitimately needs
//!    one small allocation without silently turning "the numbers regressed" into "the numbers moved".
//!
//! # Why the measurement spans frames as well as draws
//!
//! "Per frame" is the unit the case names, and a backend could in principle move work from the draw
//! into `begin_frame`/`end_frame` — which a draw-only counter would not see, and would score as an
//! improvement. The counting window therefore covers whole frames, with the frame boundary inside it.
//!
//! # Why the baseline is read from the file rather than written here
//!
//! The baseline is evidence with provenance (a number measured against the pre-unification code and
//! recorded in the plan). Reading it from a committed file keeps that provenance visible and makes the
//! comparison auditable; hard-coding the ceiling in the test would restate it without its source, and
//! deleting that file fails the gate rather than being tolerated.

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

/// The recorded `(token, size) -> U0 baseline allocations per draw` table.
fn recorded_baselines() -> Vec<((String, u32), f64)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/icon_allocations_baseline.txt");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("the I17 baseline file must be readable ({path}): {error}"));
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("token") {
            continue;
        }
        let columns: Vec<&str> = line.split_whitespace().collect();
        assert!(
            columns.len() >= 4,
            "baseline row must be `<token> <size> <baseline> <current>`: {line:?}"
        );
        let size: u32 = columns[1]
            .trim_end_matches("px")
            .parse()
            .unwrap_or_else(|_| panic!("baseline row has no numeric size: {line:?}"));
        let baseline: f64 =
            columns[2].parse().unwrap_or_else(|_| panic!("baseline row has no number: {line:?}"));
        // Without this every row could compare equal and the gate would pass vacuously.
        assert!(baseline > 0.0, "a recorded baseline must be a real measurement: {line:?}");
        rows.push(((columns[0].to_string(), size), baseline));
    }
    assert!(!rows.is_empty(), "the baseline file must declare at least one row");
    rows
}

/// Measures allocations per icon draw across whole frames, and asserts the two halves of I17.
#[test]
fn records_and_bounds_repeated_builtin_icon_draw_allocations() {
    use rust_widgets::core::{Color, Rect, Size};
    use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use rust_widgets::widget::{paint_icon, IconName, IconPaintOutcome, IconRef};

    const FRAMES: usize = 10;
    const DRAWS_PER_FRAME: usize = 3;
    let icons = [IconName::Check, IconName::Info, IconName::Settings];
    let baselines = recorded_baselines();

    for size in [24u32, 96u32] {
        let bounds = Rect::new(0, 0, size, size);
        let mut backend = SoftwarePaintBackend::new(Size::new(size, size), 1.0);

        // ── Warm-up, outside the counting window ──
        // The first paint of a token resolves its geometry and grows the backend's buffers; those are
        // one-time costs, and charging them per draw would measure start-up rather than the frame.
        for icon in icons {
            backend.begin_frame(Color::TRANSPARENT);
            {
                let mut context = RenderContext::new(&mut backend);
                paint_icon(&mut context, IconRef::Builtin(icon), bounds, Color::BLACK)
                    .expect("a bundled outline warms the paint path");
            }
            backend.end_frame();
        }

        // ── The counted window: whole frames, frame boundary included ──
        ALLOCATIONS.store(0, Ordering::Relaxed);
        COUNTING.store(true, Ordering::SeqCst);
        for _ in 0..FRAMES {
            backend.begin_frame(Color::TRANSPARENT);
            {
                let mut context = RenderContext::new(&mut backend);
                for icon in icons {
                    for _ in 0..DRAWS_PER_FRAME {
                        let outcome =
                            paint_icon(&mut context, IconRef::Builtin(icon), bounds, Color::BLACK)
                                .expect("a bundled outline paints");
                        assert_eq!(outcome, IconPaintOutcome::Drawn);
                    }
                }
            }
            backend.end_frame();
        }
        COUNTING.store(false, Ordering::SeqCst);
        let total = ALLOCATIONS.swap(0, Ordering::Relaxed);

        let draws = (FRAMES * icons.len() * DRAWS_PER_FRAME) as f64;
        let per_draw = total as f64 / draws;
        let per_frame = total as f64 / FRAMES as f64;

        for icon in icons {
            let baseline = baselines
                .iter()
                .find(|((token, row_size), _)| token == icon.as_str() && *row_size == size)
                .unwrap_or_else(|| {
                    panic!(
                        "the I17 baseline file has no row for {} at {size}px; a new measurement must \
                         be added with its provenance rather than silently skipped",
                        icon.as_str()
                    )
                })
                .1;
            assert!(
                per_draw <= baseline,
                "{} at {size}px allocates {per_draw} per draw ({per_frame} per frame), above the \
                 recorded U0 baseline of {baseline}",
                icon.as_str()
            );
        }

        // The strong form: the unified fill needs no per-draw allocation at all. Kept separate from
        // the baseline comparison above so a regression reports *which* contract broke.
        assert_eq!(
            total, 0,
            "{FRAMES} frames of {} icons at {size}px allocated {total} times ({per_draw} per draw, \
             {per_frame} per frame); the compound non-zero fill must reuse its buffers",
            icons.len()
        );

        println!(
            "size={size}px frames={FRAMES} draws={draws} allocations={total} \
             allocations_per_draw={per_draw} allocations_per_frame={per_frame}"
        );
        drop(backend);
    }
}
