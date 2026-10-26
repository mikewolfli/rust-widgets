// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Classifying a control name must not allocate on the creation / interaction hot paths.
//!
//! # What was measured
//!
//! `normalize_key` built its answer with `collect()` over a filtered, case-folded iterator, so a
//! ten-character name grew the `String` twice (4 → 8 → 16 bytes): **two** allocations per call.
//! Two of its callers sit on per-widget paths — a theme classification per created control
//! (`WidgetRole::for_kind_name`, `render::role_surface_style`) and one per interaction-state
//! change — and the factory's own lookups (`WidgetFactory::capability`,
//! `WidgetFactory::constructor_for`, `create`) run once per document node. The normalised
//! spelling in every one of those is a transient map key or `match` subject, so the owned
//! `String` was pure cost.
//!
//! # What this file asserts
//!
//! 1. Those entry points allocate **zero** times for an ordinary (ASCII) kind name.
//! 2. The answer equals the answer for the *normalised* spelling, with `normalize_key` — the
//!    rule's own definition — as the oracle. A fast path that classified differently would fail
//!    here, so the zero cannot be bought by answering a different question.
//! 3. `normalize_key` still allocates exactly **once** (it must return an owned `String`): the
//!    fast path reserves it at the exact size instead of growing it.
//!
//! The counter is thread-local, so the harness' own allocations on other threads cannot leak into
//! the measurement; that also lets this file's tests run in parallel with the rest of the suite.

#![allow(clippy::missing_const_for_thread_local)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;

use rust_widgets::render::role_surface_style;
use rust_widgets::theme::WidgetRole;
use rust_widgets::widget::capability::{normalize_key, WidgetFactory};

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

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.with(|count| count.set(count.get() + 1));
        }
        // SAFETY: the caller's pointer, layout and new size are passed through unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Serialises this file's tests so their thread-local windows cannot overlap.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs `body` with counting on and reports how many allocations it made on this thread.
fn allocations_during<R>(body: impl FnOnce() -> R) -> (R, usize) {
    // Touch both thread-locals **before** the window opens: their first access initialises the
    // TLS block, and that setup must not be counted as the operation's cost.
    let _ = COUNTING.with(Cell::get);
    let _ = ALLOCATIONS.with(Cell::get);
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|flag| flag.set(true));
    let value = body();
    COUNTING.with(|flag| flag.set(false));
    (value, ALLOCATIONS.with(Cell::get))
}

/// Ordinary spellings: canonical names, aliases and the separator/case variants the rule folds.
const SPELLINGS: &[&str] = &[
    "button",
    "Button",
    "BUTTON",
    "menu_panel",
    "MenuPanel",
    "playlist-panel",
    "scroll bar",
    "scroll_bar",
    "cupertino_switch",
    "text_areainput",
];

/// A steady-state frame of the controls this round made allocation-free paints with zero
/// allocations.
///
/// `PlaylistPanel` used to format a row number and a title per visible row per frame (and rebuild
/// a filtered index list on every read); `ListBox` used to build its fallback row face — and that
/// face's family `Arc` — once per frame; `Tag` converted its fitted `Cow` to an owned `String`
/// every frame; `LineChart` built its axis labels and its mapped-points buffer per frame. All are
/// now zero, and this pins the property the same way `tests/icon_allocations.rs` pins the icon
/// paint path.
///
/// `table` is deliberately absent: its remaining two allocations per frame are the `Cow::Owned`
/// of two genuinely **elided** cells, which is the truncated text itself rather than bookkeeping.
#[test]
fn painting_a_steady_state_frame_allocates_nothing() {
    use rust_widgets::core::{Color, Rect, Size};
    use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use rust_widgets::widget::draw_bridge::draw_of;

    let _guard = exclusive();
    let factory = WidgetFactory::new_with_defaults();
    for name in ["playlist_panel", "list_box", "tag", "line_chart"] {
        let mut widget = factory
            .create(name, Rect::new(0, 0, 240, 120), "Sample")
            .unwrap_or_else(|| panic!("{name} must be constructible"));
        let _ = rust_widgets::widget::sample_fill::apply(name, widget.as_mut());
        let drawable =
            draw_of(widget.as_mut()).unwrap_or_else(|| panic!("{name} must be drawable"));
        let mut backend = SoftwarePaintBackend::new(Size::new(240, 120), 1.0);

        // Warm-up: the first paints resolve fonts and grow the backend's own buffers. Those are
        // one-time costs, and charging them per frame would measure start-up.
        for _ in 0..3 {
            backend.begin_frame(Color::TRANSPARENT);
            {
                let mut context = RenderContext::new(&mut backend);
                drawable.draw(&mut context);
            }
            backend.end_frame();
        }

        let (_, allocations) = allocations_during(|| {
            for _ in 0..4 {
                backend.begin_frame(Color::TRANSPARENT);
                {
                    let mut context = RenderContext::new(&mut backend);
                    drawable.draw(&mut context);
                }
                backend.end_frame();
            }
        });
        assert_eq!(
            allocations, 0,
            "{name} allocated {allocations} times over four steady-state frames"
        );
    }
}

#[test]
fn classifying_a_name_allocates_nothing_and_agrees_with_normalize_key() {
    let _guard = exclusive();
    // The registry itself is one-time setup, not part of the measurement.
    let factory = WidgetFactory::new_with_defaults();

    // ── 1. The zero-allocation claim ──
    let (_, allocations) = allocations_during(|| {
        for spelling in SPELLINGS {
            black_box(role_surface_style(black_box(spelling)));
            black_box(WidgetRole::for_kind_name(black_box(spelling)));
            black_box(factory.capability(black_box(spelling)));
            black_box(factory.constructor_for(black_box(spelling)));
        }
    });
    assert_eq!(
        allocations,
        0,
        "classifying {} names cost {allocations} heap allocations; the fast path must stay on \
         the stack",
        SPELLINGS.len()
    );

    // The counter itself works: this call must allocate exactly the one `String` it returns.
    let (normalized, allocations) = allocations_during(|| normalize_key("menu_panel"));
    assert_eq!(normalized, "menupanel");
    assert_eq!(allocations, 1, "normalize_key must allocate its result exactly once, not grow it");

    // ── 2. Behavioural equivalence, with the allocating rule as the oracle ──
    // The non-ASCII and overlong inputs are the fast path's documented fallbacks; the others
    // prove the fast path folds separators and case exactly as `normalize_key` does.
    let overlong = "x".repeat(200);
    let odd = ["ménu_panel", overlong.as_str(), "___---   ", ""];
    for spelling in SPELLINGS.iter().copied().chain(odd) {
        let canonical = normalize_key(spelling);
        assert_eq!(
            role_surface_style(spelling),
            role_surface_style(&canonical),
            "role_surface_style disagreed with the normalised spelling for {spelling:?}"
        );
        assert_eq!(
            WidgetRole::for_kind_name(spelling),
            WidgetRole::for_kind_name(&canonical),
            "WidgetRole::for_kind_name disagreed with the normalised spelling for {spelling:?}"
        );
        assert_eq!(
            factory.capability(spelling).map(|capability| capability.canonical_name),
            factory.capability(&canonical).map(|capability| capability.canonical_name),
            "the factory resolved {spelling:?} differently from its normalised spelling"
        );
    }
}
