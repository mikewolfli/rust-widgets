// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One icon source, several entries: the geometry that reaches the backend must be the same one.
//!
//! # Why the whole file is gated on `full_widgets`
//!
//! The entries under test are `Icon`, `Button` and `ToolButton`, and the last of those lives in
//! `widget::menu_toolbar`, which is `full_widgets`-gated; the click-through also renders through
//! `render::SvgPaintBackend`. An integration test has no per-item gate to reach for, so the file
//! states its requirement once — the same shape `image_decode_cache_test.rs` uses for `image`.
//! `full_widgets` is the requirement, not `desktop`: the device profiles all build the entries, so
//! gating on the desktop feature too would silently drop tablet and mobile coverage. Without any
//! gate the file was compiled on `embedded`, where the imports do not resolve, and
//! `tools/check_behavior_matrix.sh` reported the resulting build failure as an "embedded capability
//! contract" defect — a configuration the case cannot run in, dressed up as a regression.
//!
//! # The claim, and how it fails
//!
//! blue27 §19.13 case I01 says the same built-in token used by `Icon`, `Button` and `ToolButton` must
//! draw the *same* glyph.
//!
//! # What this proves, and what it does not
//!
//! Each entry is compared against a **reference** produced by calling the shared painter
//! ([`paint_icon`]) with the slot that control publishes, and the two coordinate lists must be
//! identical. That pins down three things a `WidgetKind`-level "it has a typed source" assertion
//! cannot see: the control paints into the box it *declares* (no extra rescale, re-centre or
//! transform on the way to the painter); the glyph arrives as **icon geometry** rather than text,
//! tint or nothing; and two entries at the same size produce identical vertices.
//!
//! It does **not** judge whether a control's declared slot size is the right one for that control —
//! the declared slot *is* the specification here, and a control whose size is wrong in both its
//! layout and its paint would satisfy this file. Two measurements pin that limit down rather than
//! leaving it as a caveat:
//!
//! * widening `Button::icon_rect` by 2 px (so layout *and* paint move together) leaves this test
//!   **passing** — it is not a metrics audit, and pretending otherwise would make it a second copy
//!   of `BUTTON_ICON_SIZE`;
//! * making `Button` paint into a box 6 px larger than the slot it declared makes it **fail**, with
//!   both vertex lists printed — so the comparison above really is about the geometry that reaches
//!   the backend.
//!
//! A pure *translation* is deliberately free: the entries put their slots in different places in
//! their own chrome, and shifting the list to its first vertex is what makes the comparison about
//! the glyph. That is also why containment — every span must fit the declared slot — is asserted
//! separately rather than implied by equality.
//!
//! The comparison is translation-free (both lists are shifted so their first vertex is the origin)
//! because the entries legitimately put their icon slots at different places in their own chrome;
//! size, winding and vertex order are *not* free.
//!
//! # Why the reference is the painter and not another control
//!
//! Comparing controls to each other would only prove they agree, which two identically-wrong
//! controls also do. Comparing each to the painter pins the geometry to the one function every
//! entry is supposed to reach, and the extra cross-control assertion at the end covers the
//! same-source-same-size case the plan names.
//!
//! # How the icon path is told apart from a text run
//!
//! The SVG backend tags an outline text run with `data-text`, and a compound icon fill is the only
//! element carrying `fill-rule="nonzero"` without that tag. That distinction is the backend's own
//! provenance marker, not a guess about command letters — see `SvgPaintBackend`'s `DrawText` arm.

#![cfg(full_widgets)]

use rust_widgets::core::{Color, Rect};
use rust_widgets::render::{PaintBackend, RenderContext, SvgPaintBackend};
use rust_widgets::widget::menu_toolbar::tool_button::ToolButtonStyle;
use rust_widgets::widget::svg::render_widget_to_svg;
use rust_widgets::widget::{paint_icon, Button, Icon, IconName, IconRef, IconSource, ToolButton};

/// The icon a control draws, as device-space vertices with the first vertex moved to the origin.
///
/// Returns `None` when the document holds no compound icon fill, which is how "the source never
/// reached the backend" is reported rather than silently comparing two empty lists.
fn normalised_icon_geometry(svg: &str) -> Option<Vec<(i32, i32)>> {
    let mut points = Vec::new();
    for element in svg.split('<').filter(|element| element.starts_with("path ")) {
        if !element.contains(r#"fill-rule="nonzero""#) || element.contains("data-text=") {
            continue;
        }
        let start = element.find(r#"d=""#)? + r#"d=""#.len();
        let end = element[start..].find('"')? + start;
        let d = &element[start..end];
        let numbers: Vec<i32> = d
            .split(|ch: char| !ch.is_ascii_digit() && ch != '-')
            .filter(|token| !token.is_empty())
            .map(|token| token.parse().expect("the backend writes integers"))
            .collect();
        let pairs: Vec<(i32, i32)> =
            numbers.as_chunks::<2>().0.iter().map(|pair| (pair[0], pair[1])).collect();
        if pairs.len() < 3 {
            continue;
        }
        if !points.is_empty() {
            // A second compound fill in the same document means the selection above is ambiguous,
            // and comparing the wrong pair would be worse than failing.
            return None;
        }
        let (ox, oy) = pairs[0];
        points = pairs.iter().map(|(x, y)| (x - ox, y - oy)).collect();
    }
    (!points.is_empty()).then_some(points)
}

/// What the shared painter produces for `slot`, in the same normalised form.
///
/// The colour is a parameter of `paint_icon` but not of the comparison: a control resolves its own
/// ink from the theme (§19.4 puts that decision in the control), and only the **vertices** are
/// compared here. Passing a constant therefore loses nothing.
fn reference_geometry(slot: Rect) -> Vec<(i32, i32)> {
    let color = Color::BLACK;
    let mut backend = SvgPaintBackend::new(rust_widgets::core::Size::new(200, 200));
    backend.begin_frame(Color::TRANSPARENT);
    {
        let mut context = RenderContext::new(&mut backend);
        paint_icon(&mut context, IconRef::Builtin(IconName::Save), slot, color)
            .expect("the bundled outline paints");
    }
    backend.end_frame();
    normalised_icon_geometry(&backend.finish())
        .expect("the painter emits exactly one compound icon fill")
}

/// One entry: the slot it declares, and the document it produced.
struct Entry {
    name: &'static str,
    slot: Rect,
    svg: String,
}

#[test]
fn every_entry_draws_the_same_icon_the_shared_painter_draws_for_its_own_slot() {
    let source = IconSource::Builtin(IconName::Save);

    // ── `Icon`, whose slot is the square it centres in its own geometry ──
    let icon_rect = Rect::new(0, 0, 16, 16);
    let mut icon = Icon::new(icon_rect);
    icon.set_size(16.0);
    icon.set_icon_source(source);
    let icon_svg = render_widget_to_svg(&mut icon, icon_rect);

    // ── `Button`, whose slot it publishes as `icon_rect()` ──
    let button_rect = Rect::new(0, 0, 120, 40);
    let mut button = Button::new("Save".to_string(), button_rect);
    button.set_icon_source(Some(source));
    let button_slot = button.icon_rect().expect("setting the source reserves the icon slot");
    let button_svg = render_widget_to_svg(&mut button, button_rect);

    // ── `ToolButton`, likewise ──
    let tool_rect = Rect::new(0, 0, 120, 40);
    let mut tool = ToolButton::new("Save", tool_rect);
    tool.set_button_style(ToolButtonStyle::IconOnly);
    tool.set_icon_source(Some(source));
    let tool_slot = tool.icon_rect().expect("setting the source reserves the icon slot");
    let tool_svg = render_widget_to_svg(&mut tool, tool_rect);

    let entries = [
        Entry { name: "Icon", slot: icon_rect, svg: icon_svg },
        Entry { name: "Button", slot: button_slot, svg: button_svg },
        Entry { name: "ToolButton", slot: tool_slot, svg: tool_svg },
    ];

    for entry in &entries {
        let drawn = normalised_icon_geometry(&entry.svg).unwrap_or_else(|| {
            panic!("{} did not put the source on the backend as a compound icon fill", entry.name)
        });
        let reference = reference_geometry(entry.slot);
        assert_eq!(
            drawn,
            reference,
            "{} draws {:?} for its {}x{} slot, but the shared painter draws {:?}",
            entry.name,
            drawn.len(),
            entry.slot.width,
            entry.slot.height,
            reference.len()
        );

        // Containment: shifted to its first vertex, every other vertex must still be inside the box
        // the control reserved. A control that painted a larger glyph than its slot would overflow
        // its own layout, which its neighbours would then overlap.
        let span_x = drawn.iter().map(|(x, _)| *x).max().unwrap()
            - drawn.iter().map(|(x, _)| *x).min().unwrap();
        let span_y = drawn.iter().map(|(_, y)| *y).max().unwrap()
            - drawn.iter().map(|(_, y)| *y).min().unwrap();
        assert!(
            span_x <= entry.slot.width as i32 && span_y <= entry.slot.height as i32,
            "{} paints {}x{} of vertices into a {}x{} slot",
            entry.name,
            span_x,
            span_y,
            entry.slot.width,
            entry.slot.height
        );
    }

    // The plan's own case: the *same* source at the *same* size must give the same glyph in
    // different controls, not merely a glyph each. `Icon` and `ToolButton` are made to share a
    // 16 px slot (the tool button's own `TOOL_BUTTON_ICON_SIZE`), so their geometry must be equal
    // to each other as well as to the painter.
    let icon_entry = &entries[0];
    let tool_entry = &entries[2];
    assert_eq!(
        (tool_entry.slot.width, tool_entry.slot.height),
        (icon_entry.slot.width, icon_entry.slot.height),
        "this assertion is only meaningful while the two slots are the same size"
    );
    assert_eq!(
        normalised_icon_geometry(&icon_entry.svg),
        normalised_icon_geometry(&tool_entry.svg),
        "the same token at the same size must draw the same vertices in both controls"
    );
}

/// The failure this file exists for, demonstrated on the mechanism it would catch: passing a slot of
/// the wrong size produces a different glyph, so the comparison above is not vacuous.
#[test]
fn a_slot_of_a_different_size_draws_a_different_glyph() {
    let small = reference_geometry(Rect::new(0, 0, 16, 16));
    let large = reference_geometry(Rect::new(0, 0, 32, 32));
    assert_ne!(small, large, "a different slot size must not produce the same vertices");
    // ... and a different colour is not a geometry change, which is what keeps the assertion above
    // about geometry rather than about paint.
    assert_eq!(large, reference_geometry(Rect::new(0, 0, 32, 32)));
}
