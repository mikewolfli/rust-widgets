// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unit tests for the menu control.

use super::*;
use crate::compat::Vec;
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler};
use crate::widget::capability::properties_trait::WidgetProperties;
use crate::widget::capability::types::CapabilityAccessError;
use crate::widget::capability::CapabilityValue;
use crate::widget::menu_toolbar::action::Action;
use crate::widget::metrics::dimensions;
use crate::widget::svg::render_to_svg;
use crate::widget::Widget;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One ink box per text run in document order, as `(left, top, right, bottom)`.
///
/// # Why the ink and not the string
///
/// Text leaves the SVG backend as glyph geometry, not a `<text>` element, so the label is not
/// in the document in any form and a test has to locate a run by *where* it is. That is the
/// stronger check: the old form matched `>Open</text>` and read the element's `y`, so a glyph
/// placed a line away with a correct attribute would have passed it.
///
/// # Why the boxes come from the crate reader, not a local `d` parser
///
/// This used to split each `d` on `M` and read four integers as `x y w h` — the **bitmap**
/// path's grammar and only its grammar. An outline face writes `M17.00 23.84L...Z` with
/// fractional vertices, which that reader took as a rectangle at `(17, 23)` sized by the next
/// two numbers: geometry that is not ink and not where the glyph is, so the labels were located
/// off their rows. `text_ink_boxes` knows both grammars and the `data-text` provenance tag; one
/// reader, so the two cannot drift apart.
fn text_run_boxes(svg: &str) -> Vec<(i32, i32, i32, i32)> {
    crate::widget::svg::text_ink_boxes(svg)
}

/// The bitmap face fills its glyph box, so its ink top *is* the box top. An outline face draws a
/// real glyph whose ink is inset, so the ink top sits one or two pixels below the box top at these
/// sizes. Pinning the two equal encoded a property of the bitmap face, not of the layout.
const INK_INSET_TOLERANCE: i32 = 3;

/// A menu row's label is centred on its own line box, not offset by half a line.
///
/// `draw_text`'s origin is the glyph box's **top** edge, so the previous
/// `y + item_height() / 2` origin put that edge on the row's middle line and drew every
/// entry half a line low — the same mistake the heading above already avoids. Measured
/// through the rendered SVG so the assertion is about what a person sees.
#[test]
fn a_menu_row_is_centred_on_its_own_line_box() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_action("Open");
    menu.add_action("Save");
    // A menu is a popup and is born hidden, so its rows are only drawn once opened.
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
    let svg = crate::widget::svg::render_to_svg(&mut menu);

    // The popup starts below the 20 px heading and 2 px into the body, so the first row is
    // at y = 22 and is 22 px tall. A 14 px line box centred in it starts at 22 + (22-14)/2.
    let row_top = Menu::heading_height() as i32 + 2;
    let line_h = {
        let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(200, 120));
        crate::render::RenderContext::new(&mut backend)
            .measure_text("M", &crate::core::Font::default())
            .height as i32
    };
    let expected_y = row_top + (Menu::item_height() as i32 - line_h) / 2;
    // The rows are located by the band they occupy, not by the string they spell: a run
    // belongs to a row when its ink's top edge falls inside that row's own band. Every label
    // ('O', 'S', 'F') lights its glyph's top row, so the ink top is the glyph box's top edge
    // under the bitmap face and within the inset of it under an outline face — inside the
    // 22 px row either way, so the band test separates the rows.
    let runs = text_run_boxes(&svg);
    let on_row = |index: usize| -> (i32, i32, i32, i32) {
        let top = row_top + index as i32 * Menu::item_height() as i32;
        runs.iter()
            .find(|(_, t, _, _)| *t >= top && *t < top + Menu::item_height() as i32)
            .copied()
            .unwrap_or_else(|| panic!("row {index} painted no ink (rows at {top})"))
    };
    let first = on_row(0);
    // The glyph box's top edge is the line box; the ink top is that edge under the bitmap face
    // and one or two pixels in under an outline face, so the two are pinned equal to within the
    // inset rather than exactly. A row still avoids the old `+ item_height() / 2` origin — that
    // mistake is four pixels out here, past the tolerance.
    assert!(
        (first.1 - expected_y).abs() <= INK_INSET_TOLERANCE,
        "the label belongs on its row's line box: ink top {}, box top {expected_y}",
        first.1
    );
    // Every row derives its origin the same way, so the second row's line box is the first's
    // shifted down by exactly one row.
    assert!(
        (on_row(1).1 - (expected_y + Menu::item_height() as i32)).abs() <= INK_INSET_TOLERANCE,
        "and every row derives its own line box the same way: ink top {}, box top {}",
        on_row(1).1,
        expected_y + Menu::item_height() as i32
    );
    assert_ne!(
        first.1,
        row_top + Menu::item_height() as i32 / 2,
        "the row's middle is not a glyph-box top edge"
    );
    assert!(first.2 > first.0, "the label laid down ink: {first:?}");
    // The ink must stay inside the row it labels, which the old attribute check could not
    // see: the label column is inset by the indicator, so it cannot start at the row's edge.
    assert!(first.0 > 0 && first.2 <= 200, "the label stays within the popup: {first:?}");
}

/// The label's column is whatever the indicator and the trailing column leave.
///
/// BLUE22 §B.8 lists this control for exactly this: the check column, the label column and
/// the two trailing columns were four independent literals (`x + 8`, `x + 28`, `right - 8`,
/// `right - 4`), so a wider indicator or a longer shortcut did not *push* its neighbour.
/// The standard table states the relation directly.
#[test]
fn a_row_reserves_its_indicator_and_trailing_columns() {
    let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    let row = Rect::new(0, 22, 200, Menu::item_height() as u32);
    let indicator = menu.indicator_box(row);
    let label = menu.label_box(row, indicator);

    assert_eq!(
        indicator.x - row.x,
        dimensions::MENU_ROW_PADDING_H as i32,
        "the indicator uses the row's own padding"
    );
    assert_eq!(
        label.x,
        indicator.x + indicator.width as i32 + dimensions::INDICATOR_TEXT_SPACING as i32,
        "the label's column is derived from the indicator's box"
    );
    assert_eq!(
        label.x + label.width as i32,
        row.x + row.width as i32 - dimensions::MENU_ROW_TRAILING_WIDTH as i32,
        "the label yields to the shortcut/arrow column"
    );
}

/// blue27 §19.13 case I12: the columns a menu row actually draws are disjoint.
///
/// One row can carry a **check mark** (the indicator column), a label, a **shortcut** and a
/// **submenu arrow** — three of the four are icon slots, so they are exactly the boxes the unified
/// contract must keep apart, and the failure mode is silent: two boxes can overlap while every
/// individual assertion about them still passes. The check is therefore pairwise on the drawn set.
///
/// The trailing column is shared, so "at most one of shortcut/arrow" is part of the contract rather
/// than an accident: this test found the two being drawn in the *same* box before
/// [`Menu::row_shows_shortcut`] stated the precedence.
#[test]
fn a_rows_drawn_columns_are_disjoint_even_when_the_row_is_narrow() {
    let menu = Menu::new("File", Rect::new(0, 0, 120, 120));
    // Narrow enough that label + shortcut + arrow genuinely compete for the same pixels.
    let row = Rect::new(0, 22, 120, Menu::item_height() as u32);

    let with_shortcut = MenuEntry::with_shortcut(MenuEntry::new("Toggle"), "Ctrl+T");
    let with_arrow = {
        let mut entry = MenuEntry::new("More");
        entry.set_has_submenu(true);
        entry
    };
    let with_both = {
        let mut entry = MenuEntry::with_shortcut(MenuEntry::new("Both"), "Ctrl+B");
        entry.set_has_submenu(true);
        entry
    };

    // A row shows its shortcut only when it has no submenu, so the drawn set is at most three
    // boxes and never the two trailing affordances at once.
    assert!(menu.row_shows_shortcut(&with_shortcut));
    assert!(!menu.row_shows_shortcut(&with_arrow));
    assert!(
        !menu.row_shows_shortcut(&with_both),
        "the arrow takes the shared trailing column; a shortcut there would overlap it"
    );

    let indicator = menu.indicator_box(row);
    let label = menu.label_box(row, indicator);
    let arrow = menu.submenu_arrow_box(row, label);
    let shortcut = Rect::new(
        menu.trailing_column_x(row),
        row.y,
        dimensions::MENU_ROW_TRAILING_WIDTH,
        row.height,
    );

    for (name, entry) in
        [("shortcut-only", &with_shortcut), ("submenu", &with_arrow), ("both", &with_both)]
    {
        let mut drawn: Vec<(&str, Rect)> = vec![("indicator", indicator), ("label", label)];
        if menu.row_shows_shortcut(entry) {
            drawn.push(("shortcut", shortcut));
        }
        if entry.has_submenu() {
            drawn.push(("arrow", arrow));
        }
        for (i, (box_name, box_)) in drawn.iter().enumerate() {
            assert!(
                box_.width > 0,
                "{name}: {box_name} collapsed to zero width, so nothing was asserted about it"
            );
            for (other_name, other) in &drawn[i + 1..] {
                let disjoint =
                    box_.x + box_.width as i32 <= other.x || other.x + other.width as i32 <= box_.x;
                assert!(
                    disjoint,
                    "{name}: {box_name} {box_:?} overlaps {other_name} {other:?}; the drawn \
                     columns must partition the row, not be derived independently"
                );
            }
        }
    }

    // ...and they are in reading order, so "disjoint" cannot be satisfied by a permutation, and
    // every drawn box stays inside the row.
    assert!(indicator.x < label.x && label.x < shortcut.x);
    assert!(shortcut.x <= arrow.x, "the arrow follows the shortcut column");
    for (name, box_) in [("label", label), ("shortcut", shortcut), ("arrow", arrow)] {
        assert!(
            box_.x >= row.x && box_.x + box_.width as i32 <= row.x + row.width as i32,
            "{name} {box_:?} escapes the row {row:?}"
        );
    }
}

/// The check mark and the submenu arrow reach the screen as **geometry**, not as characters.
///
/// §19.11 requires an indicator not to add a duplicate node a screen reader would announce, and
/// the crate reached this by drawing `IconName::Check` / `IconName::ChevronRight` outlines instead
/// of the `✓` / `▶` characters no bundled face covers. Asserted on the document because that is
/// what a reader consumes: the mark must be an icon fill, and neither character may appear.
#[test]
fn the_check_and_arrow_are_geometry_not_text() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 200, 120));
    let mut entry = MenuEntry::new("Toggle");
    entry.set_checkable(true);
    entry.set_checked(true);
    entry.set_has_submenu(true);
    entry.set_shortcut("Ctrl+T");
    menu.add_item(entry);

    let svg = render_to_svg(&mut menu);
    assert!(
        svg.contains(r#"fill-rule="nonzero""#),
        "the tick and the arrow must arrive as icon geometry"
    );
    for character in ['\u{2713}', '\u{25B6}'] {
        assert!(
            !svg.contains(character),
            "the indicator must not be the {character:?} character: it is covered by no bundled \
             face and would read as a second node"
        );
    }
}

/// A wider indicator narrows the label rather than overlapping it (the §B.9 property).
#[test]
fn a_wider_indicator_pushes_the_label_rather_than_overlapping_it() {
    let menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    let short_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX);
    let tall_row = Rect::new(0, 0, 200, dimensions::CHECKBOX_BOX * 2);
    let short = menu.label_box(short_row, menu.indicator_box(short_row));
    let tall = menu.label_box(tall_row, menu.indicator_box(tall_row));
    assert_eq!(
        short.x, tall.x,
        "the indicator column's width is fixed, so the label's start does not move"
    );
    assert!(tall.x + tall.width as i32 <= tall_row.x + tall_row.width as i32);
    assert!(short.x > short_row.x, "the label never starts at the row's own edge");
}

#[test]
fn menu_item_state_accessors_handle_valid_and_oob_indices() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    let idx = menu.add_action("Open");

    assert_eq!(menu.item_enabled(idx), Some(true));
    menu.set_item_enabled(idx, false);
    assert_eq!(menu.item_enabled(idx), Some(false));
    assert_eq!(menu.item_enabled(99), None);

    menu.items[idx].set_checkable(true);
    assert_eq!(menu.item_checked(idx), Some(false));
    menu.set_item_checked(idx, true);
    assert_eq!(menu.item_checked(idx), Some(true));
    assert_eq!(menu.item_checked(99), None);
}

fn context_menu() -> Menu {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
    menu.add_action("Cut");
    menu.add_action("Copy");
    menu.add_separator();
    menu.add_action("Paste");
    menu
}

#[test]
fn the_add_separator_command_appends_through_the_command_channel() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_action("Open");
    assert!(menu.command("add_separator").is_ok());
    assert!(
        menu.items().last().expect("an entry was appended").is_separator(),
        "the command appends the control's own separator"
    );
    // The payload-bearing sibling still needs its list.
    assert_eq!(menu.command("add_action"), Err(CapabilityAccessError::OutOfRange));
}

/// A context menu must appear where the pointer was, and report that origin.
#[test]
fn open_at_places_the_popup_at_the_pointer() {
    let mut menu = context_menu();
    // A menu is born hidden: it is a popup, not a panel, so `open_at` is what
    // makes it appear. Asserting that first keeps the test from passing on a
    // menu that was already visible for an unrelated reason.
    assert!(!menu.is_visible(), "a menu must start hidden");

    menu.open_at(Point::new(300, 200), Rect::new(0, 0, 1000, 800));

    assert!(menu.is_visible());
    assert_eq!(menu.invoker_position(), Some(Point::new(300, 200)));
    assert_eq!(menu.geometry().x, 300);
    assert_eq!(menu.geometry().y, 200);
}

/// Opened near an edge, the popup must shift so it stays fully on screen —
/// otherwise part of the menu is unreachable.
#[test]
fn open_at_clamps_a_popup_that_would_overflow() {
    let viewport = Rect::new(0, 0, 200, 150);
    let mut menu = context_menu();

    menu.open_at(Point::new(195, 148), viewport);

    let rect = menu.geometry();
    assert!(
        rect.x + rect.width as i32 <= viewport.width as i32,
        "popup must not extend past the right edge: {rect:?}",
    );
    assert!(
        rect.y + menu.popup_height() as i32 <= viewport.height as i32,
        "popup must not extend past the bottom edge",
    );
    // The invoker position is recorded unclamped: it describes where the user
    // clicked, which is a different fact from where the popup ended up.
    assert_eq!(menu.invoker_position(), Some(Point::new(195, 148)));
}

/// A press inside the popup that hits an entry triggers it and closes the menu.
#[test]
fn pressing_an_entry_triggers_it_and_closes_the_menu() {
    use std::sync::{Arc, Mutex};

    let mut menu = context_menu();
    menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
    // Signal slots are `Send + Sync`, so the sink is an `Arc<Mutex<_>>` rather
    // than the `Rc<RefCell<_>>` a single-threaded test would otherwise use.
    let fired = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&fired);
    menu.triggered.connect(move |text: Arc<String>| {
        sink.lock().expect("sink poisoned").push((*text).clone());
    });

    // The first entry is drawn below the heading strip and the body's 2px top
    // padding. The test used to click at `y = 14`, inside the heading — it passed
    // only because the hit path started its row walk at the popup's top edge while
    // `draw` started it below the heading, i.e. because the two disagreed.
    let body_top = menu.body_top();
    menu.handle_event(&Event::MousePress {
        pos: Point::new(20, body_top + 4),
        button: 1,
        modifiers: 0,
    });

    assert_eq!(&*fired.lock().expect("sink poisoned"), &["Cut".to_string()]);
    assert!(!menu.is_visible(), "choosing an entry must close the menu");
}

/// The dismissal gesture people actually use is a left-click elsewhere, so an
/// outside press must close the menu whichever button was used.
#[test]
fn a_press_outside_the_popup_closes_it_with_either_button() {
    for button in [crate::event::mouse_button::PRIMARY, crate::event::mouse_button::SECONDARY] {
        let mut menu = context_menu();
        menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));
        assert!(menu.is_visible());

        menu.handle_event(&Event::MousePress { pos: Point::new(5, 5), button, modifiers: 0 });

        assert!(!menu.is_visible(), "button {button} outside the popup must dismiss it");
    }
}

/// A press on the popup's own padding is still "inside": it must not dismiss,
/// or clicking a menu's edge would close it before an entry could be chosen.
#[test]
fn a_press_inside_the_popup_does_not_dismiss_it() {
    let mut menu = context_menu();
    menu.open_at(Point::new(100, 100), Rect::new(0, 0, 400, 400));

    // x is inside the popup but past the text column; y is the top padding.
    menu.handle_event(&Event::MousePress { pos: Point::new(250, 101), button: 1, modifiers: 0 });

    assert!(menu.is_visible(), "padding inside the popup must not dismiss the menu");
}

/// Reopening must not report the previous invocation's position.
#[test]
fn hiding_clears_the_invoker_position() {
    let mut menu = context_menu();
    menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
    assert!(menu.invoker_position().is_some());

    menu.hide();

    assert_eq!(menu.invoker_position(), None);
    assert_eq!(menu.hovered_index(), None);
}

/// The popup's hit area follows its drawn height, not the stored geometry
/// height, so the outside test agrees with what the user sees.
#[test]
fn contains_point_follows_the_drawn_height() {
    let mut menu = context_menu();
    // Geometry height is 100 but the four entries plus padding draw shorter, and the
    // heading sits above them.
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

    let drawn = menu.body_top() + menu.popup_height() as i32;
    assert!(menu.contains_point(Point::new(5, drawn - 1)));
    assert!(!menu.contains_point(Point::new(5, drawn)));
}

/// The rows tile the popup in sequence, below its own padding.
///
/// # What this pins
///
/// BLUE22 §B.8 lists this control's defect as "the check and arrow paddings written by hand".
/// The columns inside a row were already derived; what this pins is the **run**: each row starts
/// where the previous one ended, separators get `MENU_SEPARATOR_HEIGHT` and entries
/// `MENU_ROW_HEIGHT`, and the first row starts `MENU_POPUP_PADDING` below the popup's top edge.
/// The old form carried that arithmetic as a `let mut y` inside the painting loop, so nothing
/// could ask for a row's box without painting the menu.
#[test]
fn the_rows_tile_the_popup_in_sequence() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_action("Open");
    menu.add_separator();
    menu.add_action("Save");

    let bands = menu.item_bands();
    assert_eq!(bands.len(), 3, "one band per entry, separators included");
    assert_eq!(
        bands[0].y,
        dimensions::MENU_POPUP_PADDING as i32,
        "the first row starts below the popup's own padding: {bands:?}"
    );
    for pair in bands.windows(2) {
        assert_eq!(
            pair[1].y,
            pair[0].y + pair[0].height as i32,
            "each row follows the previous one's own box: {bands:?}"
        );
    }
    assert_eq!(bands[0].height, Menu::item_height() as u32);
    assert_eq!(bands[1].height, Menu::separator_height() as u32);
    assert_eq!(bands[2].height, Menu::item_height() as u32);
    // The run is exactly as tall as the popup the control reports.
    let last = bands.last().expect("three bands");
    assert_eq!(
        last.y + last.height as i32 + dimensions::MENU_POPUP_PADDING as i32,
        menu.popup_height() as i32,
        "the rows and the popup's padding add up to the popup's own height"
    );
}

/// A separator sits between two entries and does not disturb their alignment.
///
/// The companion invariant to the run: the columns of a row are read from the row's own box, so
/// a separator shifting every later row down must not shift their *labels* sideways.
#[test]
fn a_separator_shifts_the_rows_but_not_their_columns() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_action("Open");
    menu.add_action("Save");
    let without: Vec<i32> = menu
        .item_bands()
        .iter()
        .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
        .collect();

    menu.add_separator();
    let with: Vec<i32> = menu
        .item_bands()
        .iter()
        .take(2)
        .map(|row| menu.label_box(*row, menu.indicator_box(*row)).x)
        .collect();
    assert_eq!(without, with, "a separator must not move the label columns");
}

/// The highlight must walk the entry list, wrap at both ends, and step over separators.
///
/// # The defect this pins
///
/// The menu handled Escape and Enter and nothing else, so every entry past the first was
/// unreachable without a mouse even though `open_at` pre-selects one "so keyboard navigation has
/// a starting point". A selection that no key can move is an affordance that is declared and not
/// delivered.
#[test]
fn arrow_keys_walk_the_entries_and_skip_separators() {
    let mut menu = context_menu();
    menu.open_at(Point::new(10, 10), Rect::new(0, 0, 400, 400));
    // `open_at` pre-selects the first actionable entry.
    assert_eq!(menu.hovered_index(), Some(0));

    // Down: 0 -> 1 -> (separator skipped) -> 3.
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(1), "Down must move to the next entry");
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(3), "Down must step over the separator");
    // Down again wraps to the top.
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(0), "Down past the end must wrap");

    // Up wraps the other way, again without landing on the separator.
    menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(3), "Up past the start must wrap to the last entry");
    assert!(menu.is_visible(), "navigating must not dismiss the menu");
}

/// A disabled entry is not a destination: the highlight must step over it too.
///
/// Stopping *on* one would leave Enter doing nothing with no visible explanation, which reads as a
/// frozen menu rather than a disabled entry.
#[test]
fn arrow_keys_skip_disabled_entries() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
    menu.add_action("Cut");
    let disabled = menu.add_action("Copy");
    menu.add_action("Paste");
    menu.set_item_enabled(disabled, false);
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

    assert_eq!(menu.hovered_index(), Some(0));
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(2), "Down must step over the disabled entry");
    menu.handle_event(&Event::KeyPress { key: 38, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(0), "and Up must step back over it");
}

/// Enter chooses the highlighted entry; a menu with nothing choosable stays open.
#[test]
fn enter_activates_the_highlight_and_leaves_an_empty_menu_open() {
    let mut menu = context_menu();
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });

    let chosen = Arc::new(Mutex::new(Vec::<usize>::new()));
    let sink = chosen.clone();
    menu.triggered_index.connect(move |index| {
        sink.lock().unwrap().push(*index);
    });
    menu.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
    assert_eq!(*chosen.lock().unwrap(), vec![1], "Enter must trigger the highlighted entry");
    assert!(!menu.is_visible(), "choosing an entry must dismiss the menu");

    // Nothing highlighted: Enter is inert rather than a dismissal with no selection.
    let mut empty = Menu::new("Edit", Rect::new(0, 0, 160, 100));
    empty.add_separator();
    empty.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    assert_eq!(empty.hovered_index(), None, "a separator is not a starting point");
    empty.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
    assert!(empty.is_visible(), "Enter with no highlight must leave the menu open");
}

/// Right/Left follow the writing direction; Up/Down do not.
///
/// A list of entries runs downward in every locale, but the horizontal pair steps along a line of
/// reading order — and a menu is the child of a menu bar, where that pair is what moves between a
/// bar's drop-downs and into a submenu. Mirroring the vertical pair instead would make an Arabic
/// menu's Up key walk backwards.
#[test]
fn horizontal_arrows_follow_the_direction_and_vertical_ones_do_not() {
    for (direction, right_is_forward) in [
        (crate::core::TextDirection::LeftToRight, true),
        (crate::core::TextDirection::RightToLeft, false),
    ] {
        let mut menu = context_menu();
        menu.set_direction(direction);
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        assert_eq!(menu.hovered_index(), Some(0));

        // Key 39 is Right, key 37 is Left.
        let (key, expected) = if right_is_forward { (39, Some(1)) } else { (39, Some(3)) };
        menu.handle_event(&Event::KeyPress { key, modifiers: 0 });
        assert_eq!(
            menu.hovered_index(),
            expected,
            "{direction:?}: Right must move {}",
            if right_is_forward { "forward" } else { "backward" }
        );

        // The vertical pair is untouched by the direction: Down always advances a row.
        let mut vertical = context_menu();
        vertical.set_direction(direction);
        vertical.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
        vertical.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
        assert_eq!(
            vertical.hovered_index(),
            Some(1),
            "{direction:?}: Down must advance a row in every locale"
        );
    }
}

/// The default must be byte-identical, so a menu that never asks behaves exactly as it did.
#[test]
fn the_default_menu_is_still_left_to_right() {
    // The two renders are compared to each other, so the appearance has to be the same for both
    // halves: a concurrent theme switch between them would make this fail for a reason that has
    // nothing to do with direction.
    let _guard = crate::style::theme_test_guard();
    let mut untouched = context_menu();
    untouched.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    let mut ltr = context_menu();
    ltr.set_direction(crate::core::TextDirection::LeftToRight);
    ltr.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    assert_eq!(render_to_svg(&mut untouched), render_to_svg(&mut ltr));
}

/// The direction is reachable from the property API with a matching read-back token.
#[test]
fn the_direction_round_trips_through_the_property_api() {
    let mut menu = context_menu();
    assert_eq!(menu.get("direction").unwrap().as_str(), Some("ltr"));
    menu.set("direction", CapabilityValue::String("rtl".to_string())).unwrap();
    assert_eq!(menu.direction(), crate::core::TextDirection::RightToLeft);
    assert_eq!(menu.get("direction").unwrap().as_str(), Some("rtl"));
}

// ── G-3: a submenu parent is a branch, not a leaf ──────────────────

/// Choosing an entry that owns a submenu must ask for the submenu, never run the entry.
///
/// # The defect this closes (BLUE22 · G-3)
///
/// `MenuEntry::has_submenu` had exactly one consumer: the draw path, which paints the arrow.
/// Both activation paths ignored it, so a click or an Enter on `Cut >` emitted `triggered`
/// with `"Cut"` and hid the popup — the user got a command they had not chosen and the
/// submenu was unreachable by pointer at all. Every test that drove a *leaf* entry passed
/// either way, which is why this needs a test written specifically for the branch.
#[test]
fn a_submenu_parent_is_a_branch_and_not_a_leaf() {
    // A menu built for this test: `context_menu()` already carries four entries, so adding to
    // it would put the branch on row 4 and the press below would land on a leaf.
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
    let mut parent = MenuEntry::new("Cut");
    parent.set_has_submenu(true);
    menu.add_item(parent);
    menu.add_item(MenuEntry::new("Copy"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

    let opened = Arc::new(AtomicUsize::new(0));
    let chosen = Arc::new(AtomicUsize::new(0));
    let opened_clone = Arc::clone(&opened);
    let chosen_clone = Arc::clone(&chosen);
    menu.submenu_requested.connect(move |_: Arc<String>| {
        opened_clone.fetch_add(1, Ordering::SeqCst);
    });
    menu.triggered.connect(move |_: Arc<String>| {
        chosen_clone.fetch_add(1, Ordering::SeqCst);
    });

    // A press on the first row, which is the branch. The popup and its rows are drawn
    // below the heading, so the press uses the same `body_top` derivation the painter does.
    let row_y = menu.body_top() + 4;
    menu.handle_event(&Event::MousePress { pos: Point::new(20, row_y), button: 1, modifiers: 0 });

    assert_eq!(opened.load(Ordering::SeqCst), 1, "the branch must be reported as opened");
    assert_eq!(
        chosen.load(Ordering::SeqCst),
        0,
        "a branch must never report a committed choice -- that is the defect"
    );
    assert!(menu.is_visible(), "a branch that opened must leave the popup up, not hide it");
}

/// A leaf entry still commits exactly as before, so the branch rule did not swallow it.
///
/// The dual of the test above, and the reason the rule is stated as "submenu parent" rather
/// than "any entry": without this, guarding both paths could have made *every* click a
/// branch and the menu would never choose anything.
#[test]
fn a_leaf_entry_commits_and_hides_the_menu() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 160, 100));
    menu.add_item(MenuEntry::new("Copy"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));

    let opened = Arc::new(AtomicUsize::new(0));
    let chosen = Arc::new(AtomicUsize::new(0));
    let opened_clone = Arc::clone(&opened);
    let chosen_clone = Arc::clone(&chosen);
    menu.submenu_requested.connect(move |_: Arc<String>| {
        opened_clone.fetch_add(1, Ordering::SeqCst);
    });
    menu.triggered.connect(move |_: Arc<String>| {
        chosen_clone.fetch_add(1, Ordering::SeqCst);
    });

    menu.handle_event(&Event::MousePress {
        pos: Point::new(20, menu.body_top() + 4),
        button: 1,
        modifiers: 0,
    });

    assert_eq!(chosen.load(Ordering::SeqCst), 1, "a leaf must commit");
    assert_eq!(opened.load(Ordering::SeqCst), 0, "a leaf must not ask for a submenu");
    assert!(!menu.is_visible(), "committing a leaf must dismiss the popup");
}

/// The submenu request is published, so a `forward_all` subscriber can reach it.
///
/// An event that fires but is absent from the capability's list is unreachable through
/// `connect_event` — the designer would offer no way to subscribe to it. This reads the
/// capability the way a host does, through the factory, rather than from the string table:
/// that is the same answer the event panel and `forward_all` get.
#[test]
fn the_submenu_event_is_published_by_the_capability() {
    let factory = crate::widget::WidgetFactory::shared();
    let menu = Menu::new("File", Rect::new(0, 0, 200, 200));
    let capability =
        factory.capability_for_kind_instance(&menu).expect("a menu resolves a capability");
    assert!(
        capability.events.iter().any(|event| event.name == "submenu_requested"),
        "an emitted event must be published: {:?}",
        capability.events.iter().map(|e| e.name).collect::<Vec<_>>()
    );
}

// ── D09-STYLE-01: the menu consumes the resolved theme/caller font ──

/// A caller-authored `style.font` must change the heading and entry rows the menu paints.
///
/// # The defect this pins
///
/// The heading and every row label built a `Font::default()` by hand, so the theme body font
/// and the user's text scale never reached a menu. They now read `metrics::effective_font`.
#[test]
fn a_custom_font_changes_the_menu_ink_width() {
    let render = |font: Option<crate::core::Font>| {
        let mut menu = Menu::new("File", Rect::new(0, 0, 260, 140));
        menu.add_action("Open");
        if let Some(font) = font {
            menu.set_style(crate::style::WidgetStyle::default().with_font(font));
        }
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 1000, 800));
        let svg = crate::widget::svg::render_to_svg(&mut menu);
        crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
    };

    let base_ink = render(None);
    let big_ink = render(Some(crate::core::Font::simple("Test", 30.0)));
    assert!(base_ink > 0, "the default menu must paint ink for this test to mean anything");
    assert!(
        big_ink > base_ink,
        "a larger font must widen the painted menu ink: {big_ink} vs {base_ink}"
    );
}

/// The open reveal must actually reach its end, or the popup stays a slit.
///
/// This is the model half of the defect the GTK demo showed: the first screenshot after a
/// click had the heading and a sliver of body, which is exactly `reveal < 1`. Isolating the
/// tick here says whether the value arrives (a driver problem) or the value is right and the
/// painter/frame loop is not (a repaint problem).
#[test]
fn the_open_reveal_settles_after_a_second_of_frames() {
    let mut menu = context_menu();
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    for _ in 0..80 {
        menu.tick(16);
    }
    assert_eq!(menu.reveal.value(), 1.0, "the reveal must reach fully open");
    assert!(!menu.is_animating(), "a settled reveal owes no more frames");
    assert_eq!(menu.item_bands().len(), menu.items().len());
}

/// A configured cap windows the rows, and both the wheel and the hit test address the
/// **visible** entry rather than its position in the whole list.
#[test]
fn a_configured_cap_windows_the_rows() {
    use crate::widget::menu_toolbar::effects::with_test_effects;

    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 200));
    for index in 0..12 {
        menu.add_action(format!("Item {index}"));
    }
    let mut effects = crate::style::MenuEffects::legacy();
    effects.enabled = true;
    effects.max_visible_items = Some(5);

    with_test_effects(effects, || {
        menu.open_at(Point::new(0, 0), Rect::new(0, 0, 800, 600));
        assert_eq!(menu.visible_range(), 0..5);
        assert_eq!(menu.popup_height(), 5.0 * 22.0 + 4.0);

        // Wheel down scrolls one row at a time and stops at the last page.
        for _ in 0..20 {
            menu.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        }
        assert_eq!(menu.scroll_offset, 7, "12 rows - 5 visible = 7");
        assert_eq!(menu.visible_range(), 7..12);

        // The first painted row is entry 7, not entry 0.
        let first_row_y = menu.body_top() + 2 + 4;
        let hit = menu.item_index_at(Point::new(menu.geometry().x + 4, first_row_y));
        assert_eq!(hit, Some(7), "the hit test must follow the scrolled window");
    });
}

/// A row that owns children opens a real nested panel, and the child row commits.
#[test]
fn a_nested_panel_opens_and_its_child_row_commits() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 180, 120));
    let branch =
        MenuEntry::new("Select").with_submenu(vec![MenuEntry::new("All"), MenuEntry::new("None")]);
    menu.add_item(branch);
    menu.add_item(MenuEntry::new("Copy"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));

    let row_y = menu.body_top() + 4;
    menu.handle_event(&Event::MousePress { pos: Point::new(20, row_y), button: 1, modifiers: 0 });
    assert_eq!(menu.open_submenu_index(), Some(0), "the branch must open its children");
    let panel = menu.submenu_panel_rect().expect("an open panel has a rectangle");
    assert!(
        panel.x >= menu.geometry().x + menu.geometry().width as i32 - 8,
        "the panel hangs to the right of the parent: {panel:?}"
    );

    let chosen = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&chosen);
    menu.triggered.connect(move |_: Arc<String>| {
        sink.fetch_add(1, Ordering::SeqCst);
    });
    let child_y = panel.y + 2 + 4;
    menu.handle_event(&Event::MousePress {
        pos: Point::new(panel.x + 20, child_y),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(chosen.load(Ordering::SeqCst), 1, "a child row must commit");
    assert!(!menu.is_visible(), "committing a child dismisses the menu");
    assert_eq!(menu.open_submenu_index(), None, "hide clears the open panel");
}

/// Escape and Left unwind the nested panel before the menu itself.
#[test]
fn escape_closes_the_nested_panel_first() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 180, 120));
    menu.add_item(
        MenuEntry::new("Select").with_submenu(vec![MenuEntry::new("All"), MenuEntry::new("None")]),
    );
    menu.add_item(MenuEntry::new("Copy"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
    assert!(menu.open_submenu(0));
    assert!(menu.is_visible());

    menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
    assert_eq!(menu.open_submenu_index(), None, "Escape closes one level");
    assert!(menu.is_visible(), "the menu itself stays open after one Escape");
    menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
    assert!(!menu.is_visible(), "the second Escape dismisses the menu");
}

/// A menu whose branch chain is deeper than the control may open.
///
/// Every level carries a branch named after the level below it plus a leaf named after its
/// own level, and a second top-level branch exercises sibling replacement. The deepest
/// branch ("Level 5") sits on level [`MAX_SUBMENU_DEPTH`], where opening is refused.
fn menu_with_deep_chain() -> Menu {
    let level5 = MenuEntry::new("Level 5").with_submenu(vec![MenuEntry::new("Leaf 5")]);
    let level4 = MenuEntry::new("Level 4").with_submenu(vec![level5, MenuEntry::new("Leaf 4")]);
    let level3 = MenuEntry::new("Level 3").with_submenu(vec![level4, MenuEntry::new("Leaf 3")]);
    let level2 = MenuEntry::new("Level 2").with_submenu(vec![level3, MenuEntry::new("Leaf 2")]);
    let level1 = MenuEntry::new("Level 1").with_submenu(vec![level2, MenuEntry::new("Leaf 1")]);
    let mut menu = Menu::new("File", Rect::new(0, 0, 180, 120));
    menu.add_item(level1);
    menu.add_item(MenuEntry::new("Other").with_submenu(vec![MenuEntry::new("Other 1")]));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 4000, 4000));
    menu
}

/// Presses row `index` of `level` through the control's pointer path.
fn press_row(menu: &mut Menu, level: usize, index: usize) {
    let row = menu.row_rect_at(level, index).expect("the row exists");
    menu.handle_event(&Event::MousePress {
        pos: Point::new(row.x + 10, row.y + row.height as i32 / 2),
        button: 1,
        modifiers: 0,
    });
}

/// Four Rights open four panels, each to the right of the level that opened it.
#[test]
fn a_menu_opens_four_nested_panels() {
    let mut menu = menu_with_deep_chain();
    for _ in 0..MAX_SUBMENU_DEPTH {
        menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
    }

    assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);
    assert!(menu.is_visible(), "opening panels must never dismiss the menu");
    assert_eq!(menu.open_submenu_index(), Some(0), "the first step names the top branch");
    let mut previous = menu.geometry();
    for level in 1..=MAX_SUBMENU_DEPTH {
        let panel = menu.submenu_panel_rect_at(level).expect("each level has a panel");
        assert_eq!(
            panel.x,
            previous.x + previous.width as i32 - 6,
            "level {level} hangs off the level above it"
        );
        assert!(panel.width > 0 && panel.height > 0);
        previous = panel;
    }
}

/// The depth contract: a branch on the deepest level announces itself but opens nothing.
#[test]
fn a_menu_refuses_a_fifth_panel_but_announces_the_branch() {
    let mut menu = menu_with_deep_chain();
    let requested = Arc::new(Mutex::new(Vec::new()));
    let captured = requested.clone();
    menu.submenu_requested.connect(move |text| {
        captured.lock().unwrap().push(text.to_string());
    });
    let triggered = Arc::new(AtomicUsize::new(0));
    let captured_triggered = triggered.clone();
    menu.triggered.connect(move |_| {
        captured_triggered.fetch_add(1, Ordering::SeqCst);
    });
    for _ in 0..MAX_SUBMENU_DEPTH {
        menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
    }
    assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);

    // "Level 5" is a branch on the deepest allowed level.
    menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });

    assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH, "no fifth panel may open");
    assert!(menu.is_visible());
    assert_eq!(triggered.load(Ordering::SeqCst), 0, "a branch is not a committed choice");
    let after = requested.lock().unwrap().clone();
    assert_eq!(after.last().map(String::as_str), Some("Level 5"));
}

/// Escape unwinds one panel per press, deepest first, then the menu itself.
#[test]
fn escape_unwinds_every_nested_panel() {
    let mut menu = menu_with_deep_chain();
    for _ in 0..MAX_SUBMENU_DEPTH {
        menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
    }
    for expected in (0..MAX_SUBMENU_DEPTH).rev() {
        menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
        assert_eq!(menu.submenu_depth(), expected);
        assert!(menu.is_visible(), "Escape leaves the menu open until the last press");
    }
    menu.handle_event(&Event::KeyPress { key: 27, modifiers: 0 });
    assert!(!menu.is_visible());
}

/// A leaf in the deepest panel commits and dismisses through the pointer path.
#[test]
fn a_deep_leaf_commits_and_dismisses_the_menu() {
    let mut menu = menu_with_deep_chain();
    let chosen = Arc::new(Mutex::new(String::new()));
    let captured = chosen.clone();
    menu.triggered.connect(move |text| {
        *captured.lock().unwrap() = text.to_string();
    });
    for level in 0..MAX_SUBMENU_DEPTH {
        press_row(&mut menu, level, 0);
    }
    assert_eq!(menu.submenu_depth(), MAX_SUBMENU_DEPTH);

    press_row(&mut menu, MAX_SUBMENU_DEPTH, 1);

    assert_eq!(*chosen.lock().unwrap(), "Leaf 4");
    assert!(!menu.is_visible(), "a leaf selection dismisses the whole popup");
    assert_eq!(menu.submenu_depth(), 0);
}

/// Choosing a sibling branch replaces the deeper panels instead of stacking them.
#[test]
fn switching_a_parent_branch_discards_the_deeper_panels() {
    let mut menu = menu_with_deep_chain();
    press_row(&mut menu, 0, 0);
    press_row(&mut menu, 1, 0);
    assert_eq!(menu.submenu_depth(), 2);

    press_row(&mut menu, 0, 1);

    assert_eq!(menu.submenu_depth(), 1);
    let entries = menu.level_entries(1).expect("the sibling panel is open");
    assert_eq!(entries.first().map(MenuEntry::text), Some("Other 1"));
}

/// The hit rule must accept a press on any open panel, not just the popup rectangle.
///
/// The panels hang to the right of the popup; the runtime asks `Widget::contains_point`
/// before it delivers an event, so a control that only answered for its own rectangle
/// made every child row unreachable at runtime even though the direct-event tests passed.
#[test]
fn contains_point_covers_every_open_panel() {
    let mut menu = menu_with_deep_chain();
    press_row(&mut menu, 0, 0);
    press_row(&mut menu, 1, 0);
    let panel = menu.submenu_panel_rect().expect("the deepest panel is open");
    let child = Point::new(panel.x + 2, panel.y + 2);
    assert!(
        !menu.geometry().contains_point(child),
        "the panel must lie outside the popup rectangle for this test to mean anything"
    );
    assert!(Widget::contains_point(&menu, child), "the runtime hit rule must accept the panel");
}

/// `&` markers parse to a mnemonic, strip from the drawn label, and answer Alt+letter.
#[test]
fn mnemonic_markers_parse_strip_and_activate() {
    use super::{menu_mnemonic, strip_menu_mnemonic};

    assert_eq!(menu_mnemonic("&File"), Some('F'));
    assert_eq!(menu_mnemonic("Save && Exit"), None, "a doubled marker is a literal ampersand");
    assert_eq!(strip_menu_mnemonic("&File"), "File");
    assert_eq!(strip_menu_mnemonic("Save && Exit"), "Save & Exit");
    assert_eq!(strip_menu_mnemonic("Plain"), "Plain");

    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_item(MenuEntry::new("&Open"));
    menu.add_item(MenuEntry::new("&Close"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));

    let chosen = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&chosen);
    menu.triggered.connect(move |_: Arc<String>| {
        sink.fetch_add(1, Ordering::SeqCst);
    });
    // Alt is bit 2 in the event modifier convention.
    menu.handle_event(&Event::KeyPress { key: 79, modifiers: 0b0100 });
    assert_eq!(chosen.load(Ordering::SeqCst), 1, "Alt+O must run the &Open entry");
    assert!(!menu.is_visible());
}

/// Gap 5: the flat panel mirrors the menu's rows and follows every mutation.
#[test]
fn an_attached_panel_mirrors_the_menu_rows() {
    use crate::widget::companions::MenuPanel;

    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    assert!(menu.attach_panel(Some(MenuPanel::new(Rect::new(0, 0, 120, 160)))));
    menu.add_action("Open");
    menu.add_separator();
    menu.add_action("Close");
    {
        let panel = menu.panel().expect("attached");
        assert_eq!(panel.items().len(), 2, "the separator is not a row");
        assert_eq!(panel.items()[0].text, "Open");
        assert_eq!(panel.items()[1].text, "Close");
    }

    menu.set_item_enabled(0, false);
    assert!(!menu.panel().expect("attached").items()[0].enabled);

    menu.clear();
    assert!(menu.panel().expect("attached").items().is_empty());
}

/// A section caption is painted but never highlighted or chosen.
#[test]
fn section_captions_are_not_selectable() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 160));
    menu.add_item(MenuEntry::section("Recent"));
    menu.add_item(MenuEntry::new("One"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
    assert!(menu.items()[0].is_section());
    assert!(!menu.items()[0].is_selectable());

    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(1), "Down must skip the caption");
    assert_eq!(menu.mnemonic_index('r'), None, "a caption's letter is not an accelerator");
}

/// The tear-off strip occupies its own band and reports a press without dismissing.
#[test]
fn the_tearoff_strip_reports_a_press_and_keeps_the_menu_open() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_item(MenuEntry::new("Open"));
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
    let without = menu.popup_height();
    menu.set_tearoff(true);
    assert_eq!(menu.popup_height(), without + 8.0, "the strip adds its own band");

    let torn = Arc::new(AtomicUsize::new(0));
    let sink = Arc::clone(&torn);
    menu.tearoff_requested.connect(move || {
        sink.fetch_add(1, Ordering::SeqCst);
    });
    menu.handle_event(&Event::MousePress {
        pos: Point::new(40, menu.body_top() + 4),
        button: 1,
        modifiers: 0,
    });
    assert_eq!(torn.load(Ordering::SeqCst), 1);
    assert!(menu.is_visible(), "tearing off must not dismiss the source menu");
}

/// Auto flips a popup up when the space below the anchor cannot hold it.
#[test]
fn the_popup_direction_anchors_up_when_the_bottom_is_crowded() {
    let viewport = Rect::new(0, 0, 220, 160);
    let mut auto = context_menu();
    auto.open_at(Point::new(10, 150), viewport);
    let total = auto.geometry().height as i32;
    assert!(
        auto.geometry().y + total <= 150,
        "an Auto menu must end at or above the anchor: {}",
        auto.geometry().y
    );
    assert!(auto.geometry().y >= 0, "and stay inside the viewport");

    // Down keeps the historical top-left anchor when there is room.
    let wide = Rect::new(0, 0, 800, 800);
    let mut down = context_menu();
    down.set_popup_direction(PopupDirection::Down);
    down.open_at(Point::new(100, 200), wide);
    assert_eq!(down.geometry().y, 200);

    let mut up = context_menu();
    up.set_popup_direction(PopupDirection::Up);
    up.open_at(Point::new(100, 200), wide);
    assert_eq!(up.geometry().y + up.geometry().height as i32, 200);
}

/// The host can place the highlight on a row by index.
#[test]
fn set_active_action_highlights_only_a_selectable_row() {
    let mut menu = context_menu();
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 400, 400));
    assert!(menu.set_active_action(1));
    assert_eq!(menu.hovered_index(), Some(1));

    // Index 2 is the separator: it is not a destination.
    assert!(!menu.set_active_action(2));
    assert_eq!(menu.hovered_index(), Some(1), "a refused write must not move the highlight");
    assert!(!menu.set_active_action(99));
}

/// Items can be inserted, removed and looked up by their action id.
#[test]
fn items_can_be_inserted_and_removed_by_position_and_action() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    menu.add_action("A");
    menu.add_action("C");
    let at = menu.insert_item(1, MenuEntry::new("B"));
    assert_eq!(at, 1);
    assert_eq!(menu.items().iter().map(MenuEntry::text).collect::<Vec<_>>(), ["A", "B", "C"]);

    let removed = menu.remove_item(1).expect("an in-range index returns the row");
    assert_eq!(removed.text(), "B");
    assert!(menu.remove_item(99).is_none());

    menu.insert_separator(1);
    assert!(menu.items()[1].is_separator());

    let mut action = Action::new("Save", Rect::new(0, 0, 80, 22));
    action.set_command_id("save");
    menu.add_action_item(&action);
    assert!(menu.remove_action("save"), "the linked row is found by command id");
    assert!(!menu.remove_action("save"), "and removed only once");
}

/// An action's metadata reaches the row it creates.
#[test]
fn add_action_item_mirrors_the_action() {
    let mut action = Action::new("Save", Rect::new(0, 0, 80, 22));
    action.set_command_id("save");
    action.set_checkable(true);
    action.set_checked(true);
    action.set_status_tip("Write the document");
    let id = action.base().id();

    let mut menu = Menu::new("File", Rect::new(0, 0, 200, 120));
    let index = menu.add_action_item(&action);
    let row = &menu.items()[index];
    assert_eq!(row.text(), "Save");
    assert_eq!(row.action_id(), Some("save"));
    assert_eq!(row.action(), Some(id), "the live widget identity is recorded");
    assert_eq!(row.tooltip(), "Write the document");
    assert!(row.is_checkable() && row.is_checked());
}

/// A column count lays rows out as a row-major grid and navigation follows it.
#[test]
fn a_column_count_makes_a_grid_that_the_pointer_and_keys_follow() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 400, 200));
    for index in 0..6 {
        menu.add_action(format!("Item {index}"));
    }
    menu.set_column_count(2);
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 800, 800));
    assert_eq!(menu.columns(), 2);

    let bands = menu.item_bands();
    assert_eq!(bands[1].y, bands[0].y, "the second entry shares the first row");
    assert!(
        bands[1].x >= bands[0].x + bands[0].width as i32,
        "and sits in the next column: {bands:?}"
    );
    assert_eq!(bands[2].x, bands[0].x, "entry two starts the second row");
    assert_eq!(bands[2].y, bands[0].y + Menu::item_height() as i32);

    assert!(menu.set_active_action(0));
    menu.handle_event(&Event::KeyPress { key: 40, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(2), "Down moves one grid row");
    menu.handle_event(&Event::KeyPress { key: 39, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(3), "Right moves one column");

    let row = bands[3];
    let pos = Point::new(menu.geometry().x + row.x + 2, menu.body_top() + row.y + 2);
    assert_eq!(menu.item_index_at(pos), Some(3), "the hit test addresses the column");
}

/// M-005: a bare letter jumps the menu highlight, and an `&` marker is not part
/// of the compared label.
#[test]
fn a_bare_letter_runs_type_ahead_over_the_menu_rows() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 200, 160));
    menu.add_action("&Alpha");
    menu.add_action("Beta");
    menu.add_action("Charlie");
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));

    menu.handle_event(&Event::KeyPress { key: 67, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(2), "C jumps to Charlie");

    // Repeating the letter cycles from the current row, so two rows sharing a
    // first letter are both reachable without a timing window.
    assert!(menu.set_active_action(0));
    menu.handle_event(&Event::KeyPress { key: 65, modifiers: 0 });
    assert_eq!(menu.hovered_index(), Some(0), "A wraps forward to the marked label");
}

/// M-006: a cascading panel near the right edge flips to its parent's leading edge.
#[test]
fn a_submenu_panel_flips_before_it_leaves_the_viewport() {
    let mut menu = Menu::new("File", Rect::new(0, 0, 180, 120));
    menu.add_item(MenuEntry::new("Export").with_submenu(vec![MenuEntry::new("PDF")]));
    menu.open_at(Point::new(200, 0), Rect::new(0, 0, 500, 400));
    assert!(menu.open_submenu(0), "the branch opens");

    let panel = menu.submenu_panel_rect().expect("the panel exists");
    assert!(
        panel.x < menu.geometry().x,
        "the panel flips to the parent's leading edge: {panel:?} vs {:?}",
        menu.geometry()
    );
    assert!(panel.x >= 0 && panel.x + panel.width as i32 <= 500, "and stays on screen");
}

/// M-003: the trailing column is measured once per list revision, not once per row.
#[test]
fn the_trailing_column_is_cached_until_the_list_changes() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 200, 120));
    menu.add_action_with_shortcut("Save", "Ctrl+S");
    let first = menu.trailing_width();
    assert!(menu.trailing_width_cache.get().is_some(), "the first pass fills the cache");
    assert_eq!(menu.trailing_width(), first, "a second read is the cached answer");

    menu.add_action_with_shortcut("Save As", "Ctrl+Shift+S");
    let second = menu.trailing_width();
    assert!(second > first, "a wider shortcut invalidates and widens the column");
}

/// M-004: moving within one row does not re-register its hint on every event.
#[test]
fn hovering_the_same_row_keeps_one_tooltip_registration() {
    let mut menu = Menu::new("Edit", Rect::new(0, 0, 200, 120));
    let mut entry = MenuEntry::new("Open");
    entry.set_tooltip("Open a document");
    menu.add_item(entry);
    menu.open_at(Point::new(0, 0), Rect::new(0, 0, 600, 600));
    menu.hovered_index = Some(0);
    menu.sync_hover_tooltip();
    let id = menu.tooltip_source.expect("the row registered a hint");
    assert_eq!(menu.tooltips.text_for(id), Some("Open a document"));

    // A second move on the same row compares the stored text and does not copy it
    // again; the registration is still the one hint for that row.
    menu.sync_hover_tooltip();
    assert_eq!(menu.tooltips.text_for(id), Some("Open a document"));
}
