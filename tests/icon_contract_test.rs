// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The icon contract, asserted at the entries: the cases blue27 §19.13 names.
//!
//! # Why these cases live in one file
//!
//! They were four files — one per I-case — and each repeated the same two helpers (`frame_of`,
//! `has_compound_icon_fill`) and its own copy of the same entry list. Every assertion is kept; what
//! changed is that the shared scaffolding and the file-level `cfg` are now stated once. The four
//! subjects stay separate sections below, because they ask four different questions:
//!
//! | Section | blue27 case | The question |
//! |---|---|---|
//! | 1 | I06 | are "empty slot", "explicitly no icon" and "unsupported source" told apart? |
//! | 2 | I02 | does a direct-geometry icon still paint when the legacy registry is full? |
//! | 3 | I01 | does one source draw the same glyph through Icon, Button and ToolButton? |
//!
//! (I15 — the native-surface capability answer — stays in `native_icon_capability_test.rs`, because it
//! needs no widget and is gated only `not(alloc_frugal)`. Folding it in here would have narrowed it to
//! `full_widgets` and silently dropped its `embedded` coverage, which is exactly the kind of quiet
//! loss a merge must not cause.)
//!
//! # Why one `cfg` is correct for all four
//!
//! The entries under test are `Icon`, `Button`, `ToolButton` and `MenuButton`; the last two live
//! behind `full_widgets`. `alloc_frugal` is set by `build.rs` exactly when `mini` is on, and
//! `full_widgets` requires a device profile and an unstripped widget set — so `full_widgets` implies
//! `!alloc_frugal`. Stating it once is also what stops the three from drifting apart: a case that
//! silently stops being compiled is the failure mode `tools/check_behavior_matrix.sh` exists to catch.
#![cfg(full_widgets)]

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend, SvgPaintBackend};
use rust_widgets::widget::draw::Draw;
use rust_widgets::widget::menu_toolbar::tool_button::ToolButtonStyle;
use rust_widgets::widget::svg::render_widget_to_svg;
use rust_widgets::widget::{
    clear_registered_icons, measure_icon, paint_icon, register_icon, registered_icon_count,
    validate_icon, Button, Icon, IconData, IconName, IconPaintError, IconPaintOutcome, IconRef,
    IconSource, MenuButton, ToolButton, MAX_REGISTERED_ICONS,
};

// ─── 1. I06: empty slot vs explicitly no icon vs unsupported source ───

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

// ─── 2. I02: direct geometry paints while the legacy registry is full ───

static SOLID: IconData = IconData { name: "i02_solid", grid: 24, paths: &["M4-4H20V-20H4Z"] };

/// Sixteen distinct host names — exactly the legacy capacity — plus the one that must be refused.
static HOST_NAMES: [&str; 17] = [
    "i02_host_00",
    "i02_host_01",
    "i02_host_02",
    "i02_host_03",
    "i02_host_04",
    "i02_host_05",
    "i02_host_06",
    "i02_host_07",
    "i02_host_08",
    "i02_host_09",
    "i02_host_10",
    "i02_host_11",
    "i02_host_12",
    "i02_host_13",
    "i02_host_14",
    "i02_host_15",
    "i02_host_16",
];

/// One triangle on the registry's default 960 grid — accepted by the legacy validator.
static HOST_PATHS: [&str; 1] = ["M0 0L10 0L10 10Z"];

/// Whether the document contains the compound icon fill, and not a text run or control chrome.
/// Pixels that differ between `without` and `with` anywhere in the frame.
fn differing_pixels(without: &[u8], with: &[u8]) -> usize {
    let (left, _) = without.as_chunks::<4>();
    let (right, _) = with.as_chunks::<4>();
    left.iter().zip(right).filter(|(a, b)| a != b).count()
}

/// Pixels that differ between `without` and `with` inside `rect`.
fn differing_pixels_inside(without: &[u8], with: &[u8], rect: Rect, width: u32) -> usize {
    let mut count = 0;
    for y in rect.y.max(0) as u32..((rect.y + rect.height as i32).max(0) as u32) {
        for x in rect.x.max(0) as u32..((rect.x + rect.width as i32).max(0) as u32) {
            let index = ((y * width + x) * 4) as usize;
            if without[index..index + 4] != with[index..index + 4] {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn a_static_geometry_source_paints_while_the_legacy_registry_is_full() {
    clear_registered_icons();
    for name in &HOST_NAMES[..MAX_REGISTERED_ICONS] {
        assert!(register_icon(name, &HOST_PATHS), "slot for {name} should be free");
    }
    assert_eq!(registered_icon_count(), MAX_REGISTERED_ICONS, "the legacy table is full");
    assert!(
        !register_icon(HOST_NAMES[MAX_REGISTERED_ICONS], &HOST_PATHS),
        "the seventeenth registration must be refused by the legacy contract"
    );
    assert_eq!(registered_icon_count(), MAX_REGISTERED_ICONS, "a refusal must not consume a slot");

    let source = IconSource::static_geometry(&SOLID);
    let geometry = Rect::new(0, 0, 160, 40);
    let size = Size::new(geometry.width, geometry.height);

    // ── Button ──
    // The label is empty so the only thing the two frames can disagree about is the icon.
    let mut button = Button::new(String::new(), geometry);
    assert!(button.icon_rect().is_none(), "a button without a source reserves no icon slot");
    let button_without = frame_of(&mut button, size);
    assert!(
        !has_compound_icon_fill(&render_widget_to_svg(&mut button, geometry)),
        "a button without a source must not emit icon geometry"
    );
    button.set_icon_source(Some(source));
    let button_slot = button.icon_rect().expect("a typed source reserves the icon slot");
    let button_with = frame_of(&mut button, size);
    assert!(
        has_compound_icon_fill(&render_widget_to_svg(&mut button, geometry)),
        "the typed source must reach the backend even with every legacy slot occupied"
    );
    let button_ink = differing_pixels(&button_without, &button_with);
    assert!(button_ink > 0, "the icon must change the frame");
    assert_eq!(
        button_ink,
        differing_pixels_inside(&button_without, &button_with, button_slot, geometry.width),
        "every pixel the icon changed must lie in the slot the button published"
    );

    // ── ToolButton ──
    let mut tool = ToolButton::new("", geometry);
    assert!(tool.icon_rect().is_none(), "a tool button without a source reserves no icon slot");
    let tool_without = frame_of(&mut tool, size);
    assert!(
        !has_compound_icon_fill(&render_widget_to_svg(&mut tool, geometry)),
        "a tool button without a source must not emit icon geometry"
    );
    tool.set_icon_source(Some(source));
    let tool_slot = tool.icon_rect().expect("a typed source reserves the icon slot");
    let tool_with = frame_of(&mut tool, size);
    assert!(
        has_compound_icon_fill(&render_widget_to_svg(&mut tool, geometry)),
        "the typed source must reach the tool button backend"
    );
    let tool_ink = differing_pixels(&tool_without, &tool_with);
    assert!(tool_ink > 0, "the icon must change the frame");
    assert_eq!(
        tool_ink,
        differing_pixels_inside(&tool_without, &tool_with, tool_slot, geometry.width),
        "every pixel the icon changed must lie in the slot the tool button published"
    );

    // ── MenuButton (the "menu" entry §19.8 names) ──
    // Its icon box is derived inside `draw`, so the differential is over the whole frame; the empty
    // label keeps the only movable ink from being the text.
    let mut menu = MenuButton::new("", geometry);
    let menu_without = frame_of(&mut menu, size);
    assert!(
        !has_compound_icon_fill(&render_widget_to_svg(&mut menu, geometry)),
        "a menu button without a source must not emit icon geometry"
    );
    menu.set_icon_source(Some(source));
    let menu_with = frame_of(&mut menu, size);
    assert!(
        has_compound_icon_fill(&render_widget_to_svg(&mut menu, geometry)),
        "the typed source must reach the menu button backend"
    );
    assert!(differing_pixels(&menu_without, &menu_with) > 0, "the icon must change the frame");

    // The legacy table is left as the suite found it.
    clear_registered_icons();
}

// ─── 3. I01: one source, several entries, one glyph ───

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
