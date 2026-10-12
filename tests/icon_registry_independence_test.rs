// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! §19.8's acceptance sentence: a direct-geometry icon must not need a registry slot.
//!
//! blue27 case I02 states the claim in one line — *with the registry full, `StaticGeometry` still
//! shows on Button/ToolButton/menu, while the 17th legacy registration is refused exactly as
//! before*. Two properties have to hold at the same time, which is why a test of only one of them
//! could pass while the contract was broken:
//!
//! * **the new path is registry-independent** — a geometry the bundled table has never heard of
//!   paints while every legacy slot is occupied;
//! * **the old contract is unchanged** — the seventeenth `register_icon` still returns `false`
//!   rather than replacing a slot or growing the table.
//!
//! # How "it painted" is told apart from "nothing happened"
//!
//! Two independent pieces of evidence, because either alone can be satisfied by accident:
//!
//! 1. The icon outline is identified the way the SVG backend tags it — the same marker
//!    `icon_source_equivalence_test.rs` documents: a compound icon fill is the only element that
//!    carries `fill-rule="nonzero"` **and** no `data-text`. Button chrome, a popup arrow and a text
//!    run therefore cannot be mistaken for the icon, and a control that ignored its typed source
//!    emits no such element at all.
//! 2. A software pixel differential against the slot the control *itself* publishes: the two frames
//!    must differ, and every differing pixel must lie inside `icon_rect()`. A control that painted
//!    its icon outside the box it reserved fails that. The buttons are built with an empty label so
//!    the only thing that can change between the two frames is the icon.
//!
//! The differential is against `icon_rect()` rather than "some non-background pixel" because
//! §19.4 makes the declared slot the contract: layout reserves it and paint fills it, and the two
//! must be the same box.
//!
//! # Why the whole file is one test
//!
//! `register_icon` writes process-global state that every other icon test can observe, so filling
//! the table and asserting on "full" has to be one uninterrupted sequence. Splitting it into
//! parallel `#[test]`s would let another test run against a half-filled table.
#![cfg(full_widgets)]

use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
use rust_widgets::widget::draw::Draw;
use rust_widgets::widget::svg::render_widget_to_svg;
use rust_widgets::widget::{
    clear_registered_icons, register_icon, registered_icon_count, Button, IconData, IconSource,
    MenuButton, ToolButton, MAX_REGISTERED_ICONS,
};

/// A shape no bundled token draws, so painting it can only come from the typed source.
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
