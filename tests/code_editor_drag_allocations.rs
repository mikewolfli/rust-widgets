// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime allocation sample for a code-editor pointer drag.
//!
//! The control exists only with the `code-editor` feature, so the whole target
//! is gated rather than half-compiled (BLUE31 §1.5).
//!
//! The issue's runtime-sampling item asked for an allocation count rather than a
//! description. This measures the steady-state paint of a 200-line editor while a pointer
//! drag is extending the selection: the first frames warm the row-layout and occurrence
//! caches, then a thread-local counting allocator observes 20 drag frames.
//!
//! The counter is thread-local for the same reason `signal_allocation_test.rs` uses one: a
//! process-global counter also sees whatever the test harness allocates on its own thread,
//! which turns an exact property into a flaky one.
#![allow(clippy::missing_const_for_thread_local)]
#![cfg(feature = "code-editor")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use rust_widgets::core::{Color, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::widget::special_widgets::code_editor::{CodeEditor, CodeEditorConfig};
use rust_widgets::widget::Draw;

struct CountingAllocator;

thread_local! {
    /// Whether *this thread* is inside a measuring window.
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    /// Allocations this thread made while counting.
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to `System` with the caller's own layout and pointer.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: `layout` comes from the allocator caller and is passed through unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` come from the allocator caller and are passed through.
        unsafe { System.dealloc(ptr, layout) }
    }
}

fn measure<T>(work: impl FnOnce() -> T) -> (T, usize) {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    let result = work();
    COUNTING.with(|counting| counting.set(false));
    (result, ALLOCATIONS.with(Cell::get))
}

fn paint(editor: &mut CodeEditor, backend: &mut SoftwarePaintBackend) {
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(backend);
        editor.draw(&mut context);
    }
    backend.end_frame();
}

fn drag_point(step: usize) -> Point {
    Point::new(60 + (step as i32 % 600), 120 + (step as i32 % 240))
}

#[test]
fn a_code_editor_drag_frame_allocates_a_bounded_number_of_times() {
    let mut source = String::new();
    for line in 0..200 {
        source.push_str(&format!("let value_{line} = compute({line}, \"alpha alpha\");\n"));
    }
    let mut editor =
        CodeEditor::with_config(Rect::new(0, 0, 800, 600), CodeEditorConfig::new().word_wrap(true))
            .expect("valid configuration");
    editor.set_text(source);
    editor.set_search_query("alpha");
    let mut backend = SoftwarePaintBackend::new(Size::new(800, 600), 1.0);

    // Begin the gesture and warm every lazily measured cache before counting.
    editor.handle_event(&Event::MousePress { pos: Point::new(60, 120), button: 1, modifiers: 0 });
    for step in 0..6 {
        editor.handle_event(&Event::MouseMove { pos: drag_point(step) });
        paint(&mut editor, &mut backend);
    }

    const FRAMES: usize = 20;
    let mut allocations = 0usize;
    for step in 0..FRAMES {
        editor.handle_event(&Event::MouseMove { pos: drag_point(step + 6) });
        backend.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut backend);
            let (_, frame_allocations) = measure(|| editor.draw(&mut context));
            allocations += frame_allocations;
        }
        backend.end_frame();
    }
    let per_frame = allocations as f64 / FRAMES as f64;
    eprintln!(
        "code_editor_drag_allocations: {allocations} allocations over {FRAMES} frames \
         ({per_frame:.2} per frame)"
    );
    assert!(editor.selected_text().is_some(), "the drag extended a selection");
    assert!(
        allocations <= FRAMES * 32,
        "a steady drag frame must stay allocation-bounded: {per_frame:.2} per frame"
    );
}
