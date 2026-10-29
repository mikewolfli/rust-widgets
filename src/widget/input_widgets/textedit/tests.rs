// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unit tests for [`TextEdit`](super::TextEdit), kept out of the production files.

use super::*;

use crate::core::{Font, Rect};
use crate::event::{Event, EventHandler};
use crate::widget::capability::WidgetProperties;
#[cfg(full_widgets)]
#[cfg(full_widgets)]
use crate::widget::companions::FindReplaceBar;
use crate::widget::{Widget, WidgetKind};

/// The wrap table is memoised and re-wrapped when text, width or wrap mode changes (WDGT-01).
#[test]
fn the_row_table_is_cached_and_refreshes_when_an_input_changes() {
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
    te.set_text("hello\nworld");
    let first = te.layout_rows();
    assert_eq!(first.len(), 2);
    assert_eq!(te.layout_rows(), first, "a second read reuses the table");
    assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").rows, first);

    te.set_text("hello\nworld\nthird");
    assert_eq!(te.layout_rows().len(), 3, "a text change re-wraps");

    te.set_geometry(Rect::new(0, 0, 40, 100));
    let narrow = te.layout_rows();
    assert!(narrow.len() >= 3, "a narrower interior re-wraps: {narrow:?}");

    te.set_line_wrap(false);
    assert_eq!(te.layout_rows().len(), 3, "wrap off is one row per line");
}

#[test]
fn textedit_property_route_reaches_its_own_accessors() {
    // `text_edit` publishes these five names, so the property route must
    // accept them rather than answering `UnknownProperty`.
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    use crate::widget::capability::types::CapabilityValue;

    te.set("text", CapabilityValue::String("hello".to_string()))
        .expect("`text` is a published, writable property");
    assert_eq!(te.text(), "hello");
    assert_eq!(te.get("text").unwrap(), CapabilityValue::String("hello".to_string()));

    te.set("placeholder_text", CapabilityValue::String("type here".to_string()))
        .expect("`placeholder_text` is writable");
    assert_eq!(te.placeholder_text(), "type here");

    te.set("max_length", CapabilityValue::UInt(16)).expect("`max_length` is writable");
    assert_eq!(te.max_length(), Some(16));

    te.set("read_only", CapabilityValue::Bool(true)).expect("`read_only` is writable");
    assert!(te.is_read_only());

    te.set("line_wrap", CapabilityValue::Bool(false)).expect("`line_wrap` is writable");
    assert!(!te.line_wrap());

    // A type mismatch is still reported rather than silently coerced.
    assert!(te.set("read_only", CapabilityValue::UInt(1)).is_err());
}

#[test]
fn textedit_creation_defaults() {
    let te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(te.text().is_empty());
    assert!(te.placeholder_text().is_empty());
    assert_eq!(te.max_length(), None);
    assert!(!te.is_read_only());
    assert!(te.line_wrap());
    assert!(te.is_empty());
    assert_eq!(te.line_count(), 1);
}

#[test]
fn textedit_set_text() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("Hello World".to_string());
    assert_eq!(te.text(), "Hello World");
    assert!(!te.is_empty());
}

#[test]
fn textedit_set_text_empty() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("Some text".to_string());
    te.set_text(String::new());
    assert!(te.text().is_empty());
}

#[test]
fn textedit_placeholder() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(te.placeholder_text().is_empty());
    te.set_placeholder_text("Enter text here".to_string());
    assert_eq!(te.placeholder_text(), "Enter text here");
    te.set_placeholder_text(String::new());
    assert!(te.placeholder_text().is_empty());
}

#[test]
fn textedit_max_length() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert_eq!(te.max_length(), None);
    te.set_max_length(Some(10));
    assert_eq!(te.max_length(), Some(10));
    te.set_max_length(None);
    assert_eq!(te.max_length(), None);
}

#[test]
fn textedit_max_length_truncates() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("Hello World Too Long".to_string());
    te.set_max_length(Some(10));
    assert_eq!(te.text().len(), 10);
}

#[test]
fn textedit_read_only() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(!te.is_read_only());
    te.set_read_only(true);
    assert!(te.is_read_only());
    te.set_read_only(false);
    assert!(!te.is_read_only());
}

#[test]
fn textedit_line_wrap() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(te.line_wrap());
    te.set_line_wrap(false);
    assert!(!te.line_wrap());
    te.set_line_wrap(true);
    assert!(te.line_wrap());
}

#[test]
fn textedit_line_count() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert_eq!(te.line_count(), 1);
    te.set_text("Line 1\nLine 2\nLine 3".to_string());
    assert_eq!(te.line_count(), 3);
}

#[test]
fn textedit_line_text() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("First\nSecond\nThird".to_string());
    assert_eq!(te.line_text(0), Some("First"));
    assert_eq!(te.line_text(1), Some("Second"));
    assert_eq!(te.line_text(2), Some("Third"));
    assert_eq!(te.line_text(5), None);
}

#[test]
fn textedit_append() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.append("Hello");
    assert_eq!(te.text(), "Hello");
    te.append(" World");
    assert_eq!(te.text(), "Hello World");
}

#[test]
fn textedit_undo_redo_restores_text() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("one");
    te.set_text("two");
    assert!(te.undo());
    assert_eq!(te.text(), "one");
    assert!(te.redo());
    assert_eq!(te.text(), "two");
}

#[test]
fn textedit_control_z_and_control_y_drive_history() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.set_text("before");
    te.set_text("after");
    // Bit 3 is the event's Meta/Command bit, i.e. the portable primary accelerator
    // (`Modifiers::from_event_bits` folds it into both PRIMARY and CTRL). The physical-Control
    // bit is bit 1.
    //
    // This test used to pass with `modifiers: 2` for a different reason: the catch-all arm typed
    // every unmapped printable key, so Ctrl+Z wrote a literal `Z` — which *also* changed the
    // value, and the assertion only looked at the text. Fixing the chord guard exposed it.
    te.handle_event(&Event::key_press(90, 0b1000));
    assert_eq!(te.text(), "before");
    te.handle_event(&Event::key_press(89, 0b1000));
    assert_eq!(te.text(), "after");
}

#[test]
fn textedit_clear() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("Some text".to_string());
    te.clear();
    assert!(te.is_empty());
}

#[test]
fn textedit_geometry_delegation() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_geometry(Rect::new(10, 10, 400, 300));
    assert_eq!(te.geometry(), Rect::new(10, 10, 400, 300));
}

#[test]
fn textedit_visibility() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(te.is_visible());
    te.hide();
    assert!(!te.is_visible());
    te.show();
    assert!(te.is_visible());
}

#[test]
fn textedit_enabled() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    assert!(te.is_enabled());
    te.set_enabled(false);
    assert!(!te.is_enabled());
    te.set_enabled(true);
    assert!(te.is_enabled());
}

#[test]
fn textedit_id_kind() {
    let te_a = TextEdit::new(Rect::new(0, 0, 100, 100));
    let te_b = TextEdit::new(Rect::new(0, 0, 100, 100));
    assert_ne!(te_a.id(), te_b.id());
    assert_eq!(te_a.kind(), WidgetKind::TextEdit);
    assert_eq!(te_b.kind(), WidgetKind::TextEdit);
}

#[test]
fn textedit_signal_accessors() {
    let te = TextEdit::new(Rect::new(0, 0, 100, 100));
    let _ = &te.text_changed;
}

// ── `max_length` bounds every path into the field ──

/// The limit is enforced on **typing**, not only on the value that was already there.
///
/// # What was dead, and what proves it is alive now
///
/// `max_length` was stored, published (getter, setter, schema row, round-trip test) and read by
/// nothing in the edit path: the truncation only ran when the *limit itself* changed. So
/// `set_max_length(Some(10))` on a short field followed by typing twenty characters produced a
/// twenty-character field, and the property's reader could not tell — it answered `Some(10)`
/// either way. This drives the actual keystrokes through `handle_event`, which is the path a
/// user takes.
#[test]
fn max_length_bounds_typing_not_only_the_existing_value() {
    use crate::event::Event;
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    // The keyboard is gated on focus; a user reaches an editor by clicking it.
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.set_max_length(Some(5));
    for ch in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
        te.handle_event(&Event::KeyPress { key: ch as u32, modifiers: 0 });
    }
    assert_eq!(te.text(), "abcde", "the field stopped at the limit, and did not lose a prefix");
    assert_eq!(te.text().len(), 5);

    // A paste is the case a limit most obviously exists for, so it is refused wholesale rather
    // than silently cut: a user cannot tell a completed paste from a truncated one.
    let before = te.text().to_string();
    te.set_text("a much longer pasted string");
    assert_eq!(te.text(), before, "an over-long paste is refused, not truncated");

    // Deleting still works, so a full field is not a stuck one.
    te.handle_event(&Event::KeyPress { key: 8, modifiers: 0 });
    assert_eq!(te.text(), "abcd");
    // And the field is usable again now that there is room.
    te.handle_event(&Event::KeyPress { key: 'z' as u32, modifiers: 0 });
    assert_eq!(te.text(), "abcdz");

    // Raising the limit re-opens the field; clearing it removes the bound entirely.
    te.set_max_length(Some(10));
    te.set_text("0123456789");
    assert_eq!(te.text(), "0123456789");
    te.set_max_length(None);
    te.set_text("a string of any length at all");
    assert_eq!(te.text(), "a string of any length at all");

    // Lowering the limit truncates the text already in the field, counting **characters** and
    // cutting on a char boundary — the family-wide budget (TE-01/I-014), not bytes.
    let mut unicode = TextEdit::new(Rect::new(0, 0, 300, 200));
    unicode.set_text("你好世界");
    unicode.set_max_length(Some(7));
    assert_eq!(
        unicode.text(),
        "你好世界",
        "a character budget above the value's char count keeps it"
    );
    unicode.set_max_length(Some(2));
    assert_eq!(unicode.text(), "你好", "a multi-byte truncation lands on a boundary");
}

// ── `line_wrap` decides the layout of the value ──

/// Wrapping folds a long line onto the next row; not wrapping keeps one row and clips.
///
/// # What was dead, and what proves it is alive now
///
/// `line_wrap` was stored, published (`get`/`set`/schema row/round-trip test) and read by
/// nothing. `draw` was a single `draw_text` of the whole document, commented in place as "in
/// real implementation would handle line wrapping" — a stored promise. This asserts the two
/// settings paint different pictures *and* that the difference is the wrapping: with wrapping on
/// the value occupies several rows, with it off exactly one.
#[test]
fn line_wrap_folds_the_value_and_its_absence_does_not() {
    use crate::widget::svg::{render_to_svg, text_ink_boxes};

    let build = |wrap: bool| {
        let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
        te.set_line_wrap(wrap);
        te.set_text("the quick brown fox jumps over the lazy dog");
        te
    };

    let wrapped = render_to_svg(&mut build(true));
    let unwrapped = render_to_svg(&mut build(false));
    assert_ne!(
        wrapped, unwrapped,
        "the two settings must not render identically -- that was the dead state"
    );

    // The question `line_wrap` answers is "how many rows does the value occupy", so that is what
    // is asserted -- a bare "the SVG differs" would pass on a one-pixel shift. The layout emits
    // one `draw_text` per row and one glyph-geometry `<path>` per `draw_text`, so the row count
    // is the number of text ink boxes.
    let wrapped_rows = text_ink_boxes(&wrapped).len();
    let unwrapped_rows = text_ink_boxes(&unwrapped).len();
    assert!(
        wrapped_rows > unwrapped_rows,
        "wrapping must use more rows: {wrapped_rows} vs {unwrapped_rows}"
    );
    // Not wrapping is exactly one row of text, however long the line is.
    assert_eq!(unwrapped_rows, 1, "an unwrapped value stays on one row");

    // An explicit newline breaks on either setting, because it is the text's own decision rather
    // than the layout's.
    let mut explicit = TextEdit::new(Rect::new(0, 0, 300, 200));
    explicit.set_line_wrap(false);
    explicit.set_text("first\nsecond");
    assert_eq!(
        text_ink_boxes(&render_to_svg(&mut explicit)).len(),
        2,
        "an explicit newline is a break regardless of `line_wrap`"
    );

    // A value taller than the field is clipped rather than drawn outside it: the row count is
    // bounded by the interior's height (30 px less the padding, over an 8 px line).
    let mut overflowing = TextEdit::new(Rect::new(0, 0, 40, 30));
    overflowing.set_text("word ".repeat(40));
    let rows = text_ink_boxes(&render_to_svg(&mut overflowing)).len();
    assert!(rows <= 3, "a small field draws only the rows it has room for, not {rows} rows");
}

/// The ink follows the scroll offset, on the same row grid as the caret.
///
/// # The defect this pins
///
/// `draw_text_layout` always walked the document from the interior's top edge, while
/// `first_visible_row` moved the caret, the selection and the hit test. Once the caret's row
/// scrolled past the interior, the painter still drew the document's **first** rows — and the
/// caret, drawn from the same offset, fell outside the box and was skipped. The user then typed
/// into a row that was not on screen, over text that claimed to be the document's start.
///
/// The assertion is on the order the painter consumes: the first row it paints is
/// `rows[first_visible_row]`, and the caret's own row is among the rows it reaches.
#[test]
fn the_painted_rows_follow_the_scroll_offset() {
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 60));
    te.set_text("alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\neta\ntheta".to_string());
    assert_eq!(te.first_visible_row(), 0, "a freshly installed document opens at the top");

    // An edit at the end is a movement the user made, so the view follows the caret.
    te.insert_str("!");
    let scroll = te.first_visible_row();
    assert!(scroll > 0, "the caret's row left the field, so the view scrolled");

    let text = te.text().to_string();
    let rows = te.layout_rows();
    let painted: Vec<RowSpan> = te.rows_from_viewport(&rows).map(|(_, span)| *span).collect();
    assert_eq!(
        painted.first().copied(),
        rows.get(scroll).copied(),
        "the painter starts at the scroll offset, not at the document's start"
    );
    assert!(
        !painted.iter().any(|span| text[span.start..span.end].starts_with("alpha")),
        "the row above the viewport is not painted: {painted:?}"
    );
    assert!(
        painted.iter().any(|span| span.start <= te.cursor() && te.cursor() <= span.end),
        "the caret's own row is among the painted rows"
    );
}

// ─── The caret (the capability this control did not have) ───

/// The framework's Shift bit on an event mask.
const SHIFT_BIT: u32 = 0b0001;
/// The framework's primary-accelerator bit on an event mask.
const PRIMARY_BIT: u32 = 0b1000;

fn editor(text: &str) -> TextEdit {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text(text.to_string());
    // Focused, because the keyboard is gated on it — a user reaches an editor by clicking it, and
    // a fixture that skipped that would be testing a state no user can be in. The gate itself is
    // covered by `an_unfocused_editor_ignores_the_keyboard`.
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    // `set_text` leaves the caret at the end, which is the position this control used to have
    // implicitly; every test below that cares about the caret places it explicitly.
    te
}

// ─── D09-INPUT-01: platform-committed text reaches the value ───

/// A `TextInput` from the platform must enter the editor's value.
///
/// # The defect this pins (D09-INPUT-01)
///
/// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
/// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
/// test feeds `TextInput` to prove that path itself is wired.
#[test]
fn text_input_enters_the_value() {
    let mut te = editor("");
    te.handle_event(&Event::TextInput { text: "héllo".to_string() });
    assert_eq!(te.text(), "héllo");
}

/// An IME commit enters the value the same way.
#[test]
fn ime_commit_enters_the_value() {
    let mut te = editor("");
    te.handle_event(&Event::ime_commit("你好"));
    assert_eq!(te.text(), "你好");
}

/// A `TextInput` replaces a selection rather than appending to it.
#[test]
fn text_input_replaces_the_selection() {
    let mut te = editor("abcdef");
    te.select_all();
    te.handle_event(&Event::TextInput { text: "Z".to_string() });
    assert_eq!(te.text(), "Z", "the committed text replaced the selected range");
}

/// Committed text respects `max_length`.
#[test]
fn text_input_respects_the_max_length() {
    let mut te = editor("");
    te.set_max_length(Some(3));
    te.handle_event(&Event::TextInput { text: "abcdef".to_string() });
    assert!(te.text().len() <= 3, "the limit applies: got {:?}", te.text());
}

/// A read-only editor ignores committed text, just as it ignores a printable key.
#[test]
fn read_only_ignores_text_input() {
    let mut te = editor("locked");
    te.set_read_only(true);
    te.handle_event(&Event::TextInput { text: "X".to_string() });
    assert_eq!(te.text(), "locked");
}

/// An unfocused editor ignores committed text, matching the key gate.
#[test]
fn unfocused_ignores_text_input() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::TextInput { text: "X".to_string() });
    assert_eq!(te.text(), "");
}

/// Typing mid-document inserts **at the caret** instead of at the end.
///
/// # The defect this pins
///
/// The value was append-only: every printable key ran `next.push(ch)` and then `set_text`, so
/// there was no insertion point at all. Correcting one character in the middle of a document was
/// impossible — the only edit available was to delete everything after it and retype. This is the
/// difference between a text editor and a text sink, and the control's own name promises the
/// former.
#[test]
fn typing_inserts_at_the_caret_not_at_the_end() {
    let mut te = editor("ac");
    te.set_cursor(1);
    te.handle_event(&Event::key_press(98, 0)); // 'b'
    assert_eq!(te.text(), "abc", "the character went between `a` and `c`");
    assert_eq!(te.cursor(), 2, "and the caret advanced past it");
}

/// The caret defaults to the **end**, so a caller that only appended sees no change.
///
/// This is the compatibility half of adding a caret: the control's previous contract was
/// "typing appends", and it still is whenever the caret has not been moved.
#[test]
fn typing_with_an_untouched_caret_still_appends() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    assert_eq!(te.cursor(), 0);
    te.handle_event(&Event::key_press(97, 0)); // 'a'
    te.handle_event(&Event::key_press(98, 0)); // 'b'
    assert_eq!(te.text(), "ab", "an empty field's typing lands where it always did");
    assert_eq!(te.cursor(), 2);
}

/// Backspace deletes **behind the caret**, not the last character of the document.
///
/// # The defect this pins
///
/// `Backspace` was `text.pop()`, which removes the final character wherever the user was looking.
/// With the caret in the middle it deleted the wrong character entirely — the end of the document
/// rather than the one behind the cursor.
#[test]
fn backspace_deletes_behind_the_caret() {
    let mut te = editor("abcd");
    te.set_cursor(2);
    te.handle_event(&Event::key_press(8, 0));
    assert_eq!(te.text(), "acd", "the `b` behind the caret went, not the `d`");
    assert_eq!(te.cursor(), 1);
}

/// Delete removes the character **after** the caret.
///
/// # The defect this pins
///
/// `Delete` was not handled, so key 46 fell through to the printable arm where 46 is `.` —
/// pressing Delete typed a full stop. The one key whose entire job is forward deletion was the one
/// key that inserted text (the same defect `textarea` had).
#[test]
fn delete_removes_the_character_after_the_caret() {
    let mut te = editor("abc");
    te.set_cursor(1);
    te.handle_event(&Event::key_press(46, 0));
    assert_eq!(te.text(), "ac", "the `b` ahead of the caret went");
    assert_eq!(te.cursor(), 1, "and the caret stayed put");
}

// ─── Selection ───

/// A Shift-arrow extends; a plain arrow replaces.
#[test]
fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
    let mut te = editor("abcdef");
    te.set_cursor(4);

    te.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(te.selected_text(), "d", "Shift+Left selects the character behind");
    te.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(te.selected_text(), "cd", "the second extends the same range");

    te.handle_event(&Event::key_press(37, 0));
    assert_eq!(te.selection_anchor(), None, "a plain arrow is not an extend");
    assert_eq!(te.selected_text(), "");
}

/// Typing over a selection replaces it, and the whole thing is **one** undo step.
///
/// # Why the undo count is asserted
///
/// The obvious implementation routes each keystroke through its own snapshot, so replacing a
/// selection costs two presses to undo (the insert, then the deletion). Recording one step per
/// user action is what makes `Primary+Z` mean "take back what I just did".
#[test]
fn typing_over_a_selection_replaces_it_in_one_undo_step() {
    let mut te = editor("hello world");
    te.set_cursor(5);
    for _ in 0..5 {
        te.handle_event(&Event::key_press(37, SHIFT_BIT));
    }
    assert_eq!(te.selected_text(), "hello");

    te.handle_event(&Event::key_press(88, 0)); // 'X'
    assert_eq!(te.text(), "X world", "the selection was consumed");
    assert!(te.undo(), "one press");
    assert_eq!(te.text(), "hello world", "one undo restores the whole operation");
}

/// Home and End are row bounds, and Primary reaches the document.
#[test]
fn home_and_end_are_row_bounds_and_primary_reaches_the_document() {
    let mut te = editor("one\ntwo\nthree");
    te.set_cursor(6); // inside the second line
    te.handle_event(&Event::key_press(36, 0));
    assert_eq!(te.cursor(), 4, "plain Home is the start of this row");
    te.handle_event(&Event::key_press(35, 0));
    assert_eq!(te.cursor(), 7, "plain End is the end of this row, before its newline");
    te.handle_event(&Event::key_press(36, PRIMARY_BIT));
    assert_eq!(te.cursor(), 0, "Primary+Home is the start of the document");
    te.handle_event(&Event::key_press(35, PRIMARY_BIT));
    assert_eq!(te.cursor(), 13, "Primary+End is the end of the document");
}

/// Primary+A selects the whole document.
#[test]
fn primary_a_selects_the_whole_document() {
    let mut te = editor("one\ntwo");
    te.set_cursor(0);
    te.handle_event(&Event::key_press(65, PRIMARY_BIT));
    assert_eq!(te.selected_text(), "one\ntwo");
}

/// A control chord is not text.
#[test]
fn a_control_chord_is_not_typed_into_the_editor() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", PRIMARY_BIT)] {
        te.handle_event(&Event::key_press(98, bits));
        assert!(te.text().is_empty(), "{name}+b must not be typed");
    }
    te.handle_event(&Event::key_press(98, 0));
    assert_eq!(te.text(), "b", "a plain `b` is content");
}

// ─── The wrap table ───

/// A newline starts a new row, and the row includes its own terminator.
#[test]
fn rows_follow_the_newlines() {
    let te = editor("ab\ncd");
    let rows = te.layout_rows();
    assert_eq!(rows.len(), 2, "two logical lines, two rows when wrapping is off");
    assert_eq!((rows[0].start, rows[0].end), (0, 3), "the first row owns its newline");
    assert_eq!((rows[1].start, rows[1].end), (3, 5));
}

/// A value ending in a newline still has a final row, so the caret can sit on the empty line.
#[test]
fn a_trailing_newline_leaves_a_final_row() {
    let te = editor("ab\n");
    let rows = te.layout_rows();
    assert_eq!(rows.len(), 2, "the empty line after the newline is a row");
    assert_eq!(rows[1].start, 3);
    assert_eq!(rows[1].end, 3, "and it is empty");
}

/// Wrapping splits a long line into rows that **together cover the whole value**.
///
/// This is the invariant that makes the table safe to use for hit testing: no byte may fall
/// outside every row, or a click past a break would resolve to the wrong offset.
#[test]
fn wrapped_rows_tile_the_whole_value() {
    let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
    te.set_text("aaaa bbbb cccc dddd eeee ffff".to_string());
    assert!(te.line_wrap(), "the fixture must be in wrapping mode");
    let rows = te.layout_rows();
    assert!(rows.len() > 1, "a 60 px field must wrap this value into several rows");
    assert_eq!(rows[0].start, 0, "the first row starts at the beginning");
    assert_eq!(rows[rows.len() - 1].end, te.text().len(), "the last row reaches the end");
    for pair in rows.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "rows are contiguous, with no gap or overlap");
    }
}

/// With wrapping off, one row per logical line, however wide.
#[test]
fn line_wrap_off_gives_one_row_per_line() {
    let mut te = TextEdit::new(Rect::new(0, 0, 60, 200));
    te.set_text("aaaa bbbb cccc dddd\neeee ffff".to_string());
    te.set_line_wrap(false);
    let rows = te.layout_rows();
    assert_eq!(rows.len(), 2, "no wrapping, so the two lines are the two rows");
}

/// The row for an offset is the row that contains it.
#[test]
fn an_offset_maps_to_the_row_containing_it() {
    let te = editor("ab\ncd");
    assert_eq!(te.row_of_offset(0), 0, "`a` is on the first row");
    assert_eq!(te.row_of_offset(2), 0, "the newline belongs to the first row");
    assert_eq!(te.row_of_offset(3), 1, "`c` is on the second row");
    assert_eq!(te.row_of_offset(5), 1, "and so is the end of the value");
}

// ─── The pointer ───

/// A press places the caret, and the map is total on multi-byte wrapped text.
#[test]
fn a_press_places_the_caret_and_never_splits_a_character() {
    let mut te = TextEdit::new(Rect::new(0, 0, 120, 200));
    te.set_text("héllo 中文 wörld".to_string());
    let rect = te.geometry();

    for dy in (0..rect.height as i32).step_by(11) {
        for dx in (0..rect.width as i32).step_by(7) {
            te.handle_event(&Event::mouse_press(rect.x + dx, rect.y + dy, 1));
            let caret = te.cursor();
            assert!(caret <= te.text().len(), "the caret is inside the value at ({dx},{dy})");
            assert!(
                te.text().is_char_boundary(caret),
                "the caret at ({dx},{dy}) split a character: {caret} in {:?}",
                te.text()
            );
        }
    }
}

/// A drag extends the selection; a hover that was never pressed does not move the caret.
#[test]
fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
    let mut te = editor("abcdef");
    let rect = te.geometry();

    te.handle_event(&Event::mouse_move(rect.x + 40, rect.y + 10));
    assert_eq!(te.cursor(), 6, "an unpressed hover must not move the caret");
    assert_eq!(te.selection_anchor(), None);

    te.handle_event(&Event::mouse_press(rect.x + 10, rect.y + 10, 1));
    let (start, _) = te.selection_range().unwrap_or((te.cursor(), 0));
    te.handle_event(&Event::mouse_move(rect.x + 44, rect.y + 10));
    let (_, end) = te.selection_range().unwrap_or((0, te.cursor()));
    assert!(end > start, "the drag grew the range: {start} -> {end}");

    te.handle_event(&Event::mouse_release(rect.x + 44, rect.y + 10, 1));
    let kept = te.selected_text();
    te.handle_event(&Event::mouse_move(rect.x + 4, rect.y + 10));
    assert_eq!(te.selected_text(), kept, "a hover after the release does not extend");
}

/// A click on the first row and a click on the second resolve to different rows.
#[test]
fn a_press_on_a_later_row_resolves_to_that_row() {
    let te = editor("ab\ncd");
    let rect = te.geometry();
    let line_height = Font::default().effective_line_height().max(1.0) as i32;

    let first = te.offset_at_point(crate::core::Point::new(rect.x + 6, rect.y + 6));
    let second = te.offset_at_point(crate::core::Point::new(rect.x + 6, rect.y + 6 + line_height));
    assert!(first < 3, "the top row is the first line, got {first}");
    assert!(second >= 3, "the row below is the second line, got {second}");
}

// ─── Scrolling ───

/// Typing past the last visible row scrolls the viewport instead of writing out of sight.
#[test]
fn the_viewport_follows_the_caret_down() {
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 60));
    te.set_text("one\ntwo\nthree\nfour\nfive\nsix".to_string());
    // `set_text` installs a whole document, so the view opens at its **top** even though the
    // caret goes to the end (see that method's note): opening a file at its last page would be
    // the wrong reading position for a replacement the user did not make.
    assert_eq!(te.first_visible_row(), 0, "a freshly installed document opens at the top");

    // An *edit* at the end, which is a movement the user made, does scroll the view.
    te.insert_str("!");
    assert!(
        te.first_visible_row() > 0,
        "editing at the last row scrolled the viewport, got {}",
        te.first_visible_row()
    );

    // And back to the start: the viewport returns, minimally.
    te.set_cursor(0);
    assert_eq!(te.first_visible_row(), 0, "returning to the first row shows the first row");
}

/// A value that fits never scrolls.
#[test]
fn a_value_that_fits_never_scrolls() {
    let mut te = editor("one\ntwo");
    te.select_all();
    assert_eq!(te.first_visible_row(), 0, "nothing to scroll to");
}

// ─── Undo ───

/// A deletion through the caret is undoable, and restores the caret with the text.
#[test]
fn a_caret_deletion_is_undoable() {
    let mut te = editor("abcd");
    te.set_cursor(2);
    te.handle_event(&Event::key_press(8, 0));
    assert_eq!(te.text(), "acd");
    assert!(te.undo());
    assert_eq!(te.text(), "abcd", "the deleted character came back");
}

/// `max_length` bounds a mid-document insert too, and refuses it whole.
#[test]
fn the_limit_bounds_an_insert_at_the_caret() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.set_max_length(Some(3));
    te.set_text("abc".to_string());
    te.set_cursor(1);
    te.insert_str("z");
    assert_eq!(te.text(), "abc", "the insert was refused, not truncated");
}

/// An unfocused editor ignores the keyboard, and a focused one accepts it.
///
/// # The defect this pins
///
/// The `KeyPress` arm ran for **every** event, so an editor nobody had clicked into still consumed
/// the keyboard. With two editors on a page one keystroke edited both, and a control the user had
/// never reached swallowed accelerators the host meant for something else. The framework does
/// deliver `FocusGained` / `FocusLost`, so the fact was available — the control simply never
/// checked it.
#[test]
fn an_unfocused_editor_ignores_the_keyboard() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.handle_event(&Event::key_press(97, 0)); // 'a'
    assert!(te.text().is_empty(), "an unfocused editor must not take the keystroke");

    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.handle_event(&Event::key_press(97, 0));
    assert_eq!(te.text(), "a", "once focused it does");

    te.handle_event(&Event::FocusLost);
    te.handle_event(&Event::key_press(98, 0)); // 'b'
    assert_eq!(te.text(), "a", "and losing focus takes the keyboard away again");
}

/// A press focuses the editor, which is how a user reaches it with the pointer.
#[test]
fn a_press_focuses_the_editor() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    let rect = te.geometry();
    te.handle_event(&Event::mouse_press(rect.x + 6, rect.y + 6, 1));
    te.handle_event(&Event::key_press(97, 0)); // 'a'
    assert_eq!(te.text(), "a", "the press made the editor own the keyboard");
}

/// The wheel scrolls the wrapped row window and clamps at both ends.
#[test]
fn textedit_wheel_scrolls_and_clamps() {
    let text: Vec<String> = (0..20).map(|n| format!("line{n}")).collect();
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 80));
    te.set_text(text.join("\n"));
    assert_eq!(te.first_visible_row(), 0);

    // One notch toward the end of the value reveals three rows.
    te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
    assert_eq!(te.first_visible_row(), 3, "a downward notch moves the viewport down");

    // The opposite notch returns toward the top and clamps there.
    te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
    assert_eq!(te.first_visible_row(), 0);
    te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, 1), modifiers: 0 });
    assert_eq!(te.first_visible_row(), 0, "the viewport clamps at the first row");

    // Scrolling on stops at the last full window rather than past the document.
    let max_first = te.layout_rows().len() - te.visible_row_count();
    for _ in 0..20 {
        te.handle_event(&Event::Wheel { delta: crate::core::Point::new(0, -1), modifiers: 0 });
    }
    assert_eq!(te.first_visible_row(), max_first, "the viewport clamps at the last window");
}

/// Soft wrapping breaks at word boundaries instead of splitting a word.
#[test]
fn soft_wrap_breaks_at_word_boundaries() {
    let mut te = TextEdit::new(Rect::new(0, 0, 70, 400));
    te.set_text("alpha beta gamma delta".to_string());
    let text = te.text().to_string();
    let rows = te.layout_rows();
    assert!(rows.len() > 1, "the value has to wrap: {rows:?}");
    for pair in rows.windows(2) {
        let boundary = pair[0].end;
        assert!(
            text[..boundary].ends_with(' ') || boundary == text.len(),
            "a word must not be split at {boundary}: {rows:?}"
        );
    }
}

/// Styled spans are addressable and change the painted document.
#[test]
fn spans_format_the_named_run() {
    use crate::widget::svg::render_to_svg;
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
    te.set_text("bold and plain".to_string());
    let plain = render_to_svg(&mut te);
    te.set_spans(std::vec![TextSpan {
        range: 0..4,
        format: CharFormat { bold: true, ..CharFormat::default() },
    }]);
    assert!(te.format_at(0).bold);
    assert!(!te.format_at(6).bold);
    assert_ne!(plain, render_to_svg(&mut te), "a bold run must change the ink");
}

/// Primary+V pastes the clipboard at the caret.
#[test]
fn primary_v_pastes_at_the_caret() {
    let _clipboard = crate::clipboard::clipboard_test_guard();
    crate::set_clipboard_text("pasted");
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 200));
    te.set_text("ab".to_string());
    te.set_cursor(1);
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.handle_event(&Event::KeyPress { key: 86, modifiers: 0b1000 });
    assert_eq!(te.text(), "apastedb");
}

/// A double click selects the word the pointer landed on.
#[test]
fn a_double_click_selects_a_word() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 100));
    te.set_text("alpha beta".to_string());
    te.handle_event(&Event::MouseDoubleClick { pos: crate::core::Point::new(6, 10), button: 1 });
    assert_eq!(te.selected_text(), "alpha");
}

/// Page Down and Page Up move the caret by a window of rows.
#[test]
fn page_keys_move_by_a_window() {
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 80));
    te.set_text((0..30).map(|n| format!("line {n}\n")).collect::<String>());
    te.set_cursor(0);
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });
    te.handle_event(&Event::KeyPress { key: 34, modifiers: 0 });
    let after_down = te.cursor();
    assert!(after_down > 0, "Page Down must move the caret");
    te.handle_event(&Event::KeyPress { key: 33, modifiers: 0 });
    assert!(te.cursor() < after_down, "Page Up must move it back");
}

/// The wrap cache is keyed by the mutation counter, not by the document text.
#[test]
fn the_wrap_cache_uses_the_revision_counter() {
    let mut te = TextEdit::new(Rect::new(0, 0, 200, 100));
    te.set_text("one".to_string());
    let first = te.layout_rows();
    let first_revision = te.revision;
    assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").revision, first_revision);
    assert_eq!(te.layout_rows(), first);
    te.set_text("one two".to_string());
    assert!(te.revision > first_revision, "a text change bumps the revision");
    let _ = te.layout_rows();
    assert_eq!(te.row_cache.borrow().as_ref().expect("cache filled").revision, te.revision);
    assert_ne!(te.layout_rows(), first);
}

/// I-P51: a keyboard edit must bump the revision the wrap table is keyed by, or the next
/// frame reads the pre-edit breaks.
#[test]
fn keyboard_edits_refresh_the_wrap_table() {
    let mut te = TextEdit::new(Rect::new(0, 0, 80, 200));
    te.set_text("aaaa aaaa aaaa".to_string());
    te.set_cursor(0);
    let before = te.layout_rows().len();
    let before_revision = te.revision;

    te.insert_str("\n");
    assert!(te.revision > before_revision, "an insertion bumps the revision");
    assert_eq!(
        te.row_cache.borrow().as_ref().expect("cache filled").revision,
        te.revision,
        "the wrap cache records the post-edit revision"
    );
    let after_insert = te.layout_rows().len();
    assert!(after_insert > before, "the newline adds a row: {before} -> {after_insert}");

    assert!(te.backspace());
    assert_eq!(te.layout_rows().len(), before, "deleting the newline removes the row again");
}

/// I-P53: formatting after the edit point follows the characters it was applied to.
#[test]
fn existing_spans_move_with_an_insertion_and_deletion() {
    let mut te = TextEdit::new(Rect::new(0, 0, 300, 40));
    te.set_text("hello world".to_string());
    let bold = CharFormat { bold: true, ..CharFormat::default() };
    te.set_spans(vec![TextSpan { range: 6..11, format: bold }]);
    te.set_cursor(0);

    te.insert_str("X");
    assert_eq!(te.spans()[0].range, 7..12, "the span moves past the insertion");
    assert_eq!(te.format_at(7), bold, "the styled run still starts at 'world'");

    assert!(te.backspace(), "the inserted X is removed");
    assert_eq!(te.spans()[0].range, 6..11, "the span returns with the deletion");
    assert_eq!(te.format_at(6), bold);
}

/// I-121: undo/redo must not mutate a read-only editor, while select-all and copy still run.
#[test]
fn read_only_blocks_undo_but_not_copy() {
    let mut te = editor("abc");
    te.set_cursor(3);
    te.insert_str("d");
    te.set_read_only(true);
    te.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Programmatic });

    te.handle_event(&Event::KeyPress { key: 90, modifiers: PRIMARY_BIT });
    assert_eq!(te.text(), "abcd", "a read-only editor refuses undo");
    te.handle_event(&Event::KeyPress { key: 65, modifiers: PRIMARY_BIT });
    assert_eq!(te.selection_range(), Some((0, 4)), "select-all still works");
}

/// I-120: the IME query reports the caret's row and measured column.
#[test]
fn input_method_query_reports_the_caret_rectangle() {
    let mut te = TextEdit::new(Rect::new(10, 20, 200, 80));
    te.set_text("abc\ndef".to_string());
    te.set_cursor(5);
    let query = te.input_method_query().expect("an editor has a caret");
    assert_eq!(query.caret_byte, Some(5));
    assert!(query.caret_rect.y >= te.geometry().y, "the query stays inside the field");
    assert!(query.caret_rect.x >= te.geometry().x);
}

/// A document taller than the viewport publishes and paints a scrollbar (I-015).
#[test]
fn a_long_document_has_a_scrollbar() {
    let mut te = TextEdit::new(Rect::new(0, 0, 240, 60));
    let many = (0..40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    te.set_text(many);
    let track = te.scrollbar_track().expect("a 40-line document does not fit 60 px");
    assert_eq!(track.1, te.layout_rows().len());
    assert!(track.2 >= 1, "at least one row is visible");
    // Dragging to the bottom of the track reaches the last viewport.
    let bottom = track.0.y + track.0.height as i32;
    te.scrollbar_scroll_to(bottom);
    assert!(te.first_visible_row() > 0, "the drag scrolled the viewport");
}

// ─── COMP-A8: FindReplaceBar wiring ───

/// COMP-A8: attaching reports the current query's matches and queues nothing.
#[test]
#[cfg(full_widgets)]
fn the_find_bar_counts_matches_when_attached() {
    let mut te = editor("one two one two one");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");

    te.attach_find_replace_bar(&bar);
    assert!(te.is_find_replace_bar_attached());
    assert_eq!(bar.match_count(), 3, "attach reports the current query's matches");
    assert!(!te.poll_find_replace_bar(), "attaching queues no intent");
}

/// COMP-A8: a queued navigation intent moves the editor's selection on the next poll.
#[test]
#[cfg(full_widgets)]
fn a_find_intent_moves_the_selection_on_poll() {
    let mut te = editor("one two one two one");
    te.set_cursor(0);
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");
    te.attach_find_replace_bar(&bar);

    bar.find_next.emit();
    assert!(te.poll_find_replace_bar(), "the queued intent is applied");
    assert_eq!(te.selection_range(), Some((0, 3)));
    assert_eq!((bar.match_count(), bar.current_match()), (3, 0));

    bar.find_next.emit();
    assert!(te.poll_find_replace_bar());
    assert_eq!(te.selection_range(), Some((8, 11)));
    assert_eq!(bar.current_match(), 1);

    bar.find_previous.emit();
    assert!(te.poll_find_replace_bar());
    assert_eq!(te.selection_range(), Some((0, 3)), "previous wraps to the first match");
    assert!(!te.poll_find_replace_bar(), "the queue is drained");
}

/// COMP-A8: replace and replace-all go through the editor's own edit path.
#[test]
#[cfg(full_widgets)]
fn replace_intents_go_through_the_editors_edit_path() {
    let mut te = editor("one two one");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("one");
    bar.set_replacement("1");
    te.attach_find_replace_bar(&bar);

    bar.replace_one.emit();
    assert!(te.poll_find_replace_bar());
    assert_eq!(te.text(), "1 two one", "the first match is replaced");
    assert_eq!(bar.match_count(), 1, "the counter follows the new document");

    bar.replace_all.emit();
    assert!(te.poll_find_replace_bar());
    assert_eq!(te.text(), "1 two 1");
    assert_eq!(bar.match_count(), 0, "nothing matches after the replacement");

    bar.replace_all.emit();
    assert!(!te.poll_find_replace_bar(), "nothing left to replace");
}

/// COMP-A8: detaching stops the link and keeps the last reported state.
#[test]
#[cfg(full_widgets)]
fn a_detached_find_bar_is_not_polled() {
    let mut te = editor("aaa");
    let mut bar = FindReplaceBar::new(Rect::new(0, 0, 400, 32));
    bar.set_query("a");
    te.attach_find_replace_bar(&bar);
    assert_eq!(bar.match_count(), 3);

    te.detach_find_replace_bar();
    assert!(!te.is_find_replace_bar_attached());
    bar.find_next.emit();
    assert!(!te.poll_find_replace_bar(), "a detached bar reports nothing");
    assert_eq!(bar.match_count(), 3, "detach leaves the last count");
    assert_eq!(te.selection_range(), None, "and the selection did not move");
}
