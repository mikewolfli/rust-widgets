// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

#[test]
fn records_repeated_builtin_icon_draw_allocations() {
    use rust_widgets::core::{Color, Rect, Size};
    use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use rust_widgets::widget::{paint_icon, IconName, IconPaintOutcome, IconRef};

    const REPEATS: usize = 100;
    let icons = [IconName::Check, IconName::Info, IconName::Settings];

    for size in [24, 96] {
        let bounds = Rect::new(0, 0, size, size);
        let mut backend = SoftwarePaintBackend::new(Size::new(size, size), 1.0);
        backend.begin_frame(Color::TRANSPARENT);
        let mut context = RenderContext::new(&mut backend);

        for icon in icons {
            paint_icon(&mut context, IconRef::Builtin(icon), bounds, Color::BLACK)
                .expect("bundled outline should warm the paint path");
            ALLOCATIONS.store(0, Ordering::Relaxed);
            COUNTING.store(true, Ordering::SeqCst);
            for _ in 0..REPEATS {
                let outcome =
                    paint_icon(&mut context, IconRef::Builtin(icon), bounds, Color::BLACK)
                        .expect("bundled outline should paint");
                assert_eq!(outcome, IconPaintOutcome::Drawn);
            }
            COUNTING.store(false, Ordering::SeqCst);
            let total = ALLOCATIONS.swap(0, Ordering::Relaxed);
            assert_eq!(
                total,
                0,
                "steady-state painting of {} at {size}px allocated {total} times",
                icon.as_str()
            );
            println!(
                "icon={} size={} repetitions={} allocations={} allocations_per_draw={:.2}",
                icon.as_str(),
                size,
                REPEATS,
                total,
                total as f64 / REPEATS as f64
            );
        }

        drop(context);
        backend.end_frame();
    }
}
