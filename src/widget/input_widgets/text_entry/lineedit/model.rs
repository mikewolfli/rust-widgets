// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The text, selection, caret and undo model of `LineEdit`, plus its editing operations.

use crate::compat::{Box, Rc, RefCell, String, ToString, Vec};
use crate::core::{Font, Point, Rect};
use crate::platform::EchoMode;
use crate::signal::{GenericSignal, Signal1};
use crate::undo::{TextSpliceCommand, UndoStack};

use crate::widget::capability::types::CapabilityValue;
use crate::widget::decorations::{DecorationLayout, DecorationSlots};
use crate::widget::input_widgets::input_validator::{ValidationState, Validator};
use crate::widget::input_widgets::mask_engine::MaskEngine;
use crate::widget::input_widgets::validation_bubble::ValidationBubble;
use crate::widget::metrics::estimate_text_width;
use crate::widget::text_utils::{byte_index_of_char, floor_char_boundary};
use crate::widget::{BaseWidget, WidgetKind};

/// Trailing room a text field leaves for the caret, added to the measured value.
pub(super) const LINE_EDIT_CARET_ROOM: u32 = 10;
/// Narrowest a text field may be, so an empty one is still a field.
pub(super) const LINE_EDIT_MIN_WIDTH: u32 = 80;
/// Height of a text field.
pub(super) const LINE_EDIT_HEIGHT: u32 = 24;
/// How long a click counts toward a multi-click streak, in milliseconds.
///
/// Matches the platform convention for double- and triple-click detection: two presses
/// inside this window are one gesture, and a third makes it a line selection.
pub(super) const MULTI_CLICK_WINDOW_MS: u32 = 400;
/// Width of the opt-in clear button, in logical pixels.
pub(super) const CLEAR_BUTTON_SIZE: u32 = 16;

/// One host action a [`LineEdit`] publishes for its context menu (I-011).
///
/// The field does not own a menu; it carries the descriptors a host builds one from, which is the
/// same division the `context_menu_requested` signal already uses for cut/copy/paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEditAction {
    /// Stable identifier the host uses to route a chosen item.
    pub id: String,
    /// Text the menu shows.
    pub label: String,
}

impl LineEditAction {
    /// Creates an action descriptor.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Single-line text edit widget.
pub struct LineEdit {
    pub(super) base: BaseWidget,
    pub(super) text: String,
    pub(super) placeholder_text: String,
    pub(super) max_length: Option<usize>,
    pub(super) echo_mode: EchoMode,
    pub(super) cursor_position: usize,
    pub(super) selection_start: Option<usize>,
    pub(super) read_only: bool,
    pub(super) undo_stack: UndoStack,
    pub(super) history_target: Rc<RefCell<String>>,
    /// Whether this field currently owns keyboard focus.
    ///
    /// Tracked here because the caret is only drawn for the focused field, and the
    /// library's focus router sends `FocusGained` / `FocusLost` (see
    /// `crate::widget::runtime::focus_widget`). Without it the field had no way to
    /// know, and the caret was never drawn at all.
    pub(super) focused: bool,
    /// The five non-value strings this field shows: `prefix`/`suffix` inside it, and
    /// `helper`/`error`/`counter` on the row below.
    ///
    /// Held as one record rather than five fields so a caller assembling a form from a document can
    /// replace all five in one step, and so the painted state is one value that can be compared.
    pub(super) decorations: DecorationSlots,
    /// The caret's blink state, advanced by [`LineEdit::tick`].
    ///
    /// Borrowed from [`crate::style::CursorBlink`] rather than reimplemented, so this field's caret
    /// keeps the tempo and phase logic of every other caret in the crate.
    pub(super) cursor_blink: crate::style::CursorBlink,
    /// The rule this field enforces as text arrives, or `None` for a free-form field.
    ///
    /// # Why this is a boxed trait object
    ///
    /// A host that owns a domain type — an order code, a range, a product identifier — has a
    /// rule the library cannot enumerate. The field therefore holds the extension point itself
    /// rather than a closed enum, and the two shipped grammars implement the same trait. See
    /// [`Validator`].
    pub(super) validator: Option<Box<dyn Validator>>,
    /// The current three-way outcome, recomputed whenever the value or the rule changes.
    ///
    /// Cached rather than derived on every read so a form that asks `is_valid()` while it
    /// builds a frame does not re-run the rule per call.
    pub(super) validation_state: ValidationState,
    /// The message for the current invalid value, or `""` when it is valid.
    pub(super) validation_message: &'static str,
    /// The validator's value-specific message for the current value, when it supplied one.
    ///
    /// [`Validator::message`] is a static sentence; [`Validator::message_for`] may name the bound
    /// and the refused input. Held apart from `validation_message` so the static fallback stays
    /// allocation-free and `validation_message()` keeps its published meaning (I-27).
    pub(super) validation_message_detail: Option<String>,
    /// Whether the current `error` decoration was written by the validator.
    ///
    /// Only an error the validator set is cleared by it: a caller's own refusal message must
    /// not disappear because the text happened to become valid for the field's grammar.
    pub(super) validation_error_active: bool,
    /// The text revision the validator last ran against (I-008).
    ///
    /// The rule is a function of the **whole** value — an anchored pattern or an `@`-check
    /// cannot be decided from the changed interval alone — so it cannot be validated
    /// incrementally in general. It can be memoised per value, which is what this records: a
    /// second `revalidate` for an unchanged revision (a theme or decoration write) reuses the
    /// result instead of re-running the rule.
    pub(super) validation_revision: Option<u64>,
    /// The field-anchored message bubble kept in step with [`Self::validation_state`].
    ///
    /// The companion is the shared presenter (COMP-B02); the host mounts it beside the field and
    /// this control writes the message and anchor into it whenever the rule changes, so the
    /// bubble is not a second hand-drawn error row (I-029).
    pub(super) validation_bubble: ValidationBubble,
    /// The uncommitted input-method composition, or `None` when no IME session is active.
    /// # Why the preedit is a field of the *field*, and not an overlay widget
    ///
    /// [`crate::widget::input_widgets::ime_preedit::ImePreedit`] draws a composition string with an
    /// underline, and that is the right thing to draw — but on its own it is an *overlay*: the string
    /// it shows never reaches any text model, so committing it had nowhere to go and the "preedit"
    /// could not be edited, cancelled or committed. That is BLUE24 §12 U-5's gap exactly
    /// ("how composition strings enter the text model").
    ///
    /// The three methods a real IME needs are [`LineEdit::set_composition`],
    /// [`LineEdit::commit_composition`] and [`LineEdit::cancel_composition`], and they are correct
    /// **because** the composition is held apart from `text`: while a session is active the field's
    /// value must not change (a `text_changed` per keystroke would fire a form's validation on a word
    /// the user has not finished typing), and cancelling must restore the text exactly as it was.
    pub(super) composition: Option<String>,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the value is centred vertically in the field band as a matter of the
    /// field's own layout, so a `top`/`bottom` value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left, so a caller that never asks
    /// behaves exactly as it did.
    pub(super) alignment: crate::core::Alignment,
    /// Whether a pointer drag is currently extending a selection.
    ///
    /// Set by [`LineEdit::press_at`] and cleared by [`LineEdit::end_drag`]. It exists so a
    /// `MouseMove` that is *not* part of a gesture — a bare hover, which arrives on every
    /// pointer step over the field — cannot move a caret the user never grabbed.
    pub(super) dragging_selection: bool,
    /// Emitted after the widget's text changes: on edit commits, and after an
    /// undo/redo restores a snapshot. Not emitted when a programmatic
    /// `set_text` is given the text the field already holds.
    pub text_changed: Signal1<String>,
    /// Emitted when editing ends — the field loses focus or Enter is pressed.
    /// Carries no payload.
    pub editing_finished: GenericSignal,
    /// Emitted when Enter is pressed in the field, after the edit is committed.
    pub return_pressed: GenericSignal,
    /// Emitted when a context menu is requested at this point in the field.
    ///
    /// The field deliberately does not build a menu: cut/copy/paste items belong to the
    /// host's menu implementation, and the same signal serves a right-click and the
    /// Menu/Application key. The payload is the request position in window coordinates;
    /// the host decides which items apply (a read-only field, for example, offers copy
    /// only) and supplies the menu itself.
    pub context_menu_requested: Signal1<Point>,
    /// Emitted when the selected range changes.
    ///
    /// Carries no payload because the range is read back through
    /// [`Self::selection_range`], which is also what a keyboard or pointer extension updates.
    pub selection_changed: GenericSignal,
    /// Emitted with the new caret byte offset whenever it moves.
    pub cursor_position_changed: Signal1<usize>,
    /// Emitted when the value changes through a *user* edit path.
    ///
    /// [`Self::text_changed`] fires for every mutation, including a programmatic
    /// [`Self::set_text`]; this signal is the user-only half, so a host can tell a keystroke,
    /// paste, cut, drop or undo apart from a field refresh. Programmatic writes must not make a
    /// form look edited, which is exactly what the undifferentiated signal could not express.
    pub text_edited: Signal1<String>,
    /// Emitted with the action id when a host activates one of [`Self::actions`] (I-011).
    pub action_triggered: Signal1<String>,
    /// Emitted with the new value whenever undo availability changes.
    pub undo_available: Signal1<bool>,
    /// Emitted with the new value whenever redo availability changes.
    pub redo_available: Signal1<bool>,
    /// Emitted with the new value whenever copying the selection becomes possible or impossible.
    pub copy_available: Signal1<bool>,
    /// Whether a pointer drag may carry the selection out of the field.
    ///
    /// **On** by default, which is what every field did before the switch existed and what
    /// [`Self::set_drag_enabled`] documents: a press and drag inside the field selects, and a drag
    /// that leaves the box starts moving the selected text. The field, the constructor and the
    /// setter all say the same thing (I-03).
    pub(super) drag_enabled: bool,
    /// Last availability values announced, so each signal is edge-triggered rather than emitted
    /// on every caret move that left it unchanged.
    pub(super) undo_available_announced: bool,
    pub(super) redo_available_announced: bool,
    pub(super) copy_available_announced: bool,
    /// Whether the value has been edited since the last programmatic [`Self::set_text`].
    ///
    /// A form can use this to warn about unsaved changes without comparing snapshots.
    pub(super) modified: bool,
    /// Whether a clear button is painted at the trailing edge when the field holds text.
    ///
    /// Opt-in because it is a visible change to every field that turns it on; the default
    /// keeps a plain field byte for byte.
    pub(super) clear_button_enabled: bool,
    /// Whether [`EchoMode::Password`] reveals the value while the field is focused.
    ///
    /// The canonical echo enum is shared with the platform layer, so the fourth echo mode is
    /// expressed as a field-level modifier instead of a new enum variant that every backend
    /// would have to learn.
    pub(super) password_echo_on_edit: bool,
    /// Horizontal scroll offset in logical pixels, for a value wider than the field.
    ///
    /// Zero unless the caret or the pointer has driven the value past the field's trailing
    /// edge, so a value that fits paints exactly where it always did.
    pub(super) scroll_x: i32,
    /// Clock accumulated from [`Self::tick`], used to measure click streaks.
    pub(super) clock_ms: u32,
    /// Number of presses in the current multi-click streak.
    pub(super) press_streak: u8,
    /// Clock reading of the most recent press, in milliseconds.
    pub(super) last_press_ms: u32,
    /// Bumped whenever an input of the painted string or of its advances changes.
    ///
    /// The value's text, the echo mode, the placeholder and the focus flag all decide what
    /// `draw` paints, so each of them advances this revision. The advance cache keys on it, which
    /// is what makes its entries unreachable as soon as the string they measured no longer exists.
    pub(super) text_revision: u64,
    /// Memoised advances of the painted value, the selection edges and the caret prefix.
    ///
    /// A frame used to shape the same string up to five times: the whole value for its origin,
    /// the value again for the caret, one prefix per selection edge, and the caret's prefix. The
    /// advances are pure functions of the text, the font, the echo mode and the DPI scale, so the
    /// cache reuses them until one of those inputs changes.
    pub(super) text_advances: RefCell<TextAdvanceCache>,
    /// Memoised password mask, keyed by the value's **character count**.
    ///
    /// The mask is one `*` per character, so it changes only when the value's length does — but
    /// the paint path rebuilt the `String` every frame. A hit reuses the previous string (I-003).
    pub(super) mask_cache: RefCell<(usize, String)>,
    /// Cumulative advances at every character boundary of the field's value.
    ///
    /// The **hit test** and the **scroll bounds** read this table, which is measured with the same
    /// context-free ruler ([`estimate_text_width`]) for both. Before it existed, a press built a
    /// zero-sized paint backend and shaped the whole value (I-P03), while `ensure_caret_visible`
    /// estimated with a different ruler than `draw` measured with (I-P13) — two rulers that drifted
    /// apart and made the scroll boundary jitter.
    pub(super) prefix_advances: RefCell<PrefixAdvances>,
    /// Memoised decoration layout, keyed by the field box, text revision, DPI and font.
    ///
    /// `draw` asks for the layout once per frame, and every frame used to re-measure the prefix,
    /// the suffix and the counter. Issue I-P13.
    pub(super) layout_cache: RefCell<Option<LayoutCacheEntry>>,
    /// Memoised value-derived readings: the character count and the usage counter text.
    pub(super) value_readings: RefCell<ValueReadings>,
    /// The reactive model value this field consumes through the property bridge.
    ///
    /// Present only after [`LineEdit::bind_text`]. The reverse direction is explicit
    /// ([`LineEdit::sync_to_binding`]) because the setter path runs inside the widget borrow, so
    /// writing to the model there would re-enter the runtime.
    pub(super) binding: Option<crate::data_binding::Binding<CapabilityValue>>,
    /// The opt-in input mask, parsed by the masked-edit engine (I-011).
    pub(super) input_mask_text: Option<String>,
    pub(super) mask_engine: Option<MaskEngine>,
    /// Candidates the completer slot offers (I-011); empty means no completer.
    pub(super) completion_source: Vec<String>,
    /// Action descriptors a host may surface in the field's context menu (I-011).
    pub(super) actions: Vec<LineEditAction>,
}

/// Which of the field's two strings an advance is measured from.
///
/// The caret measures a prefix of the field's **value** while the selection band measures a
/// prefix of what is **painted**; the two differ whenever the echo mode masks the value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AdvanceSource {
    /// The string `draw` paints, after echo mode and placeholder are applied.
    Display,
    /// The field's own value.
    Value,
}

/// Where a value mutation came from, which decides what happens to the undo history.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EditOrigin {
    /// A programmatic write: the field is the "saved" state, so no undo entry is recorded and
    /// the history that described the previous value is discarded.
    Programmatic,
    /// A user edit: the previous value is recorded so it can be undone.
    User,
}

/// Cumulative advances at every character boundary of the field's value (I-P03).
///
/// Keyed by the same `(revision, font)` pair the paint-time cache uses, so an edit, an echo-mode
/// change or a new font rebuilds the table exactly once. `scale` is fixed at `1.0`, which is the
/// ruler the hit test and the scroll bounds already used — the value's own box is in logical
/// pixels.
#[derive(Default)]
pub(super) struct PrefixAdvances {
    pub(super) revision: u64,
    pub(super) font: Option<Font>,
    /// `(byte offset, cumulative advance)` for every character boundary, from `0` to `len`.
    pub(super) cuts: Vec<(usize, i32)>,
}

impl PrefixAdvances {
    /// Rebuilds the table for `text`, measuring each cluster with the shared estimator.
    pub(super) fn rebuild(&mut self, revision: u64, font: &Font, text: &str) {
        self.revision = revision;
        self.font = Some(font.clone());
        self.cuts.clear();
        self.cuts.push((0, 0));
        let mut advance = 0i32;
        for (start, ch) in text.char_indices() {
            let end = start + ch.len_utf8();
            advance =
                advance.saturating_add(estimate_text_width(&text[start..end], font, 1.0) as i32);
            self.cuts.push((end, advance));
        }
    }

    /// The total advance of the value.
    pub(super) fn total(&self) -> i32 {
        self.cuts.last().map(|(_, advance)| *advance).unwrap_or(0)
    }

    /// The cumulative advance at `byte`, or `None` when `byte` is not a boundary of the value.
    pub(super) fn advance_at(&self, byte: usize) -> Option<i32> {
        let index = self.cuts.partition_point(|(offset, _)| *offset < byte);
        match self.cuts.get(index) {
            Some((offset, advance)) if *offset == byte => Some(*advance),
            _ => None,
        }
    }

    /// The boundary index whose cumulative advance is nearest `x` (I-004).
    ///
    /// The hit test used to place the caret at `x * chars / total`, which assumes every character
    /// has the same advance. On a proportional face or a CJK run that put the caret a character
    /// or more away from the glyph under the pointer. The table already holds a measured advance
    /// per boundary, so the nearest one is a binary search over those real numbers.
    pub(super) fn boundary_at_advance(&self, x: i32) -> usize {
        if self.cuts.is_empty() {
            return 0;
        }
        let index = self.cuts.partition_point(|(_, advance)| *advance < x);
        if index == 0 {
            return 0;
        }
        if index >= self.cuts.len() {
            return self.cuts.len() - 1;
        }
        let before = self.cuts[index - 1].1;
        let after = self.cuts[index].1;
        if (x - before).abs() <= (after - x).abs() {
            index - 1
        } else {
            index
        }
    }

    /// The byte offset of the boundary that follows `count` characters.
    pub(super) fn byte_at(&self, count: usize) -> usize {
        self.cuts.get(count).map(|(offset, _)| *offset).unwrap_or(0)
    }
}

/// The memoised decoration layout and the inputs it was derived from (I-P13).
pub(super) struct LayoutCacheEntry {
    pub(super) field: Rect,
    pub(super) revision: u64,
    pub(super) dpi_bits: u32,
    pub(super) font: Font,
    pub(super) layout: DecorationLayout,
}

/// The character count and usage counter for the current value, memoised per revision (I-P13).
#[derive(Default)]
pub(super) struct ValueReadings {
    pub(super) revision: u64,
    pub(super) limit: Option<usize>,
    pub(super) count: usize,
    pub(super) counter: Option<String>,
}

/// Memoised advances for the value, the selection edges and the caret prefix.
#[derive(Default)]
pub(super) struct TextAdvanceCache {
    pub(super) revision: u64,
    pub(super) font: Option<Font>,
    pub(super) echo_mode: Option<EchoMode>,
    pub(super) dpi_bits: u32,
    /// `(from, to, advance)` for slices of the painted value.
    pub(super) display: Vec<(usize, usize, i32)>,
    /// `(from, to, advance)` for slices of the field's own value.
    pub(super) value: Vec<(usize, usize, i32)>,
}

impl LineEdit {
    /// Creates an empty line edit with geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::LineEdit, geometry, "LineEdit"),
            text: String::new(),
            placeholder_text: String::new(),
            max_length: None,
            echo_mode: EchoMode::Normal,
            cursor_position: 0,
            selection_start: None,
            read_only: false,
            validator: None,
            validation_state: ValidationState::Acceptable,
            validation_message: "",
            validation_message_detail: None,
            validation_error_active: false,
            validation_revision: None,
            validation_bubble: ValidationBubble::new(Rect::new(0, 0, 0, 0)),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            focused: false,
            decorations: DecorationSlots::default(),
            cursor_blink: crate::style::CursorBlink::new(),
            composition: None,
            alignment: crate::core::Alignment::Left,
            dragging_selection: false,
            text_changed: Signal1::new(),
            editing_finished: GenericSignal::new(),
            return_pressed: GenericSignal::new(),
            context_menu_requested: Signal1::new(),
            selection_changed: GenericSignal::new(),
            cursor_position_changed: Signal1::new(),
            text_edited: Signal1::new(),
            action_triggered: Signal1::new(),
            undo_available: Signal1::new(),
            redo_available: Signal1::new(),
            copy_available: Signal1::new(),
            drag_enabled: true,
            undo_available_announced: false,
            redo_available_announced: false,
            copy_available_announced: false,
            modified: false,
            clear_button_enabled: false,
            password_echo_on_edit: false,
            scroll_x: 0,
            clock_ms: 0,
            press_streak: 0,
            text_revision: 0,
            text_advances: RefCell::new(TextAdvanceCache::default()),
            mask_cache: RefCell::new((0, String::new())),
            prefix_advances: RefCell::new(PrefixAdvances::default()),
            layout_cache: RefCell::new(None),
            value_readings: RefCell::new(ValueReadings::default()),
            last_press_ms: 0,
            binding: None,
            input_mask_text: None,
            mask_engine: None,
            completion_source: Vec::new(),
            actions: Vec::new(),
        }
    }

    /// Binds this field's `text` property to a reactive model value.
    ///
    /// The initial model value is applied immediately, and (when the field is mounted) later
    /// model changes are written through the property contract. Both the slider and the field are
    /// the two low-risk controls COMP-26 names as the first consumers of the bridge.
    pub fn bind_text(&mut self, binding: &crate::data_binding::Binding<CapabilityValue>) {
        self.binding = Some(binding.clone());
        #[cfg(not(alloc_frugal))]
        {
            if let Some(id) = crate::widget::runtime::registry_id_of(self.base.id()) {
                crate::data_binding::bind_widget_property(binding, "line_edit.text", id, "text");
                return;
            }
        }
        if let CapabilityValue::String(value) = binding.get() {
            self.set_text(value);
            binding.announce_synced("text");
        }
    }

    /// Pushes this field's current text into the bound model, reporting whether one is bound.
    pub fn sync_to_binding(&mut self) -> bool {
        let Some(binding) = self.binding.clone() else {
            return false;
        };
        binding.set(CapabilityValue::String(self.text.clone()));
        binding.announce_synced("text");
        true
    }

    /// The reactive value this field consumes, if it is bound.
    pub fn bound_text(&self) -> Option<&crate::data_binding::Binding<CapabilityValue>> {
        self.binding.as_ref()
    }
    /// Returns current text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl LineEdit {
    /// Returns whether this field currently has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// The mutation counter for the value.
    ///
    /// Companion controls that host a `LineEdit` and cache measurements of its text key their
    /// tables on this number, so an edit invalidates the cache without comparing the string.
    pub(crate) fn text_revision(&self) -> u64 {
        self.text_revision
    }
    /// Sets the focus flag directly.
    ///
    /// The runtime normally drives this through `FocusGained` / `FocusLost`, the same
    /// way the other input controls are driven; this setter exists for hosts and tests
    /// that need to place focus without an event round-trip.
    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.invalidate_text_advances();
        // The caret blinks exactly while the field holds focus, so the blink state is driven from
        // the same flag `draw` reads rather than from a parallel notion of "active".
        if focused {
            self.cursor_blink.start();
        } else {
            self.cursor_blink.stop();
        }
        self.base.request_redraw();
    }

    /// Advances the caret's blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The crate's `tick(delta_ms) -> bool` convention: `true` while the caret is still cycling, so
    /// a host schedules the next frame only for a field that is actually blinking. A blurred field
    /// or a read-only one returns `false` immediately — a steady caret has no next frame.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        // The clock runs even while blurred: the click streak it measures begins with the press
        // that focuses the field, and a stopped clock would make every first click look like a
        // continuation of the previous one.
        self.clock_ms = self.clock_ms.wrapping_add(delta_ms);
        if !self.focused || self.read_only {
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }
    /// Sets text and emits text_changed signal if different.
    ///
    /// This is the **programmatic** entry point: the write is the field's clean state, so it is
    /// neither recorded in the undo history nor allowed to leave a history entry that predates it
    /// (Ctrl+Z must not jump back to a value the application itself replaced). User edits go
    /// through [`Self::insert_str`], [`Self::backspace`] and [`Self::delete`], which record their
    /// previous value.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.apply_text(text.into(), EditOrigin::Programmatic);
    }

    /// Filters, clamps and commits `text`, attributing the change to `origin`.
    pub(super) fn apply_text(&mut self, text: String, origin: EditOrigin) -> bool {
        // A masked field keeps the value in its formatted form, including a programmatic write
        // (I-011): the alternative is a "masked" field that can be given an unmasked value.
        let text = match self.mask_engine.as_ref() {
            Some(engine) => engine.format(&engine.sanitize(&text)),
            None => text,
        };
        // The grammar is a property of the field, not of the typing path: a value it cannot
        // represent must not enter the model from a programmatic write either. Surrounding
        // whitespace is trimmed here (not on a keystroke, where a space is a separator the
        // user is typing *between* groups).
        let text = match self.validator.as_deref() {
            Some(validator) => validator.filter(&text).trim().to_string(),
            None => text,
        };
        // `max_length` is enforced here as well as in `insert_text`.
        //
        // # Why the two paths have to agree
        //
        // `insert_text` clamped to the limit and this did not, so a programmatic `set_text` could leave
        // the field holding a longer value than its own limit — and once the `counter` is derived from
        // the value and that limit, the two disagreed visibly (`8/5`). A limit the control enforces only
        // for typing is not a limit, it is a hint. The truncation goes through `floor_char_boundary`
        // because `max_length` counts **characters** and the slice is in bytes.
        let text = match self.max_length {
            Some(max) if text.chars().count() > max => {
                let byte = byte_index_of_char(&text, max);
                text[..byte].to_string()
            }
            _ => text,
        };
        self.commit_text(text, origin)
    }

    /// Stores `text` and publishes the consequences, honouring `origin` for history handling.
    ///
    /// Returns whether the value actually changed, so an edit path that produced the same string
    /// can avoid reporting an edit that did not happen. The previous value moves into the history
    /// record instead of being copied into a scratch string (I-P01).
    pub(super) fn commit_text(&mut self, text: String, origin: EditOrigin) -> bool {
        if self.text == text {
            return false;
        }
        let before = core::mem::replace(&mut self.text, text);
        self.finish_text_change(before, origin)
    }

    /// Publishes a value mutation the caller made **in place** on `self.text`.
    ///
    /// `before` is the value as it was before the mutation. The typing paths hand it over rather
    /// than cloning the value into a scratch buffer and assigning it back, which is what made a
    /// single keystroke allocate the whole string several times (I-P01).
    ///
    /// The grammar and the limit are still enforced here, because they are properties of the
    /// field rather than of the typing path; the allocation that needs only happens when one of
    /// them is actually installed.
    pub(super) fn commit_in_place(&mut self, before: String, origin: EditOrigin) -> bool {
        if let Some(validator) = self.validator.as_deref() {
            let filtered = validator.filter(&self.text);
            let trimmed = filtered.trim();
            if trimmed != self.text {
                self.text = trimmed.to_string();
            }
        }
        if let Some(max) = self.max_length {
            if self.text.chars().count() > max {
                let byte = byte_index_of_char(&self.text, max);
                self.text.truncate(byte);
            }
        }
        if self.text == before {
            return false;
        }
        self.finish_text_change(before, origin)
    }

    /// Whether an edit path has to keep a full before-image for its undo record.
    ///
    /// The validator and the character limit can rewrite a value beyond the interval the user
    /// changed (a filter may drop characters anywhere, the limit truncates the tail), so those
    /// two configurations still need the pre-edit string. Without them the ordinary typing path
    /// records only the splice and copies nothing proportional to the document (I-001).
    pub(super) fn needs_full_before_image(&self) -> bool {
        self.validator.is_some() || self.max_length.is_some()
    }

    /// Publishes an in-place splice performed on `self.text` as one undo step.
    ///
    /// `offset` and `removed` describe the value *before* the mutation, `inserted` what replaced
    /// it. When `before` is `Some`, the grammar/limit still run and may rewrite the value; the
    /// record then falls back to the snapshot diff, which is still splice-shaped (only the
    /// changed interval is stored).
    pub(super) fn commit_splice(
        &mut self,
        offset: usize,
        removed: &str,
        inserted: &str,
        before: Option<String>,
        origin: EditOrigin,
    ) -> bool {
        if let Some(before) = before {
            return self.commit_in_place(before, origin);
        }
        if removed.is_empty() && inserted.is_empty() {
            return false;
        }
        // The painted string changed, so every advance/readings cache keyed by the revision is
        // stale. The snapshot path does this inside `finish_text_change`; the splice path has to
        // do it here because it skips that function (I-001).
        self.invalidate_text_advances();
        self.revalidate();
        match origin {
            EditOrigin::User => {
                // Advance the undo mirror with the same splice instead of replacing it with a
                // whole-document clone, then file the matching record (I-001).
                {
                    let mut target = self.history_target.borrow_mut();
                    let start = offset.min(target.len());
                    let end = (start + removed.len()).min(target.len());
                    target.replace_range(start..end, inserted);
                }
                self.undo_stack.push(Box::new(TextSpliceCommand::from_splice(
                    self.history_target.clone(),
                    offset,
                    removed.to_string(),
                    inserted.to_string(),
                    "Edit text",
                    "line_edit_text",
                )));
            }
            EditOrigin::Programmatic => {
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.clear();
            }
        }
        self.finish_text_change_common(origin)
    }

    /// Publishes the consequences of a value that has already been written to `self.text`.
    ///
    /// `before` is the previous value, moved in from the caller so the undo record does not need
    /// a third copy of the string.
    pub(super) fn finish_text_change(&mut self, before: String, origin: EditOrigin) -> bool {
        self.invalidate_text_advances();
        self.revalidate();
        match origin {
            EditOrigin::User => {
                // One copy for the shared history target; the record itself stores only the
                // changed interval, so a validator/limit field no longer pays two full copies
                // per keystroke (I-001).
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.push(Box::new(TextSpliceCommand::new(
                    self.history_target.clone(),
                    &before,
                    &self.text,
                    "Edit text",
                    "line_edit_text",
                )));
            }
            EditOrigin::Programmatic => {
                // A programmatic write is the saved state by definition: the history that could
                // restore a value before it no longer describes this field.
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.clear();
            }
        }
        self.finish_text_change_common(origin)
    }

    /// The caret, signal and repaint half of a value mutation.
    ///
    /// Split from [`Self::finish_text_change`] so the splice path can publish a record that was
    /// already built without repeating this sequence (I-001).
    pub(super) fn finish_text_change_common(&mut self, origin: EditOrigin) -> bool {
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        self.cursor_position = self.text.len();
        self.selection_start = None;
        if origin == EditOrigin::Programmatic {
            self.modified = false;
        }
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.refresh_edit_capabilities();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
        true
    }

    /// Undo the latest text mutation.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Redo the latest undone text mutation.
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns whether a text mutation can be undone.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether a text mutation can be redone.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    pub(super) fn restore_history_text(&mut self) {
        let text = self.history_target.borrow().clone();
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        self.text = text;
        self.invalidate_text_advances();
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.modified = true;
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.refresh_edit_capabilities();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }
    /// Returns placeholder text.
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder_text
    }
    /// Sets placeholder text.
    pub fn set_placeholder_text(&mut self, text: String) {
        // A same-value write paints the same placeholder; queueing a frame for it is the
        // wheel-spinning the other text setters already avoid (WDGT-15).
        if self.placeholder_text == text {
            return;
        }
        self.placeholder_text = text;
        self.invalidate_text_advances();
        self.base.request_redraw();
    }
    /// Returns maximum text length **in characters**.
    pub fn max_length(&self) -> Option<usize> {
        self.max_length
    }
    /// Sets maximum text length, **in characters**, truncating the value if needed.
    ///
    /// # `max_length` counts characters, so this cannot truncate by byte
    ///
    /// The limit is a character budget: it is what the user typing sees and what
    /// [`Self::counter_text`] reports. An earlier revision truncated with
    /// `self.text.truncate(max)`, which is a **byte** index — so `"héllo"` (5 chars, 6 bytes)
    /// under a limit of 3 was cut to `"hé"` (2 chars) rather than `"hél"`, and a limit that
    /// happened to fall inside a multi-byte character would have split it. The truncation
    /// therefore goes through [`byte_index_of_char`], the same conversion `insert_text` and
    /// `set_text` use, so all three agree about what the limit means.
    pub fn set_max_length(&mut self, max_length: Option<usize>) {
        if self.max_length == max_length {
            return;
        }
        self.max_length = max_length;
        // The counter decoration (and therefore the layout) depends on the limit, so the memoised
        // layout and readings are invalidated even when nothing is truncated (I-P13).
        self.invalidate_text_advances();
        // Truncate if needed
        if let Some(max) = max_length {
            if self.text.chars().count() > max {
                let byte = byte_index_of_char(&self.text, max);
                self.text.truncate(byte);
                self.clamp_caret();
                // A shrinking limit makes every longer value unreachable, so the
                // history that could restore one is cleared and the shared target
                // is re-synced to the truncated value. Without this, undo/redo
                // would re-apply an over-limit string.
                *self.history_target.borrow_mut() = self.text.clone();
                self.undo_stack.clear();
                self.refresh_edit_capabilities();
                self.text_changed.emit(self.text.clone());
            }
        }
        self.base.request_redraw();
    }

    /// The current value's length **in characters**.
    ///
    /// The frame `max_length` and the counter are expressed in, so a limit test never has to
    /// compare a character budget against a byte length. Memoised per text revision, because
    /// `draw` and `semantic_state` both read it every frame (I-P13).
    pub(super) fn char_count(&self) -> usize {
        // Count-only path: `value_readings` returns the counter `String` by value, so calling it
        // here would clone the counter on every limit check even though the caller only reads the
        // count. The cached counter is still refreshed, so a later `counter_text` stays current.
        if let Ok(mut cache) = self.value_readings.try_borrow_mut() {
            if cache.revision != self.text_revision || cache.limit != self.max_length {
                let count = self.text.chars().count();
                cache.revision = self.text_revision;
                cache.limit = self.max_length;
                cache.count = count;
                cache.counter = DecorationLayout::counter_text(count, self.max_length);
            }
            return cache.count;
        }
        self.text.chars().count()
    }

    /// The character count and usage counter text for the current value, memoised per revision.
    pub(super) fn value_readings(&self) -> (usize, Option<String>) {
        if let Ok(mut cache) = self.value_readings.try_borrow_mut() {
            if cache.revision != self.text_revision || cache.limit != self.max_length {
                let count = self.text.chars().count();
                cache.revision = self.text_revision;
                cache.limit = self.max_length;
                cache.count = count;
                cache.counter = DecorationLayout::counter_text(count, self.max_length);
            }
            return (cache.count, cache.counter.clone());
        }
        let count = self.text.chars().count();
        (count, DecorationLayout::counter_text(count, self.max_length))
    }

    /// Snaps the caret and the selection anchor to a real character boundary inside the value.
    ///
    /// The caret is a **byte** offset (that is what a slice needs), while every move the user makes
    /// — Left, Right, Backspace, Delete — is a **character** step. Rounding the stored offset down to
    /// a boundary after any of those keeps `&text[..cursor]` total without making the caret's own
    /// arithmetic guess where a character begins, which is what a bare `+= 1` did: on `"é"` a single
    /// Right put the caret *inside* the character, and the next insert then snapped to the end
    /// instead of at the position the user had moved to.
    pub(super) fn clamp_caret(&mut self) {
        let len = self.text.len();
        self.cursor_position = floor_char_boundary(&self.text, self.cursor_position.min(len));
        if let Some(start) = self.selection_start {
            self.selection_start = Some(floor_char_boundary(&self.text, start.min(len)));
        }
    }
    /// Returns whether the value has been edited since the last programmatic write.
    ///
    /// Typing, pasting, cutting and deleting set this flag; [`Self::set_text`] clears it, so a
    /// host can implement a "discard unsaved changes" prompt without holding its own snapshot.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Sets the modified flag, for a host that has just saved the value.
    pub fn set_modified(&mut self, modified: bool) {
        if self.modified == modified {
            return;
        }
        self.modified = modified;
    }

    /// Whether a clear button is painted at the trailing edge of the field.
    pub fn clear_button_enabled(&self) -> bool {
        self.clear_button_enabled
    }

    /// Enables or disables the trailing clear button.
    ///
    /// The button appears only while the field holds text, and a press on it clears the value
    /// through the ordinary edit path (undoable, announced through `text_changed`). Off by
    /// default, because it changes the pixels of a field that turns it on.
    pub fn set_clear_button_enabled(&mut self, enabled: bool) {
        if self.clear_button_enabled == enabled {
            return;
        }
        self.clear_button_enabled = enabled;
        self.base.request_redraw();
    }

    /// Whether the value is revealed while the field is focused in password mode.
    pub fn password_echo_on_edit(&self) -> bool {
        self.password_echo_on_edit
    }

    /// Reveals the value while the field is focused in [`EchoMode::Password`].
    ///
    /// This is the fourth echo mode a text field needs: the value is masked while the user is
    /// away, and readable while they are typing it. It only affects [`EchoMode::Password`];
    /// [`EchoMode::NoEcho`] never paints the value at all.
    pub fn set_password_echo_on_edit(&mut self, enabled: bool) {
        if self.password_echo_on_edit == enabled {
            return;
        }
        self.password_echo_on_edit = enabled;
        self.invalidate_text_advances();
        self.base.request_redraw();
    }

    /// The horizontal scroll offset applied to the value, in logical pixels.
    pub fn scroll_x(&self) -> i32 {
        self.scroll_x
    }

    /// Sets the horizontal scroll offset, clamped to the value's own width.
    ///
    /// A value that fits the field always reports zero, so the common case never scrolls.
    pub fn set_scroll_x(&mut self, offset: i32) {
        let max = self.max_scroll_x();
        let offset = offset.clamp(0, max);
        if self.scroll_x == offset {
            return;
        }
        self.scroll_x = offset;
        self.base.request_redraw();
    }
}

impl LineEdit {
    /// Whether the field paints the plain value rather than its mask.
    ///
    /// Password echo on edit reveals only while the field owns focus; every other mode is
    /// unaffected.
    pub(super) fn reveals_value(&self) -> bool {
        self.echo_mode == EchoMode::Password && self.password_echo_on_edit && self.focused
    }

    /// Invalidates the memoised advances after an input they depend on changes.
    pub(super) fn invalidate_text_advances(&mut self) {
        self.text_revision = self.text_revision.wrapping_add(1);
    }
}

impl LineEdit {
    /// Reports a caret/selection change to the host when either actually moved.
    ///
    /// Centralised so every path that moves the caret — keyboard, pointer, IME, programmatic
    /// writes — has the same notification timing.
    pub(super) fn notify_caret_change(
        &mut self,
        before_cursor: usize,
        before_selection: Option<usize>,
    ) {
        if self.cursor_position != before_cursor {
            self.cursor_position_changed.emit(self.cursor_position);
        }
        if self.selection_start != before_selection {
            self.selection_changed.emit();
        }
        // Copyability follows the selection and the echo mode, so every caret move is a place it
        // can change; `refresh_edit_capabilities` emits only when the answer actually differs.
        self.refresh_edit_capabilities();
    }

    /// Whether the current selection can be copied to the clipboard.
    ///
    /// [`EchoMode::NoEcho`] deliberately contributes nothing (see the copy path), so a selection it
    /// hides is not a copyable one.
    pub(super) fn can_copy(&self) -> bool {
        self.echo_mode != EchoMode::NoEcho && self.selection_range().is_some()
    }

    /// Emits the three availability signals when their answers have changed.
    pub(super) fn refresh_edit_capabilities(&mut self) {
        let undo = self.can_undo();
        if undo != self.undo_available_announced {
            self.undo_available_announced = undo;
            self.undo_available.emit(undo);
        }
        let redo = self.can_redo();
        if redo != self.redo_available_announced {
            self.redo_available_announced = redo;
            self.redo_available.emit(redo);
        }
        let copy = self.can_copy();
        if copy != self.copy_available_announced {
            self.copy_available_announced = copy;
            self.copy_available.emit(copy);
        }
    }

    /// Emits [`Self::text_edited`] when a user gesture actually changed the value.
    ///
    /// The change is read from the text revision instead of comparing against a snapshot taken
    /// before the gesture: the snapshot cloned the whole value on **every** key, including the
    /// navigation keys that change nothing (I-P01).
    pub(super) fn emit_text_edited_if_revision_moved(&self, revision: u64) {
        if self.text_revision != revision {
            self.text_edited.emit(self.text.clone());
        }
    }
}

impl LineEdit {
    /// The class a character belongs to for word navigation and double-click selection.
    ///
    /// Three classes, not two: whitespace, word characters, and punctuation/symbols. The old
    /// predicate only asked `is_whitespace`, so `Ctrl+Right` on `foo,bar` treated the comma as
    /// part of the word and never stopped at it (I-012).
    pub(super) fn char_class(ch: char) -> u8 {
        if ch.is_whitespace() {
            0
        } else if ch.is_alphanumeric() || ch == '_' {
            1
        } else {
            2
        }
    }

    /// The byte range of the word around `index`, for a double-click selection.
    ///
    /// A word is a run of characters of one class (see [`Self::char_class`]); a press on
    /// whitespace or punctuation selects that run itself, which is what keeps a double-click on a
    /// gap or a separator from silently doing nothing.
    pub(super) fn word_range_at(&self, index: usize) -> (usize, usize) {
        let index = floor_char_boundary(&self.text, index.min(self.text.len()));
        // The character the press landed on; at the end of the value the class is taken from the
        // character before it, so a double-click on the trailing edge selects the last word.
        let sampled = self.text[index..]
            .chars()
            .next()
            .or_else(|| self.text[..index].chars().next_back())
            .unwrap_or(' ');
        let class = Self::char_class(sampled);
        let same_class = |ch: char| Self::char_class(ch) == class;
        let mut start = index;
        for (i, ch) in self.text[..index].char_indices().rev() {
            if !same_class(ch) {
                break;
            }
            start = i;
        }
        let mut end = index;
        for (i, ch) in self.text[index..].char_indices() {
            if !same_class(ch) {
                break;
            }
            end = index + i + ch.len_utf8();
        }
        // A press on the boundary between two words selects the word, not the empty run.
        if start == end {
            if let Some((i, ch)) = self.text[..index].char_indices().next_back() {
                return (i, i + ch.len_utf8());
            }
        }
        (start, end)
    }

    /// The byte offset of the start of the word before `from`.
    ///
    /// Skips separators, then walks back through one class of word characters, so punctuation
    /// and symbols are their own boundaries (I-012).
    pub(super) fn previous_word_boundary(&self, from: usize) -> usize {
        let mut offset = floor_char_boundary(&self.text, from.min(self.text.len()));
        while offset > 0 {
            let (index, ch) = self.text[..offset].char_indices().next_back().unwrap_or((0, ' '));
            if Self::char_class(ch) != 0 {
                break;
            }
            offset = index;
        }
        if offset == 0 {
            return 0;
        }
        let class = Self::char_class(self.text[..offset].chars().next_back().unwrap_or(' '));
        while offset > 0 {
            let (index, ch) = self.text[..offset].char_indices().next_back().unwrap_or((0, ' '));
            if Self::char_class(ch) != class {
                break;
            }
            offset = index;
        }
        offset
    }

    /// The byte offset of the start of the word after `from`.
    pub(super) fn next_word_boundary(&self, from: usize) -> usize {
        let mut offset = floor_char_boundary(&self.text, from.min(self.text.len()));
        while offset < self.text.len() {
            let ch = self.text[offset..].chars().next().unwrap_or(' ');
            if Self::char_class(ch) != 0 {
                break;
            }
            offset += ch.len_utf8();
        }
        if offset >= self.text.len() {
            return self.text.len();
        }
        let class = Self::char_class(self.text[offset..].chars().next().unwrap_or(' '));
        while offset < self.text.len() {
            let ch = self.text[offset..].chars().next().unwrap_or(' ');
            if Self::char_class(ch) != class {
                break;
            }
            offset += ch.len_utf8();
        }
        offset
    }
}

impl LineEdit {
    /// Returns echo mode.
    pub fn echo_mode(&self) -> EchoMode {
        self.echo_mode
    }
    /// Sets echo mode.
    pub fn set_echo_mode(&mut self, mode: EchoMode) {
        if self.echo_mode == mode {
            return;
        }
        self.echo_mode = mode;
        self.invalidate_text_advances();
        // NoEcho removes the copyable payload, and the other modes restore it.
        self.refresh_edit_capabilities();
        self.base.request_redraw();
    }
    /// Returns cursor position.
    pub fn cursor_position(&self) -> usize {
        self.cursor_position
    }
    /// Sets cursor position (a byte offset into the value), snapped to a character boundary.
    ///
    /// The offset is a byte index because that is what a slice needs, but a caller passing an
    /// arbitrary number must not be able to leave the caret inside a character — otherwise the next
    /// insert would silently round to the end of the value instead of landing where it was put.
    pub fn set_cursor_position(&mut self, position: usize) {
        let position = floor_char_boundary(&self.text, position.min(self.text.len()));
        if self.cursor_position == position && self.selection_start.is_none() {
            return;
        }
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        self.cursor_position = position;
        self.selection_start = None;
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.base.request_redraw();
    }
    /// Returns selection start position.
    pub fn selection_start(&self) -> Option<usize> {
        self.selection_start
    }

    /// Returns the selected byte range, if anything is selected.
    ///
    /// This is the spelling `TextArea` and `TextEdit` already publish (WDGT-14), so selection
    /// handling can move between the three fields without a per-widget adapter. The ends are
    /// snapped to character boundaries and returned low-to-high, which is the same rule
    /// [`Self::selected_text`] slices with — the two cannot disagree about what is selected.
    /// Returns `None` when the anchor is absent or collapses onto the caret, so an empty selection
    /// is not reported as a zero-length range.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_start?;
        let len = self.text.len();
        let start = floor_char_boundary(&self.text, anchor.min(len));
        let end = floor_char_boundary(&self.text, self.cursor_position.min(len));
        if start == end {
            return None;
        }
        Some(if start < end { (start, end) } else { (end, start) })
    }

    /// Selects `length` bytes starting at `start`, clamped to the value.
    ///
    /// Both ends are snapped to character boundaries and clamped to the value's length, so a
    /// caller computing a highlight from a search offset cannot leave the selection inside a
    /// character. The caret is left at the range's end, which is the convention
    /// [`Self::select_all`] and [`Self::select_with_modifiers`] already follow, so a following
    /// `Shift`-movement extends from the end the user just selected to.
    ///
    /// The unit matches [`Self::selection_range`] and [`Self::cursor_position`] — **byte**
    /// offsets — because those are the offsets this control's public surface already speaks.
    /// A zero length clears the selection instead of reporting an empty range.
    pub fn set_selection(&mut self, start: usize, length: usize) {
        let len = self.text.len();
        let start = floor_char_boundary(&self.text, start.min(len));
        let end = floor_char_boundary(&self.text, start.saturating_add(length).min(len));
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        if start == end {
            self.selection_start = None;
            self.cursor_position = start;
        } else {
            self.selection_start = Some(start);
            self.cursor_position = end;
        }
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.base.request_redraw();
    }

    /// The selected length **in bytes**, or zero when nothing is selected.
    ///
    /// The same unit as [`Self::set_selection`] and the offsets [`Self::selection_range`] returns.
    pub fn selection_length(&self) -> usize {
        self.selection_range().map(|(start, end)| end - start).unwrap_or(0)
    }

    /// Returns selected text.
    ///
    /// The two offsets are snapped to character boundaries before slicing, so a caret left inside a
    /// character (by a caller's `set`, or by an older `+= 1` step) reads the characters it actually
    /// covers instead of panicking.
    pub fn selected_text(&self) -> String {
        if let Some(start) = self.selection_start {
            let start = floor_char_boundary(&self.text, start.min(self.text.len()));
            let end = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            self.text[start..end].to_string()
        } else {
            String::new()
        }
    }
    /// Selects all text.
    pub fn select_all(&mut self) {
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        self.selection_start = Some(0);
        self.cursor_position = self.text.len();
        self.notify_caret_change(before_cursor, before_selection);
        self.base.request_redraw();
    }
    /// Clears selection.
    pub fn clear_selection(&mut self) {
        if self.selection_start.is_none() {
            return;
        }
        self.selection_start = None;
        self.selection_changed.emit();
        self.refresh_edit_capabilities();
        self.base.request_redraw();
    }

    /// Selects `target` (a byte offset) honouring the modifier keys held at the time.
    ///
    /// # What this is the single entry point for
    ///
    /// Every keyboard movement in this field — Left, Right, Home, End — and every pointer
    /// gesture funnels through here, so the anchor rule is written once. Before it existed,
    /// each of the four movements carried its own copy of the shift test, and a press could
    /// not extend at all. The two behaviours are:
    ///
    /// * **Shift** — extend: the caret moves to `target` and the *anchor* stays put, so the
    ///   selection becomes the range between them. With no anchor yet, the current caret is
    ///   adopted as the anchor first (`Home` on a fresh field then selects back to the start).
    /// * **No modifier** — replace: the caret moves and any selection is dropped.
    ///
    /// # Why Home/End must not re-anchor while extending
    ///
    /// The first cut of this helper re-anchored at the caret before moving, for every key. That
    /// made an extension **irreversible**: `Shift+Home` on a caret at 5 selected `0..=5` and left
    /// the anchor at 5, so the following `Shift+End` selected `5..=len` rather than sweeping to
    /// the end. The anchor belongs to the whole gesture — it is where the gesture *began* — so
    /// only a press with no Shift (which starts one) may move it.
    ///
    /// `target` is clamped and snapped to a character boundary, so a caller passing a byte
    /// offset computed from a pixel can never put the caret inside a character.
    pub fn select_with_modifiers(&mut self, target: usize, modifiers: crate::shortcut::Modifiers) {
        let target = floor_char_boundary(&self.text, target.min(self.text.len()));
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        if modifiers.contains(crate::shortcut::Modifiers::SHIFT) {
            // Adopt the caret as the anchor only when this extension is the *first* one; a
            // gesture already in flight keeps the anchor it started with.
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_position);
            }
            self.cursor_position = target;
        } else {
            self.selection_start = None;
            self.cursor_position = target;
        }
        self.normalize_selection();
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.base.request_redraw();
    }

    /// A primary press at `pos`: the clear button owns its corner of the field, and anywhere else
    /// the press focuses the field and places the caret / anchors the selection.
    ///
    /// Shared by the mouse and touch press arms, so a finger enters exactly the state machine a
    /// pointer does instead of a second copy of it.
    pub(super) fn press_with_pointer(&mut self, pos: Point, modifiers: u32) {
        // The clear button owns its corner of the field and consumes the press before the
        // caret can move there: a click meant to erase must not also select text.
        if self.clear_button_hit(pos) {
            let revision = self.text_revision;
            // The button is a user gesture, so its erase stays undoable even though the
            // programmatic `clear()` entry point is a pristine write (I-04).
            self.apply_text(String::new(), EditOrigin::User);
            self.modified = true;
            self.press_streak = 0;
            self.emit_text_edited_if_revision_moved(revision);
            return;
        }
        // Track a multi-click streak so a third press can select the whole value. The
        // platform reports a double click separately; the press that precedes it is what
        // tells this field the streak is still growing.
        let elapsed = self.clock_ms.wrapping_sub(self.last_press_ms);
        if self.press_streak > 0 && elapsed <= MULTI_CLICK_WINDOW_MS {
            self.press_streak = self.press_streak.saturating_add(1);
        } else {
            self.press_streak = 1;
        }
        self.last_press_ms = self.clock_ms;
        self.set_focused(true);
        // A press places the caret where the user aimed and starts a selection.
        //
        // `press_at` is the pointer's counterpart to `select_with_modifiers`: it anchors where
        // the press landed, and a following drag extends without a key being held, because a
        // drag *is* the gesture that means "extend".
        let index = self.byte_index_at_x(pos.x);
        self.press_at(index, crate::shortcut::Modifiers::from_event_bits(modifiers));
    }

    /// Places the caret at `pos` and starts a pointer selection.
    ///
    /// A press with no modifier begins a *new* selection anchored at the pressed character:
    /// the anchor is where the pointer went down, so dragging away from it selects the span
    /// between them (the behaviour every text field has). Holding Shift extends from the
    /// existing anchor instead, which is the pointer's version of the same modifier rule
    /// [`Self::select_with_modifiers`] applies to the keyboard.
    ///
    /// # Why the anchor is a **character index**, not a position
    ///
    /// `index` is a byte offset and is converted once, here, by `Self::byte_index_at_x`.
    /// Pressing past the end of the value is not a no-op: the press anchors at the end and
    /// the following drag then selects backwards, which is how a user selects the tail of a
    /// value shorter than the field.
    pub fn press_at(&mut self, index: usize, modifiers: crate::shortcut::Modifiers) {
        let index = floor_char_boundary(&self.text, index.min(self.text.len()));
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
        if !shift {
            // A press with no Shift **starts** a gesture, so it re-anchors where it landed.
            self.selection_start = Some(index);
            self.cursor_position = index;
        } else {
            // A Shift-press **extends**: the anchor is kept (adopted from the caret when the
            // gesture is new) and the caret moves to where the pointer went down. Leaving the
            // caret alone here — an earlier revision did — made a shift-press a no-op that only
            // re-anchored, so `Shift`-clicking to the start of a value selected nothing and a
            // following drag grew from a point the user had not pressed.
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor_position);
            }
            self.cursor_position = index;
        }
        self.dragging_selection = true;
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        // Deliberately **not** passed through `normalize_selection`.
        //
        // A press that lands exactly on the caret produces a zero-width anchor/caret pair, and
        // collapsing it to "no selection" is right for a *result* — but this is the *start* of a
        // gesture, and the anchor is the whole point: the drag that follows grows the range from
        // it. Normalising here deleted the anchor the press had just set, so a drag backwards over
        // the value selected nothing; the end of the gesture normalises instead.
        self.base.request_redraw();
    }

    /// Extends an in-progress pointer selection to the character at `index`.
    ///
    /// Only a gesture that began with [`Self::press_at`] continues here, so a bare hover —
    /// this control is not animated and a `MouseMove` arrives on every pointer step — cannot
    /// move a caret the user never grabbed. The anchor is left where the press put it.
    pub fn drag_to(&mut self, index: usize) {
        if !self.dragging_selection {
            return;
        }
        let before_cursor = self.cursor_position;
        let before_selection = self.selection_start;
        self.cursor_position = floor_char_boundary(&self.text, index.min(self.text.len()));
        self.normalize_selection();
        self.ensure_caret_visible();
        self.notify_caret_change(before_cursor, before_selection);
        self.base.request_redraw();
    }

    /// Ends a pointer selection. Returns whether one was in progress.
    ///
    /// This is where a zero-width gesture is collapsed: a press that never moved leaves the caret
    /// exactly on its anchor, and a highlight of no width is not a selection. The collapse happens
    /// here, at the end, rather than in [`Self::press_at`] — see the note there, and
    /// `Self::normalize_selection`.
    pub fn end_drag(&mut self) -> bool {
        let was_dragging = core::mem::replace(&mut self.dragging_selection, false);
        if was_dragging {
            self.normalize_selection();
        }
        was_dragging
    }

    /// Collapses a zero-width anchor/caret pair to "no selection".
    ///
    /// A caret that sits exactly on its anchor has selected nothing, but keeping the anchor
    /// set would make a later Shift-press extend from a position the user did not choose and
    /// leave `selected_text` reporting an empty range. Dropping it here keeps "there is a
    /// selection" and "`selected_text` is non-empty" the same statement.
    pub(super) fn normalize_selection(&mut self) {
        if self.selection_start == Some(self.cursor_position) {
            self.selection_start = None;
        }
    }

    /// The byte offset of the character *before* the caret.
    ///
    /// Delegates to the shared [`crate::widget::text_utils::previous_char_boundary`]: this control,
    /// `TextArea`, `TextEdit` and `MentionEdit` all step a caret the same way, and the copies had
    /// drifted (WDGT-11).
    pub(super) fn previous_char_boundary(&self) -> usize {
        crate::widget::text_utils::previous_char_boundary(&self.text, self.cursor_position)
    }

    /// The byte offset of the character *after* the caret.
    pub(super) fn next_char_boundary(&self) -> usize {
        crate::widget::text_utils::next_char_boundary(&self.text, self.cursor_position)
    }
}

impl LineEdit {
    /// Runs `f` with the value's cumulative advance table, rebuilding it when its inputs moved.
    ///
    /// Falls back to a one-shot table when the cache is already borrowed (a re-entrant paint that
    /// asks this field to measure while it is inside the cache), rather than panicking.
    pub(super) fn with_prefix_advances<R>(
        &self,
        font: &Font,
        f: impl FnOnce(&PrefixAdvances) -> R,
    ) -> R {
        if let Ok(mut cache) = self.prefix_advances.try_borrow_mut() {
            if cache.revision != self.text_revision || cache.font.as_ref() != Some(font) {
                cache.rebuild(self.text_revision, font, &self.text);
            }
            return f(&cache);
        }
        let mut fresh = PrefixAdvances::default();
        fresh.rebuild(self.text_revision, font, &self.text);
        f(&fresh)
    }
    /// Inserts text at cursor position.
    ///
    /// # The limit is in characters, so the room left is too
    ///
    /// `max_length` is a **character** budget (it is what the user sees and what the counter
    /// reports), but this compared it against `self.text.len()`, a byte length. On `"héllo"` — five
    /// characters, six bytes — a limit of six therefore computed `6 - 6 == 0` bytes of room and
    /// refused a perfectly legal insert. Counting both sides in characters makes the field accept
    /// exactly the value its own counter says it is allowed to hold.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // A masked field routes input through the mask engine: the value stays formatted and a
        // keystroke fills the next cell (I-011). Unset, the free-form path below is unchanged.
        if self.mask_engine.is_some() {
            let raw = match self.mask_engine.as_ref() {
                Some(engine) => engine.sanitize(&self.text),
                None => return,
            };
            let mut raw = raw;
            raw.extend(text.chars().filter(|ch| !ch.is_control()));
            self.rewrite_masked(raw);
            return;
        }
        // Check max length and truncate if needed
        // The slice is taken through `byte_index_of_char` because the budget is in characters and
        // the slice is in bytes, so cutting at the budget's byte offset could split a character.
        let effective_text = if let Some(max) = self.max_length {
            let available = max.saturating_sub(self.char_count());
            if available == 0 {
                return;
            }
            let incoming = text.chars().count();
            if incoming > available {
                let boundary = byte_index_of_char(text, available);
                &text[..boundary]
            } else {
                text
            }
        } else {
            text
        };
        // The grammar filters what may enter the model, before the caret/selection math so the
        // insertion length is the filtered one. A plain field pays nothing here.
        let filtered;
        let effective_text = match self.validator.as_deref() {
            Some(validator) => {
                filtered = validator.filter(effective_text);
                filtered.as_str()
            }
            None => effective_text,
        };
        // Handle selection, **in place**: the value is edited where it lives instead of being
        // cloned into a scratch buffer for every key (I-P01). Only a validator/limit field keeps
        // a full before-image; the common path records the splice alone (I-001).
        if effective_text.is_empty() && self.selection_start.is_none() {
            return;
        }
        let before = self.needs_full_before_image().then(|| self.text.clone());
        let (start, removed) = if let Some(start) = self.selection_start {
            let start = floor_char_boundary(&self.text, start.min(self.text.len()));
            let end = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            let removed = self.text[start..end].to_string();
            self.text.replace_range(start..end, effective_text);
            self.cursor_position = start + effective_text.len();
            (start, removed)
        } else {
            let at = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            self.text.insert_str(at, effective_text);
            self.cursor_position = at + effective_text.len();
            (at, String::new())
        };
        self.selection_start = None;
        self.commit_splice(start, &removed, effective_text, before, EditOrigin::User);
        self.modified = true;
        // `set_text` may have clamped the value to `max_length`, which can move the caret it
        // recomputed; snap it back onto a boundary inside whatever the field now holds.
        self.clamp_caret();
        self.ensure_caret_visible();
    }

    /// Inserts a single character at the caret, replacing any selection.
    ///
    /// Shares `TextArea`'s
    /// [`insert_char`](crate::widget::input_widgets::textarea::TextArea::insert_char) name and goes
    /// through [`Self::insert_str`], so the character budget, the mask/validator routing, the undo
    /// snapshot and the caret advance are the same ones a pasted string uses. The glyph is
    /// encoded into a stack buffer, so typing allocates no intermediate `String`.
    pub fn insert_char(&mut self, ch: char) {
        let mut glyph = [0u8; 4];
        self.insert_str(ch.encode_utf8(&mut glyph));
    }

    /// Inserts text at the caret.
    ///
    /// # Deprecated
    ///
    /// [`Self::insert_str`] is the name shared by `TextArea` and `TextEdit` (WDGT-14). This alias
    /// enters exactly the same path, so migrating is a rename with no behaviour change.
    #[deprecated(note = "use `insert_str`, the name shared by the other text fields (WDGT-14)")]
    pub fn insert_text(&mut self, text: &str) {
        self.insert_str(text);
    }

    /// Starts or updates an input-method composition without changing the field's value.
    ///
    /// # The three methods an IME session needs, and why they are a set (BLUE24 §12 U-5)
    ///
    /// A platform input method reports a **preedit** (the uncommitted string it is building) and then
    /// either commits it or cancels. Mapping that onto a text field takes exactly these three
    /// operations, and none of them is expressible as `insert_str`:
    ///
    /// * `set_composition` — the user is still typing. The field's **value must not change**: firing
    ///   `text_changed` on every keystroke of an unfinished word would run a form's validation
    ///   against text the user has not committed, which is the defect this separation exists to
    ///   prevent.
    /// * `commit_composition` — the input method accepted its candidate. Now the string enters the
    ///   model, through the ordinary edit path (so `max_length`, the selection replacement and
    ///   `text_changed` all apply as they do for any other insert).
    /// * `cancel_composition` — the user pressed Escape. The composition is dropped and the value is
    ///   exactly what it was, because it was never touched.
    ///
    /// An empty string clears the composition without committing it, which is what an input method
    /// sends when it withdraws.
    pub fn set_composition(&mut self, preedit: &str) {
        let next = if preedit.is_empty() { None } else { Some(preedit.to_string()) };
        if self.composition == next {
            return;
        }
        self.composition = next;
        self.base.request_redraw();
    }

    /// The uncommitted composition, or `None` when no IME session is active.
    pub fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }

    /// Whether an input-method composition is in progress.
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    /// Accepts the composition: the preedit becomes text, through the ordinary insert path.
    ///
    /// Returns `true` when something was committed. A commit with no active composition is a no-op
    /// rather than an error: an input method may commit and clear in either order, and a field that
    /// refused the second call would report a failure for a normal sequence.
    pub fn commit_composition(&mut self) -> bool {
        let Some(preedit) = self.composition.take() else {
            return false;
        };
        // Through `insert_text`, so `max_length`, the selection replacement and `text_changed`
        // behave exactly as they do for any other insertion — a committed composition is text.
        self.insert_str(&preedit);
        self.base.request_redraw();
        true
    }

    /// Drops the composition without changing the field's value.
    ///
    /// Returns `true` when there was one. The text is untouched **by construction**, not by
    /// restoring a snapshot: the composition was never written into it.
    pub fn cancel_composition(&mut self) -> bool {
        let had = self.composition.take().is_some();
        if had {
            self.base.request_redraw();
        }
        had
    }
}

impl LineEdit {
    /// Deletes selected text or the character before the caret.
    ///
    /// # The deletion is one *character*, not one byte
    ///
    /// `new_text.remove(self.cursor_position - 1)` removes the byte before the caret, which on a
    /// multi-byte character deletes a fragment of it — and panics outright when the caret sits
    /// immediately after a multi-byte character, because `cursor_position - 1` is then inside that
    /// character and `String::remove` requires a boundary. Stepping back to the previous character's
    /// start deletes the whole thing, which is what a user means by “backspace”.
    pub fn backspace(&mut self) {
        // In a masked field Backspace removes the last placed character and re-formats, so the
        // separator literals cannot be deleted independently of the value (I-011).
        if self.mask_engine.is_some() {
            let mut raw = match self.mask_engine.as_ref() {
                Some(engine) => engine.sanitize(&self.text),
                None => return,
            };
            raw.pop();
            self.rewrite_masked(raw);
            return;
        }
        if let Some(start) = self.selection_start {
            // Delete selection
            let start = floor_char_boundary(&self.text, start.min(self.text.len()));
            let end = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let (start, end) = if start < end { (start, end) } else { (end, start) };
            if start == end {
                // An empty selection is not a deletion; fall through to the character case.
                self.selection_start = None;
                self.backspace();
                return;
            }
            let before = self.needs_full_before_image().then(|| self.text.clone());
            let removed = self.text[start..end].to_string();
            self.text.replace_range(start..end, "");
            self.cursor_position = start;
            self.selection_start = None;
            self.commit_splice(start, &removed, "", before, EditOrigin::User);
            self.modified = true;
        } else if self.cursor_position > 0 {
            // Delete the character before the caret, from its own start to the caret.
            let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let previous =
                self.text[..caret].char_indices().next_back().map(|(index, _)| index).unwrap_or(0);
            let before = self.needs_full_before_image().then(|| self.text.clone());
            let removed = self.text[previous..caret].to_string();
            self.text.replace_range(previous..caret, "");
            self.cursor_position = previous;
            self.commit_splice(previous, &removed, "", before, EditOrigin::User);
            self.modified = true;
        }
    }
    /// Deletes selected text or the character after the caret.
    pub fn delete(&mut self) {
        if self.mask_engine.is_some() {
            let mut raw = match self.mask_engine.as_ref() {
                Some(engine) => engine.sanitize(&self.text),
                None => return,
            };
            raw.pop();
            self.rewrite_masked(raw);
            return;
        }
        if let Some(_start) = self.selection_start {
            // Delete selection
            self.backspace(); // Same logic
        } else if self.cursor_position < self.text.len() {
            // Delete the *character* after the caret, not the single byte it starts on.
            let caret = floor_char_boundary(&self.text, self.cursor_position.min(self.text.len()));
            let next = self.text[caret..]
                .char_indices()
                .nth(1)
                .map(|(index, _)| caret + index)
                .unwrap_or(self.text.len());
            let before = self.needs_full_before_image().then(|| self.text.clone());
            let removed = self.text[caret..next].to_string();
            self.text.replace_range(caret..next, "");
            self.commit_splice(caret, &removed, "", before, EditOrigin::User);
            self.modified = true;
        }
    }
    /// Returns whether the line edit is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Sets the read-only state.
    ///
    /// Repaints because the state is draw-visible: a read-only field suppresses the caret (see
    /// `Draw`), so toggling it without a redraw left the marker on a field that had just become
    /// uneditable — or, going the other way, left the user with no caret in a field they could now
    /// type into.
    pub fn set_read_only(&mut self, ro: bool) {
        if self.read_only == ro {
            return;
        }
        self.read_only = ro;
        self.base.request_redraw();
    }

    /// Clears all text.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }

    // ---------------------------------------------------------------------------
    // Input mask, completer and action slots (I-011)
    // ---------------------------------------------------------------------------

    /// The input mask pattern, or `None` for a free-form field.
    pub fn input_mask(&self) -> Option<&str> {
        self.input_mask_text.as_deref()
    }

    /// Installs or removes an input mask.
    ///
    /// The mask is parsed by [`crate::widget::input_widgets::mask_engine`], the same grammar the
    /// dedicated masked control uses. With a
    /// mask installed the value is kept in its formatted form and typing fills the next cell;
    /// removing it leaves the current text untouched. `None` (the default) is the previous
    /// free-form behaviour.
    pub fn set_input_mask(&mut self, mask: Option<&str>) {
        let engine = mask.and_then(MaskEngine::parse);
        let text = match engine.as_ref() {
            Some(engine) => engine.format(&engine.sanitize(&self.text)),
            None => self.text.clone(),
        };
        self.input_mask_text = mask.map(|value| value.to_string());
        self.mask_engine = engine;
        if text != self.text {
            let before = core::mem::replace(&mut self.text, text);
            self.cursor_position = self.text.len();
            self.selection_start = None;
            self.finish_text_change(before, EditOrigin::Programmatic);
        }
    }
}

impl LineEdit {
    /// The published spelling of an [`EchoMode`].
    pub(super) fn echo_mode_token(mode: EchoMode) -> &'static str {
        match mode {
            EchoMode::Normal => "normal",
            EchoMode::Password => "password",
            EchoMode::NoEcho => "no_echo",
        }
    }

    /// The `*`-masked form of `text`, as [`EchoMode::Password`] presents it.
    ///
    /// One definition for the two readers that must agree: the paint path
    /// (`LineEdit::draw`) and the clipboard path copies with it, so a copy can never mask the
    /// value differently from the way the field drew it.
    pub(super) fn mask_text(text: &str) -> String {
        // One star per **character**: `len()` is a byte count, so a password containing a
        // multi-byte character produced several stars per character and the mask's length did not
        // match the value's (I-02).
        "*".repeat(text.chars().count())
    }

    /// The mask for the current value, rebuilt only when the character count changes (I-003).
    pub(super) fn mask_text_cached(&self) -> core::cell::Ref<'_, String> {
        let count = self.text.chars().count();
        {
            let mut cache = self.mask_cache.borrow_mut();
            if cache.0 != count {
                cache.1.clear();
                cache.1.extend(std::iter::repeat_n('*', count));
                cache.0 = count;
            }
        }
        core::cell::Ref::map(self.mask_cache.borrow(), |(_, text)| text)
    }

    /// The text a copy or cut gesture may place on the platform clipboard, or `None`
    /// when the echo mode forbids copying the value at all.
    ///
    /// # Why masking is not only a paint concern
    ///
    /// The system clipboard is process-global: anything written to it is readable by every
    /// other application. So [`EchoMode`] has to gate the copy path as well as the drawing
    /// path — a masked field that copied the real value would leak the plaintext the moment a
    /// user selected it and pressed Copy, and the mask on screen would have been the only
    /// thing that was ever masked. The masked form is exactly what the field paints, so the
    /// clipboard never carries more than the screen shows, and [`EchoMode::NoEcho`], which
    /// shows nothing, copies nothing.
    #[cfg(not(alloc_frugal))]
    pub(super) fn clipboard_selection(&self) -> Option<String> {
        let selection = self.selected_text();
        if selection.is_empty() {
            return None;
        }
        match self.echo_mode {
            EchoMode::Normal => Some(selection),
            EchoMode::Password => Some(Self::mask_text(&selection)),
            EchoMode::NoEcho => None,
        }
    }

    /// Copy the current selection to the platform clipboard (no-op when empty).
    ///
    /// The clipboard receives [`LineEdit::clipboard_selection`] rather than
    /// [`LineEdit::selected_text`], so a masked field copies its mask.
    #[cfg(not(alloc_frugal))]
    pub(super) fn copy_selection_to_clipboard(&self) {
        if let Some(text) = self.clipboard_selection() {
            crate::set_clipboard_text(&text);
        }
    }

    /// Whether there is a selection that a clipboard action would consume.
    ///
    /// A field whose echo mode is [`EchoMode::NoEcho`] reports `false` even with an anchor set:
    /// the characters are not on screen, so there is nothing a user could mean by copying them.
    pub fn has_selected_text(&self) -> bool {
        self.can_copy()
    }

    /// Copies the current selection to the platform clipboard.
    ///
    /// This is the public entry point for a host toolbar or context menu; it runs the same
    /// [`Self::clipboard_selection`] path as the primary+C shortcut, so a masked field copies its
    /// mask and an empty selection is a no-op.
    pub fn copy(&mut self) {
        #[cfg(not(alloc_frugal))]
        {
            self.copy_selection_to_clipboard();
        }
        // A host driving the action still expects the field's own feedback (the selected range
        // stays selected); the repaint is harmless on an empty selection.
        self.base.request_redraw();
    }

    /// Cuts the current selection: copies it, then removes it.
    ///
    /// The two halves keep the conditions the shortcut arm documents — the clipboard receives
    /// the masked form, while the deletion happens whenever there is a selection — so a cut from
    /// a password field still removes what the user cut.
    pub fn cut(&mut self) {
        if self.read_only {
            return;
        }
        #[cfg(not(alloc_frugal))]
        {
            let revision = self.text_revision;
            let had_selection = !self.selected_text().is_empty();
            if let Some(text) = self.clipboard_selection() {
                crate::set_clipboard_text(&text);
            }
            if had_selection {
                // Backspace removes the selection through the one deletion path, so the undo
                // record and the `text_changed` signal are the same ones a key press produces.
                self.backspace();
            }
            self.announce_value_changed_if_revision_moved(revision);
            self.emit_text_edited_if_revision_moved(revision);
        }
        #[cfg(alloc_frugal)]
        {
            self.base.request_redraw();
        }
    }

    /// Pastes the platform clipboard into the selection (or at the caret).
    ///
    /// Text goes through [`Self::insert_str`], the same entry point the primary+V shortcut uses,
    /// so filtering, `max_length`, undo and the change signals behave identically.
    pub fn paste(&mut self) {
        if self.read_only {
            return;
        }
        #[cfg(not(alloc_frugal))]
        {
            let revision = self.text_revision;
            let pasted = crate::get_clipboard_text();
            if !pasted.is_empty() {
                self.insert_str(&pasted);
            }
            self.announce_value_changed_if_revision_moved(revision);
            self.emit_text_edited_if_revision_moved(revision);
        }
        #[cfg(alloc_frugal)]
        {
            self.base.request_redraw();
        }
    }
}
