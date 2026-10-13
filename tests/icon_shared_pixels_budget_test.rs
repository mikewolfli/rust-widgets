// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! blue27 §19.14.1's two named U2 gaps: a **peak-memory budget** for shared icon pixels, and a
//! **wider size/disabled matrix** across the icon-bearing controls.
//!
//! # The budget this pins down
//!
//! §19.2 asks for typed image content that "borrows without copying", and §19.11 asks for three
//! separate caches so one cannot pollute another. What was never asserted is the arithmetic the two
//! add up to: **N consumers of one image cost one buffer, not N**. A handle that copies on clone, or
//! a control that copies the bytes into its own `Vec`, satisfies "it draws the right picture" while
//! silently making peak memory proportional to the number of buttons on screen — which is the
//! failure mode a toolbar with 30 identical icons would show and no correctness test would catch.
//!
//! The measurement is an allocation counter (the same technique `tests/icon_allocations.rs` uses for
//! frame cost), because bytes-reachable cannot be observed through the public API and a
//! `Arc::strong_count` check would only prove the `Arc` was shared, not that nothing else allocated.
//!
//! # The matrix, and what each axis is for
//!
//! * **size** — a control's slot is `min(its fixed constant, the box it was given)`, so a shrinking
//!   box is the case where a slot can collapse to nothing. The matrix drives both sides of that
//!   clamp instead of only the comfortable one.
//! * **disabled** — §19.11 requires a disabled state to be *observable*. Asserting only "geometry is
//!   still drawn" would pass for a control that ignores `set_enabled` entirely, so the colour must
//!   differ while the geometry stays identical (state changes appearance, not shape).
#![cfg(all(full_widgets, feature = "image"))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext};
use rust_widgets::widget::svg::render_widget_to_svg;
use rust_widgets::widget::{
    Button, Draw, Icon, IconName, IconSource, SharedIconPixels, ToolButton,
};
use rust_widgets::Widget;

// ---------------------------------------------------------------------------------------------
// Allocation counting
// ---------------------------------------------------------------------------------------------

struct CountingAllocator;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

// SAFETY: every operation delegates to `System` with the caller's own layout and pointer.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `layout` comes from the allocator caller and is passed through unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` and `layout` come from the allocator caller and are passed through.
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Serialises this file's tests.
///
/// The counter is **process-global**, and `cargo test` runs a file's tests on several threads by
/// default: an unrelated test allocating while a measurement is open is counted inside it. Measured:
/// 64 handle clones reported 9 allocations when the file ran in parallel and 1 when the measuring
/// test ran alone. Every test here therefore takes this guard, which is what makes the numbers
/// attributable.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Runs `body` with counting on and reports how many allocations it made.
///
/// The window is closed before the result is returned, so the counting itself cannot distort the
/// number it reports.
fn allocations_during<R>(body: impl FnOnce() -> R) -> (R, usize) {
    ALLOCATIONS.store(0, Ordering::SeqCst);
    COUNTING.store(true, Ordering::SeqCst);
    let value = body();
    COUNTING.store(false, Ordering::SeqCst);
    (value, ALLOCATIONS.load(Ordering::SeqCst))
}

/// A 256x256 RGBA8 image: large enough that a second copy would show up as a second allocation
/// rather than being lost in allocator noise.
fn big_image() -> Vec<u8> {
    let mut pixels = vec![0u8; 256 * 256 * 4];
    for (index, byte) in pixels.iter_mut().enumerate() {
        *byte = (index % 251) as u8;
    }
    pixels
}

/// An image handle taken from an owned buffer, so the construction path is exercised.
fn handle_from_owned_buffer() -> SharedIconPixels {
    SharedIconPixels::from_rgba8(big_image(), 256, 256)
        .expect("a 256x256 RGBA8 buffer is a valid image")
}

/// Builds 32 buttons and 32 tool buttons, each holding `handle` when one is given.
///
/// The pair (with / without a handle) is what makes the control-vs-image split measurable: the
/// control's own construction cost is identical in both runs and cancels out of the delta.
fn build_controls(
    geometry: Rect,
    handle: Option<&SharedIconPixels>,
) -> (Vec<Button>, Vec<ToolButton>) {
    // Pre-sized, so the containers' growth is not part of the measurement.
    let mut buttons = Vec::with_capacity(32);
    let mut tools = Vec::with_capacity(32);
    for _ in 0..32 {
        let mut button = Button::new(String::from("Save"), geometry);
        button.set_shared_icon_pixels(handle.cloned());
        buttons.push(button);

        let mut tool = ToolButton::new("Save", geometry);
        tool.set_shared_icon_pixels(handle.cloned());
        tools.push(tool);
    }
    (buttons, tools)
}

// ---------------------------------------------------------------------------------------------
// 1. The peak-memory budget: N consumers of one image cost one allocation.
// ---------------------------------------------------------------------------------------------

#[test]
fn constructing_a_handle_from_an_owned_buffer_moves_rather_than_copies_the_pixels() {
    let _exclusive = exclusive();
    // The buffer is built **outside** the counting window: the handle's own cost is the question,
    // and including the source `Vec` would make the number depend on how the fixture was made.
    let pixels = big_image();
    let (handle, allocations) = allocations_during(|| {
        SharedIconPixels::from_rgba8(pixels, 256, 256).expect("a 256x256 RGBA8 buffer is valid")
    });

    assert_eq!(handle.width(), 256);
    assert_eq!(handle.height(), 256);
    assert_eq!(handle.pixels().len(), 256 * 256 * 4);
    // Measured: exactly one allocation, the `Arc`. A copy of the payload would show up as a second
    // one (and as 256 KiB of extra peak memory per image).
    assert_eq!(
        allocations, 1,
        "wrapping the buffer must cost the `Arc` and nothing else; a second allocation is a copy"
    );
}

#[test]
fn cloning_a_handle_many_times_allocates_nothing_and_shares_one_allocation() {
    let _exclusive = exclusive();
    let original = handle_from_owned_buffer();

    let (clones, allocations) = allocations_during(|| {
        // Pre-sized on purpose: a growing `Vec` would reallocate, and its growth would be counted as
        // the handles' cost. The measurement is about the clones.
        let mut clones = Vec::with_capacity(64);
        for _ in 0..64 {
            clones.push(original.clone());
        }
        clones
    });

    // The container allocates once by construction, so the baseline is an empty container of the
    // same capacity; anything above it is the clones' own cost.
    let (_, baseline) = allocations_during(|| Vec::<SharedIconPixels>::with_capacity(64));
    assert_eq!(clones.len(), 64);
    assert_eq!(
        allocations, baseline,
        "cloning 64 handles allocated {allocations} times against a container baseline of \
         {baseline}; each clone must be a handle, not a copy"
    );
    for (index, clone) in clones.iter().enumerate() {
        assert!(
            clone.shares_allocation_with(&original),
            "clone {index} does not share the source allocation"
        );
    }
    // The other direction of the same claim: the handle is not secretly still `original` under a
    // copy, and the pixels really are the ones that went in.
    assert_eq!(clones[63].pixels()[0], original.pixels()[0]);
}

#[test]
fn many_controls_holding_the_same_handle_do_not_multiply_the_buffer() {
    let _exclusive = exclusive();
    let handle = handle_from_owned_buffer();
    let geometry = Rect::new(0, 0, 96, 48);

    // A control's own construction allocates (measured: ~19 per button, and one more for each
    // `set_shared_icon_pixels` call), and the process's first construction also pays one-time
    // initialisation. None of that belongs to the image, so the claim is the **delta** between two
    // identical batches that differ only in whether a handle is assigned — taken after a warm-up
    // pair so neither measured window carries first-construction cost.
    drop(build_controls(geometry, None));
    drop(build_controls(geometry, Some(&handle)));

    let (controls, with_handle) = allocations_during(|| build_controls(geometry, Some(&handle)));
    let (_, without_handle) = allocations_during(|| build_controls(geometry, None));

    assert_eq!(controls.0.len(), 32, "the batch built 32 buttons");
    assert_eq!(controls.1.len(), 32, "and 32 tool buttons");
    assert_eq!(
        with_handle, without_handle,
        "assigning one shared handle to 64 controls cost {} extra allocations ({with_handle} with, \
         {without_handle} without); peak memory must not grow with the number of consumers",
        with_handle.saturating_sub(without_handle)
    );
    for button in &controls.0 {
        assert!(
            button
                .shared_icon_pixels()
                .expect("the button kept the handle")
                .shares_allocation_with(&handle),
            "a button copied the pixels instead of holding the shared handle"
        );
    }
    for tool in &controls.1 {
        assert!(
            tool.shared_icon_pixels()
                .expect("the tool button kept the handle")
                .shares_allocation_with(&handle),
            "a tool button copied the pixels instead of holding the shared handle"
        );
    }
}

#[test]
fn drawing_shared_images_does_not_scale_with_the_pixel_payload() {
    let _exclusive = exclusive();
    use rust_widgets::render::SoftwarePaintBackend;

    // The plan's §19.13 line is "the dynamic clone cost of a common glyph does not grow with the
    // image's pixel count". That is a claim about the *payload*, so the measurement compares a large
    // image against a small one at the same consumer count:
    //
    //   measured (empty-label buttons, image icon): 8 consumers -> 8 allocations,
    //                                               32 consumers -> 32 allocations,
    //                                               i.e. exactly one per consumer per frame
    //
    // One allocation per consumer is the command/geometry record for that draw — a cost that does
    // not depend on how many bytes the source holds. A copy-the-bytes-per-frame implementation
    // would instead allocate on the order of the payload (262 144 bytes for the fixture below), so
    // the two are distinguishable by this comparison rather than by a fixed number.
    let large = SharedIconPixels::from_rgba8(vec![7u8; 256 * 256 * 4], 256, 256)
        .expect("a 256x256 RGBA8 buffer is valid");
    let small = SharedIconPixels::from_rgba8(vec![7u8; 8 * 8 * 4], 8, 8)
        .expect("an 8x8 RGBA8 buffer is valid");

    let consumers = 16usize;
    let geometry = Rect::new(0, 0, 96, 48);

    let draw_frame = |handle: &SharedIconPixels| -> usize {
        let mut buttons: Vec<Button> = (0..consumers)
            .map(|_| {
                let mut button = Button::new(String::new(), geometry);
                button.set_shared_icon_pixels(Some(handle.clone()));
                button
            })
            .collect();
        let mut backend = SoftwarePaintBackend::new(Size::new(96, 48 * consumers as u32), 1.0);
        let frame = |buttons: &mut Vec<Button>, backend: &mut SoftwarePaintBackend| {
            backend.begin_frame(Color::WHITE);
            {
                let mut context = RenderContext::new(backend);
                for button in buttons.iter_mut() {
                    button.draw(&mut context);
                }
            }
            backend.end_frame();
        };
        // One warm-up frame: preparation work is legitimate on the first draw, and the claim is
        // about the steady state (the same distinction `tests/icon_allocations.rs` makes).
        let (_, _warm) = allocations_during(|| frame(&mut buttons, &mut backend));
        let (_, steady) = allocations_during(|| frame(&mut buttons, &mut backend));
        steady
    };

    let with_large = draw_frame(&large);
    let with_small = draw_frame(&small);

    assert_eq!(
        with_large, with_small,
        "a 256x256 source cost {with_large} allocations and an 8x8 source cost {with_small}: the \
         per-frame cost grew with the payload, so the pixels are being copied per frame"
    );
    assert_eq!(
        with_large, consumers,
        "expected one record per consumer per frame ({consumers}); measured {with_large}"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. The size/disabled matrix.
// ---------------------------------------------------------------------------------------------

/// The fill colour of the compound icon element, or `None` when no icon geometry reached the
/// document.
fn icon_fill(svg: &str) -> Option<String> {
    svg.split('<').find_map(|element| {
        if !element.starts_with("path ")
            || !element.contains(r#"fill-rule="nonzero""#)
            || element.contains("data-text=")
        {
            return None;
        }
        let marker = r#"fill=""#;
        let start = element.find(marker)? + marker.len();
        let end = element[start..].find('"')? + start;
        Some(element[start..end].to_string())
    })
}

/// The icon vertices, as the backend wrote them (untranslated, so containment can be checked).
fn icon_vertices(svg: &str) -> Option<Vec<(i32, i32)>> {
    let element = svg.split('<').find(|element| {
        element.starts_with("path ")
            && element.contains(r#"fill-rule="nonzero""#)
            && !element.contains("data-text=")
    })?;
    let marker = r#"d=""#;
    let start = element.find(marker)? + marker.len();
    let end = element[start..].find('"')? + start;
    let numbers: Vec<i32> = element[start..end]
        .split(|ch: char| !ch.is_ascii_digit() && ch != '-')
        .filter(|token| !token.is_empty())
        .map(|token| token.parse().expect("the backend writes integers"))
        .collect();
    let vertices: Vec<(i32, i32)> =
        numbers.as_chunks::<2>().0.iter().map(|pair| (pair[0], pair[1])).collect();
    (vertices.len() >= 3).then_some(vertices)
}

/// One row of the matrix: what the entry drew, and the slot it declared.
struct EntryDraw {
    svg: String,
    slot: Rect,
}

/// Draws one entry at the given available box size, enabled or disabled.
fn draw_entry(entry: &str, box_size: u32, enabled: bool) -> EntryDraw {
    // Two box shapes per size: the entry's slot is `min(constant, box)`, so a box narrower than the
    // constant is the clamp this matrix has to cover.
    let geometry = Rect::new(0, 0, box_size, box_size);
    match entry {
        "button" => {
            let mut button = Button::new(String::from("Save"), geometry);
            button.set_icon_source(Some(IconSource::Builtin(IconName::Save)));
            button.set_enabled(enabled);
            let slot = button.icon_rect().expect("a button with an icon declares its slot");
            EntryDraw { svg: render_widget_to_svg(&mut button, geometry), slot }
        }
        "tool_button" => {
            let mut tool = ToolButton::new(String::from("Save"), geometry);
            tool.set_icon_source(Some(IconSource::Builtin(IconName::Save)));
            tool.set_enabled(enabled);
            let slot = tool.icon_rect().expect("a tool button with an icon declares its slot");
            EntryDraw { svg: render_widget_to_svg(&mut tool, geometry), slot }
        }
        "icon" => {
            let mut icon = Icon::new(geometry);
            icon.set_icon_source(IconSource::Builtin(IconName::Save));
            icon.set_enabled(enabled);
            EntryDraw { svg: render_widget_to_svg(&mut icon, geometry), slot: geometry }
        }
        other => panic!("unknown entry {other}"),
    }
}

#[test]
fn every_entry_draws_inside_its_declared_slot_across_sizes_and_states() {
    let _exclusive = exclusive();
    // `12` is included deliberately: a control's slot is `min(its constant, the content box it was
    // given)`, so a box too small for the padding is the case where the clamp collapses it. Measured:
    // a `button` at 12px declares no slot and draws no glyph — an honest empty result, not a failure.
    // The assertion is therefore the *equivalence*: a slot with area iff a glyph was drawn.
    let sizes = [12u32, 16, 24, 48, 96];
    for entry in ["button", "tool_button", "icon"] {
        for size in sizes {
            for enabled in [true, false] {
                let drawn = draw_entry(entry, size, enabled);
                let vertices = icon_vertices(&drawn.svg);
                let has_slot = drawn.slot.width > 0 && drawn.slot.height > 0;

                assert_eq!(
                    has_slot,
                    vertices.is_some(),
                    "{entry} at {size}px (enabled={enabled}): slot {:?} (area={has_slot}) but \
                     glyph={}. A control must either reserve a slot and fill it, or reserve none \
                     and draw nothing. Document:\n{}",
                    drawn.slot,
                    vertices.is_some(),
                    drawn.svg
                );

                for (x, y) in vertices.unwrap_or_default() {
                    assert!(
                        x >= drawn.slot.x
                            && x <= drawn.slot.x + drawn.slot.width as i32
                            && y >= drawn.slot.y
                            && y <= drawn.slot.y + drawn.slot.height as i32,
                        "{entry} at {size}px (enabled={enabled}): vertex ({x}, {y}) escapes the \
                         declared slot {:?}",
                        drawn.slot
                    );
                }
            }
        }
    }
}

#[test]
fn a_disabled_entry_keeps_its_geometry_and_changes_only_its_ink() {
    let _exclusive = exclusive();
    // Sizes where **every** entry below reserves a slot. Measured: a `button`'s slot collapses to
    // zero width below 32px (its constant is 18 and the content box subtracts the padding), so a
    // smaller size would make "disabled keeps the geometry" vacuous — there would be none.
    let sizes = [32u32, 48, 96];
    for entry in ["button", "tool_button", "icon"] {
        for size in sizes {
            let (enabled, disabled) =
                (draw_entry(entry, size, true), draw_entry(entry, size, false));

            let (on, off) = (
                icon_vertices(&enabled.svg).expect("the enabled entry draws its icon"),
                icon_vertices(&disabled.svg).expect("the disabled entry still draws its icon"),
            );
            assert_eq!(
                on, off,
                "{entry} at {size}px changed its geometry when disabled; only the ink should \
                 change"
            );

            let (on_fill, off_fill) = (
                icon_fill(&enabled.svg).expect("the enabled icon has a fill"),
                icon_fill(&disabled.svg).expect("the disabled icon has a fill"),
            );
            assert_ne!(
                on_fill, off_fill,
                "{entry} at {size}px paints the same ink enabled and disabled, so the state is not \
                 observable"
            );
        }
    }
}

#[test]
fn drawing_the_same_entry_twice_produces_the_same_document() {
    let _exclusive = exclusive();
    // Determinism is what makes the assertions above about a *state* rather than about an
    // allocation order: a document that changed between two identical draws could not distinguish
    // "disabled changes the ink" from "the second draw happened to differ".
    for entry in ["button", "tool_button", "icon"] {
        for enabled in [true, false] {
            let first = draw_entry(entry, 24, enabled);
            let second = draw_entry(entry, 24, enabled);
            assert_eq!(
                first.svg, second.svg,
                "{entry} (enabled={enabled}) drew two different documents"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. The same handle, drawn and measured, is what the control reports.
// ---------------------------------------------------------------------------------------------

#[test]
fn a_non_square_image_is_fitted_rather_than_stretched_into_a_square_slot() {
    let _exclusive = exclusive();
    // 64x32 is deliberately not square: an implementation that fills the slot would pass a
    // square-source test, so the source has to be the one that would be distorted.
    let handle = SharedIconPixels::from_rgba8(vec![200u8; 64 * 32 * 4], 64, 32)
        .expect("a 64x32 RGBA8 buffer is a valid image");
    let slot = Rect::new(0, 0, 32, 32);
    let fitted = handle.fit_into(slot);
    assert_eq!(fitted.width, 32, "the long side takes the slot's width");
    assert_eq!(fitted.height, 16, "the short side keeps the source aspect ratio");
    assert_eq!(
        fitted.x, slot.x,
        "a fitted image is centred: the leading edge must not be pushed out"
    );
    assert_eq!(fitted.y, slot.y + 8, "the vertical remainder is split around the centre");

    // The opposite direction, so the rule is "preserve aspect" rather than "shrink height".
    let tall = SharedIconPixels::from_rgba8(vec![200u8; 32 * 64 * 4], 32, 64)
        .expect("a 32x64 RGBA8 buffer is a valid image");
    let fitted = tall.fit_into(slot);
    assert_eq!(fitted.width, 16);
    assert_eq!(fitted.height, 32);
    assert_eq!(fitted.x, slot.x + 8);
    assert_eq!(fitted.y, slot.y);
}

#[test]
fn the_shared_handle_keeps_the_pixels_the_caller_handed_it() {
    let _exclusive = exclusive();
    // A guard against the fit rule above passing for a handle that lost its content: the bytes must
    // survive the trip into and out of the shared representation.
    let mut pixels = vec![0u8; 4 * 4 * 4];
    for (index, byte) in pixels.iter_mut().enumerate() {
        *byte = (index * 3 % 256) as u8;
    }
    let expected = pixels.clone();
    let handle = SharedIconPixels::from_rgba8(pixels, 4, 4).expect("a 4x4 RGBA8 buffer is valid");
    assert_eq!(handle.pixels(), expected.as_slice());
}
