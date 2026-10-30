// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Unit tests for [`LineEdit`](super::LineEdit), kept out of the production files.

use super::model::*;
use crate::compat::{Box, MiniToString, String, Vec};
// Used only by the `#[cfg(not(alloc_frugal))]` measurement helpers below.
use crate::core::{Point, Rect};
use crate::event::{Event, EventHandler, FocusReason};
use crate::platform::EchoMode;
#[cfg(not(alloc_frugal))]
use crate::render::RenderContext;
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::input_validator::{InputValidator, ValidationState, Validator};
use crate::widget::input_widgets::validation_bubble::ValidationSeverity;
use crate::widget::metrics::dimensions;
use crate::widget::{Widget, WidgetKind};

/// A context-menu request is emitted with the point it asked about, and the field
/// reports that it handled it. The host supplies the menu; the field supplies the
/// request.
#[test]
fn a_context_menu_request_emits_the_requested_point() {
    let mut field = LineEdit::new(Rect::new(0, 0, 160, 32));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
    let sink = std::sync::Arc::clone(&seen);
    field.context_menu_requested.connect_ref(move |pos: &Point| {
        *sink.lock().unwrap() = Some(*pos);
    });

    assert!(field.context_menu_event(Point::new(18, 6)));
    assert_eq!(*seen.lock().unwrap(), Some(Point::new(18, 6)));
}

/// The bitmap face fills its glyph box, so its ink left *is* the box left. An outline face draws
/// a real glyph whose ink is inset, so the ink's left edge sits one or two pixels right of the
/// box edge at these sizes. Pinning the two equal encoded a property of the bitmap face, not of
/// the layout.
const INK_INSET_TOLERANCE: i32 = 3;

#[test]
fn lineedit_creation_defaults() {
    let le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert!(le.text().is_empty());
    assert!(le.placeholder_text().is_empty());
    assert_eq!(le.max_length(), None);
    assert_eq!(le.echo_mode(), EchoMode::Normal);
    assert_eq!(le.cursor_position(), 0);
    assert!(le.selection_start().is_none());
    assert!(!le.is_focused(), "a fresh field is not focused");
}

/// The caret blinks only while the field holds focus, and it stops asking for frames when it
/// does not.
///
/// The caret was a solid line for the whole life of the field, so `tick` did not exist and a
/// host had no signal about whether more frames were owed. This pins the contract in both
/// directions: a blurred field is inert, and a focused one keeps reporting work until it
/// loses focus again.
#[test]
fn the_caret_blinks_while_focused_and_is_inert_while_blurred() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

    // Blurred: no frames owed, whatever the delta.
    assert!(!le.tick(10_000), "a blurred field owes no frame");

    // Focused: it reports work, and keeps doing so because a blink never settles.
    le.set_focused(true);
    assert!(le.tick(0), "a focused field is blinking");
    for _ in 0..10 {
        assert!(le.tick(500), "a blink is periodic, so it never reports settled");
    }

    // A read-only field shows no caret, so it must not animate one either.
    le.set_focused(false);
    assert!(!le.tick(500));
}

/// The field must learn about focus from the events the runtime sends, since
/// that is what gates the caret. Before this the field had no focus state, so
/// `FocusGained` was ignored and the caret was never drawn.
#[test]
fn lineedit_tracks_focus_from_events() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

    le.handle_event(&Event::FocusGained { reason: FocusReason::Programmatic });
    assert!(le.is_focused(), "FocusGained must mark the field focused");

    le.handle_event(&Event::FocusLost);
    assert!(!le.is_focused(), "FocusLost must clear it");
}

/// Setting focus directly must be idempotent in effect: the flag is a boolean,
/// so a repeated set cannot leave it in a contradictory state.
#[test]
fn lineedit_set_focused_round_trips() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_focused(true);
    assert!(le.is_focused());
    le.set_focused(true);
    assert!(le.is_focused());
    le.set_focused(false);
    assert!(!le.is_focused());
}

/// A focused field must actually paint a caret. The previous code drew nothing,
/// and the comment said why — there was no focus state to read.
#[test]
fn focused_lineedit_draws_a_caret() {
    let rect = Rect::new(0, 0, 200, 24);
    let mut focused = LineEdit::new(rect);
    focused.set_text("ab");
    focused.set_focused(true);

    let mut unfocused = LineEdit::new(rect);
    unfocused.set_text("ab");

    let focused_frame = render(&mut focused, rect);
    let unfocused_frame = render(&mut unfocused, rect);
    assert_ne!(
        focused_frame, unfocused_frame,
        "focusing must change what is painted, because it draws the caret"
    );
}

/// Renders a widget into an RGBA frame for comparison.
fn render(widget: &mut LineEdit, rect: Rect) -> Vec<u8> {
    use crate::core::{Color, Size};
    use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
    use crate::widget::Draw;

    let mut surface = SoftwarePaintBackend::new(Size::new(rect.width, rect.height), 1.0);
    surface.begin_frame(Color::WHITE);
    {
        let mut context = RenderContext::new(&mut surface);
        widget.draw(&mut context);
    }
    surface.end_frame();
    surface.frame_rgba().to_vec()
}

#[test]
fn lineedit_set_text() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    assert_eq!(le.text(), "Hello");
    assert_eq!(le.cursor_position(), 5);
}

#[test]
fn lineedit_set_text_empty() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.set_text(String::new());
    assert!(le.text().is_empty());
    assert_eq!(le.cursor_position(), 0);
}

#[test]
fn lineedit_undo_redo_restores_text() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    // A programmatic `set_text` is the saved state and records no history, so the two values
    // are produced through the user edit path (see I-04).
    le.insert_str("one");
    le.select_all();
    le.insert_str("two");
    assert!(le.can_undo());
    assert!(le.undo());
    assert_eq!(le.text(), "one");
    assert!(le.can_redo());
    assert!(le.redo());
    assert_eq!(le.text(), "two");
}

#[test]
fn lineedit_control_z_and_control_y_drive_history() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    // Programmatic writes are pristine (I-04), so the history is built by real edits.
    le.insert_str("before");
    le.select_all();
    le.insert_str("after");
    // Bit 3 is the event's Meta/Command bit, i.e. the portable "primary accelerator"
    // (`Modifiers::from_event_bits`). The physical-Control bit is bit 1 and means Control.
    le.handle_event(&Event::key_press(90, 0b1000));
    assert_eq!(le.text(), "before");
    le.handle_event(&Event::key_press(89, 0b1000));
    assert_eq!(le.text(), "after");
}

#[test]
fn primary_shift_z_is_the_other_redo_chord() {
    // `Ctrl+Shift+Z` / `Cmd+Shift+Z` used to fall into the undo arm and undo the edit the
    // user meant to redo (I-01).
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.insert_str("before");
    le.select_all();
    le.insert_str("after");
    le.handle_event(&Event::key_press(90, 0b1000));
    assert_eq!(le.text(), "before");
    // Bit 0 is the Shift bit beside the primary accelerator.
    le.handle_event(&Event::key_press(90, 0b1001));
    assert_eq!(le.text(), "after");
}

#[test]
fn a_multi_byte_password_produces_one_star_per_character() {
    // `mask_text` repeated by byte length, so `héllo` (5 characters, 6 bytes) painted six
    // stars and the mask disagreed with the value it hid (I-02).
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_echo_mode(EchoMode::Password);
    le.set_text("héllo");
    assert_eq!(LineEdit::mask_text(le.text()), "*****");
}

#[test]
fn lineedit_placeholder() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert!(le.placeholder_text().is_empty());
    le.set_placeholder_text("Enter name".to_string());
    assert_eq!(le.placeholder_text(), "Enter name");
    le.set_placeholder_text(String::new());
    assert!(le.placeholder_text().is_empty());
}

#[test]
fn lineedit_max_length() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert_eq!(le.max_length(), None);
    le.set_max_length(Some(5));
    assert_eq!(le.max_length(), Some(5));
    le.set_max_length(None);
    assert_eq!(le.max_length(), None);
}

#[test]
fn lineedit_max_length_truncates() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello World".to_string());
    le.set_max_length(Some(5));
    assert_eq!(le.text(), "Hello");
}

/// Reducing the limit truncates the value *and* invalidates the history that
/// could restore the over-limit string.
#[test]
fn max_length_change_keeps_undo_within_the_limit() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdef");
    le.set_max_length(Some(3));
    assert_eq!(le.text(), "abc", "the value is truncated to the limit");

    assert!(!le.can_undo(), "an over-limit value must not be undoable");
    assert!(!le.undo());
    assert_eq!(le.text(), "abc");
    assert!(!le.redo());
    assert_eq!(le.text(), "abc");
}

#[test]
fn lineedit_cursor_position() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.set_cursor_position(3);
    assert_eq!(le.cursor_position(), 3);
    le.set_cursor_position(100); // clamps to text len
    assert_eq!(le.cursor_position(), 5);
}

#[test]
fn lineedit_echo_mode() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert_eq!(le.echo_mode(), EchoMode::Normal);
    le.set_echo_mode(EchoMode::Password);
    assert_eq!(le.echo_mode(), EchoMode::Password);
    le.set_echo_mode(EchoMode::NoEcho);
    assert_eq!(le.echo_mode(), EchoMode::NoEcho);
    le.set_echo_mode(EchoMode::Normal);
    assert_eq!(le.echo_mode(), EchoMode::Normal);
}

/// A masked field must not hand its plaintext to the process-global clipboard.
///
/// `selected_text` keeps returning the real value — the host needs it to store a
/// credential — but the clipboard path is a *different* reader, and the clipboard is
/// readable by every other process. In `Password` mode it must therefore receive the
/// same `*` mask the field paints; in `NoEcho` mode, which paints nothing, nothing may
/// be copied at all.
#[cfg(not(alloc_frugal))]
#[test]
fn lineedit_masked_selection_copies_the_mask_not_the_plaintext() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("s3cret".to_string());
    le.select_all();
    // The host's own read still sees the real value.
    assert_eq!(le.selected_text(), "s3cret");

    le.set_echo_mode(EchoMode::Password);
    assert_eq!(le.clipboard_selection().as_deref(), Some("******"));

    le.set_echo_mode(EchoMode::NoEcho);
    assert_eq!(le.clipboard_selection(), None);

    le.set_echo_mode(EchoMode::Normal);
    assert_eq!(le.clipboard_selection().as_deref(), Some("s3cret"));
}

/// An empty selection is not a copy, whatever the echo mode.
#[cfg(not(alloc_frugal))]
#[test]
fn lineedit_no_selection_copies_nothing() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("s3cret".to_string());
    le.clear_selection();
    assert_eq!(le.clipboard_selection(), None);
}

/// The widget layer and the platform layer must name the **same** echo-mode
/// enum, so a mode read back from a native control can be handed straight to
/// this widget. Distinct types would make the assignment below ill-typed.
#[test]
fn lineedit_shares_the_canonical_echo_mode_type() {
    let canonical: crate::platform::EchoMode = crate::platform::EchoMode::Password;
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_echo_mode(canonical);
    assert_eq!(le.echo_mode(), canonical);
}

#[test]
fn lineedit_select_all() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello World".to_string());
    le.select_all();
    assert_eq!(le.selection_start(), Some(0));
    assert_eq!(le.cursor_position(), 11);
    assert_eq!(le.selected_text(), "Hello World");
}

#[test]
fn lineedit_clear_selection() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.select_all();
    le.clear_selection();
    assert!(le.selection_start().is_none());
}

// ─── Shift-arrow selection (BLUE24 §12 U-6) ───

/// A Shift-arrow **extends** the selection; a plain arrow replaces it.
///
/// # The defect this pins
///
/// The anchor rule was written out once per arrow key, six copies of one decision. Beyond the
/// duplication, the copies meant "Shift extends" lived only in the event loop — nothing else
/// could reach it, so the pointer gesture and the entry point a host calls had no way to
/// express a range. The assertions are on the *range*, not on the anchor, because a range is
/// what the user sees selected: an implementation that moved the caret and forgot the anchor
/// would still satisfy `cursor_position` and silently select nothing.
///
/// The modifiers are spelled as the **event bitmask** (see
/// [`crate::shortcut::Modifiers`]), not as `Modifiers` bits: an `Event` carries the framework's
/// wire convention, and the field translates it with `from_event_bits`.
#[test]
fn a_shift_arrow_extends_and_a_plain_arrow_replaces() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdef".to_string()); // caret at the end

    // Shift+Left twice grows a two-character selection, without collapsing to a caret.
    le.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(le.selected_text(), "f", "the first Shift+Left selects the character behind");
    le.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(le.selected_text(), "ef", "the second extends the same range");

    // A plain Left keeps the caret where it is and drops the selection entirely.
    le.handle_event(&Event::key_press(37, 0));
    assert_eq!(le.selection_start(), None, "a plain arrow is not an extend");
    assert_eq!(le.cursor_position(), 3, "and the caret is unmoved by the deselect");
}

/// Shift+Home and Shift+End select from the caret to the ends of the value.
#[test]
fn a_shift_home_and_end_select_to_the_ends_of_the_value() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdef".to_string());
    le.set_cursor_position(2);

    le.handle_event(&Event::key_press(35, SHIFT_BIT));
    assert_eq!(le.selected_text(), "cdef", "Shift+End runs from the anchor to the end");

    // Shift+Home extends from the *same* anchor at 2, back past the caret to the start: the
    // range becomes 0..=2, the other side of the same anchor rather than a rewrite of it.
    le.handle_event(&Event::key_press(36, SHIFT_BIT));
    assert_eq!(le.cursor_position(), 0, "the caret walks to the start");
    assert_eq!(le.selection_start(), Some(2), "the anchor is unchanged by the second key");
    assert_eq!(le.selected_text(), "ab", "so the selection flips to the other side of it");

    // Home with no modifier collapses the range, the same as any plain movement.
    le.handle_event(&Event::key_press(36, 0));
    assert_eq!(le.selection_start(), None);
    assert_eq!(le.cursor_position(), 0);
}

/// Shift+Home on a field that has never been selected anchors at the live caret.
///
/// The anchor is adopted from the caret when there is none, rather than assumed to be 0. A
/// field whose caret sits at the end would otherwise select the whole value for a gesture the
/// user aimed at the start of the line they were on.
#[test]
fn a_shift_move_with_no_anchor_anchors_at_the_caret() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdef".to_string());
    le.set_cursor_position(3);
    assert_eq!(le.selection_start(), None, "a fresh caret is not an anchor");

    le.handle_event(&Event::key_press(36, SHIFT_BIT));
    assert_eq!(le.cursor_position(), 0, "Shift+Home walks the caret to the start");
    assert_eq!(le.selection_start(), Some(3), "adopting the caret as the anchor, not 0");
    assert_eq!(le.selected_text(), "abc", "so the range is the caret's own line prefix");
}

/// A Shift-arrow does not move the anchor once a selection exists, so the range grows from a
/// fixed point instead of sliding along with the caret.
#[test]
fn an_extended_range_grows_from_a_fixed_anchor() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdef".to_string());
    le.set_cursor_position(3);
    // The caret steps back one *character per press*, so the anchor stays at 3 while the
    // caret walks 3 -> 2 -> 1 and the range grows `"c"`, `"bc"`.
    le.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(le.cursor_position(), 2, "one press is one character");
    assert_eq!(le.selection_start(), Some(3), "the anchor is where the gesture began");
    assert_eq!(le.selected_text(), "c", "so the first step selects one character");
    le.handle_event(&Event::key_press(37, SHIFT_BIT));
    assert_eq!(le.selection_start(), Some(3), "and it does not slide with the caret");
    assert_eq!(le.selected_text(), "bc", "so the range is anchor-to-caret");
    assert_eq!(le.cursor_position(), 1);
}

/// An extension is **reversible**: Shift+Home then Shift+End returns the range to the anchor.
///
/// # The defect this pins
///
/// Home and End re-anchored at the caret before moving, so a second extension grew from the
/// wrong end: Shift+Home on a caret at 5 selected 0..=5 and moved the anchor to 5; the following
/// Shift+End then selected 5..=len instead of 0..=len. The anchor belongs to the *gesture*, and
/// only a press that begins one may move it.
#[test]
fn extending_back_and_forth_returns_to_the_anchor() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abcdefgh".to_string());
    le.set_cursor_position(5);

    le.handle_event(&Event::key_press(36, SHIFT_BIT));
    assert_eq!(le.cursor_position(), 0, "Shift+Home walks the caret to the start");
    assert_eq!(le.selected_text(), "abcde", "the range is anchor(5)-to-caret(0)");
    assert_eq!(le.selection_start(), Some(5), "the anchor is still where it began");

    // The gesture is reversible: Shift+End **extends** from the same anchor, so the range
    // flips to the other side of it rather than being replaced with 5..=len. This is the
    // assertion that fails if Home/End copy the old `selection_start = Some(cursor)` rule.
    le.handle_event(&Event::key_press(35, SHIFT_BIT));
    assert_eq!(le.cursor_position(), 8, "Shift+End walks the caret to the end");
    assert_eq!(le.selection_start(), Some(5), "the anchor never moved");
    assert_eq!(le.selected_text(), "fgh", "so the range is anchor(5)-to-caret(8)");
}

/// Movement saturates rather than running off either end of the value.
///
/// # The defect this pins
///
/// The arrows used to be guarded by `if cursor > 0` / `if cursor < len`, so at an end the key was
/// *dropped* — including the deselect a plain press owes the user. A ragged selection then
/// survived an arrow press that plainly meant "just move". Saturation keeps the caret in range
/// and keeps the plain-press contract intact.
#[test]
fn movement_at_an_end_still_drops_the_selection() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("abc".to_string());

    // Caret already at the end: Right cannot move, but it still ends the selection.
    le.select_all();
    assert_eq!(le.selection_start(), Some(0));
    le.handle_event(&Event::key_press(39, 0));
    assert_eq!(le.cursor_position(), 3, "the caret stays inside the value");
    assert_eq!(le.selection_start(), None, "a plain press at the end still deselects");

    // Caret already at the start with a range straddling it: Left cannot move, but the same
    // deselect is owed. The range is built the way a shift-click does — press at the end,
    // then shift-press at the start — so the caret lands on 0 with the anchor at 3.
    le.select_all();
    le.press_at(3, crate::shortcut::Modifiers::NONE);
    le.press_at(0, crate::shortcut::Modifiers::SHIFT);
    assert_eq!(le.cursor_position(), 0);
    assert_eq!(le.selection_start(), Some(3), "a real range straddling the start");
    assert_eq!(le.selected_text(), "abc");

    le.handle_event(&Event::key_press(37, 0));
    assert_eq!(le.cursor_position(), 0, "Left cannot move past the start");
    assert_eq!(le.selection_start(), None, "but it still drops the stale range");
    assert_eq!(le.selected_text(), "", "so nothing is left selected");
}

/// A control chord is not text: `Ctrl+B` must not insert a literal `b`.
///
/// # The defect this pins
///
/// The catch-all arm treated every unmapped key as a character and only excluded the chords it
/// happened to list (copy, paste, cut, undo, redo). So Ctrl+B, Ctrl+Q, Alt+1 and every other
/// shortcut the host may own were typed into the field as their bare key. A field that swallows
/// the application's own accelerators is unusable as a text input in a real form.
///
/// The comment describes Ctrl/Alt/Meta, and the event convention says the Meta bit is the
/// primary accelerator — so those are the bits asserted, not `Modifiers` bits.
#[test]
fn a_control_chord_is_not_typed_into_the_field() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));

    // Bit 1 is physical Control, bit 2 is Alt, bit 3 is the primary accelerator (Command on
    // macOS). `from_event_bits` folds bit 3 into *both* PRIMARY and CTRL, and `contains`
    // compares the mask's value rather than testing a bit, so the guard must ask about the
    // three modifiers separately — a combined `contains(CTRL|ALT|META)` would only fire when
    // all three were held, letting a plain Ctrl chord through.
    for (name, bits) in [("Ctrl", 0b0010u32), ("Alt", 0b0100), ("Primary", 0b1000)] {
        le.handle_event(&Event::key_press(98, bits));
        assert!(le.text().is_empty(), "{name}+B must not insert a `b`");
    }
    // And a control code with no modifier at all is not content either: `key: 9` is Tab.
    le.handle_event(&Event::key_press(9, 0));
    assert!(le.text().is_empty(), "a bare Tab is not text");

    // The unmodified key is still text, so the guard did not disable typing. The key codes
    // are ASCII, so 66 is the uppercase `B` and 98 the lowercase `b`.
    le.handle_event(&Event::key_press(98, 0));
    assert_eq!(le.text(), "b", "a plain `b` is content");
}

/// The framework's Shift bit on an event mask.
const SHIFT_BIT: u32 = 0b0001;

// ─── Pointer selection ───

/// A press puts the caret where the user aimed and anchors a selection there.
///
/// # The defect this pins
///
/// A press used to *only* focus the field. `set_text` leaves the caret at the end, so clicking
/// into the middle of an existing value and typing appended to it rather than inserting at the
/// click — and there was no pointer selection at all, because nothing computed an index from `x`.
/// This bites the common case first: opening a form, clicking a prefilled field to correct one
/// word, and finding every keystroke going to the end of the line.
#[cfg(not(alloc_frugal))]
#[cfg(device_profile)]
#[test]
fn a_press_places_the_caret_where_the_pointer_landed() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("abcdefgh".to_string());
    assert_eq!(le.cursor_position(), 8, "`set_text` leaves the caret at the end");
    let field = le.field_rect();

    // One character cell past the value's origin is the boundary after character 1.
    let cell = cell_width();
    let origin = value_origin(&mut le);
    le.handle_event(&Event::mouse_press(
        origin + cell + cell / 2,
        field.y + field.height as i32 / 2,
        1,
    ));
    assert_eq!(le.cursor_position(), 1, "the caret lands on the character clicked");
    assert_eq!(le.selection_start(), Some(1), "and it is the anchor for a drag");
}

/// A drag extends that selection; a hover that was never pressed does not move the caret.
///
/// Both halves matter. The first is the gesture: press on character 1, drag to character 4,
/// release leaves `"bcd"` selected. The second is the guard — `MouseMove` is delivered on every
/// pointer step over the control, so a field that acted on it unconditionally would have its
/// caret follow the mouse whenever the pointer crossed it, with no button pressed at all.
#[cfg(not(alloc_frugal))]
#[cfg(device_profile)]
#[test]
fn a_drag_extends_the_selection_but_a_bare_hover_does_not() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("abcdefgh".to_string());
    let field = le.field_rect();
    let mid = field.y + field.height as i32 / 2;
    let cell = cell_width();
    let origin = value_origin(&mut le);

    // A hover with no press ever delivered leaves the caret exactly where it was.
    le.handle_event(&Event::mouse_move(field.x + 4 * cell, mid));
    assert_eq!(le.cursor_position(), 8, "an unpressed hover must not move the caret");
    assert_eq!(le.selection_start(), None);

    // Press on character 1, drag to character 4: `"bcd"` is selected.
    le.handle_event(&Event::mouse_press(origin + cell + cell / 2, mid, 1));
    assert_eq!(le.selection_start(), Some(1), "the press anchors where it landed");
    le.handle_event(&Event::mouse_move(origin + 4 * cell + cell / 2, mid));
    assert_eq!(le.selected_text(), "bcd", "the drag selected the span between the two points");

    // The release ends the gesture, so a later hover is inert again.
    le.handle_event(&Event::mouse_release(origin + 4 * cell, mid, 1));
    le.handle_event(&Event::mouse_move(field.x + 7 * cell, mid));
    assert_eq!(le.cursor_position(), 4, "a hover after the release does not extend");
    assert_eq!(le.selected_text(), "bcd", "and the selection the user made is kept");
}

/// A press past the end of the value anchors at the end and readies a backward drag.
#[cfg(not(alloc_frugal))]
#[test]
fn a_press_beyond_the_value_anchors_at_its_end() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("ab".to_string());
    let field = le.field_rect();
    let mid = field.y + field.height as i32 / 2;

    // Well past the two characters that exist: the caret saturates at the end. The press must
    // land **inside the painted band** to be delivered, so it is taken from `field`, not from
    // the control's rectangle — on a tall cell the two are different boxes.
    le.handle_event(&Event::mouse_press(field.x + field.width as i32 - 2, mid, 1));
    assert_eq!(
        le.cursor_position(),
        2,
        "a press past the value lands at its end, where the next character would go"
    );
    assert_eq!(le.selection_start(), Some(2), "and that position is the anchor");

    // Dragging back over the value selects it, which is the gesture this enables.
    let origin = value_origin(&mut le);
    le.handle_event(&Event::mouse_move(origin, mid));
    assert_eq!(le.cursor_position(), 0, "the drag reaches the value's leading edge");
    assert_eq!(le.selected_text(), "ab", "so the range is anchor(2)-to-caret(0)");
}

/// The click-to-index map and the painted value agree about where character zero is.
///
/// Read off the ink rather than off the same helper the hit test calls: a test that asked
/// `decoration_layout` where the value starts would agree with a bug in it. The pen that draws
/// the string is at `field.x + TEXT_FIELD_PADDING_H` (see `the_value_starts_at_the_fields_horizontal_padding`),
/// so clicking that pixel must resolve to index 0.
#[cfg(not(alloc_frugal))]
#[test]
fn a_click_on_the_first_glyph_resolves_to_index_zero() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("Sample".to_string());
    let field = le.field_rect();
    let pen = field.x + dimensions::TEXT_FIELD_PADDING_H as i32;

    le.handle_event(&Event::mouse_press(pen, field.y + field.height as i32 / 2, 1));
    assert_eq!(le.cursor_position(), 0, "the first pixel of the value is index 0");
}

/// The pixel-to-index map is total on multi-byte values: no click can split a character.
///
/// A byte-counting map lands inside `é` or a CJK glyph and the next `&text[..caret]` panics.
#[cfg(not(alloc_frugal))]
#[test]
fn a_click_never_lands_inside_a_multi_byte_character() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("héllo wörld 中文".to_string());
    let field = le.field_rect();
    let mid = field.y + field.height as i32 / 2;

    for dx in 0..field.width as i32 {
        le.handle_event(&Event::mouse_press(field.x + dx, mid, 1));
        let caret = le.cursor_position();
        assert!(caret <= le.text().len(), "the caret is inside the value at +{dx}");
        assert!(
            le.text().is_char_boundary(caret),
            "the caret at +{dx} split a character: {caret} in {:?}",
            le.text()
        );
    }
}

/// The width of one character cell in the field's shipped face, as the shaper computes it.
///
/// Kept in the test module rather than as a private method on the control: it is the fixture's
/// model of the metrics, and a control method for it would be dead code the gate would flag.
///
/// Gated with its only consumers: they drive the pointer path, so they are
/// `#[cfg(not(alloc_frugal))]`, and they need a device profile for the text metrics the
/// measurement rests on. An ungated helper became dead code under `mini` (which *is*
/// `alloc_frugal`) and again under a stripped-but-not-frugal profile (`embedded`), where
/// `clippy -D warnings` reported it while the device profiles stayed green.
#[cfg(all(not(alloc_frugal), device_profile))]
fn cell_width() -> i32 {
    (crate::core::Font::default().size() * 0.6).round().max(1.0) as i32
}

/// The x the value's glyphs are laid out from, read from the field's own layout.
#[cfg(not(alloc_frugal))]
fn value_origin(le: &mut LineEdit) -> i32 {
    let mut measurement =
        crate::render::SoftwarePaintBackend::new(crate::core::Size::new(0, 0), 1.0);
    let mut context = RenderContext::new(&mut measurement);
    le.decoration_layout(&mut context).value.x
}

#[test]
fn lineedit_insert_text() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.insert_str("Hello");
    assert_eq!(le.text(), "Hello");
}

#[test]
fn lineedit_backspace() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.backspace();
    assert_eq!(le.text(), "Hell");
}

#[test]
fn lineedit_delete() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.set_cursor_position(0);
    le.delete();
    assert_eq!(le.text(), "ello");
}

#[test]
fn lineedit_clear() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("Hello".to_string());
    le.clear();
    assert!(le.text().is_empty());
}

#[test]
fn lineedit_geometry_delegation() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_geometry(Rect::new(10, 10, 150, 30));
    assert_eq!(le.geometry(), Rect::new(10, 10, 150, 30));
}

/// The field is a full-width band one text-field height tall, centred in the control.
///
/// The control used to paint its whole rectangle, so a 240x120 census cell drew a
/// 240x120 white slab — a panel, not a field — and the drawn height disagreed with
/// `size_hint`'s 24 by a factor of five. This pins the shared `full_width_band`
/// derivation in both directions: the height is the table's constant whatever the
/// cell, and a control smaller than the field clamps rather than painting outside.
#[cfg(device_profile)]
#[test]
fn the_field_is_a_text_field_height_in_any_rectangle() {
    // The assertion is the Material height, so a concurrent test that activates the Macos
    // profile must not change what this control measures.
    let _theme_guard = crate::style::theme_test_guard();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    for height in [48u32, 120, 300] {
        let le = LineEdit::new(Rect::new(0, 0, 240, height));
        let field = le.field_rect();
        assert_eq!(field.height, dimensions::TEXT_FIELD_MIN_HEIGHT, "at control height {height}");
        assert_eq!(field.width, 240, "the field spans the control's width");
        assert_eq!(field.y, (height - field.height) as i32 / 2, "at control height {height}");
    }

    let short = LineEdit::new(Rect::new(0, 0, 240, 20));
    assert_eq!(short.field_rect().height, 20, "a short control clamps the field");
}

/// A Macos-profile theme paints and hit-tests macOS's 22 px field band.
///
/// The field is the single derivation both the paint path and the pointer use, so asserting
/// its height pins the whole control rather than one call site.
#[cfg(device_profile)]
#[test]
fn a_macos_profile_theme_uses_the_compact_field_height() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-lineedit-macos".to_string();
    theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Macos;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-lineedit-macos"));
    }

    let le = LineEdit::new(Rect::new(0, 0, 240, 120));
    let field = le.field_rect();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(field.height, 22, "macOS's text field is 22 px tall: {field:?}");
    assert_eq!(field.y, (120 - 22) / 2, "the compact band is still centred: {field:?}");
    assert_eq!(field.width, 240, "and still spans the control");
}

/// The Cupertino profile keeps the Material text-field band unchanged.
#[cfg(device_profile)]
#[test]
fn the_cupertino_profile_keeps_the_material_field_height() {
    let _theme_guard = crate::style::theme_test_guard();
    let mut theme = crate::theme::Theme::default();
    theme.name = "p2-6-lineedit-cupertino".to_string();
    theme.metrics_profile = crate::widget::metrics::ControlMetricsProfile::Cupertino;
    {
        let mut manager = crate::theme::global_theme_manager();
        manager.register_theme(theme);
        assert!(manager.set_theme("p2-6-lineedit-cupertino"));
    }

    let field = LineEdit::new(Rect::new(0, 0, 240, 120)).field_rect();
    assert!(crate::theme::global_theme_manager().set_theme("default"));

    assert_eq!(field.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
}

/// Hit-testing follows the ink: a press below the field does not focus it.
///
/// With the field centred in a 120 px cell, a press on the control's rectangle but
/// 60 px below the drawn band belongs to the window background. The control must not
/// claim it — the clickable area has to be the one a user can see.
#[test]
fn a_press_outside_the_drawn_band_does_not_focus_the_field() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    let field = le.field_rect();

    // Inside the band focuses.
    le.handle_event(&Event::MousePress {
        pos: Point::new(field.x + 10, field.y + field.height as i32 / 2),
        button: 1,
        modifiers: 0,
    });
    assert!(le.is_focused(), "a press on the drawn field focuses it");

    // Below the band, inside the control's rectangle, does not.
    le.set_focused(false);
    le.handle_event(&Event::MousePress {
        pos: Point::new(field.x + 10, field.y + field.height as i32 + 40),
        button: 1,
        modifiers: 0,
    });
    assert!(!le.is_focused(), "a press below the drawn field must not focus it");
}

/// The value is drawn on the field's own middle line.
///
/// The origin of a text run is its glyph box's top-left corner, so the old
/// `rect.y + rect.height / 2` put that corner on the field's middle line and drew the
/// value half a line low. Pinning the drawn line box — not the origin — to the centre is
/// what keeps the two from agreeing by accident.
///
/// # Why the check reads the ink
///
/// The value is no longer a `<text>` element carrying a `y`: the backend emits the same
/// `font8x8` rectangles the software rasteriser fills, as subpaths of one `<path>` (see
/// `crate::widget::svg::text_ink_box`). The ink box is also the better witness, because it
/// is where the glyphs actually landed rather than what an element claimed.
#[cfg(not(alloc_frugal))]
#[test]
fn the_value_sits_on_the_fields_middle_line() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("Sample");
    let svg = crate::widget::svg::render_to_svg(&mut le);
    let field = le.field_rect();

    let (_, top, _, bottom) = crate::widget::svg::text_ink_box(&svg)
        .expect("a field with a value draws it as glyph geometry");
    assert!(top >= field.y, "the value starts inside the field: y={top}");
    assert!(
        bottom <= field.y + field.height as i32,
        "and above its bottom edge: y={bottom}, field={field:?}"
    );
    // And it is *centred*, not merely contained: the field's own line box is the reference
    // the value was moved onto, so its ink shares that box's middle line. A value left on
    // the old `rect.y + rect.height / 2` anchor sits a whole half line below it.
    let mut backend = crate::render::SvgPaintBackend::new(crate::core::Size::new(240, 120));
    let context = RenderContext::new(&mut backend);
    let font = crate::core::Font::default();
    let line = context.text_line(field, &font);
    assert_eq!(
        top + bottom,
        line.y * 2 + line.height as i32,
        "the value hangs on the field's own line box middle line"
    );
}

/// The value's horizontal origin is the field's own padding.
///
/// A field's text is inset from its edge by [`dimensions::TEXT_FIELD_PADDING_H`]; the
/// origin was a local literal `4`, which is a different fact written in a second place.
///
/// # Why the edge is read off the ink
///
/// There is no `x` attribute to read any more: the backend emits the string as `font8x8`
/// glyph rectangles inside one `<path>` (see `crate::widget::svg::text_ink_box`), so the
/// run's left edge is where the pen actually put its first set bit. `context.text_line`
/// centres the line box's *height*, not each cluster, so the pen itself is `field.x +
/// TEXT_FIELD_PADDING_H` exactly; what is left over is the first glyph's own blank lead
/// column, which the `font8x8` table gives as 0 for `S`. The assertion is still a real
/// constraint on the padding: the pen is what the padding places, and the test would read
/// `padding + 20` (or any other literal) instead of `padding` if the draw site stopped
/// reading [`dimensions::TEXT_FIELD_PADDING_H`].
#[cfg(not(alloc_frugal))]
#[test]
fn the_value_starts_at_the_fields_horizontal_padding() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("Sample");
    let svg = crate::widget::svg::render_to_svg(&mut le);
    // The fixture paints only the field's own chrome — a fill and a border — so what is
    // left is the value's ink, which no other string in this control can supply.
    assert_eq!(svg.matches("<rect").count(), 2, "only the field's fill and border are rects");
    let field = le.field_rect();

    let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg)
        .expect("a field with a value draws it as glyph geometry");
    assert_eq!(
        left,
        field.x + dimensions::TEXT_FIELD_PADDING_H as i32,
        "the value starts at the field's own padding: field={field:?}"
    );
}

// ─── Decoration slots (F-12) ───

/// A field that never sets a slot must paint exactly as it did, and reserve nothing extra.
///
/// This is the non-change guarantee: the slots are additive, so every existing caller keeps its
/// pixels and its height.
#[test]
fn a_field_with_no_slots_is_unchanged() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_text("Sample");
    assert!(le.decorations().is_empty());
    assert_eq!(le.get("prefix").unwrap().as_str(), Some(""));
    assert_eq!(le.get("counter").unwrap(), CapabilityValue::Null, "no limit means no counter");

    let svg = crate::widget::svg::render_to_svg(&mut le);
    // The value is still the only ink, and it still starts at the field's padding.
    let (left, _, _, _) = crate::widget::svg::text_ink_box(&svg).expect("the value is drawn");
    assert_eq!(left, le.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32);
}

/// A prefix is drawn **before** the value, and the value moves right to make room.
///
/// # The defect this pins
///
/// The shortcut is to concatenate `prefix + value` into one string. That draws nearly the same
/// glyphs in nearly the same place, so it looks right — while it also (a) makes `cursor_position ==
/// 0` render after the `$`, and (b) makes a select-all copy the `$`. The distinguishing fact is that
/// the two are **separate ink runs** with the prefix to the left of the value, so the test reads all
/// runs and orders them rather than trusting which one was painted first.
#[test]
fn a_prefix_is_drawn_before_the_value_and_shifts_it() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    let mut plain = LineEdit::new(Rect::new(0, 0, 240, 120));
    plain.set_text("12");
    let plain_svg = crate::widget::svg::render_to_svg(&mut plain);
    let plain_runs = crate::widget::svg::text_ink_boxes(&plain_svg);
    assert_eq!(plain_runs.len(), 1, "a plain field draws its value as one run");
    let (plain_left, _, plain_right, _) = plain_runs[0];

    let mut prefixed = LineEdit::new(Rect::new(0, 0, 240, 120));
    prefixed.set_text("12");
    prefixed.set_prefix("$");
    let prefixed_svg = crate::widget::svg::render_to_svg(&mut prefixed);
    let mut runs = crate::widget::svg::text_ink_boxes(&prefixed_svg);
    assert_eq!(runs.len(), 2, "the prefix and the value are two runs, not one concatenated string");
    runs.sort_by_key(|b| b.0);
    let (prefix_left, prefix_right) = (runs[0].0, runs[0].2);
    let (value_left, value_right) = (runs[1].0, runs[1].2);

    // The `$` takes the field's leading padding. The run's left edge is the *ink*, and neither
    // face's ink starts exactly at the origin — the `$` bitmap has no set bit in its leftmost
    // column, and an outline `$` is inset by its own shape — so the assertion allows the ink
    // inset and nothing more. It is still a comparison against where a *value with no prefix*
    // starts, so a prefix that ignored the padding and began on the border (or after the
    // value) is a whole glyph away and fails.
    assert!(
        (prefix_left - plain_left).abs() <= INK_INSET_TOLERANCE,
        "the prefix starts where a value with no prefix starts: it begins at {prefix_left}, \
         the field's leading padding puts a run at {plain_left}"
    );
    // …and the value is pushed clear of it rather than starting there too.
    assert!(
        value_left > prefix_right,
        "the value must begin past the prefix: prefix ends at {prefix_right}, value at {value_left}"
    );
    assert!(
        value_left > plain_left,
        "and further right than it sat without a prefix ({plain_left})"
    );
    // The value's own ink is the same width as before — only its origin moved, so the prefix did not
    // disturb the glyphs it marks.
    assert_eq!(
        value_right - value_left,
        plain_right - plain_left,
        "the prefix shifted the value without reshaping it"
    );

    assert_eq!(prefixed.get("prefix").unwrap().as_str(), Some("$"));
    assert!(
        plain_left >= plain.field_rect().x + dimensions::TEXT_FIELD_PADDING_H as i32,
        "the value is inset by the field's padding, not on the border"
    );
}

/// The caret is measured against the **value**, so at `cursor_position == 0` it sits at the value's
/// origin — after a prefix, not after the `$`'s own box.
///
/// This is the property that makes keeping the slots separate worth the trouble: with the prefix
/// folded into the text, `cursor_position == 0` would place the caret to the right of the `$` and
/// the start of the value would be unreachable.
#[test]
fn the_caret_at_position_zero_sits_at_the_values_origin_not_after_the_prefix() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_prefix("$");
    le.set_text("12");
    le.set_cursor_position(0);
    le.set_focused(true);

    // The layout is computed against the **real** SVG backend's metrics — the same backend the
    // painter uses — and then read for the two facts that matter: the prefix has a box, and the
    // value begins past it.
    let mut backend = crate::render::svg::SvgPaintBackend::new(le.geometry().size());
    let layout = {
        let mut context = crate::render::RenderContext::new(&mut backend);
        le.decoration_layout(&mut context)
    };
    let prefix = layout.prefix.expect("the prefix has a box of its own");
    assert!(
        layout.value.x >= prefix.right(),
        "the value must begin at or past the prefix's trailing edge: {layout:?}"
    );
    assert!(layout.value.width > 0, "and it must keep some room: {layout:?}");
}

/// An error displaces the helper and paints on its own row; the counter shares that row and is
/// derived from the value and the limit.
#[test]
fn the_support_row_shows_the_error_and_a_derived_counter() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set_max_length(Some(5));
    le.set_text("abc");
    assert_eq!(le.get("counter").unwrap().as_str(), Some("3/5"));
    assert_eq!(le.get("over_limit").unwrap().as_bool(), Some(false));

    // The helper shows while there is no error…
    le.set_helper_text("Up to five characters");
    assert_eq!(le.decorations().support_message(), "Up to five characters");
    assert!(le.decorations().needs_support_row());

    // …and the error displaces it rather than joining it.
    le.set_error_text("Too long");
    assert_eq!(le.decorations().support_message(), "Too long");
    assert!(le.decorations().has_error());
    assert_eq!(le.get("error").unwrap().as_str(), Some("Too long"));
    assert_eq!(
        le.get("helper").unwrap().as_str(),
        Some("Up to five characters"),
        "the helper is displaced, not deleted"
    );

    // Over the limit is reported, and the counter says how far. `set_text` enforces the limit too —
    // a limit the control honours only while typing is a hint, not a limit — so the value is clamped
    // and the pair stays consistent.
    le.set_text("abcdefgh");
    assert_eq!(le.text(), "abcde", "`set_text` must honour `max_length` like typing does");
    assert_eq!(le.get("counter").unwrap().as_str(), Some("5/5"));
    assert_eq!(
        le.get("over_limit").unwrap().as_bool(),
        Some(false),
        "the stored value cannot exceed a limit it was clamped to"
    );

    // Clearing the error restores the helper without the caller having to re-set it.
    le.set_error_text("");
    assert_eq!(le.decorations().support_message(), "Up to five characters");
}

/// The counter is read-only, and the two in-field slots round-trip through the contract.
#[test]
fn the_slots_round_trip_through_the_property_api() {
    let mut le = LineEdit::new(Rect::new(0, 0, 240, 120));
    le.set("prefix", CapabilityValue::String("$".to_string())).unwrap();
    le.set("suffix", CapabilityValue::String("%".to_string())).unwrap();
    le.set("helper", CapabilityValue::String("Hint".to_string())).unwrap();
    le.set("error", CapabilityValue::String("Bad".to_string())).unwrap();

    assert_eq!(le.get("prefix").unwrap().as_str(), Some("$"));
    assert_eq!(le.get("suffix").unwrap().as_str(), Some("%"));
    assert_eq!(le.get("helper").unwrap().as_str(), Some("Hint"));
    assert_eq!(le.get("error").unwrap().as_str(), Some("Bad"));

    // The two derived names are refused, so there is no second writer to disagree with.
    assert!(le.set("counter", CapabilityValue::String("1/1".to_string())).is_err());
    assert!(le.set("over_limit", CapabilityValue::Bool(true)).is_err());
}

/// A field too narrow for its slots still draws its field chrome and does not panic.
#[test]
fn a_field_squeezed_by_its_slots_still_paints() {
    let mut le = LineEdit::new(Rect::new(0, 0, 40, 120));
    le.set_prefix("verylongprefix");
    le.set_suffix("verylongsuffix");
    le.set_text("value");
    let svg = crate::widget::svg::render_to_svg(&mut le);
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn lineedit_visibility() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert!(le.is_visible());
    le.hide();
    assert!(!le.is_visible());
    le.show();
    assert!(le.is_visible());
}

#[test]
fn lineedit_enabled() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    assert!(le.is_enabled());
    le.set_enabled(false);
    assert!(!le.is_enabled());
    le.set_enabled(true);
    assert!(le.is_enabled());
}

#[test]
fn lineedit_id_kind() {
    let le_a = LineEdit::new(Rect::new(0, 0, 100, 24));
    let le_b = LineEdit::new(Rect::new(0, 0, 100, 24));
    assert_ne!(le_a.id(), le_b.id());
    assert_eq!(le_a.kind(), WidgetKind::LineEdit);
    assert_eq!(le_b.kind(), WidgetKind::LineEdit);
}

#[test]
fn lineedit_signal_accessors() {
    let le = LineEdit::new(Rect::new(0, 0, 100, 24));
    let _text_changed = &le.text_changed;
    let _editing_finished = &le.editing_finished;
    let _return_pressed = &le.return_pressed;
}

#[cfg(not(alloc_frugal))]
#[test]
fn the_caret_is_drawn_at_cursor_position_not_at_the_end() {
    // Holds the crate-wide theme guard: this test renders, and a concurrent
    // test that switches the appearance would otherwise change a later frame.
    let _theme_guard = crate::style::theme_test_guard();
    // The caret used to be measured against the **whole** value, so it was always drawn
    // after the last glyph however the control was positioned: with `cursor_position = 0` it
    // still sat past the `e` of "Sample". The field answers `cursor_position`, clamps it on
    // every write, and acts on it in `backspace`/`delete`, so the marker was the one part of
    // the field ignoring the field's own cursor.
    //
    // The assertion reads the drawn geometry out of the SVG, because the defect is entirely
    // about where the ink lands — asserting the stored index would have passed before the fix
    // too, which is why it survived.
    fn caret_x(cursor_position: usize) -> i32 {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_text("Sample".to_string());
        // `focus()` places the caret at the end for the autofocus case, so the requested
        // position is applied *after* it — the same order a caller that positions a caret
        // uses.
        field.set_focused(true);
        field.set_cursor_position(cursor_position);
        let svg = crate::widget::svg::render_to_svg(&mut field);
        // The caret is the only vertical line the control draws, emitted as a `<line>` with
        // `x1 == x2`.
        svg.lines()
            .filter(|line| line.contains("<line"))
            .find_map(|line| {
                let x1 = line.split("x1=\"").nth(1)?.split('"').next()?;
                let x2 = line.split("x2=\"").nth(1)?.split('"').next()?;
                if x1 != x2 {
                    return None;
                }
                x1.parse::<i32>().ok()
            })
            .expect("a focused, editable field must draw its caret as a vertical line")
    }

    let at_start = caret_x(0);
    let at_three = caret_x(3);
    let at_end = caret_x(6);
    // This assertion used to pin the literal `4`, which was a local padding constant
    // written at the draw site. The origin is now [`dimensions::TEXT_FIELD_PADDING_H`],
    // the same table value every other field insets its content by, so what this pins is
    // "the caret starts at the field's own horizontal padding" — the fact the old
    // literal was an unshared spelling of, not a different fact.
    assert_eq!(
        at_start,
        dimensions::TEXT_FIELD_PADDING_H as i32,
        "a caret at position 0 sits at the text origin (the field's padding inset)"
    );
    assert!(
        at_start < at_three && at_three < at_end,
        "the caret must advance with the cursor position, not jump to the end \
         ({at_start}, {at_three}, {at_end})"
    );
}

/// `max_length` is a **character** budget, so truncating at it must count characters.
///
/// The limit is what the user sees and what `counter_text` reports, but an earlier revision
/// truncated with `String::truncate(max)` — a **byte** index. On `"héllo"` (five characters,
/// six bytes) a limit of three cut to `"hé"`, dropping a legal character; and a limit that
/// landed inside a multi-byte character split it, which `String::truncate` panics on rather
/// than permitting.
#[test]
fn max_length_truncates_whole_characters_not_bytes() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("héllo");
    assert_eq!(le.text().chars().count(), 5);
    le.set_max_length(Some(3));
    assert_eq!(le.text(), "hél", "three *characters* are kept, not three bytes");
    assert!(le.text().is_char_boundary(le.cursor_position()));

    // A limit that falls inside a character must not panic and must not split it.
    let mut wide = LineEdit::new(Rect::new(0, 0, 200, 24));
    wide.set_text("éééé");
    wide.set_max_length(Some(2));
    assert_eq!(wide.text(), "éé");
}

/// The limit is compared against the value in the *same* frame it is expressed in.
///
/// `insert_text` computed its remaining room as `max - self.text.len()`, mixing a character
/// budget with a byte length: on `"héllo"` a limit of eight left `8 - 6 == 2` characters of
/// room instead of three, and a limit of six — which the counter shows as "5/6", i.e. one
/// character free — left **zero**, so the field refused every further keystroke.
#[test]
fn insert_text_measures_the_remaining_room_in_characters() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("héllo");
    le.set_max_length(Some(6));
    le.insert_str("z");
    assert_eq!(le.text(), "hélloz", "the counter said 5/6, so one more character fits");
    assert_eq!(le.text().chars().count(), 6);

    // A multi-character insert is clipped to the room that remains, by character.
    let mut clipped = LineEdit::new(Rect::new(0, 0, 200, 24));
    clipped.set_text("éé");
    clipped.set_max_length(Some(4));
    clipped.insert_str("wxyz");
    assert_eq!(clipped.text(), "ééwx");
}

/// `selection_range` and `insert_str` are the spellings the other text fields publish
/// (WDGT-14); this pins that they describe and edit the same selection.
#[test]
fn selection_range_and_insert_str_share_the_fields_selection() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("hello world");
    assert_eq!(le.selection_range(), None, "no anchor means no range");

    le.select_all();
    assert_eq!(le.selection_range(), Some((0, 11)));
    assert_eq!(le.selected_text(), "hello world", "the range and the text agree");

    le.insert_str("bye");
    assert_eq!(le.text(), "bye", "the canonical insert replaced the selection");
    assert_eq!(le.selection_range(), None);
}

/// The widget layer's half of IME: a preedit updates the preview without touching the value,
/// and a commit replaces the preview with text through the ordinary edit path.
#[test]
fn ime_preedit_and_commit_update_the_model() {
    let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
    field.handle_event(&Event::ime_preedit("に", 1));
    assert_eq!(field.text(), "", "an uncommitted composition is not the value");
    assert_eq!(field.composition(), Some("に"));

    field.handle_event(&Event::ime_commit("你好"));
    assert_eq!(field.text(), "你好", "the commit enters the model");
    assert!(!field.has_composition(), "and ends the composition");
}

/// The IME query reports a real caret rectangle, and it moves with the caret.
///
/// A non-zero rectangle is the whole point: a native candidate window anchored to a zero-sized
/// or off-field box has no insertion point to sit beside. The x advance also has to follow the
/// measured prefix, so the anchor lands on the character the user is inserting at.
#[test]
fn input_method_query_reports_the_caret_rectangle() {
    let mut field = LineEdit::new(Rect::new(10, 20, 200, 24));
    field.set_text("abc");

    field.set_cursor_position(0);
    let at_start =
        field.input_method_query().expect("an editable text field always has an insertion point");
    assert!(at_start.caret_rect.width > 0, "the caret must have a width: {at_start:?}");
    assert!(at_start.caret_rect.height > 0, "the caret must have a height: {at_start:?}");
    assert_eq!(at_start.caret_byte, Some(0));

    field.set_cursor_position(3);
    let at_end = field.input_method_query().expect("query after moving the caret");
    assert_eq!(at_end.caret_byte, Some(3));
    assert!(
        at_end.caret_rect.x > at_start.caret_rect.x,
        "the caret must advance with the measured prefix ({} -> {})",
        at_start.caret_rect.x,
        at_end.caret_rect.x
    );
    assert_eq!(at_end.caret_rect.height, at_start.caret_rect.height);
}

/// The caret and the edit keys step by **character**, not by byte.
///
/// A bare `cursor_position += 1` landed inside a multi-byte character, so `insert_text`'s
/// `floor_char_boundary` then rounded forward to the end of the value: on `"éé"` a single
/// Right followed by a keystroke appended instead of inserting one character in. The same
/// arithmetic made `backspace`/`delete` slice mid-character.
#[test]
fn caret_and_edit_keys_step_by_character() {
    let mut le = LineEdit::new(Rect::new(0, 0, 200, 24));
    le.set_text("éé");
    le.set_cursor_position(0);
    le.handle_event(&Event::key_press(39, 0)); // Right
    assert_eq!(le.cursor_position(), 2, "one character forward is two bytes for 'é'");
    le.insert_str("x");
    assert_eq!(le.text(), "éxé", "the insert lands at the caret, not at the end");

    // Backspace removes the whole character before the caret.
    let mut back = LineEdit::new(Rect::new(0, 0, 200, 24));
    back.set_text("aé");
    back.set_cursor_position(back.text().len());
    back.backspace();
    assert_eq!(back.text(), "a");

    // Delete removes the whole character after the caret.
    let mut fwd = LineEdit::new(Rect::new(0, 0, 200, 24));
    fwd.set_text("éa");
    fwd.set_cursor_position(0);
    fwd.delete();
    assert_eq!(fwd.text(), "a");
}

/// The caret rides with the glyphs of a centred or right-aligned value.
///
/// The value is laid out from `fitted_origin(value_box, advance, alignment)`; the caret was
/// measured from the field's fixed leading inset instead, so on a right-aligned field the
/// marker sat at the left edge while its own text sat at the right edge. The two now read the
/// same origin function, which is what keeps them together.
#[test]
fn the_caret_follows_a_non_left_aligned_value() {
    let _theme_guard = crate::style::theme_test_guard();
    fn caret_x(alignment: crate::core::Alignment) -> i32 {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_text("Sample".to_string());
        field.set_alignment(alignment);
        field.set_focused(true);
        field.set_cursor_position(0);
        let svg = crate::widget::svg::render_to_svg(&mut field);
        svg.lines()
            .filter(|line| line.contains("<line"))
            .find_map(|line| {
                let x1 = line.split("x1=\"").nth(1)?.split('"').next()?;
                let x2 = line.split("x2=\"").nth(1)?.split('"').next()?;
                (x1 == x2).then(|| x1.parse::<i32>().ok())?
            })
            .expect("a focused, editable field must draw its caret as a vertical line")
    }

    let left = caret_x(crate::core::Alignment::Left);
    let centre = caret_x(crate::core::Alignment::Center);
    let right = caret_x(crate::core::Alignment::Right);
    assert!(
        left < centre && centre < right,
        "a caret at position 0 must follow the value's own alignment \
         (left={left}, centre={centre}, right={right})"
    );
}

#[cfg(not(alloc_frugal))]
#[test]
fn lineedit_clipboard_copy_paste_cut() {
    use crate::event::Event::KeyPress;
    // The clipboard is process-wide, so this test must not run concurrently with any
    // other test that copies or pastes. Without the guard it failed intermittently
    // under the parallel harness while passing on its own.
    let _clipboard = crate::clipboard::clipboard_test_guard();

    let mut source = LineEdit::new(Rect::new(0, 0, 200, 24));
    source.set_text("hello world");
    source.select_all();
    // Ctrl+C copies the selection to the platform clipboard.
    source.handle_event(&KeyPress { key: 67, modifiers: 0b1000 });
    assert_eq!(crate::get_clipboard_text(), "hello world");

    // Ctrl+V pastes into another field.
    let mut target = LineEdit::new(Rect::new(0, 0, 200, 24));
    target.handle_event(&KeyPress { key: 86, modifiers: 0b1000 });
    assert_eq!(target.text(), "hello world");

    // Ctrl+X on a selected field copies then removes the selection.
    source.select_all();
    source.handle_event(&KeyPress { key: 88, modifiers: 0b1000 });
    assert_eq!(source.text(), "");
    assert_eq!(crate::get_clipboard_text(), "hello world");

    // Read-only fields still copy via Ctrl+C.
    let mut read_only = LineEdit::new(Rect::new(0, 0, 200, 24));
    read_only.set_text("secret");
    read_only.set_read_only(true);
    read_only.select_all();
    // Read-only fields still copy via the platform's primary accelerator. The event bit is
    // the portable "primary" one (Command on macOS, Control on Windows/Linux); a raw Control
    // bit is not the primary accelerator on macOS, so using it made this assertion pass only
    // on the platforms whose shortcut style treats Control as primary.
    read_only.handle_event(&KeyPress { key: 67, modifiers: 0b1000 });
    assert_eq!(crate::get_clipboard_text(), "secret");
    // …but editing stays blocked.
    assert_eq!(read_only.text(), "secret");
}

/// A refusal travels the **meaning** channel, so it never competes with the interaction one.
///
/// # The defect this pins
///
/// The preset themes declare ten `"<kind>:error"` overrides, and the meaning used to be
/// reported from `widget_state()` — a single-valued chain the interaction also wanted. The
/// interaction won whenever the pointer was over the field, so `"line_edit:error"` stopped
/// applying exactly when the user moved onto the field to read the refusal. The meaning is now
/// orthogonal (`semantic_state`), so the first half of this test proves the control still asks
/// the question, and the second proves the theme's answer arrives on a border.
///
/// # Why this is gated on `full_widgets`
///
/// It installs the preset appearances through `widget::census`, which is the module that owns
/// them and exists only where the widget set is unstripped. On a stripped profile the theme is
/// absent entirely, so there is no preset to install and no `:error` override to read — the
/// subject of the test does not exist there, which is what a profile gate has to say.
#[cfg(full_widgets)]
#[test]
fn a_refused_field_reports_the_error_meaning_and_takes_the_override() {
    use crate::style::{SemanticState, WidgetState};

    let _theme_guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));

    assert_eq!(field.widget_state(), WidgetState::Normal, "a fresh field is at rest");
    assert_eq!(field.semantic_state(), SemanticState::None, "and carries no meaning");

    field.set_error_text("Too long");
    assert_eq!(
        field.semantic_state(),
        SemanticState::Error,
        "a field with a refusal message must say so, or the `:error` override is unreachable"
    );
    assert_eq!(
        field.widget_state(),
        WidgetState::Normal,
        "but the interaction channel stays at rest: the refusal is not an interaction"
    );

    // The theme resolves a border for that meaning — the whole point of reporting it.
    let border = crate::style::resolved_semantic_border("line_edit", field.semantic_state());
    assert!(border.is_some(), "the preset must define a border on `line_edit:error`");

    // Clearing the message returns the field to no meaning, so it is not latched.
    field.set_error_text("");
    assert_eq!(
        field.semantic_state(),
        SemanticState::None,
        "clearing the error restores no meaning"
    );
}

/// BLUE24 §3.3 criterion 1: a hovered refusal keeps **both** channels at once.
///
/// This is the assertion the single-valued chain could not satisfy: `widget_state()` was the
/// only way to say "error", so a hovered field could report either the hover or the error but
/// never both. With the channels split, the fill follows the hover and the border follows the
/// meaning in the same frame.
#[cfg(full_widgets)]
#[test]
fn a_hovered_refusal_keeps_hover_fill_and_error_border() {
    use crate::event::Event;
    use crate::style::{SemanticState, WidgetState};

    let _theme_guard = crate::style::theme_test_guard();
    crate::widget::census::install_preset_appearances();
    let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
    field.set_error_text("Too long");

    field.handle_event(&Event::MouseEnter { pos: crate::core::Point::new(10, 10) });

    assert_eq!(
        field.widget_state(),
        WidgetState::Hover,
        "the fill answers to the interaction, so a hovered field reports hover"
    );
    assert_eq!(
        field.semantic_state(),
        SemanticState::Error,
        "and the meaning survives the hover, which is the defect being pinned"
    );

    // Both channels reach the theme in the same frame: the hover fill and the error border.
    let hover_style = crate::theme::resolved_theme_style_for_state("line_edit", WidgetState::Hover);
    assert!(hover_style.is_some(), "the preset defines `line_edit:hover`");
    assert!(
        crate::style::resolved_semantic_border("line_edit", field.semantic_state()).is_some(),
        "and a border for `line_edit:error` at the same time"
    );
}

/// The interaction channel still reports `Disabled` for an inert field, refusal or not.
#[test]
fn a_disabled_field_reports_disabled_even_with_a_refusal_message() {
    use crate::style::{SemanticState, WidgetState};

    let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
    field.set_error_text("Too long");
    field.set_enabled(false);
    assert_eq!(
        field.widget_state(),
        WidgetState::Disabled,
        "a disabled field cannot be acted on, so its state must say disabled"
    );
    assert_eq!(
        field.semantic_state(),
        SemanticState::Error,
        "while the meaning is still reported on its own channel"
    );
}

mod composition_tests {
    use super::*;
    use crate::compat::Vec;

    fn field() -> LineEdit {
        LineEdit::new(Rect::new(0, 0, 200, 24))
    }

    /// Renders the field into an RGBA frame, for pixel-level assertions.
    fn render(widget: &mut LineEdit, rect: Rect) -> Vec<u8> {
        use crate::core::{Color, Size};
        use crate::render::{PaintBackend, RenderContext, SoftwarePaintBackend};
        use crate::widget::Draw;

        let mut surface = SoftwarePaintBackend::new(Size::new(rect.width, rect.height), 1.0);
        surface.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut surface);
            widget.draw(&mut context);
        }
        surface.end_frame();
        surface.frame_rgba().to_vec()
    }

    /// A composition does **not** change the field's value.
    ///
    /// # The defect this pins (BLUE24 §12 U-5)
    ///
    /// A platform input method reports the preedit one keystroke at a time. If that went through
    /// `insert_text`, the field's value would change on every partial character and `text_changed`
    /// would fire for a word the user has not finished — so a form's validation would run against
    /// text nobody committed. Holding the composition apart is what makes the value stable until the
    /// commit.
    #[test]
    fn a_composition_does_not_change_the_value() {
        let mut field = field();
        field.set_text("ab");
        field.set_composition("し");
        assert_eq!(field.text(), "ab", "the value is untouched while composing");
        assert_eq!(field.composition(), Some("し"));
        assert!(field.has_composition());
    }

    /// Committing puts the preedit into the model, at the caret, through the ordinary insert path.
    #[test]
    fn committing_inserts_the_preedit_as_text() {
        let mut field = field();
        field.set_text("ab");
        field.set_cursor_position(1);
        field.set_composition("X");
        assert!(field.commit_composition());
        assert_eq!(field.text(), "aXb", "a commit is an insert, not a replacement");
        assert!(!field.has_composition(), "and the session is over");
        assert!(!field.commit_composition(), "a second commit has nothing to accept");
    }

    /// Cancelling restores nothing, because nothing was changed.
    #[test]
    fn cancelling_leaves_the_value_exactly_as_it_was() {
        let mut field = field();
        field.set_text("keep");
        field.set_composition("discard me");
        assert!(field.cancel_composition());
        assert_eq!(field.text(), "keep");
        assert!(!field.has_composition());
        assert!(!field.cancel_composition(), "there was nothing left to cancel");
    }

    /// Committing a composition respects `max_length`, exactly as typing does.
    ///
    /// This is the reason the commit goes through `insert_text` rather than writing `text` directly:
    /// a shortcut would have been the one edit path in the control that ignores the limit.
    #[test]
    fn committing_respects_the_max_length() {
        let mut field = field();
        field.set_max_length(Some(4));
        field.set_text("ab");
        field.set_composition("cdef");
        field.commit_composition();
        assert!(field.text().len() <= 4, "the limit applies: got {:?}", field.text());
    }

    /// A composition can replace a selection, like any other insertion.
    #[test]
    fn committing_replaces_a_selection() {
        let mut field = field();
        field.set_text("abc");
        field.select_all();
        field.set_composition("Z");
        field.commit_composition();
        assert_eq!(field.text(), "Z", "the commit replaced the selected range");
    }

    /// An empty preedit withdraws the session without committing anything.
    #[test]
    fn an_empty_preedit_withdraws_the_session() {
        let mut field = field();
        field.set_text("ab");
        field.set_composition("x");
        field.set_composition("");
        assert!(!field.has_composition());
        assert_eq!(field.text(), "ab", "withdrawing is not committing");
    }

    /// The preedit is drawn **inline at the caret**, with an underline.
    ///
    /// # The gap this pins (BLUE24 §12 U-5)
    ///
    /// `ImePreedit` is an overlay in a box of its own; it cannot place a composition at a field's
    /// caret, because it does not know where that is. The field can, and this asserts it does: the
    /// preedit appears after the value's own ink and a horizontal rule is drawn beneath it.
    #[test]
    fn the_preedit_is_painted_at_the_caret_with_an_underline() {
        let bounds = Rect::new(0, 0, 200, 24);
        let mut field = LineEdit::new(bounds);
        field.set_text("ab");
        field.set_cursor_position(2);
        field.set_focused(true);

        let without = crate::widget::svg::render_widget_to_svg(&mut field, bounds);
        field.set_composition("し");
        let with = crate::widget::svg::render_widget_to_svg(&mut field, bounds);

        assert_ne!(with, without, "an active composition must change what is painted");
        // A horizontal rule is a `<line>` with `y1 == y2`; the caret is a vertical one. The underline
        // is the only horizontal line this change adds.
        let horizontal_lines = |svg: &str| {
            svg.lines()
                .filter(|line| line.contains("<line "))
                .filter(|line| {
                    let get = |name: &str| -> Option<i32> {
                        line.split(&format!("{name}=\"")).nth(1)?.split('"').next()?.parse().ok()
                    };
                    get("y1").is_some() && get("y1") == get("y2")
                })
                .count()
        };
        assert!(
            horizontal_lines(&with) > horizontal_lines(&without),
            "the preedit must be underlined, or a user cannot see it is uncommitted"
        );

        // And cancelling removes it again, so the picture is tied to the session.
        field.cancel_composition();
        let after = crate::widget::svg::render_widget_to_svg(&mut field, bounds);
        assert_eq!(horizontal_lines(&after), horizontal_lines(&without));
    }

    // ── D09-STYLE-02: the hint measures with the font the paint uses ──

    /// A caller-authored `style.font` must widen the field's hint and its drawn value.
    ///
    /// # The defect this pins
    ///
    /// `size_hint()` measured the value with `Font::default()` while `draw()` used `style.font`,
    /// so a custom face or a scaled theme font left the field asking for less room than its ink
    /// needed.
    #[test]
    fn a_custom_font_grows_the_line_edit_hint_and_the_drawn_value() {
        let _theme_guard = crate::style::theme_test_guard();
        let mut baseline = LineEdit::new(Rect::new(0, 0, 400, 24));
        baseline.set_text("A longer value");
        let base_hint = baseline.size_hint();
        let base_ink = value_ink_width(&mut baseline);

        let mut big = LineEdit::new(Rect::new(0, 0, 400, 24));
        big.set_text("A longer value");
        big.set_style(
            crate::style::WidgetStyle::default().with_font(crate::core::Font::simple("Test", 30.0)),
        );
        let big_hint = big.size_hint();
        let big_ink = value_ink_width(&mut big);

        assert!(
            big_hint.width > base_hint.width,
            "a larger font must widen the field hint: {} vs {}",
            big_hint.width,
            base_hint.width
        );
        assert!(
            big_ink > base_ink,
            "a larger font must widen the drawn value: {big_ink} vs {base_ink}"
        );
    }

    /// A text scale above 1.0, resolved through the theme, must widen the hint.
    #[test]
    #[cfg(device_profile)]
    fn a_theme_text_scale_above_one_grows_the_line_edit_hint() {
        use crate::style::environment::{
            install_environment, uninstall_environment, EnvironmentProvider,
        };
        use crate::style::MotionPreference;

        struct Scaled(f32);
        impl EnvironmentProvider for Scaled {
            fn text_scale(&self) -> f32 {
                self.0
            }
            fn motion_preference(&self) -> MotionPreference {
                MotionPreference::NoPreference
            }
        }

        let _guard = crate::style::theme_test_guard();
        let previous = install_environment(Box::new(Scaled(1.0)));
        {
            let mut manager = crate::theme::global_theme_manager();
            manager.register_theme(crate::theme::Theme::default());
            manager.set_appearance(crate::theme::AppearanceMode::Light);
        }
        let mut at_one = LineEdit::new(Rect::new(0, 0, 400, 24));
        at_one.set_text("Scaled value");
        crate::theme::apply_active_theme(&mut at_one);
        let hint_one = at_one.size_hint();

        install_environment(Box::new(Scaled(1.5)));
        let mut scaled = LineEdit::new(Rect::new(0, 0, 400, 24));
        scaled.set_text("Scaled value");
        crate::theme::apply_active_theme(&mut scaled);
        let hint_large = scaled.size_hint();

        let _ = uninstall_environment();
        if let Some(previous) = previous {
            install_environment(previous);
        }

        assert!(
            hint_large.width > hint_one.width,
            "a text scale above 1.0 must widen the field hint: {} vs {}",
            hint_large.width,
            hint_one.width
        );
    }

    /// The ink width of the value a field paints, read back from an SVG render.
    fn value_ink_width(field: &mut LineEdit) -> i32 {
        let svg = crate::widget::svg::render_to_svg(field);
        crate::widget::svg::text_ink_box(&svg).map(|(x, _, right, _)| right - x).unwrap_or(0)
    }

    // ── D09-INPUT-01: platform-committed text reaches the value ──

    /// A `TextInput` from the platform must enter the field's value.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as
    /// `Event::TextInput`, not as a `KeyPress`. This handler only matched `KeyPress`, so the text
    /// the platform committed was dropped and a focused field never changed. The test feeds
    /// `TextInput` (never `KeyPress`) to prove the committed-text path itself is wired.
    #[test]
    fn text_input_enters_the_value() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        field.handle_event(&Event::TextInput { text: "hé".to_string() });
        assert_eq!(field.text(), "hé", "committed text must reach the value");
    }

    /// An IME commit is committed text and enters the value the same way.
    #[test]
    fn ime_commit_enters_the_value() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        field.handle_event(&Event::ime_commit("你好"));
        assert_eq!(field.text(), "你好");
    }

    /// A `TextInput` replacing a selection must replace the range, not append to it.
    #[test]
    fn text_input_replaces_the_selection() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        field.set_text("abcdef".to_string());
        field.select_all();
        field.handle_event(&Event::TextInput { text: "Z".to_string() });
        assert_eq!(field.text(), "Z", "the committed text replaced the selected range");
    }

    /// Committed text respects `max_length`, exactly as a typed character does.
    #[test]
    fn text_input_respects_the_max_length() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        field.set_max_length(Some(3));
        field.handle_event(&Event::TextInput { text: "abcdef".to_string() });
        assert_eq!(field.text().chars().count(), 3, "the limit applies to a commit too");
    }

    /// A read-only field ignores committed text.
    #[test]
    fn read_only_ignores_text_input() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        field.set_text("locked".to_string());
        field.set_read_only(true);
        field.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(field.text(), "locked", "a read-only field must not accept committed text");
    }

    /// A disabled field ignores committed text.
    #[test]
    fn disabled_ignores_text_input() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_enabled(false);
        field.set_focused(true);
        field.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(field.text(), "", "a disabled field must not accept committed text");
    }

    /// Committed text emits `text_changed` once with the new value, and the edit is undoable.
    #[test]
    fn text_input_emits_changed_and_is_undoable() {
        use crate::compat::Vec;
        use std::sync::{Arc, Mutex};
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_focused(true);
        let seen = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen_clone = seen.clone();
        field.text_changed.connect(move |text| seen_clone.lock().unwrap().push(text.to_string()));

        field.handle_event(&Event::TextInput { text: "hi".to_string() });
        assert_eq!(field.text(), "hi");
        assert_eq!(*seen.lock().unwrap(), vec!["hi".to_string()]);
        assert!(field.undo(), "committed text is on the undo stack like any other edit");
        assert_eq!(field.text(), "");
    }

    /// The phone grammar filters what may enter the model and reports the digit span.
    #[test]
    fn the_phone_grammar_filters_and_reports() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_validator(Some(InputValidator::Phone));
        // Letters are refused at the insertion path; the separators and digits survive.
        field.insert_str("1 (555) 123-4567 x9");
        assert_eq!(field.text(), "1 (555) 123-4567 9");
        assert!(field.is_valid(), "twelve digits is a valid phone number");

        // A programmatic write is trimmed and filtered too.
        field.set_text("  +44 20 7946 0958  ");
        assert_eq!(field.text(), "+44 20 7946 0958");
        assert!(field.is_valid());

        // Too few digits is reported, with the message in the error decoration.
        field.set_text("123");
        assert!(!field.is_valid());
        assert!(!field.validation_message().is_empty());
        assert_eq!(field.error_text(), field.validation_message());

        // Nothing typed is not an error, and the validator's message is cleared.
        field.set_text("");
        assert!(field.is_valid());
        assert_eq!(field.error_text(), "");
    }

    /// The e-mail grammar refuses whitespace and checks the `local@domain.tld` shape.
    #[test]
    fn the_email_grammar_validates_shape() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_validator(Some(InputValidator::Email));
        field.insert_str("name @domain");
        assert_eq!(field.text(), "name@domain", "whitespace never enters the model");
        assert!(!field.is_valid(), "a domain without a dot is not an address");

        field.set_text("name@domain.com");
        assert!(field.is_valid());
        assert_eq!(field.validation_message(), "");

        // One '@' and non-empty labels are part of the rule.
        for invalid in ["@domain.com", "name@.com", "name@domain.", "a@b@c.com"] {
            field.set_text(invalid);
            assert!(!field.is_valid(), "{invalid} must be rejected");
        }
    }

    /// A caller's own refusal message survives a value that satisfies the grammar.
    #[test]
    fn the_validator_only_clears_its_own_error() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_validator(Some(InputValidator::Email));
        field.set_text("not-an-address");
        assert_eq!(field.error_text(), field.validation_message());

        field.set_text("name@domain.com");
        assert_eq!(field.error_text(), "", "the validator clears the message it wrote");

        field.set_error_text("This address is already registered");
        assert!(field.is_valid());
        assert_eq!(
            field.error_text(),
            "This address is already registered",
            "a caller's refusal must not be cleared by the field's own grammar"
        );
    }

    /// A host rule is consulted through the trait, and its three-way state is published.
    ///
    /// # What this pins
    ///
    /// The field used to hold a closed enum of two grammars, so a caller with its own domain
    /// rule had no way to install it. It now holds `Box<dyn Validator>`, and the state is
    /// three-way: an empty value is *intermediate* — not yet acceptable, but not an error
    /// either — so a form can keep submit disabled without painting the field red.
    #[test]
    fn a_host_rule_can_be_installed_behind_the_trait() {
        #[derive(Debug)]
        struct DigitsOnly;

        impl Validator for DigitsOnly {
            fn state(&self, text: &str) -> ValidationState {
                if text.is_empty() {
                    ValidationState::Intermediate
                } else if text.chars().all(|ch| ch.is_ascii_digit()) {
                    ValidationState::Acceptable
                } else {
                    ValidationState::Invalid
                }
            }

            fn message(&self) -> &'static str {
                "Enter digits only"
            }

            fn filter(&self, text: &str) -> String {
                text.chars().filter(|ch| ch.is_ascii_digit()).collect()
            }

            fn fixup(&self, text: &str) -> Option<String> {
                text.is_empty().then(|| "0".to_string())
            }
        }

        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_dyn_validator(Some(Box::new(DigitsOnly)));
        assert_eq!(field.validation_state(), ValidationState::Intermediate);
        assert!(!field.is_valid(), "intermediate is not acceptable for submission");
        assert_eq!(field.error_text(), "", "an incomplete value is not an accusation");
        assert_eq!(
            field.get("validation_state").unwrap(),
            CapabilityValue::String("intermediate".to_string())
        );

        // The rule's own filter runs on the insertion path.
        field.insert_str("a1b2");
        assert_eq!(field.text(), "12");
        assert_eq!(field.validation_state(), ValidationState::Acceptable);
        assert!(field.is_valid());
        assert_eq!(
            field.get("validation_state").unwrap(),
            CapabilityValue::String("acceptable".to_string())
        );
        assert_eq!(
            field.set("validation_state", CapabilityValue::String("invalid".to_string())),
            Err(CapabilityAccessError::ReadOnlyProperty),
            "the state follows the value and cannot be assigned"
        );

        // Removing the rule returns the field to its free-form, always-acceptable behaviour.
        field.set_dyn_validator(None);
        assert_eq!(field.validation_state(), ValidationState::Acceptable);
        assert!(field.is_valid());
    }

    /// I-029: the shared bubble is the field's message carrier, kept in step with the rule.
    #[test]
    fn the_validation_bubble_follows_the_rule_outcome() {
        #[derive(Debug)]
        struct RefusesEverything;

        impl Validator for RefusesEverything {
            fn state(&self, text: &str) -> ValidationState {
                if text.is_empty() {
                    ValidationState::Intermediate
                } else {
                    ValidationState::Invalid
                }
            }

            fn message(&self) -> &'static str {
                "This value is refused"
            }
        }

        let mut field = LineEdit::new(Rect::new(10, 20, 200, 24));
        field.set_dyn_validator(Some(Box::new(RefusesEverything)));
        assert!(!field.validation_bubble().is_visible(), "empty is not an accusation");

        field.insert_str("x");
        assert!(field.validation_bubble().is_visible());
        assert_eq!(field.validation_bubble().message(), "This value is refused");
        assert_eq!(field.validation_bubble().severity(), ValidationSeverity::Error);
        assert_eq!(field.validation_bubble().anchor(), Some(Rect::new(10, 20, 200, 24)));

        field.set_text("");
        assert!(!field.validation_bubble().is_visible(), "an empty value clears the bubble");
    }

    /// A repairable value is fixed up through the rule's own nearest form.
    #[test]
    fn a_repairable_value_can_be_fixed_up() {
        #[derive(Debug)]
        struct DomainSuffix;

        impl Validator for DomainSuffix {
            fn state(&self, text: &str) -> ValidationState {
                if text.ends_with("@example.com") {
                    ValidationState::Acceptable
                } else if text.contains('@') {
                    ValidationState::Invalid
                } else {
                    ValidationState::Intermediate
                }
            }

            fn message(&self) -> &'static str {
                "Enter a name@example.com address"
            }

            fn fixup(&self, text: &str) -> Option<String> {
                let name = text.trim();
                if name.is_empty() || name.contains('@') {
                    None
                } else {
                    Some(format!("{name}@example.com"))
                }
            }
        }

        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        field.set_dyn_validator(Some(Box::new(DomainSuffix)));
        field.insert_str("ada");
        assert_eq!(field.validation_state(), ValidationState::Intermediate);
        assert!(field.fixup(), "the rule completes the name");
        assert_eq!(field.text(), "ada@example.com");
        assert!(field.is_valid());
        assert!(!field.fixup(), "a value already in its nearest form is not rewritten");
        assert!(field.undo(), "the repair goes through the undoable path");
        assert_eq!(field.text(), "ada");
    }

    /// A user edit marks the field dirty; a programmatic write clears the flag.
    #[test]
    fn the_modified_flag_tracks_user_edits_only() {
        let mut field = LineEdit::new(Rect::new(0, 0, 200, 24));
        assert!(!field.is_modified());
        field.set_text("saved");
        assert!(!field.is_modified(), "a programmatic write is the saved state");
        field.handle_event(&Event::TextInput { text: "!".to_string() });
        assert!(field.is_modified(), "typing must mark the value dirty");
        field.set_text("saved");
        assert!(!field.is_modified(), "rewriting the value clears the flag");
    }

    /// The clear button is opt-in, and a press on it erases the value.
    #[test]
    fn the_optional_clear_button_erases_on_press() {
        let rect = Rect::new(0, 0, 200, 48);
        let mut plain = LineEdit::new(rect);
        plain.set_text("gone");
        let mut button = LineEdit::new(rect);
        button.set_clear_button_enabled(true);
        button.set_text("gone");
        assert_ne!(
            render(&mut plain, rect),
            render(&mut button, rect),
            "enabling the button must paint it"
        );
        let hit = button.clear_button_rect();
        button.handle_event(&Event::MousePress {
            pos: Point::new(hit.x + 2, hit.y + 2),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(button.text(), "", "a press on the clear button empties the field");
    }

    /// Password echo on edit reveals the value only while the field owns focus.
    #[test]
    fn password_echo_on_edit_reveals_only_while_focused() {
        let rect = Rect::new(0, 0, 200, 48);
        let mut field = LineEdit::new(rect);
        field.set_echo_mode(EchoMode::Password);
        field.set_password_echo_on_edit(true);
        field.set_text("secret");
        let masked = render(&mut field, rect);
        field.set_focused(true);
        let revealed = render(&mut field, rect);
        assert_ne!(masked, revealed, "focus must reveal the value");

        let mut plain = LineEdit::new(rect);
        plain.set_echo_mode(EchoMode::Password);
        plain.set_text("secret");
        assert_eq!(
            masked,
            render(&mut plain, rect),
            "a blurred echo-on-edit field paints the same mask as a plain password field"
        );
    }

    /// A double click selects the word the pointer landed on.
    #[test]
    fn a_double_click_selects_the_word() {
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 48));
        field.set_text("alpha beta");
        // `alpha` occupies the leading cells; a press inside it selects that word.
        field.handle_event(&Event::MouseDoubleClick { pos: Point::new(3, 10), button: 1 });
        assert_eq!(field.selected_text(), "alpha");
    }

    /// A selection paints a band behind the value.
    #[test]
    fn a_selection_paints_a_band() {
        let rect = Rect::new(0, 0, 200, 48);
        let mut plain = LineEdit::new(rect);
        plain.set_text("selected");
        let mut selected = LineEdit::new(rect);
        selected.set_text("selected");
        selected.select_all();
        assert_ne!(
            render(&mut plain, rect),
            render(&mut selected, rect),
            "a selection must change the pixels, or the range is invisible"
        );
    }

    /// A value wider than the field scrolls so the caret stays visible.
    #[test]
    fn a_long_value_scrolls_to_reveal_the_caret() {
        let mut field = LineEdit::new(Rect::new(0, 0, 90, 48));
        field.set_text("a very long value that cannot fit");
        assert_eq!(field.cursor_position(), field.text().len());
        assert!(field.scroll_x() > 0, "the caret at the end needs the value scrolled");
        field.set_cursor_position(0);
        assert_eq!(field.scroll_x(), 0, "the start of the value needs no scroll");
    }

    /// Primary+Left/Right step by word instead of by character.
    #[test]
    fn primary_arrows_step_by_word() {
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 24));
        field.set_text("one two three");
        let end = field.text().len();
        field.handle_event(&Event::KeyPress { key: 37, modifiers: 0b1000 });
        assert_eq!(field.cursor_position(), 8, "the caret moves to the start of `three`");
        field.handle_event(&Event::KeyPress { key: 39, modifiers: 0b1000 });
        assert_eq!(field.cursor_position(), end, "and back to the end");
    }

    /// A selected value is exported as a `text/plain` drag payload.
    #[test]
    fn a_selection_is_a_drag_payload() {
        use crate::event::dnd::DragSource;
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 48));
        field.set_text("dragged");
        field.select_all();
        let payload =
            field.drag_payload(Point::new(10, 10)).expect("a selected value must be draggable");
        assert_eq!(payload.label, "dragged");
        assert!(payload.is_type("text/plain"));
    }

    /// Keyboard focus draws the focus ring; a pointer press does not.
    #[test]
    fn keyboard_focus_draws_the_focus_ring() {
        let rect = Rect::new(0, 0, 200, 48);
        let mut blurred = LineEdit::new(rect);
        blurred.set_text("x");
        let mut focused = LineEdit::new(rect);
        focused.set_text("x");
        focused.handle_event(&Event::FocusGained { reason: FocusReason::Tab });
        assert!(focused.base().draws_focus_ring());
        assert_ne!(render(&mut blurred, rect), render(&mut focused, rect));
    }

    /// The selection and caret signals report a pointer selection once per real change.
    #[test]
    fn selection_and_caret_signals_report_moves() {
        use crate::compat::Vec;
        use std::sync::{Arc, Mutex};
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 48));
        field.set_text("abcdef");
        field.set_cursor_position(0);
        let carets = Arc::new(Mutex::new(Vec::<usize>::new()));
        let carets_clone = carets.clone();
        field.cursor_position_changed.connect(move |value| {
            carets_clone.lock().unwrap().push(*value);
        });
        let selections = Arc::new(Mutex::new(0usize));
        let selections_clone = selections.clone();
        field.selection_changed.connect(move || {
            *selections_clone.lock().unwrap() += 1;
        });
        field.select_all();
        assert!(!carets.lock().unwrap().is_empty(), "the caret moved to the end");
        assert_eq!(*selections.lock().unwrap(), 1);
    }

    /// The input mask reuses the masked-edit grammar and keeps the value formatted (I-011).
    #[test]
    fn an_input_mask_formats_the_value() {
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 24));
        field.set_input_mask(Some("000-000"));
        assert_eq!(field.input_mask(), Some("000-000"));
        field.insert_str("1234567");
        assert_eq!(field.text(), "123-456", "the mask keeps six digits and inserts the separator");
        // The formatted value is what an unmasked programmatic write is sanitised into.
        field.set_text("7654321");
        assert_eq!(field.text(), "765-432");

        field.set_input_mask(None);
        assert_eq!(field.input_mask(), None, "removing the mask restores free-form editing");
        assert_eq!(field.text(), "765-432", "the current value is left alone");
    }

    /// The completer slot offers the matching words and accepting one is undoable (I-011).
    #[test]
    fn the_completer_offers_and_accepts_candidates() {
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 24));
        field.set_completer(vec!["hello".to_string(), "help".to_string(), "world".to_string()]);
        field.insert_str("hel");
        let matches = field.completion_candidates();
        assert_eq!(matches, vec!["hello".to_string(), "help".to_string()]);
        assert!(field.accept_completion("hello"));
        assert_eq!(field.text(), "hello");
        assert!(field.undo());
        assert_eq!(field.text(), "hel", "accepting a completion is one undoable edit");
    }

    /// The action slot carries descriptors and reports activation by id (I-011).
    #[test]
    fn the_action_slot_reports_activation() {
        use std::sync::{Arc, Mutex};
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 24));
        field.add_action(LineEditAction::new("clear", "Clear"));
        assert_eq!(field.actions().len(), 1);
        let seen = Arc::new(Mutex::new(None));
        let seen_clone = seen.clone();
        field.action_triggered.connect(move |id| {
            *seen_clone.lock().unwrap() = Some(id.to_string());
        });
        assert!(field.activate_action("clear"));
        assert_eq!(seen.lock().unwrap().as_deref(), Some("clear"));
        assert!(!field.activate_action("missing"), "an unknown action is refused");
    }

    /// `Ctrl+Right` stops at punctuation, not only at whitespace (I-012).
    #[test]
    fn word_navigation_stops_at_punctuation() {
        let field = LineEdit::new(Rect::new(0, 0, 240, 24));
        let mut field = field;
        field.set_text("foo,bar");
        assert_eq!(field.next_word_boundary(0), 3, "the comma ends the first word");
        assert_eq!(field.previous_word_boundary(7), 4, "the second word starts after the comma");
    }

    /// The password mask is rebuilt only when the character count changes (I-003).
    #[test]
    fn the_password_mask_is_cached_by_length() {
        let mut field = LineEdit::new(Rect::new(0, 0, 240, 24));
        field.set_text("abcd");
        field.set_echo_mode(EchoMode::Password);
        let _ = render(&mut field, Rect::new(0, 0, 240, 24));
        let first = field.mask_cache.borrow().clone();
        let _ = render(&mut field, Rect::new(0, 0, 240, 24));
        assert_eq!(field.mask_cache.borrow().0, 4, "the cache is keyed by the character count");
        assert_eq!(first.1, "****");
    }
}
