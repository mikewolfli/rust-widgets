// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A control the window tree paints must be **visible** — its fill must differ from
//! what is already underneath it.
//!
//! # The defect this pins
//!
//! The control demo's window showed the window background with only the controls'
//! *text* on top: every control's rectangle was the same colour as the window. Read
//! as a whole that is ambiguous — it is consistent with
//!
//! * the theme not reaching the control (its style stays unset),
//! * the control drawing but choosing the window's colour,
//! * the control not being drawn at all.
//!
//! Guessing between those is how a fix lands on the wrong one. So this test renders
//! the window's tree through the *same* entry point the Linux backend's painter uses
//! (`render_frame_tree`) and inspects the pixels of each control's rectangle. A
//! control that is painted, and painted a colour distinguishable from the window
//! background, produces a distinct set of colours inside its rect; a control that is
//! not painted, or painted the background colour, does not.
//!
//! # Why the assertion is "differs from the window background" rather than a literal
//!
//! The literal is the theme's business and changes between appearances. The property
//! that must hold — and the one the user sees — is contrast against what is beneath.

#![cfg(all(
    any(feature = "desktop", feature = "tablet", feature = "mobile"),
    not(any(feature = "mini", feature = "embedded"))
))]

use rust_widgets::app::{App, WidgetHandle, WindowHandle};
use rust_widgets::core::{Color, Rect, Size};
use rust_widgets::theme::{global_theme_manager, AppearanceMode};
use rust_widgets::widget::runtime::with_widget;

/// The rectangle a control was placed at, in the window's coordinate space.
const COMBO_BOX: Rect = Rect { x: 20, y: 142, width: 180, height: 26 };
const LIST_BOX: Rect = Rect { x: 210, y: 142, width: 200, height: 90 };
const BUTTON: Rect = Rect { x: 20, y: 20, width: 150, height: 32 };

/// Renders the window tree and returns its frame as RGBA rows.
fn render(win: &WindowHandle, size: Size) -> Vec<[u8; 4]> {
    let frame = rust_widgets::widget::runtime::render_frame_tree(
        win.raw_id(),
        size,
        // The same clear colour the Linux painter passes. If the window's own draw
        // covers it, the frame's background is the window's colour rather than this.
        Color::rgb(240, 240, 240),
    )
    .expect("the window tree must produce a frame");

    assert_eq!(
        frame.len(),
        (size.width * size.height * 4) as usize,
        "the frame must be `width * height * 4` bytes of RGBA"
    );
    frame.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
}

/// The distinct opaque colours inside `rect` of a `size`-wide frame.
fn distinct_colors(frame: &[[u8; 4]], size: Size, rect: Rect) -> Vec<[u8; 4]> {
    let mut seen: Vec<[u8; 4]> = Vec::new();
    for y in rect.y..(rect.y + rect.height as i32) {
        for x in rect.x..(rect.x + rect.width as i32) {
            if x < 0 || y < 0 || x as u32 >= size.width || y as u32 >= size.height {
                continue;
            }
            let index = (y as u32 * size.width + x as u32) as usize;
            let pixel = frame[index];
            if !seen.contains(&pixel) {
                seen.push(pixel);
            }
        }
    }
    seen
}

/// The single most common colour inside `rect`.
fn dominant_color(frame: &[[u8; 4]], size: Size, rect: Rect) -> [u8; 4] {
    let mut counts: Vec<([u8; 4], usize)> = Vec::new();
    for y in rect.y..(rect.y + rect.height as i32) {
        for x in rect.x..(rect.x + rect.width as i32) {
            if x < 0 || y < 0 || x as u32 >= size.width || y as u32 >= size.height {
                continue;
            }
            let index = (y as u32 * size.width + x as u32) as usize;
            let pixel = frame[index];
            match counts.iter_mut().find(|(c, _)| *c == pixel) {
                Some((_, n)) => *n += 1,
                None => counts.push((pixel, 1)),
            }
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).expect("a non-empty rect").0
}

/// Builds the window the control demo builds, in miniature: the same three controls
/// at the same coordinates, created the same way.
///
/// # Why the window is created away from the origin
///
/// The first version created it at `(0, 0)` — and then the test passed **with the defect
/// reverted**, because a window at the origin has no offset to get wrong. The bug is that
/// a window's geometry is in screen coordinates while its children are in client
/// coordinates; only a window placed away from `(0, 0)` can expose that mixture. This is
/// the control demo's real position, so the test measures the configuration the user saw.
const WINDOW_AT: (i32, i32) = (100, 100);

fn demo_window(app: &mut App) -> WindowHandle {
    let win = app.new_window("paint", WINDOW_AT.0, WINDOW_AT.1, 1120, 620);

    let button = win.new_button("Click Me", BUTTON.x, BUTTON.y, BUTTON.width, BUTTON.height);
    assert_ne!(button.raw_id(), 0, "the button must be created");

    let combo = win.new_combo_box(COMBO_BOX.x, COMBO_BOX.y, COMBO_BOX.width, COMBO_BOX.height);
    assert_ne!(combo.raw_id(), 0, "the combo box must be created");
    combo.add_item("Red");
    combo.add_item("Green");
    combo.add_item("Blue");
    combo.set_current_index(0);

    let list = win.new_list_box(LIST_BOX.x, LIST_BOX.y, LIST_BOX.width, LIST_BOX.height);
    assert_ne!(list.raw_id(), 0, "the list box must be created");
    list.add_item("Alpha");
    list.add_item("Bravo");

    win
}

/// Serialises the tests that switch the process-wide theme.
fn theme_guard() -> std::sync::MutexGuard<'static, ()> {
    rust_widgets::theme::theme_test_guard()
}

#[test]
fn the_window_paints_its_own_background_rather_than_leaving_the_clear_colour() {
    let _guard = theme_guard();
    let _ = global_theme_manager().set_appearance(AppearanceMode::Light);

    let mut app = App::new();
    app.init();
    let win = demo_window(&mut app);

    let size = Size::new(1120, 620);

    // A corner is outside every control, so it shows what the window itself painted.
    //
    // The expected colour is derived through the **same precedence the window's `draw` uses** —
    // explicit style, then the active theme's `background`, then the literal — rather than being
    // hard-coded. It was `[240, 240, 240]`, which happened to equal the light preset's background
    // when this test was written; the window classifies as `Surface`, so `apply_active_theme`
    // resolved it to `surface_container`, and when that role moved the hard-coded expectation went
    // stale for a reason that has nothing to do with the painting. Reading the live widget keeps the
    // two from disagreeing again.
    let expected_fill = |win: &WindowHandle, appearance: AppearanceMode| -> [u8; 4] {
        let _ = global_theme_manager().set_appearance(appearance);
        rust_widgets::reapply_active_theme();
        let manager = global_theme_manager();
        let theme_background =
            manager.current_theme().expect("a preset is active").colors.background;
        let resolved =
            with_widget(win.raw_id(), |w| w.style().background_color.or(Some(theme_background)))
                .flatten()
                .unwrap_or(Color::rgb(240, 240, 240));
        [resolved.r, resolved.g, resolved.b, 255]
    };
    let dark_expected = expected_fill(&win, AppearanceMode::Dark);

    // Back to light for the render under test, which also re-establishes the light theme the
    // window will read when it paints.
    let light_expected = expected_fill(&win, AppearanceMode::Light);
    let frame = render(&win, size);
    let corner = frame[0];
    assert_eq!(
        [corner[0], corner[1], corner[2], corner[3]],
        light_expected,
        "in the light appearance the window must paint its own light background"
    );

    // Switch to dark and the window must repaint dark — the frame must not depend on
    // the clear colour the caller happened to pass.
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);
    rust_widgets::reapply_active_theme();
    let dark_frame = render(&win, size);
    let dark_corner = dark_frame[0];
    assert_eq!(
        [dark_corner[0], dark_corner[1], dark_corner[2], dark_corner[3]],
        dark_expected,
        "after switching to dark the window must paint the dark preset's own background"
    );
    assert_ne!(
        [dark_corner[0], dark_corner[1], dark_corner[2]],
        [light_expected[0], light_expected[1], light_expected[2]],
        "and that must differ from the light background, or the switch proved nothing"
    );

    let _ = global_theme_manager().set_appearance(AppearanceMode::Light);
}

#[test]
fn every_control_paints_something_that_differs_from_the_window_background() {
    let _guard = theme_guard();
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);

    let mut app = App::new();
    app.init();
    let win = demo_window(&mut app);

    let size = Size::new(1120, 620);
    let frame = render(&win, size);

    // The window's own background, read from a corner where no control sits.
    let background = frame[0];

    for (label, rect) in [("button", BUTTON), ("combo_box", COMBO_BOX), ("list_box", LIST_BOX)] {
        let colors = distinct_colors(&frame, size, rect);
        let visible: Vec<[u8; 4]> = colors.iter().copied().filter(|c| *c != background).collect();
        assert!(
            !visible.is_empty(),
            "{label} painted only the window background {background:?} inside {rect:?}: \
             it is invisible on screen. Colours found: {colors:?}"
        );

        // Contrast is not enough on its own: a control must own most of its rectangle,
        // not merely outline it. The dominant colour inside the rect is what fills it.
        let dominant = dominant_color(&frame, size, rect);
        assert_ne!(
            dominant, background,
            "{label} is filled with the window background {background:?}; only its edges \
             differ, which is not what a {label} should look like"
        );
    }

    let _ = global_theme_manager().set_appearance(AppearanceMode::Light);
}

#[test]
fn a_combo_box_paints_a_dropdown_affordance_apart_from_its_text() {
    let _guard = theme_guard();
    let _ = global_theme_manager().set_appearance(AppearanceMode::Dark);

    let mut app = App::new();
    app.init();
    let win = demo_window(&mut app);

    let size = Size::new(1120, 620);
    let frame = render(&win, size);
    let background = frame[0];

    // The control's fill, which the previous test proves differs from the background.
    let fill = dominant_color(&frame, size, COMBO_BOX);

    // The arrow sits in the right-hand strip; the current item's text starts at the
    // left padding. A combo box that draws no arrow has that strip filled with
    // nothing but the control's own fill.
    let arrow_strip = Rect {
        x: COMBO_BOX.x + COMBO_BOX.width as i32 - 16,
        y: COMBO_BOX.y,
        width: 16,
        height: COMBO_BOX.height,
    };
    let arrow_colors: Vec<[u8; 4]> = distinct_colors(&frame, size, arrow_strip)
        .into_iter()
        .filter(|c| *c != fill && *c != background)
        .collect();
    assert!(
        !arrow_colors.is_empty(),
        "the combo box's dropdown strip contains only its own fill {fill:?} (and the \
         background {background:?}): no arrow is drawn, so nothing marks it as a \
         combo box rather than a label in a box"
    );

    let _ = global_theme_manager().set_appearance(AppearanceMode::Light);
}
