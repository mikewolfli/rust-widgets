// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! §19.13 case I06: the three "no glyph" states must stay distinguishable.
//!
//! §19.4 makes this an explicit rule — *do not use `false` for "empty slot", "unknown name" and
//! "failure" at once* — and §19.13 I06 asks for the three states as separate observations and for
//! an unknown entry to be **unable to succeed silently**. Conflating them is how a broken icon
//! becomes invisible: the caller sees "nothing drawn", which is also what a control with no icon
//! legitimately looks like, so a typo, a missing feature and an empty box all read the same.
//!
//! | State | Question the caller asked | Observable answer |
//! |---|---|---|
//! | empty slot | "draw into this box" (box has no area) | `Ok(SkippedEmptySlot)` — **not** an error |
//! | explicitly no icon | "draw my icon" (there is none) | no call at all; no slot reserved |
//! | an unsupported source | "draw this geometry" | `Err(..)` naming *why* |
//!
//! The last row is checked twice with two different causes, because a single error variant could
//! satisfy the table while still collapsing "zero grid" and "malformed path" into one bucket — the
//! same collapsing the case objects to, one level down.
#![cfg(full_widgets)]

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::widget::draw::Draw;
use rust_widgets::widget::{
    measure_icon, paint_icon, validate_icon, Button, IconData, IconName, IconPaintError,
    IconPaintOutcome, IconRef, IconSource,
};

/// A shape with no dimension: the flattener cannot derive a scale from a zero grid.
static ZERO_GRID: IconData = IconData { name: "zero", grid: 0, paths: &["M0 0L1 0Z"] };

/// Path data that is not a path: the second command is not a command.
static BROKEN: IconData = IconData { name: "broken", grid: 24, paths: &["M0 0L oops"] };

/// Whether the document carries the compound icon fill rather than text or control chrome.
fn has_compound_icon_fill(svg: &str) -> bool {
    svg.split('<').any(|element| {
        element.starts_with("path ")
            && element.contains(r#"fill-rule="nonzero""#)
            && !element.contains("data-text=")
    })
}

/// Renders one control into a fresh software frame and returns its RGBA pixels.
fn frame_of<T: Draw + ?Sized>(widget: &mut T, size: Size) -> Vec<u8> {
    let mut backend = SoftwarePaintBackend::new(size, 1.0);
    backend.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut backend);
        widget.draw(&mut context);
    }
    backend.end_frame();
    backend.frame_rgba().to_vec()
}

#[test]
fn an_empty_slot_is_skipped_while_a_broken_source_is_an_error() {
    let mut backend = SoftwarePaintBackend::new(Size::new(8, 8), 1.0);
    backend.begin_frame(Color::TRANSPARENT);
    let mut context = RenderContext::new(&mut backend);

    // ── Empty slot: skipped, and deliberately not an error ──
    let empty = Rect::new(0, 0, 0, 0);
    assert_eq!(
        paint_icon(&mut context, IconRef::Builtin(IconName::Info), empty, Color::BLACK),
        Ok(IconPaintOutcome::SkippedEmptySlot),
        "a slot with no area is 'nothing to do', not a resource failure"
    );
    // The measurement half answers the same question the same way, and the same source really does
    // draw in a box with room — otherwise the assertion above would hold for a source that never
    // paints at all.
    assert!(
        measure_icon(IconRef::Builtin(IconName::Info), empty)
            .expect("an empty slot still measures")
            .is_empty(),
        "an empty slot measures empty rather than failing"
    );
    let drawn = Rect::new(0, 0, 8, 8);
    assert_eq!(
        paint_icon(&mut context, IconRef::Builtin(IconName::Info), drawn, Color::BLACK),
        Ok(IconPaintOutcome::Drawn)
    );

    // ── An unsupported source: named errors, not a silent blank ──
    // §19.3 splits validation in two and the stages answer different questions, so both are
    // asserted: the zero grid is a property of the **source**, which stage one names, while stage
    // two reports what that source produced at the size asked for. Asserting only one stage would
    // let the other collapse the cause back into "empty" — the conflation this case exists to
    // prevent, one level down.
    assert_eq!(
        validate_icon(IconRef::StaticGeometry(&ZERO_GRID)),
        Err(IconPaintError::ZeroGrid),
        "the source validator names the zero design grid"
    );
    assert_eq!(
        paint_icon(&mut context, IconRef::StaticGeometry(&ZERO_GRID), drawn, Color::BLACK),
        Err(IconPaintError::EmptyGeometry),
        "the same source paints to nothing, reported as its own outcome"
    );
    assert!(
        matches!(
            validate_icon(IconRef::StaticGeometry(&BROKEN)),
            Err(IconPaintError::InvalidGeometry(_))
        ),
        "malformed path data must not be indistinguishable from a zero grid"
    );

    // ── Unknown token: unrepresentable in the typed vocabulary ──
    assert!(
        IconName::from_name("definitely_not_a_token").is_none(),
        "the typed entry point cannot name a token the crate does not have, so a host cannot \
         reach the paint path with one"
    );
}

#[test]
fn an_explicitly_icon_less_control_reserves_no_slot_and_draws_no_ink() {
    let geometry = Rect::new(0, 0, 160, 40);
    let size = Size::new(geometry.width, geometry.height);

    let mut without = Button::new(String::new(), geometry);
    assert!(without.icon_rect().is_none(), "no source means no reserved slot at all");
    let bare = frame_of(&mut without, size);
    assert!(
        !has_compound_icon_fill(&rust_widgets::widget::svg::render_widget_to_svg(
            &mut without,
            geometry
        )),
        "a control with no icon emits no icon geometry"
    );

    // Clearing an icon returns the control to exactly that state rather than leaving a stale slot.
    let mut cleared = Button::new(String::new(), geometry);
    cleared.set_icon_source(Some(IconSource::Builtin(IconName::Info)));
    assert!(cleared.icon_rect().is_some());
    cleared.set_icon_source(None);
    assert!(cleared.icon_rect().is_none(), "clearing the source releases the reserved slot");
    assert_eq!(
        frame_of(&mut cleared, size),
        bare,
        "a cleared control must render exactly like one that never had an icon"
    );
}
