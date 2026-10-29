// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unit tests for the `TextArea` control.
//!
//! The cases moved with the control when the single file became a module folder. The imports below
//! are what the inline module used to take from its parent's `use` block.

use super::model::*;
use super::paint::{PendingEdit, FIRST_LINE_INSET, LINE_H};
use crate::compat::{Rc, RefCell};
use crate::core::{Font, Point, Rect, Size};
use crate::event::{Event, EventHandler, FocusReason};
use crate::widget::capability::types::CapabilityValue;
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::FindReplaceBar;
use crate::widget::metrics::dimensions;
use crate::widget::{Draw, Widget};

/// The area turns a context-menu request into its own signal at the requested point.
#[test]
fn a_context_menu_request_emits_the_requested_point() {
    let mut area = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&seen);
    area.context_menu_requested.connect_ref(move |pos: &Point| {
        *sink.lock().unwrap() = Some(*pos);
    });

    assert!(area.context_menu_event(Point::new(12, 20)));
    assert_eq!(*seen.lock().unwrap(), Some(Point::new(12, 20)));
}

/// The bitmap face fills its glyph box, so its ink top *is* the box top. An outline face draws a
/// real glyph whose ink is inset, so the ink top sits one or two pixels below the box top at these
/// sizes. Pinning the two equal encoded a property of the bitmap face, not of the layout.
///
/// Gated with its only consumer:
/// `the_first_line_is_padded_consistently_with_every_other_field` renders through the SVG backend
/// and is `#[cfg(not(alloc_frugal))]`. An ungated constant therefore became dead code under `mini`
/// (which *is* `alloc_frugal`) and `clippy -D warnings` reported it there while every other
/// profile stayed green.
#[cfg(not(alloc_frugal))]
const INK_INSET_TOLERANCE: i32 = 3;

#[test]
fn textarea_creation_defaults() {
    let ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    assert_eq!(ta.text(), "");
    assert_eq!(ta.cursor_pos(), 0);
    assert_eq!(ta.max_length, 0);
    assert!(!ta.is_read_only());
    assert!(ta.placeholder().is_empty());
    assert!(!ta.focused);
}

#[test]
fn textarea_set_text() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_text("Hello\nWorld".to_string());
    assert_eq!(ta.text(), "Hello\nWorld");
    assert_eq!(ta.cursor_pos(), 11); // cursor at end
}

#[test]
fn textarea_insert_char() {
    let mut ta = TextArea::new("Helo".to_string(), Rect::new(0, 0, 300, 200));
    ta.set_cursor_pos(3);
    ta.insert_char('l');
    assert_eq!(ta.text(), "Hello");
    assert_eq!(ta.cursor_pos(), 4);
}

#[test]
fn textarea_delete_char() {
    let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
    ta.set_cursor_pos(5);
    ta.delete_char();
    assert_eq!(ta.text(), "Hell");
    assert_eq!(ta.cursor_pos(), 4);
}

#[test]
fn textarea_delete_char_at_start() {
    let mut ta = TextArea::new("Hello".to_string(), Rect::new(0, 0, 300, 200));
    ta.set_cursor_pos(0);
    ta.delete_char();
    assert_eq!(ta.text(), "Hello");
    assert_eq!(ta.cursor_pos(), 0);
}

#[test]
fn textarea_delete_char_with_newline() {
    let mut ta = TextArea::new("A\nB".to_string(), Rect::new(0, 0, 300, 200));
    // "A\nB" — cursor at 3 (end), delete_char removes 'B', cursor at 2
    ta.set_cursor_pos(3);
    ta.delete_char();
    assert_eq!(ta.text(), "A\n");
    assert_eq!(ta.cursor_pos(), 2);
}

#[test]
fn textarea_cursor_movement() {
    let mut ta = TextArea::new("Hi".to_string(), Rect::new(0, 0, 300, 200));
    assert_eq!(ta.cursor_pos(), 2);
    ta.set_cursor_pos(0);
    assert_eq!(ta.cursor_pos(), 0);
    ta.set_cursor_pos(5); // clamp
    assert_eq!(ta.cursor_pos(), 2);
}

#[test]
fn textarea_placeholder() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    assert!(ta.placeholder().is_empty());
    ta.set_placeholder("Enter text...".to_string());
    assert_eq!(ta.placeholder(), "Enter text...");
}

#[test]
fn textarea_read_only() {
    let mut ta = TextArea::new("Readable".to_string(), Rect::new(0, 0, 300, 200));
    assert!(!ta.is_read_only());
    ta.set_read_only(true);
    assert!(ta.is_read_only());
}

#[test]
fn textarea_max_length() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_max_length(5);
    assert_eq!(ta.max_length, 5);
    // insert beyond limit
    ta.set_text("Hello World".to_string());
    assert_eq!(ta.text().len(), 5);
    assert_eq!(ta.text(), "Hello");
}

#[test]
fn textarea_draw_does_not_panic() {
    use crate::render::RenderContext;
    use crate::render::SoftwarePaintBackend;

    let mut ta = TextArea::new("Line1\nLine2".to_string(), Rect::new(0, 0, 200, 100));
    let mut backend = SoftwarePaintBackend::new(Size::new(200, 100), 1.0);
    let mut ctx = RenderContext::new(&mut backend);
    // Should not panic
    ta.draw(&mut ctx);
}

/// The first line is padded on the **field's** own inset from the top, not at `y = 4`.
///
/// The origin of a text run is its glyph box's top-left corner, so `rect.y + 4` put that
/// corner four pixels below the border and drew the first line half a line high — the
/// placeholder and the value that replaced it therefore sat on different baselines, and
/// `text_area.svg` showed a line whose top edge was `y = 4`. The first line now takes the
/// same line box every other text-bearing control uses.
///
/// The assertion is on the **ink**, not on a `x`/`y` attribute: text leaves the backend as
/// the `font8x8` rectangles the rasteriser fills, so the document holds a picture of the run
/// rather than the run itself. That is a stronger check than the attribute it replaced — a
/// glyph placed a line off with a correct attribute would have passed the old form.
#[cfg(not(alloc_frugal))]
#[test]
fn the_first_line_is_padded_consistently_with_every_other_field() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = TextArea::new("Line1".to_string(), Rect::new(0, 0, 240, 120));
    let svg = crate::widget::svg::render_to_svg(&mut ta);
    let (x, y, right, _) = crate::widget::svg::text_ink_box(&svg)
        .unwrap_or_else(|| panic!("a text path must be emitted for the first line: {svg}"));

    // The bitmap face fills its glyph box, so the run's left ink is the shared inset; an
    // outline face's ink is inset from the box, so the two agree to within that inset rather
    // than exactly — which was a property of the typeface, not of the layout.
    assert!(
        (x - dimensions::TEXT_FIELD_PADDING_H as i32).abs() <= INK_INSET_TOLERANCE,
        "the shared horizontal inset: ink left {x}, inset {}",
        dimensions::TEXT_FIELD_PADDING_H
    );
    assert!(right > x, "the first line laid down ink: {x}..{right}");
    // The first line's band is one line tall and starts at the field's top inset, the same
    // 4 px a single-line field's content starts at; a line centred in that band therefore
    // has its glyph-box top below the border but inside the first row. The two bounds are
    // what the old attribute assertion checked, and they still hold — what they could not
    // see is the *centred* origin, because the bare inset (4) is itself inside the row.
    assert!(y > 0, "the first line clears the border: {y}");
    assert!(y < LINE_H, "and stays inside the first row: {y}");
    // Pin the exact origin: the glyph box's top edge is the line box centred in that band,
    // which is `inset + (band - line) / 2`. Deriving it from the measured line height rather
    // than from a copied literal is what makes this a statement about the layout.
    let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
    let line_h = crate::render::RenderContext::new(&mut backend)
        .measure_text("M", &crate::core::Font::default())
        .height as i32;
    let first_band_top = 4;
    let expected = first_band_top + (LINE_H - line_h) / 2;
    // The glyph box's top edge is the centred line box; the ink top is that edge under the
    // bitmap face and one or two pixels in under an outline face, so the two agree to within
    // the inset. A line left at `y = 4` is a whole inset-plus-half-line away and fails.
    assert!(
        (y - expected).abs() <= INK_INSET_TOLERANCE,
        "the first line sits on the band's centred line box: ink top {y}, box top {expected}"
    );
}

#[test]
fn textarea_set_text_truncates_on_max_length() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_max_length(3);
    ta.set_text("Hello".to_string());
    assert_eq!(ta.text(), "Hel");
    assert_eq!(ta.cursor_pos(), 3);
}

/// Reducing the limit truncates the value *and* invalidates the history that
/// could restore the over-limit string.
#[test]
fn max_length_change_keeps_undo_within_the_limit() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_text("abcdef");
    ta.set_max_length(3);
    assert_eq!(ta.text(), "abc", "the value is truncated to the limit");

    assert!(!ta.can_undo(), "an over-limit value must not be undoable");
    assert!(!ta.undo());
    assert_eq!(ta.text(), "abc");
    assert!(!ta.redo());
    assert_eq!(ta.text(), "abc");
}

#[test]
fn textarea_insert_respects_max_length() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_max_length(3);
    ta.set_text("ABC".to_string());
    ta.insert_char('D');
    // Should not insert because max_length reached
    assert_eq!(ta.text(), "ABC");
    assert_eq!(ta.cursor_pos(), 3);
}

#[test]
fn textarea_empty_text_events_no_panic() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.handle_event(&Event::KeyPress { key: 8, modifiers: 0 }); // backspace on empty
    assert_eq!(ta.text(), "");
    ta.handle_event(&Event::KeyPress { key: 37, modifiers: 0 }); // left arrow at 0
    assert_eq!(ta.cursor_pos(), 0);
    ta.handle_event(&Event::KeyPress { key: 39, modifiers: 0 }); // right arrow at 0 (no change)
    assert_eq!(ta.cursor_pos(), 0);
}

#[test]
fn textarea_focus_events() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    assert!(!ta.focused);
    ta.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
    assert!(ta.focused);
    ta.handle_event(&Event::FocusLost);
    assert!(!ta.focused);
}

#[test]
fn textarea_keypress_insertion() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.handle_event(&Event::KeyPress { key: 72, modifiers: 0 }); // 'H'
    ta.handle_event(&Event::KeyPress { key: 105, modifiers: 0 }); // 'i'
    assert_eq!(ta.text(), "Hi");
}

#[test]
fn textarea_keypress_enter_inserts_newline() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.handle_event(&Event::KeyPress { key: 65, modifiers: 0 }); // 'A'
    ta.handle_event(&Event::KeyPress { key: 13, modifiers: 0 }); // Enter
    ta.handle_event(&Event::KeyPress { key: 66, modifiers: 0 }); // 'B'
    assert_eq!(ta.text(), "A\nB");
}

#[test]
fn textarea_setters_request_redraw() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    ta.base.redraw_requested.connect({
        let flag = std::sync::Arc::clone(&fired);
        move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
    });

    ta.set_text("hello");
    ta.set_cursor_pos(2);
    ta.set_max_length(3);
    ta.set_read_only(true);
    ta.set_placeholder("hint".to_string());

    assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
}

/// The limit counts characters and truncation lands on a character boundary (I-014).
#[test]
fn max_length_truncation_respects_character_boundaries() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_text("你好");
    ta.set_max_length(1);
    assert!(ta.text().is_char_boundary(ta.text().len()));
    assert_eq!(ta.text(), "你", "one character is one cell of the limit");
    assert!(ta.text().is_char_boundary(ta.cursor_pos()));

    // A limit of four characters leaves a two-character value alone, which is what the
    // counter says it should do; the old byte budget cut it to one.
    let mut wide = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    wide.set_text("你好");
    wide.set_max_length(4);
    assert_eq!(wide.text(), "你好");
}

/// A caret set to a mid-character byte is snapped, so the draw cannot slice through one.
///
/// The editor passes an arbitrary byte index to `set_cursor_pos`; every reader slices at the
/// caret (`delete_char`, and the caret's own screen position on the **paint** path), so a
/// mid-character value used to panic the frame rather than merely misplace the marker.
#[test]
fn a_mid_character_cursor_is_snapped_to_a_boundary() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_text("é");
    ta.set_cursor_pos(1); // inside the two-byte 'é'
    assert_eq!(ta.cursor_pos(), 0, "snapped down to a real character boundary");
    // The paint path and the edit path must both survive it.
    let _ = crate::widget::svg::render_to_svg(&mut ta);
    ta.delete_char();
    assert_eq!(ta.text(), "é", "nothing was deleted from position 0");
}

// ─── Selection: the keyboard ───

/// The framework's Shift bit on an event mask.
const SHIFT_BIT: u32 = 0b0001;

fn area(text: &str) -> TextArea {
    TextArea::new(text.to_string(), Rect::new(0, 0, 300, 200))
}

// ─── D09-INPUT-01: platform-committed text reaches the value ───

/// A `TextInput` from the platform must enter the area's value.
///
/// # The defect this pins (D09-INPUT-01)
///
/// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
/// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
/// test feeds `TextInput` to prove that path itself is wired.
#[test]
fn text_input_enters_the_value() {
    let mut ta = area("");
    ta.handle_event(&Event::TextInput { text: "héllo".to_string() });
    assert_eq!(ta.text(), "héllo");
}

/// An IME commit enters the value the same way.
#[test]
fn ime_commit_enters_the_value() {
    let mut area = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    area.handle_event(&Event::ime_commit("你好"));
    assert_eq!(area.text(), "你好");
}

/// The IME query reports a real caret rectangle on the caret's row, and it follows a newline.
///
/// A multi-line field's insertion point is a *row and a column*, not one line: an anchor that
/// stayed on the first row would place the candidate window beside the wrong line as soon as
/// the user typed past the first newline.
#[test]
fn input_method_query_reports_the_caret_row_and_column() {
    let mut area = TextArea::new("abc\ndef".to_string(), Rect::new(10, 20, 300, 200));
    area.set_cursor_pos(0);
    let on_first =
        area.input_method_query().expect("an editable text area always has an insertion point");
    assert!(on_first.caret_rect.width > 0, "the caret must have a width: {on_first:?}");
    assert!(on_first.caret_rect.height > 0, "the caret must have a height: {on_first:?}");
    assert_eq!(on_first.caret_byte, Some(0));

    // The second line starts after the newline at byte 4; byte 6 is two characters in.
    area.set_cursor_pos(6);
    let on_second = area.input_method_query().expect("query after moving to the next line");
    assert_eq!(on_second.caret_byte, Some(6));
    assert!(
        on_second.caret_rect.y > on_first.caret_rect.y,
        "the caret must move down a row on the next line ({} -> {})",
        on_first.caret_rect.y,
        on_second.caret_rect.y
    );
    assert!(
        on_second.caret_rect.x > on_first.caret_rect.x,
        "two columns into the second line is two measured advances right"
    );
}

/// A `TextInput` replaces a selection rather than appending to it.
#[test]
fn text_input_replaces_the_selection() {
    let mut ta = area("abcdef");
    ta.select_all();
    ta.handle_event(&Event::TextInput { text: "Z".to_string() });
    assert_eq!(ta.text(), "Z", "the committed text replaced the selected range");
}

/// Committed text respects `max_length`.
#[test]
fn text_input_respects_the_max_length() {
    let mut ta = area("");
    ta.set_max_length(3);
    ta.handle_event(&Event::TextInput { text: "abcdef".to_string() });
    assert!(ta.text().len() <= 3, "the limit applies: got {:?}", ta.text());
}

/// A read-only area ignores committed text, just as it ignores a printable key.
#[test]
fn read_only_ignores_text_input() {
    let mut ta = area("locked");
    ta.set_read_only(true);
    ta.handle_event(&Event::TextInput { text: "X".to_string() });
    assert_eq!(ta.text(), "locked");
}

/// A disabled area ignores committed text.
#[test]
fn disabled_ignores_text_input() {
    let mut ta = area("");
    ta.set_enabled(false);
    ta.handle_event(&Event::TextInput { text: "X".to_string() });
    assert_eq!(ta.text(), "");
}

/// A Shift-arrow **extends**; a plain arrow replaces.
///
/// # The defect this pins
///
/// This control had no selection at all: `cursor_pos` was the only position it stored, and the
/// arrows moved it. There was therefore no way to select text with the keyboard and, in turn, no
/// way to replace a word by typing over it. The assertions are on the *range* rather than on the
/// anchor, because a range is what the user sees: an implementation that moved the caret and
/// forgot the anchor would still satisfy `cursor_pos` and select nothing.
#[test]
fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
    let mut ta = area("abcdef");
    ta.set_cursor_pos(4);

    ta.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(ta.selected_text(), "d", "the first Shift+Left selects the character behind");
    assert_eq!(ta.cursor_pos(), 3);

    ta.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(ta.selected_text(), "cd", "the second extends the same range");

    // A plain arrow drops the range but keeps the caret where the user had moved it to.
    ta.handle_event(&Event::key_press(37, 0));
    assert_eq!(ta.selection_anchor(), None, "a plain arrow is not an extend");
    assert_eq!(ta.selected_text(), "", "so nothing stays selected");
}

/// An extension keeps **one** anchor, so it is reversible.
///
/// # The defect this pins
///
/// The natural shortcut is to re-anchor at the caret before every shift-movement. That makes the
/// gesture irreversible: `Shift+Home` on a caret at 5 leaves the anchor at 5, so the following
/// `Shift+End` grows from the wrong end. The anchor belongs to the whole gesture, not to each
/// key press within it.
#[test]
fn extending_back_and_forth_returns_to_the_anchor() {
    let mut ta = area("abcdefgh");
    ta.set_cursor_pos(5);

    ta.handle_event(&Event::key_press(36, SHIFT_BIT));
    assert_eq!(ta.selection_anchor(), Some(5), "the anchor is where the gesture began");
    assert_eq!(ta.cursor_pos(), 0, "Home walks the caret to the line start");
    assert_eq!(ta.selected_text(), "abcde");

    ta.handle_event(&Event::key_press(35, SHIFT_BIT));
    assert_eq!(ta.selection_anchor(), Some(5), "and it did not move");
    assert_eq!(ta.selected_text(), "fgh", "so the range is now the other side of it");
}

/// Home and End are **line** bounds here, and Primary reaches the document.
///
/// # The defect this pins
///
/// Home and End were not handled, so they fell through to the character-input catch-all where
/// key 36 is `$` and 35 is `#` — pressing Home typed a dollar sign. A multi-line control whose
/// Home jumps to the start of the document is also close to useless for reaching the line the
/// caret is already on, which is why the plain key is the line and the chord is the document.
#[test]
fn home_and_end_are_line_bounds_and_primary_reaches_the_document() {
    let mut ta = area("one\ntwo\nthree");
    ta.set_cursor_pos(6); // two characters into the second line
    assert_eq!(ta.cursor_pos(), 6);

    ta.handle_event(&Event::key_press(36, 0));
    assert_eq!(ta.cursor_pos(), 4, "plain Home is the start of this line");

    ta.handle_event(&Event::key_press(35, 0));
    assert_eq!(ta.cursor_pos(), 7, "plain End is the end of this line, before its newline");

    // Bit 3 is the event's Meta/Command bit, i.e. the portable primary accelerator.
    ta.handle_event(&Event::key_press(36, 0b1000));
    assert_eq!(ta.cursor_pos(), 0, "Primary+Home is the start of the document");

    ta.handle_event(&Event::key_press(35, 0b1000));
    assert_eq!(ta.cursor_pos(), 13, "Primary+End is the end of the document");
}

/// Up and Down cross **logical** lines and keep the column.
///
/// # The defect this pins
///
/// Up and Down were unhandled, so key 38 (`&`) and 40 (`(`) were typed as text. The only way to
/// reach another line was Home then Left, repeated — which is not a text editor. The column is
/// counted in characters so a multi-byte line does not pull the caret past its own glyph, and a
/// shorter destination line saturates at its end rather than refusing the move.
#[test]
fn up_and_down_cross_lines_keeping_the_column() {
    let mut ta = area("abcd\né\nwxyz");
    ta.set_cursor_pos(3); // column 3 of the first line

    ta.handle_event(&Event::key_press(40, 0));
    // The middle line is `é\n` over bytes 5..7, so its content ends at byte 7 — `é` is two
    // bytes, which is why the saturation point is not the byte right after the newline.
    assert_eq!(ta.cursor_pos(), 7, "down saturates at the end of the short line");

    ta.handle_event(&Event::key_press(40, 0));
    // The column is **re-derived from the caret's own line** each time rather than remembered
    // across the move. After saturating on the short line above, the caret sits at its end
    // (byte 7, i.e. column 1 of nothing), so this Down carries column 1 into `wxyz` — byte 9 —
    // and not the column 3 the gesture started with. Remembering the original column would be
    // the "sticky column" feature, which is a deliberate design choice this control has not
    // made; asserting 10 here would demand a behaviour the code does not promise.
    assert_eq!(ta.cursor_pos(), 9, "down carries the column the caret actually has");

    ta.handle_event(&Event::key_press(38, 0));
    assert_eq!(ta.cursor_pos(), 7, "up saturates on the short line again");

    ta.handle_event(&Event::key_press(38, 0));
    // Up carries column 1 into `abcd`, which is byte 1 — the caret is never left on a column
    // the destination line does not have.
    assert_eq!(ta.cursor_pos(), 1);
}

/// Vertical movement at the first and last line keeps the caret rather than being swallowed.
#[test]
fn vertical_movement_at_the_ends_keeps_the_caret() {
    let mut ta = area("one\ntwo");
    ta.set_cursor_pos(2);
    ta.handle_event(&Event::key_press(38, 0));
    assert_eq!(ta.cursor_pos(), 2, "no line above: the caret keeps its place");

    ta.set_cursor_pos(7);
    ta.handle_event(&Event::key_press(40, 0));
    assert_eq!(ta.cursor_pos(), 7, "no line below either");
}

// ─── Selection: editing over it ───

/// Typing over a selection replaces it — the operation that makes selecting worth having.
#[test]
fn typing_over_a_selection_replaces_it() {
    let mut ta = area("hello world");
    ta.set_cursor_pos(5);
    for _ in 0..5 {
        ta.handle_event(&Event::key_press(37, SHIFT_BIT));
    }
    assert_eq!(ta.selected_text(), "hello");

    ta.handle_event(&Event::key_press(88, 0)); // 'X'
    assert_eq!(ta.text(), "X world", "the selection was consumed, not kept beside the insert");
    assert_eq!(ta.cursor_pos(), 1, "and the caret sits after what replaced it");
    assert_eq!(ta.selection_anchor(), None, "the range it described is gone");
}

/// `insert_char` is the canonical spelling shared with the other text fields (WDGT-14).
#[test]
fn insert_char_is_the_canonical_single_character_insert() {
    let mut ta = area("");
    ta.insert_char('é');
    assert_eq!(ta.text(), "é");
    assert_eq!(ta.cursor_pos(), 'é'.len_utf8(), "the caret follows the character's byte length");
}

/// Backspace and Delete both remove the whole selection rather than one character of it.
#[test]
fn backspace_and_delete_consume_the_selection() {
    for key in [8u32, 46] {
        let mut ta = area("abcdef");
        ta.set_cursor_pos(1);
        for _ in 0..3 {
            ta.handle_event(&Event::key_press(39, SHIFT_BIT));
        }
        assert_eq!(ta.selected_text(), "bcd");
        ta.handle_event(&Event::key_press(key, 0));
        assert_eq!(ta.text(), "aef", "key {key} removed the selection whole");
        assert_eq!(ta.cursor_pos(), 1, "key {key} left the caret at its start");
    }
}

/// Delete removes the character **after** the caret.
///
/// # The defect this pins
///
/// `Delete` was not handled, so key 46 travelled to the character-input catch-all where 46 is
/// `.` — pressing Delete typed a full stop and deleted nothing. The one key whose entire job is
/// forward deletion was the one key that inserted text.
#[test]
fn delete_removes_the_character_after_the_caret() {
    let mut ta = area("abc");
    ta.set_cursor_pos(0);
    ta.handle_event(&Event::key_press(46, 0));
    assert_eq!(ta.text(), "bc", "the character after the caret is gone");
    assert_eq!(ta.cursor_pos(), 0);

    // At the end there is nothing forward to remove, and the value must be untouched.
    ta.set_cursor_pos(2);
    ta.handle_event(&Event::key_press(46, 0));
    assert_eq!(ta.text(), "bc");
}

/// The forward delete is undoable, like every other edit.
#[test]
fn a_forward_delete_is_undoable() {
    let mut ta = area("abc");
    ta.set_cursor_pos(0);
    ta.handle_event(&Event::key_press(46, 0));
    assert_eq!(ta.text(), "bc");
    assert!(ta.undo(), "the deletion was recorded");
    assert_eq!(ta.text(), "abc", "and undo restores the deleted character");
}

/// A control chord is not text: `Primary+b` must not insert a `b`.
///
/// # The defect this pins
///
/// The catch-all typed every unmapped key, so `Primary+B`, `Primary+Q` and every other host
/// accelerator became literal characters in the document.
#[test]
fn a_control_chord_is_not_typed_into_the_area() {
    let mut ta = area("");
    for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", 0b1000)] {
        ta.handle_event(&Event::key_press(98, bits));
        assert!(ta.text().is_empty(), "{name}+b must not insert a `b`");
    }
    // A control code on its own is not content either.
    ta.handle_event(&Event::key_press(0, 0)); // NUL
    assert!(ta.text().is_empty(), "a bare control code is not content");
    ta.handle_event(&Event::key_press(98, 0));
    assert_eq!(ta.text(), "b");
}

/// Primary+A selects the whole document.
#[test]
fn primary_a_selects_the_whole_document() {
    let mut ta = area("one\ntwo");
    ta.set_cursor_pos(0);
    ta.handle_event(&Event::key_press(65, 0b1000));
    assert_eq!(ta.selected_text(), "one\ntwo");
    assert_eq!(ta.cursor_pos(), 7, "the caret is at the end, as select-all leaves it");
}

// ─── Selection: the programmatic surface ───

/// `select_line` covers the line **and** its terminator, so a following delete removes the whole
/// line rather than leaving an empty one behind.
#[test]
fn select_line_covers_the_line_and_its_terminator() {
    let mut ta = area("one\ntwo\nthree");
    assert!(ta.select_line(1), "line 1 exists");
    assert_eq!(ta.selected_text(), "two\n", "the terminator belongs to the line it ends");

    ta.handle_event(&Event::key_press(8, 0)); // backspace over the selection
    assert_eq!(ta.text(), "one\nthree", "the line went with its newline");

    assert!(!ta.select_line(9), "a line that does not exist changes nothing");
}

/// `insert_str` is one edit, so one undo press takes it all back.
///
/// # The defect this pins
///
/// The obvious implementation is a loop over `insert`, which files one snapshot per character:
/// undoing a paste would then take as many presses as the paste had characters.
#[test]
fn insert_str_is_a_single_undo_step() {
    let mut ta = area("[]");
    ta.set_cursor_pos(1);
    ta.insert_str("hello");
    assert_eq!(ta.text(), "[hello]");
    assert_eq!(ta.cursor_pos(), 6, "the caret lands after the inserted run");
    assert!(ta.undo(), "one snapshot covers the whole insertion");
    assert_eq!(ta.text(), "[]", "and one undo takes all of it back");
}

/// A multi-byte value can be selected and deleted without splitting a character.
#[test]
fn a_multi_byte_selection_is_taken_by_character() {
    let mut ta = area("héllo 中文");
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(7, crate::shortcut::Modifiers::SHIFT);
    assert_eq!(ta.selected_text(), "héllo ", "six characters, seven bytes");
    ta.handle_event(&Event::key_press(8, 0));
    assert_eq!(ta.text(), "中文", "the whole selection went, with no fragment left behind");
}

// ─── Selection: the pointer ───

/// The x offset, from the text origin, at which the `n`-th character of `line` begins.
///
/// Computed through the renderer's own measurement rather than `n * 8`: these tests exist to show
/// that a click resolves to the character the user pointed at, so aiming them with a constant
/// that disagrees with the paint would have made them pass for the wrong reason.
fn x_of_char(line: &str, n: usize) -> i32 {
    let prefix: String = line.chars().take(n).collect();
    let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(0, 0));
    crate::render::RenderContext::new(&mut backend)
        .measure_text(&prefix, &crate::core::Font::default())
        .width as i32
}

/// The y of a point one line below the first line's text top.
fn click_y(ta: &TextArea) -> i32 {
    ta.geometry().y + FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2 + LINE_H / 2
}

/// The x of the text origin.
fn pen_x(ta: &TextArea) -> i32 {
    ta.geometry().x + dimensions::TEXT_FIELD_PADDING_H as i32
}

/// A press puts the caret where the pointer landed and anchors a selection there.
///
/// # The defect this pins
///
/// A press used to only focus the control. `set_text` leaves the caret at the end, so clicking
/// into the middle of an existing document and typing appended to it rather than inserting — and
/// there was no pointer selection at all, because nothing computed an offset from a pixel.
#[test]
fn a_press_places_the_caret_where_the_pointer_landed() {
    let mut ta = area("abcdef");
    assert_eq!(ta.cursor_pos(), 6, "`set_text` leaves the caret at the end");

    // Aim at the *middle* of the second character, measured: the half-cell offset is what makes
    // this a test of "the character the pointer is over" rather than of a rounding rule.
    let line = ta.text().lines().next().unwrap_or("").to_string();
    let (x, y) = (pen_x(&ta) + x_of_char(&line, 1) + 2, click_y(&ta));
    ta.handle_event(&Event::mouse_press(x, y, 1));
    assert_eq!(ta.cursor_pos(), 1, "the caret lands on the character clicked");
    assert_eq!(ta.selection_anchor(), Some(1), "and it is the anchor for a drag");
}

/// A drag extends the selection; a hover that was never pressed does not move the caret.
#[cfg(device_profile)]
#[test]
fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
    let mut ta = area("abcdef");
    let (origin, y) = (pen_x(&ta), click_y(&ta));

    // A hover with no press ever delivered leaves the caret exactly where it was.
    ta.handle_event(&Event::mouse_move(origin + 60, y));
    assert_eq!(ta.cursor_pos(), 6, "an unpressed hover must not move the caret");
    assert_eq!(ta.selection_anchor(), None);

    let line = ta.text().lines().next().unwrap_or("").to_string();
    let x1 = origin + x_of_char(&line, 1) + 2;
    let x4 = origin + x_of_char(&line, 4) + 2;
    ta.handle_event(&Event::mouse_press(x1, y, 1));
    ta.handle_event(&Event::mouse_move(x4, y));
    assert_eq!(ta.selected_text(), "bcd", "the drag selected the span between the points");

    // The release ends the gesture, so a later hover is inert again and the range is kept.
    ta.handle_event(&Event::mouse_release(x4, y, 1));
    ta.handle_event(&Event::mouse_move(origin + 140, y));
    assert_eq!(ta.cursor_pos(), 4, "a hover after the release does not extend");
    assert_eq!(ta.selected_text(), "bcd", "and the selection the user made is kept");
}

/// A click on the first glyph resolves to index 0 — the map and the paint agree on the origin.
#[test]
fn a_click_on_the_first_glyph_resolves_to_index_zero() {
    let mut ta = area("Sample");
    let (x, y) = (pen_x(&ta), click_y(&ta));
    ta.handle_event(&Event::mouse_press(x, y, 1));
    assert_eq!(ta.cursor_pos(), 0, "the first pixel of the value is index 0");
}

/// Every click is total: inside the value, and never inside a character.
#[test]
fn a_click_is_total_on_a_multi_line_multi_byte_value() {
    let mut ta = area("héllo\n中文");
    let rect = ta.geometry();
    for dy in (0..rect.height as i32).step_by(7) {
        for dx in (0..rect.width as i32).step_by(13) {
            ta.handle_event(&Event::mouse_press(rect.x + dx, rect.y + dy, 1));
            let caret = ta.cursor_pos();
            assert!(caret <= ta.text().len(), "the caret is inside the value at ({dx},{dy})");
            assert!(
                ta.text().is_char_boundary(caret),
                "the caret at ({dx},{dy}) split a character: {caret} in {:?}",
                ta.text()
            );
        }
    }
}

/// A press on the second line resolves to that line, not to the first.
///
/// This is the multi-line half of pixel-to-offset mapping: a single-line field never has to
/// decide, so an implementation that ignored `y` would look correct there and be wrong here.
#[test]
fn a_press_on_the_second_line_resolves_to_the_second_line() {
    let mut ta = area("abc\ndef");
    let (x, y) = (pen_x(&ta), click_y(&ta));
    // One cell into the second row, measured against that row's own text.
    let second = ta.text().lines().nth(1).unwrap_or("").to_string();
    ta.handle_event(&Event::mouse_press(x + x_of_char(&second, 1) + 2, y + LINE_H, 1));
    // First line is `abc\n` (4 bytes), so column 1 of the second line is byte 5.
    assert_eq!(ta.cursor_pos(), 5, "the caret is in `def`, not in `abc`");
}

// ─── Selection: the paint ───

/// The selection is painted **behind** the value's ink, not over it.
///
/// # Why the assertion is on the glyph box rather than on a colour
///
/// A band drawn after the text would cover the words the user just selected. Asserting that the
/// band exists does not catch that; asserting that the *ink did not move* comes much closer,
/// because a highlight painted over the glyphs sits in the same place but a highlight that
/// displaced them would show up here.
#[cfg(not(alloc_frugal))]
#[test]
fn the_selection_is_painted_without_moving_the_value() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = area("abcdef");
    let plain = crate::widget::svg::render_to_svg(&mut ta);

    // The caret starts at the end (`new` leaves it there), so it is placed explicitly — a
    // selection grown from an unstated caret would be testing the wrong range.
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(3, crate::shortcut::Modifiers::SHIFT);
    assert_eq!(ta.selected_text(), "abc");
    let selected = crate::widget::svg::render_to_svg(&mut ta);

    assert_ne!(plain, selected, "a selection must be visible");
    assert_eq!(
        crate::widget::svg::text_ink_box(&selected),
        crate::widget::svg::text_ink_box(&plain),
        "and it must not move or cover the glyphs it sits behind"
    );
}

/// Nothing selected paints exactly as it did, so the highlight is additive.
#[cfg(not(alloc_frugal))]
#[test]
fn a_caret_without_a_selection_paints_no_band() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = area("abcdef");
    ta.set_cursor_pos(3);
    assert_eq!(ta.selection_range(), None, "a caret alone is not a selection");
    let with_caret = crate::widget::svg::render_to_svg(&mut ta);

    ta.clear_selection();
    let after = crate::widget::svg::render_to_svg(&mut ta);
    assert_eq!(with_caret, after, "clearing a selection that was not there changed nothing");
}

/// A selection spanning a newline is drawn as more than one band.
#[cfg(not(alloc_frugal))]
#[test]
fn a_multi_line_selection_is_drawn_per_line() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = area("ab\ncd");

    let unselected = crate::widget::svg::render_to_svg(&mut ta);
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(4, crate::shortcut::Modifiers::SHIFT);
    assert_eq!(ta.selected_text(), "ab\nc");
    let selected = crate::widget::svg::render_to_svg(&mut ta);

    // The band is drawn as rectangles, and a two-line range needs two of them. Counting the
    // added rects is what distinguishes "per line" from "one full-height bar".
    let added = selected.matches("<rect").count() - unselected.matches("<rect").count();
    assert_eq!(added, 2, "one band per line the range touches");
}

/// The selection band is **filled**, not merely outlined.
///
/// `draw_rect` is a one-pixel outline (see `RenderContext::draw_rect`), so the band used to
/// paint a hollow box around the run: the glyphs were not highlighted, and the field colour
/// inside the box was unchanged. `TextEdit` fills the same arithmetic with `fill_rect`, and
/// this pins the text area to that shape.
#[cfg(not(alloc_frugal))]
#[test]
fn the_selection_band_is_filled_not_outlined() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = area("abcdef");
    let plain = crate::widget::svg::render_to_svg(&mut ta);
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(3, crate::shortcut::Modifiers::SHIFT);
    let selected = crate::widget::svg::render_to_svg(&mut ta);

    /// Every `<rect ... />` element, as an owned string so the two renders can be diffed.
    fn rects(svg: &str) -> crate::compat::Vec<String> {
        svg.split("<rect ")
            .skip(1)
            .filter_map(|chunk| chunk.split_once("/>"))
            .map(|(element, _)| element.to_string())
            .collect()
    }

    let before = rects(&plain);
    let added: crate::compat::Vec<String> =
        rects(&selected).into_iter().filter(|r| !before.contains(r)).collect();
    assert_eq!(added.len(), 1, "the selection adds exactly one rectangle: {added:?}");
    let band = &added[0];
    assert!(
        band.contains("fill=\"rgba") && !band.contains("fill=\"none\""),
        "the band must be filled, not an outline: {band}"
    );
    assert!(!band.contains("stroke="), "an outline would carry a stroke: {band}");
}

/// A long line is clipped to the field rather than painted over its neighbours.
///
/// The run used to be emitted straight across the control's right edge (the runtime only
/// clips a widget whose `children_clip()` is set, which a text area does not override). The
/// clip rectangle is the field's **inner box**, which is also where the caret and the
/// selection band stop, and it still bounds the run while the value is scrolled sideways.
#[cfg(not(alloc_frugal))]
#[test]
fn a_long_line_is_clipped_to_the_fields_inner_box() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = TextArea::new("x".repeat(200), Rect::new(0, 0, 120, 60));
    let svg = crate::widget::svg::render_to_svg(&mut ta);

    let padding = crate::widget::metrics::dimensions::TEXT_FIELD_PADDING_H;
    let expected =
        format!(r#"<rect x="{padding}" y="1" width="{}" height="58" />"#, 120 - 2 * padding);
    assert!(
        svg.contains(&expected),
        "the value must be clipped to the field's inner box ({expected})"
    );
}

/// A long unwrapped line scrolls sideways to keep its caret visible.
///
/// Without this the caret simply ran past the field's trailing edge: the user could move
/// it there but could not see it, and the characters around it were the part of the value
/// that disappeared. The offset is driven by the caret, so returning to the start of the
/// line brings the leading edge back.
#[test]
fn a_long_unwrapped_line_scrolls_to_reveal_the_caret() {
    let mut ta = TextArea::new("x".repeat(200), Rect::new(0, 0, 120, 60));
    assert_eq!(
        ta.get("scroll_x").unwrap(),
        CapabilityValue::Int(0),
        "a freshly installed value opens at its leading edge"
    );

    ta.handle_event(&Event::KeyPress { key: 35, modifiers: 0 }); // End
    let offset = match ta.get("scroll_x").unwrap() {
        CapabilityValue::Int(offset) => offset,
        other => panic!("scroll_x must be an Int, got {other:?}"),
    };
    assert!(offset > 0, "the end of a 200 px line must be scrolled into view");

    ta.handle_event(&Event::KeyPress { key: 36, modifiers: 0 }); // Home
    assert_eq!(
        ta.get("scroll_x").unwrap(),
        CapabilityValue::Int(0),
        "returning to the start restores the leading edge"
    );
}

// ─── Scrolling ───

/// A short field whose value is taller than it: the fixture every scroll test uses.
fn tall_value(field_height: u32) -> TextArea {
    // Built through `crate::compat::Vec` rather than `std`'s, because under `mini` the crate is
    // `no_std` and this file's test module does not import a `Vec` of its own.
    let lines: crate::compat::Vec<String> = (0..20).map(|i| format!("line{i}")).collect();
    let text = lines.join("\n");
    TextArea::new(text, Rect::new(0, 0, 200, field_height))
}

/// Typing past the bottom row scrolls the viewport instead of writing out of sight.
///
/// # The defect this pins
///
/// The paint path already stopped at the last whole row it could fit, so a value taller than the
/// field did not overrun its own rectangle — the probe measured the ink box staying inside. What
/// was missing was any way to *reach* the rows it skipped: there was no scroll position at all, so
/// the field always showed the document's first lines and a caret moved below them was editable
/// but invisible.
#[test]
fn the_viewport_follows_the_caret_down() {
    let mut ta = tall_value(60);
    assert_eq!(ta.first_visible_line(), 0, "a fresh field starts at the top");
    let visible = 3; // 60 px / 16 px rows, less the first-line inset

    // Put the caret on a line well below the viewport.
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(
        ta.text().find("line9").expect("the value has line9"),
        crate::shortcut::Modifiers::NONE,
    );
    assert!(
        ta.first_visible_line() > 0,
        "the viewport moved to keep the caret in view, got {}",
        ta.first_visible_line()
    );
    assert!(
        ta.first_visible_line() <= 9 && 9 < ta.first_visible_line() + visible,
        "line 9 is inside the visible window [{}, {})",
        ta.first_visible_line(),
        ta.first_visible_line() + visible
    );
}

/// The scroll is **minimal**: moving the caret to the top brings the view back, and it does not
/// overshoot past what is needed.
#[test]
fn the_viewport_scrolls_back_minimally() {
    let mut ta = tall_value(60);
    let line9 = ta.text().find("line9").expect("the value has line9");

    ta.set_cursor_pos(line9);
    ta.select_with_modifiers(line9, crate::shortcut::Modifiers::NONE);
    let scrolled = ta.first_visible_line();
    assert!(scrolled > 0, "we are scrolled down");

    // Back to the very start: the viewport returns to the top, not to `scrolled - 1`.
    ta.set_cursor_pos(0);
    ta.select_with_modifiers(0, crate::shortcut::Modifiers::NONE);
    assert_eq!(ta.first_visible_line(), 0, "returning to the first line shows the first line");
}

/// A value that fits never scrolls, so a small document is unaffected by the feature.
#[test]
fn a_value_that_fits_never_scrolls() {
    let mut ta = TextArea::new("one\ntwo".to_string(), Rect::new(0, 0, 200, 200));
    ta.select_all();
    assert_eq!(ta.first_visible_line(), 0, "nothing to scroll to");
    ta.set_cursor_pos(ta.text().len());
    assert_eq!(ta.first_visible_line(), 0);
}

/// A click after scrolling resolves to the line **drawn** on that row.
///
/// # The defect this pins
///
/// The pixel-to-offset map counted rows from the top of the document, so once the viewport had
/// scrolled, a click on the first visible row resolved to a line the user could not see — the
/// caret landed above the field. Translating the viewport row through the same
/// `first_visible_line` the paint uses is what keeps the click and the glyphs on one row.
#[test]
fn a_click_after_scrolling_resolves_to_the_drawn_line() {
    let mut ta = tall_value(60);
    let target = ta.text().find("line9").expect("the value has line9");

    // Scroll so that line 9 is on the first visible row.
    ta.set_cursor_pos(target);
    ta.select_with_modifiers(target, crate::shortcut::Modifiers::NONE);
    let first = ta.first_visible_line();
    assert!(first > 0, "the field is scrolled");

    // A click on the top row, at the leading edge, must land on the line painted there.
    let rect = ta.geometry();
    let y = rect.y + FIRST_LINE_INSET + (LINE_H - Font::default().size_i32()) / 2 + 1;
    let x = rect.x + dimensions::TEXT_FIELD_PADDING_H as i32 + 1;
    ta.handle_event(&Event::mouse_press(x, y, 1));

    let expected = ta.text().split_inclusive('\n').take(first).map(str::len).sum::<usize>();
    assert_eq!(
        ta.cursor_pos(),
        expected,
        "the click resolved to line {first}, the one drawn on the top row"
    );
}

/// Scrolling does not move the caret, and does not fabricate one.
#[test]
fn scrolling_alone_does_not_move_the_caret() {
    let mut ta = tall_value(60);
    let target = ta.text().find("line9").expect("the value has line9");
    ta.set_cursor_pos(target);
    let caret = ta.cursor_pos();
    // Scroll further, then back, without touching the caret.
    ta.select_with_modifiers(target, crate::shortcut::Modifiers::NONE);
    assert_eq!(ta.cursor_pos(), caret, "the caret is where it was put");
}

/// The line table is one pass over the value, and it keeps the document's own line ranges.
///
/// # What this pins
///
/// The paint path used to ask for "the nth line" once per visible row and rescan the document
/// for every answer, and it counted those rows' byte offsets from the first **visible** line
/// rather than from the document's start. [`TextArea::line_spans`] is what the ink, the
/// selection band and the click map now index, so it has to agree with the `lines()` split the
/// painter was written against — including the two cases that are easy to get wrong: an empty
/// value (one empty line, which `split_inclusive` yields nothing for) and a value ending in a
/// terminator (an empty line after it, which `line_count` counts and a caret can sit on).
#[test]
fn the_line_table_agrees_with_str_lines() {
    for text in ["", "a", "a\n", "a\n\nb", "alpha\r\nbeta\r\n", "你好\n世界"] {
        let ta = TextArea::new(text.to_string(), Rect::new(0, 0, 200, 60));
        let spans = ta.line_spans();

        let expected_len =
            text.lines().count() + usize::from(text.is_empty() || text.ends_with('\n'));
        assert_eq!(spans.len(), expected_len, "one span per line of {text:?}");
        assert_eq!(spans.len(), ta.line_count(), "and `line_count` reads the same table");

        for (index, &(start, content_end, end)) in spans.iter().enumerate() {
            let expected = text.lines().nth(index).unwrap_or("");
            assert_eq!(&ta.text()[start..content_end], expected, "line {index} of {text:?}");
            assert!(start <= content_end && content_end <= end, "the span is ordered");
            assert_eq!(
                spans.get(index + 1).map(|next| next.0),
                (index + 1 < spans.len()).then_some(end),
                "each line ends where the next one starts"
            );
        }

        // And the index arithmetic: every byte offset lands on the line that counting newlines
        // before it would name, which is the answer the caret and the scroll offset use.
        for offset in text.char_indices().map(|(index, _)| index).chain([text.len()]) {
            let naive = text[..offset].chars().filter(|&c| c == '\n').count();
            assert_eq!(
                ta.line_index_of(offset),
                naive,
                "offset {offset} in {text:?} belongs to line {naive}"
            );
        }
    }
}

/// A selection that begins on a scrolled line still paints its band.
///
/// # The defect this pins
///
/// The paint loop accumulated each row's byte offsets as it walked the **visible** rows, starting
/// from zero. Once the field was scrolled, that offset belonged to the top of the viewport rather
/// than to the line being painted, so the overlap with the selection came out empty (`from ==
/// to`) and a scrolled field drew no highlight at all — the text a user dragged over was
/// invisible. The offsets now come from the document (see [`TextArea::line_spans`]).
#[cfg(not(alloc_frugal))]
#[test]
fn a_selection_on_a_scrolled_line_still_paints_its_band() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut ta = tall_value(60);
    let line9 = ta.text().find("line9").expect("the value has line9");
    ta.set_cursor_pos(line9);
    ta.select_with_modifiers(line9, crate::shortcut::Modifiers::NONE);
    assert!(ta.first_visible_line() > 0, "the field is scrolled to line 9");

    let unselected = crate::widget::svg::render_to_svg(&mut ta);
    assert!(ta.select_line(9), "line 9 exists");
    assert_eq!(ta.selected_text(), "line9\n", "the range includes the line's terminator");
    let selected = crate::widget::svg::render_to_svg(&mut ta);

    assert!(
        selected.matches("<rect").count() > unselected.matches("<rect").count(),
        "selecting a line while scrolled must paint its band"
    );
}

/// The wheel scrolls the viewport by lines and clamps at both ends of the document.
///
/// # The defect this pins
///
/// The field had no wheel arm at all, so a value taller than its box could only be scrolled
/// with the arrow keys. This drives the same `first_visible_line` the caret path uses, so the
/// painted rows and the scroll position cannot disagree.
#[test]
fn textarea_wheel_scrolls_and_clamps() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let mut ta = TextArea::new(text.join("\n"), Rect::new(0, 0, 200, 60));
    assert_eq!(ta.first_visible_line(), 0);

    // One notch toward the content's end reveals three lines.
    ta.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    assert_eq!(ta.first_visible_line(), 3, "a downward notch moves the viewport down");

    // A notch the other way returns toward the start and cannot go below zero.
    ta.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
    assert_eq!(ta.first_visible_line(), 0);
    ta.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
    assert_eq!(ta.first_visible_line(), 0, "the viewport clamps at the first line");

    // Scrolling past the end stops at the final whole window.
    let max_first = ta.line_count() - ta.visible_line_count();
    for _ in 0..20 {
        ta.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
    }
    assert_eq!(ta.first_visible_line(), max_first, "the viewport clamps at the last window");
}

/// The Material profile keeps the text area's historical 60 px minimum.
#[cfg(device_profile)]
#[test]
fn the_material_profile_keeps_the_text_area_height_floor() {
    let _theme_guard = crate::style::theme_test_guard();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    let ta = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
    assert_eq!(ta.size_hint().height, 60, "the three-row Material budget is unchanged");
}

/// A Macos-profile theme lets an empty area start at the compact field height.
///
/// The area still grows with its text: the profile only replaces the floor, so a value with
/// more rows asks for the same height it always did.
#[cfg(device_profile)]
#[test]
fn a_macos_profile_theme_lowers_the_text_area_height_floor() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-textarea-macos".to_string();
    theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Macos;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-textarea-macos"));
    }

    let empty = TextArea::new(String::new(), Rect::new(0, 0, 240, 120));
    let one_line = TextArea::new("a\nb\nc\nd".to_string(), Rect::new(0, 0, 240, 120));
    let empty_hint = empty.size_hint();
    let tall_hint = one_line.size_hint();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(empty_hint.height, LINE_H as u32 + 10, "one compact row: {empty_hint:?}");
    assert!(empty_hint.height < 60, "the compact profile shrinks the floor: {empty_hint:?}");
    assert_eq!(
        tall_hint.height,
        4 * LINE_H as u32 + 10,
        "content-driven rows are unchanged: {tall_hint:?}"
    );
}

/// The Cupertino profile keeps the Material height floor for a text area.
#[cfg(device_profile)]
#[test]
fn the_cupertino_profile_keeps_the_text_area_height_floor() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-textarea-cupertino".to_string();
    theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Cupertino;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-textarea-cupertino"));
    }

    let hint = TextArea::new(String::new(), Rect::new(0, 0, 240, 120)).size_hint();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(hint.height, 60);
}

/// Soft wrap adds display rows and breaks long lines at word boundaries.
#[test]
fn soft_wrap_adds_word_boundary_rows() {
    let mut ta = TextArea::new("alpha beta gamma delta".to_string(), Rect::new(0, 0, 90, 200));
    let unwrapped = ta.display_rows().len();
    ta.set_soft_wrap(true);
    let wrapped = ta.display_rows();
    assert!(wrapped.len() > unwrapped, "wrapping must add rows: {wrapped:?}");
    let text = ta.text().to_string();
    for pair in wrapped.windows(2) {
        let boundary = pair[0].end;
        if boundary < text.len() && pair[1].start == boundary {
            assert!(
                text[..boundary].ends_with(' ') || text[boundary..].starts_with(' '),
                "a soft break must land at a space: {wrapped:?}"
            );
        }
    }
}

/// The line budget drops the oldest lines and moves the caret with the surviving text.
#[test]
fn a_block_budget_drops_the_oldest_lines() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 300, 200));
    ta.set_max_block_count(Some(2));
    ta.insert_str("one\ntwo\nthree");
    assert_eq!(ta.line_count(), 2);
    assert_eq!(ta.text(), "two\nthree");
    assert!(ta.cursor_pos() <= ta.text().len());
}

/// A content margin moves the ink and the hit test together.
#[test]
fn a_content_margin_insets_the_value() {
    use crate::widget::svg::render_to_svg;
    let mut plain = TextArea::new("margined".to_string(), Rect::new(0, 0, 200, 80));
    let plain_svg = render_to_svg(&mut plain);
    plain.set_content_margin(10);
    assert_eq!(plain.content_margin(), 10);
    assert_ne!(plain_svg, render_to_svg(&mut plain), "the margin must move the glyphs");
}

/// Primary+C and Primary+V move the selected text through the clipboard.
#[cfg(not(alloc_frugal))]
#[test]
fn clipboard_shortcuts_copy_and_paste() {
    let _clipboard = crate::clipboard::clipboard_test_guard();
    let mut source = TextArea::new("copy me".to_string(), Rect::new(0, 0, 200, 80));
    source.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    source.select_all();
    source.handle_event(&Event::KeyPress { key: 67, modifiers: 0b1000 });
    assert_eq!(crate::get_clipboard_text(), "copy me");

    let mut target = TextArea::new(String::new(), Rect::new(0, 0, 200, 80));
    target.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    target.handle_event(&Event::KeyPress { key: 86, modifiers: 0b1000 });
    assert_eq!(target.text(), "copy me");
}

/// A double click selects the word under the pointer.
#[test]
fn a_double_click_selects_a_word() {
    let mut ta = TextArea::new("alpha beta".to_string(), Rect::new(0, 0, 300, 80));
    ta.handle_event(&Event::MouseDoubleClick { pos: Point::new(8, 8), button: 1 });
    assert_eq!(ta.selected_text(), "alpha");
}

/// Page Down and Page Up move the caret by a window of display rows.
#[test]
fn page_keys_move_the_caret() {
    let mut ta = TextArea::new(
        (0..30).map(|n| format!("line {n}\n")).collect::<String>(),
        Rect::new(0, 0, 200, 80),
    );
    ta.set_cursor_pos(0);
    ta.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    ta.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
    let after_down = ta.cursor_pos();
    assert!(after_down > 0);
    ta.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
    assert!(ta.cursor_pos() < after_down);
}

/// Tab is content in a document editor, not a focus gesture.
#[test]
fn tab_inserts_a_tab_character() {
    let mut ta = TextArea::new(String::new(), Rect::new(0, 0, 200, 80));
    ta.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    ta.handle_event(&Event::KeyPress { key: 9, modifiers: 0 });
    assert_eq!(ta.text(), "\t");
}

/// The selection-band widths are memoised for the current revision.
#[test]
fn the_band_width_cache_is_scoped_to_the_revision() {
    let mut ta = TextArea::new("measured".to_string(), Rect::new(0, 0, 200, 80));
    let width = ta.band_advance(0, 4);
    assert_eq!(ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()), 1);
    assert_eq!(ta.band_advance(0, 4), width);
    assert_eq!(
        ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()),
        1,
        "a hit must not append"
    );
    ta.insert_str("x");
    let _ = ta.band_advance(0, 4);
    assert_eq!(
        ta.band_width_cache.borrow().as_ref().map_or(0, |c| c.entries.len()),
        1,
        "the new revision starts clean"
    );
}

/// A middle-paragraph edit patches the row table instead of re-wrapping the document.
#[test]
fn an_incremental_row_rebuild_matches_the_full_table_for_a_middle_line_edit() {
    let mut ta = TextArea::new(
        "first line\nsecond line that is long enough to wrap around\nthird line".to_string(),
        Rect::new(0, 0, 90, 200),
    );
    ta.set_soft_wrap(true);
    let _ = ta.display_rows();
    let stale_revision = ta.display_rows_cache.borrow().as_ref().map(|cache| cache.revision);
    let middle = ta.text().find("long").expect("the middle paragraph has the token");
    ta.set_cursor_pos(middle);
    ta.insert_char('X');
    // The edit's own scroll pass already folds the splice into the row table. If that pass
    // failed the guard, the cache would still be stale here and the comparison below would
    // still pass, so assert the cache was actually rebuilt for the new revision.
    assert!(ta.pending_row_edit.get().is_none(), "the splice hint was consumed");
    assert_eq!(
        ta.display_rows_cache.borrow().as_ref().map(|cache| cache.revision),
        Some(ta.revision)
    );
    assert_ne!(stale_revision, Some(ta.revision));
    let incremental = ta.display_rows();
    // Drop the cache so the next read has to take the whole-table path.
    ta.display_rows_cache.borrow_mut().take();
    ta.pending_row_edit.set(None);
    let full = ta.display_rows();
    assert_eq!(*incremental, *full);
}

/// The incremental helper itself must return rows identical to a from-scratch build.
#[test]
fn an_incremental_splice_matches_the_full_rebuild() {
    let mut ta = TextArea::new(
        "first line\nsecond line that is long enough to wrap around\nthird line".to_string(),
        Rect::new(0, 0, 90, 200),
    );
    ta.set_soft_wrap(true);
    let _ = ta.display_rows();
    let cached = ta.display_rows_cache.borrow_mut().take().expect("the rows are seeded");
    let at = ta.text.find("long").expect("the middle paragraph has the token");
    ta.text.insert(at, 'X');
    ta.line_spans_cache.borrow_mut().take();
    ta.revision = ta.revision.wrapping_add(1);
    let width = ta.content_width();
    let font = ta.text_font().clone();
    let incremental = ta
        .incremental_display_rows(
            &cached,
            PendingEdit { start: at, removed: 0, inserted: 1 },
            width,
            &font,
        )
        .expect("the middle splice is reusable");
    let full = ta.build_display_rows(width, &font, None);
    assert_eq!(*incremental.rows, *full.rows);
}

/// An edit that merges two paragraphs cannot reuse the tail, but the fallback still agrees.
#[test]
fn a_paragraph_merging_edit_matches_the_full_table() {
    let mut ta = TextArea::new("one\ntwo\nthree".to_string(), Rect::new(0, 0, 120, 120));
    ta.set_soft_wrap(true);
    let _ = ta.display_rows();
    let newline = ta.text().find('\n').expect("the value has a terminator");
    ta.set_cursor_pos(newline);
    ta.delete_forward();
    let incremental = ta.display_rows();
    ta.display_rows_cache.borrow_mut().take();
    ta.pending_row_edit.set(None);
    let full = ta.display_rows();
    assert_eq!(*incremental, *full);
}

/// Undo records only the changed run, not a snapshot of the whole document.
#[test]
fn an_edit_is_recorded_as_a_delta() {
    let target = Rc::new(RefCell::new(String::new()));
    let command = TextDeltaCommand::new(target, "hello", "hello!", "Edit text", "text_area_text");
    assert_eq!(command.offset, 5, "the common prefix is not part of the delta");
    assert_eq!(command.removed, "");
    assert_eq!(command.inserted, "!");
}

// ─── COMP-A8: FindReplaceBar wiring ───

/// COMP-A8: attaching reports the current query's matches and queues nothing.
#[test]
#[cfg(full_widgets)]
fn the_find_bar_counts_matches_when_attached() {
    let mut ta = area("one two one two one");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");

    ta.attach_find_replace_bar(&bar);
    assert!(ta.is_find_replace_bar_attached());
    assert_eq!(bar.match_count(), 3, "attach reports the current query's matches");
    assert!(!ta.poll_find_replace_bar(), "attaching queues no intent");
}

/// COMP-A8: a queued navigation intent moves the area's selection on the next poll.
#[test]
#[cfg(full_widgets)]
fn a_find_intent_moves_the_selection_on_poll() {
    let mut ta = area("one two one two one");
    ta.set_cursor_pos(0);
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");
    ta.attach_find_replace_bar(&bar);

    bar.find_next.emit();
    assert!(ta.poll_find_replace_bar(), "the queued intent is applied");
    assert_eq!(ta.selection_range(), Some((0, 3)));
    assert_eq!((bar.match_count(), bar.current_match()), (3, 0));

    bar.find_next.emit();
    assert!(ta.poll_find_replace_bar());
    assert_eq!(ta.selection_range(), Some((8, 11)));
    assert_eq!(bar.current_match(), 1);

    bar.find_previous.emit();
    assert!(ta.poll_find_replace_bar());
    assert_eq!(ta.selection_range(), Some((0, 3)), "previous wraps to the first match");
    assert!(!ta.poll_find_replace_bar(), "the queue is drained");
}

/// COMP-A8: replace and replace-all go through the area's own edit path.
#[test]
#[cfg(full_widgets)]
fn replace_intents_go_through_the_areas_edit_path() {
    let mut ta = area("one two one");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");
    bar.set_replacement("1");
    ta.attach_find_replace_bar(&bar);

    bar.replace_one.emit();
    assert!(ta.poll_find_replace_bar());
    assert_eq!(ta.text(), "1 two one", "the first match is replaced");
    assert_eq!(bar.match_count(), 1, "the counter follows the new document");

    bar.replace_all.emit();
    assert!(ta.poll_find_replace_bar());
    assert_eq!(ta.text(), "1 two 1");
    assert_eq!(bar.match_count(), 0, "nothing matches after the replacement");

    bar.replace_all.emit();
    assert!(!ta.poll_find_replace_bar(), "nothing left to replace");
}

/// COMP-A8: detaching stops the link and keeps the last reported state.
#[test]
#[cfg(full_widgets)]
fn a_detached_find_bar_is_not_polled() {
    let mut ta = area("aaa");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("a");
    ta.attach_find_replace_bar(&bar);
    assert_eq!(bar.match_count(), 3);

    ta.detach_find_replace_bar();
    assert!(!ta.is_find_replace_bar_attached());
    bar.find_next.emit();
    assert!(!ta.poll_find_replace_bar(), "a detached bar reports nothing");
    assert_eq!(bar.match_count(), 3, "detach leaves the last count");
    assert_eq!(ta.selection_range(), None, "and the selection did not move");
}
