// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Input dialog widget.
use core::fmt::Write as _;

use crate::compat::{Rc, RefCell};
use crate::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::numeric::{ordered_clamp_f64, ordered_clamp_i64};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
/// Input dialog input mode.
///
/// The mode selects which of the dialog's parallel value slots the input field
/// displays and edits; the other slots keep their values but are not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Free text, edited through [`InputDialog::text_value`].
    Text,
    /// A whole number, clamped to the `int_min`/`int_max` range set by
    /// [`InputDialog::get_int`].
    ///
    /// The dialog stores the range's `step` (read it back with
    /// [`InputDialog::int_step`]) but does not itself apply increments — a caller that
    /// wants stepping applies it through [`InputDialog::set_int_value`]. Saying "stepped
    /// by `int_step`" here, as an earlier revision of this comment did, described a
    /// behaviour no code in this widget has.
    Integer,
    /// A real number, clamped to the `double_min`/`double_max` range and shown
    /// rounded to `double_decimals` places.
    Double,
    /// A choice from [`InputDialog::items`], navigated by
    /// [`InputDialog::current_item`].
    Item,
    /// Free text drawn with one mask glyph per character (DLG-13).
    ///
    /// The stored value is the real text: [`InputDialog::text_value`] answers it unchanged,
    /// and only the paint substitutes the mask.
    Password,
}

/// The stable property spelling of [`InputMode`].
///
/// The capability schema for `mode` is `PropertySchema::enumerated(..)` over
/// `["text", "integer", "double", "item"]`, and those are the exact strings
/// returned here. Keeping one function as the single source of truth means the
/// read route, the write route and the schema cannot drift apart: a value the
/// schema advertises but this function does not produce would be a property the
/// caller can see listed and never read.
fn mode_name(mode: InputMode) -> &'static str {
    match mode {
        InputMode::Text => "text",
        InputMode::Integer => "integer",
        InputMode::Double => "double",
        InputMode::Item => "item",
        InputMode::Password => "password",
    }
}

/// Parses the stable property spelling of [`InputMode`].
///
/// Returns [`CapabilityAccessError::OutOfRange`] rather than silently falling back
/// to a default: a caller that misspells the mode has asked for something this
/// control cannot do, and the schema already told it which four strings are valid.
fn mode_from_name(name: &str) -> Result<InputMode, CapabilityAccessError> {
    match name {
        "text" => Ok(InputMode::Text),
        "integer" => Ok(InputMode::Integer),
        "double" => Ok(InputMode::Double),
        "item" => Ok(InputMode::Item),
        "password" => Ok(InputMode::Password),
        _ => Err(CapabilityAccessError::OutOfRange),
    }
}
/// Input dialog for simple user input.
pub struct InputDialog {
    base: BaseWidget,
    modal: bool,
    title: String,
    label_text: String,
    mode: InputMode,
    text_value: String,
    int_value: i64,
    double_value: f64,
    items: Vec<String>,
    current_item: usize,
    int_min: i64,
    int_max: i64,
    int_step: i64,
    double_min: f64,
    double_max: f64,
    double_step: f64,
    double_decimals: u8,
    /// How far the caret sits into [`InputDialog::text_value`], in characters.
    text_caret: usize,
    /// Whether the host has delivered committed text, which retires the raw-key fallback.
    text_input_seen: bool,
    /// Whether the text field has been focused by a click. The caret is painted only then, so
    /// an untouched dialog keeps its resting appearance.
    text_focused: bool,
    /// Whether the item-mode drop-down is open.
    dropdown_open: bool,
    /// Emitted when the text value changes. Emitted by the control's own editing as well as
    /// by [`InputDialog::set_text_value`].
    pub text_value_changed: Signal1<String>,
    /// Emitted when the integer value changes. Not emitted by this widget yet;
    /// see [`InputDialog::text_value_changed`].
    pub int_value_changed: Signal1<i64>,
    /// Emitted when the floating-point value changes. Not emitted by this widget
    /// yet; see [`InputDialog::text_value_changed`].
    pub double_value_changed: Signal1<f64>,
    /// Emitted by [`InputDialog::accept`].
    pub accepted: GenericSignal,
    /// Emitted by [`InputDialog::reject`].
    pub rejected: GenericSignal,
    /// Emitted when Tab moves the focus ring; the payload is this dialog's own object id (the
    /// dialog is one `ObjectId`; its self-drawn field and buttons are not separate objects).
    pub focus_changed: Signal1<ObjectId>,
    /// The focus target: `0` is the input field, `1 + index` is the action button at `index`.
    focus: Option<usize>,
    /// How many text lines the field offers; `1` is the historical single line (DLG-14).
    line_count: usize,
    /// Cached drop-down row rectangles, keyed by its rect and the item count (G-P14).
    dropdown_cache: DropdownCache,
    /// The text painted in the input field, rebuilt only when an input that spells it changes
    /// (G-P15): the paint used to clone the whole buffer, or allocate a fresh string, every frame.
    display_cache: String,
}

/// The cached drop-down row rectangles and the geometry/item count they were built for.
type DropdownCache = RefCell<Option<((Option<Rect>, usize), Rc<Vec<(usize, Rect)>>)>>;
impl InputDialog {
    /// Creates a modal, empty dialog in [`InputMode::Text`].
    ///
    /// The title and label are empty, the item list is empty, and the numeric
    /// ranges are left wide open (the full `i64`/`f64` ranges, step 1). Set what
    /// you need afterwards, or use one of the configured constructors.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::InputDialog, geometry, "InputDialog"),
            modal: true,
            title: String::new(),
            label_text: String::new(),
            mode: InputMode::Text,
            text_value: String::new(),
            int_value: 0,
            double_value: 0.0,
            items: Vec::new(),
            current_item: 0,
            int_min: i64::MIN,
            int_max: i64::MAX,
            int_step: 1,
            double_min: f64::MIN,
            double_max: f64::MAX,
            double_step: 1.0,
            double_decimals: 1,
            text_caret: 0,
            text_input_seen: false,
            text_focused: false,
            dropdown_open: false,
            text_value_changed: Signal1::new(),
            int_value_changed: Signal1::new(),
            double_value_changed: Signal1::new(),
            accepted: GenericSignal::new(),
            rejected: GenericSignal::new(),
            focus_changed: Signal1::new(),
            focus: None,
            line_count: 1,
            dropdown_cache: RefCell::new(None),
            display_cache: String::new(),
        }
    }
    /// Rebuilds [`InputDialog::display_cache`] from the value the current mode presents.
    ///
    /// Called from every writer of the edit buffer, the numeric values, the item selection, the
    /// echo mode and the precision, so the paint only reads it.
    fn refresh_display_text(&mut self) {
        let mode = self.mode;
        let current_item_text = self.items.get(self.current_item).map(String::as_str).unwrap_or("");
        let text_value = self.text_value.as_str();
        let cache = &mut self.display_cache;
        cache.clear();
        match mode {
            InputMode::Text => cache.push_str(text_value),
            // The password field paints one mask glyph per character; `text_value` keeps the
            // real string, so `get_text` and the signals are unchanged (DLG-13).
            InputMode::Password => {
                for character in text_value.chars() {
                    cache.push(if character == '\n' { '\n' } else { '*' });
                }
            }
            InputMode::Integer => {
                let _ = write!(cache, "{}", self.int_value);
            }
            InputMode::Double => {
                let _ = write!(
                    cache,
                    "{:.prec$}",
                    self.double_value,
                    prec = self.double_decimals as usize
                );
            }
            InputMode::Item => cache.push_str(current_item_text),
        }
    }
    /// Creates a text-input dialog with its title, label and initial text
    /// preset, in [`InputMode::Text`].
    ///
    /// `default` seeds [`InputDialog::text_value`]; it is not a placeholder — an
    /// empty `default` means the field opens genuinely empty.
    pub fn get_text(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.text_value = default.into();
        d.mode = InputMode::Text;
        d.refresh_display_text();
        d
    }
    /// Creates a whole-number dialog preset to `value`, bounded by `min` and
    /// `max`, in [`InputMode::Integer`].
    ///
    /// `value` is clamped into the inclusive range as the dialog is built, so a
    /// request outside it is silently brought inside; read
    /// [`InputDialog::int_value`] to see what was actually taken. `step` is stored
    /// but nothing in this widget applies increments, so it does not affect the
    /// value.
    ///
    /// # Crossed bounds
    ///
    /// The `min`/`max` arguments are *inputs*, so `get_int(.., 500, 100, 0, ..)`
    /// is a call a caller can make, and `i64::clamp` **panics** on `min > max`.
    /// The two bounds are therefore ordered once here and the ordered pair is both
    /// stored and clamped against, so a descending range yields the same answer as
    /// its ascending twin instead of taking the process down. `value` is `i64`, so
    /// `i64::MIN`/`i64::MAX` remain usable as open bounds.
    pub fn get_int(
        geometry: Rect,
        title: impl Into<String>,
        label: impl Into<String>,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    ) -> Self {
        let mut d = Self::new(geometry);
        d.title = title.into();
        d.label_text = label.into();
        d.int_min = min.min(max);
        d.int_max = min.max(max);
        d.int_value = value.clamp(d.int_min, d.int_max);
        d.int_step = step;
        d.mode = InputMode::Integer;
        d.refresh_display_text();
        d
    }
    /// The dialog's title, drawn in its header bar.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// The text drawn beside the input field to say what is being asked for.
    pub fn label_text(&self) -> &str {
        &self.label_text
    }
    /// Which value the input field currently presents.
    pub fn mode(&self) -> InputMode {
        self.mode
    }
    /// The free-text value. This is the value the caller cares about after an
    /// [`InputMode::Text`] dialog is accepted; it exists in every mode but is
    /// only displayed in `Text`.
    pub fn text_value(&self) -> &str {
        &self.text_value
    }
    /// The whole-number value, clamped to the `int_min`/`int_max` range.
    ///
    /// Read this after an [`InputMode::Integer`] dialog is accepted. It holds a
    /// meaningful value in every mode, but only the `Integer` mode displays it.
    pub fn int_value(&self) -> i64 {
        self.int_value
    }
    /// The whole-number increment the range was declared with.
    ///
    /// [`InputDialog::get_int`] takes a `step`, stores it, and applies no increments itself —
    /// the value only changes through [`InputDialog::set_int_value`]. This is the read side of
    /// that stored number, so a caller can drive stepping without having kept its own copy:
    ///
    /// ```ignore
    /// dialog.set_int_value(dialog.int_value() + dialog.int_step());
    /// ```
    ///
    /// Before this existed the field was written by the constructor and read by nothing at all,
    /// which made the `step` argument unobservable (principle #99).
    pub fn int_step(&self) -> i64 {
        self.int_step
    }
    /// The floating-point value, clamped to the `double_min`/`double_max` range.
    ///
    /// Stays `0.0` unless set: no constructor here seeds it, and it is only
    /// displayed in [`InputMode::Double`].
    pub fn double_value(&self) -> f64 {
        self.double_value
    }
    /// The index of the selected item, or `0` when there are no items.
    ///
    /// Always an index, never an optional — check [`InputDialog::items`] to tell
    /// "nothing to choose from" from "the first item is chosen".
    pub fn current_item(&self) -> usize {
        self.current_item
    }
    /// The choices offered in [`InputMode::Item`]. Empty unless
    /// [`InputDialog::set_items`] was called.
    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// The selected item's text, or `None` when the list is empty or the index
    /// no longer addresses an entry.
    pub fn current_item_text(&self) -> Option<&str> {
        self.items.get(self.current_item).map(|s| s.as_str())
    }
    /// Sets the title and repaints.
    pub fn set_title(&mut self, t: impl Into<String>) {
        let t = t.into();
        if self.title == t {
            return;
        }
        self.title = t;
        self.base.request_redraw();
    }
    /// Sets the label drawn beside the input field and repaints.
    pub fn set_label_text(&mut self, t: impl Into<String>) {
        let t = t.into();
        if self.label_text == t {
            return;
        }
        self.label_text = t;
        self.base.request_redraw();
    }
    /// Switches which value the input field presents, and repaints.
    ///
    /// The other values are unaffected: switching away from a mode does not
    /// clear what it held, so switching back shows it again.
    pub fn set_mode(&mut self, mode: InputMode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.refresh_display_text();
        self.base.request_redraw();
    }
    /// Sets the free-text value and repaints. Overwrites rather than appends, so
    /// it cannot be used for incremental typing.
    ///
    /// Emits [`Self::text_value_changed`] when the value actually differs. The
    /// signal is named `_changed`, so emitting on a no-op write would be a lie: a
    /// caller using it to drive an expensive downstream update would run that
    /// update for a write that changed nothing.
    pub fn set_text_value(&mut self, v: impl Into<String>) {
        let value = v.into();
        if value == self.text_value {
            return;
        }
        self.text_value = value;
        self.text_caret = self.text_caret.min(self.text_value.chars().count());
        self.refresh_display_text();
        self.base.request_redraw();
        self.text_value_changed.emit(self.text_value.clone());
    }
    /// Replaces the item list and repaints.
    ///
    /// Resets the selection to index 0, so a previously chosen item is lost even
    /// if it is still present in the new list.
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.current_item = 0;
        self.refresh_display_text();
        self.base.request_redraw();
    }
    /// Sets the whole-number value, clamped into the current
    /// `int_min`/`int_max` range, and repaints.
    ///
    /// Emits [`Self::int_value_changed`] when the *stored* value changes, so a
    /// request that was clamped back to what was already there is not reported as
    /// a change.
    pub fn set_int_value(&mut self, v: i64) {
        let clamped = ordered_clamp_i64(v, self.int_min, self.int_max);
        if clamped == self.int_value {
            return;
        }
        self.int_value = clamped;
        self.refresh_display_text();
        self.base.request_redraw();
        self.int_value_changed.emit(self.int_value);
    }

    /// Sets the floating-point value, clamped into the current
    /// `double_min`/`double_max` range, and repaints.
    ///
    /// A `NaN` is rejected without touching the stored value: it cannot be clamped
    /// into a range (every comparison is false), so accepting it would leave the
    /// control outside its own declared bounds. Emits
    /// [`Self::double_value_changed`] on a real change.
    pub fn set_double_value(&mut self, v: f64) {
        if v.is_nan() {
            return;
        }
        let clamped = ordered_clamp_f64(v, self.double_min, self.double_max);
        if clamped == self.double_value {
            return;
        }
        self.double_value = clamped;
        self.refresh_display_text();
        self.base.request_redraw();
        self.double_value_changed.emit(self.double_value);
    }

    /// The inclusive integer range the value is clamped into.
    pub fn int_range(&self) -> (i64, i64) {
        (self.int_min, self.int_max)
    }

    /// Replaces the inclusive integer range and clamps the current value into it.
    ///
    /// The bounds are ordered as they are stored, so a descending pair behaves like its
    /// ascending twin instead of tripping `i64::clamp`'s `min <= max` precondition.
    pub fn set_int_range(&mut self, min: i64, max: i64) {
        self.int_min = min.min(max);
        self.int_max = min.max(max);
        self.set_int_value(self.int_value);
        self.base.request_redraw();
    }

    /// Replaces the whole-number increment used by the arrow keys.
    pub fn set_int_step(&mut self, step: i64) {
        if self.int_step == step {
            return;
        }
        self.int_step = step;
        self.base.request_redraw();
    }

    /// The inclusive floating-point range the value is clamped into.
    pub fn double_range(&self) -> (f64, f64) {
        (self.double_min, self.double_max)
    }

    /// Replaces the inclusive floating-point range and clamps the current value into it.
    ///
    /// A `NaN` bound would make every comparison false, so the pair is normalised: a `NaN`
    /// lower bound becomes the type's minimum, a `NaN` upper bound its maximum. The result
    /// still orders the two, so the stored range is usable whatever the caller supplied.
    pub fn set_double_range(&mut self, min: f64, max: f64) {
        let min = if min.is_nan() { f64::MIN } else { min };
        let max = if max.is_nan() { f64::MAX } else { max };
        self.double_min = min.min(max);
        self.double_max = min.max(max);
        self.set_double_value(self.double_value);
        self.base.request_redraw();
    }

    /// The increment the arrow keys apply in [`InputMode::Double`].
    pub fn double_step(&self) -> f64 {
        self.double_step
    }

    /// Replaces the floating-point increment used by the arrow keys.
    pub fn set_double_step(&mut self, step: f64) {
        if !step.is_finite() || self.double_step == step {
            return;
        }
        self.double_step = step;
        self.base.request_redraw();
    }

    /// The number of fraction digits painted in [`InputMode::Double`].
    pub fn double_decimals(&self) -> u8 {
        self.double_decimals
    }

    /// Sets the painted fraction-digit count. It affects only the rendering, never the value.
    pub fn set_double_decimals(&mut self, decimals: u8) {
        if self.double_decimals == decimals {
            return;
        }
        self.double_decimals = decimals;
        self.refresh_display_text();
        self.base.request_redraw();
    }
    /// Whether the item-mode drop-down is open.
    pub fn is_dropdown_open(&self) -> bool {
        self.dropdown_open
    }

    /// Opens or closes the item-mode drop-down and repaints when it changes.
    pub fn set_dropdown_open(&mut self, open: bool) {
        if self.dropdown_open == open || self.mode != InputMode::Item {
            return;
        }
        self.dropdown_open = open;
        self.base.request_redraw();
    }

    /// The caret position in [`InputDialog::text_value`], in characters.
    pub fn text_caret(&self) -> usize {
        self.text_caret
    }

    /// Replaces the free-text value and puts the caret at its end.
    ///
    /// This is the editing entry point the control itself uses; it is distinct from
    /// [`InputDialog::set_text_value`] so typing does not have to route through a public
    /// setter that a host may have connected to.
    fn edit_text(&mut self, next: String, caret: usize) {
        if next == self.text_value {
            return;
        }
        self.text_value = next;
        self.text_caret = caret.min(self.text_value.chars().count());
        self.refresh_display_text();
        self.base.request_redraw();
        self.text_value_changed.emit(self.text_value.clone());
    }

    /// Inserts `inserted` at the caret.
    fn insert_at_caret(&mut self, inserted: &str) {
        if !matches!(self.mode, InputMode::Text | InputMode::Password) {
            return;
        }
        let caret = self.text_caret.min(self.text_value.chars().count());
        let byte = Self::byte_index(&self.text_value, caret);
        let mut next = String::with_capacity(self.text_value.len() + inserted.len());
        next.push_str(&self.text_value[..byte]);
        next.push_str(inserted);
        next.push_str(&self.text_value[byte..]);
        self.edit_text(next, caret + inserted.chars().count());
    }

    /// The byte offset of character `index` in `text`.
    fn byte_index(text: &str, index: usize) -> usize {
        text.char_indices().nth(index).map(|(offset, _)| offset).unwrap_or(text.len())
    }

    /// Deletes the character before the caret.
    fn backspace_text(&mut self) {
        if !matches!(self.mode, InputMode::Text | InputMode::Password) || self.text_caret == 0 {
            return;
        }
        let end = Self::byte_index(&self.text_value, self.text_caret);
        let start = Self::byte_index(&self.text_value, self.text_caret - 1);
        let mut next = String::with_capacity(self.text_value.len());
        next.push_str(&self.text_value[..start]);
        next.push_str(&self.text_value[end..]);
        self.edit_text(next, self.text_caret - 1);
    }

    /// Deletes the character after the caret.
    fn delete_text(&mut self) {
        if !matches!(self.mode, InputMode::Text | InputMode::Password) {
            return;
        }
        let length = self.text_value.chars().count();
        if self.text_caret >= length {
            return;
        }
        let start = Self::byte_index(&self.text_value, self.text_caret);
        let end = Self::byte_index(&self.text_value, self.text_caret + 1);
        let mut next = String::with_capacity(self.text_value.len());
        next.push_str(&self.text_value[..start]);
        next.push_str(&self.text_value[end..]);
        self.edit_text(next, self.text_caret);
    }

    /// Moves the caret to `to`, clamped to the value's length.
    fn move_text_caret(&mut self, to: usize) {
        let length = self.text_value.chars().count();
        let to = to.min(length);
        if self.text_caret != to {
            self.text_caret = to;
            self.base.request_redraw();
        }
    }

    /// Steps an integer or floating value by its configured increment.
    fn step_value(&mut self, direction: i32) {
        match self.mode {
            InputMode::Integer => {
                let step = self.int_step.saturating_mul(i64::from(direction));
                self.set_int_value(self.int_value.saturating_add(step));
            }
            InputMode::Double => {
                self.set_double_value(self.double_value + self.double_step * f64::from(direction));
            }
            _ => {}
        }
    }

    /// Whether the dialog blocks interaction with its owner while open.
    ///
    /// Records the intent, on by default; enforcement is the modal stack in
    /// [`crate::widget::runtime`] (`enter_modal` / `exit_modal`).
    pub fn is_modal(&self) -> bool {
        self.modal
    }
    /// Sets the modality flag and repaints. See [`InputDialog::is_modal`].
    pub fn set_modal(&mut self, modal: bool) {
        if self.modal == modal {
            return;
        }
        self.modal = modal;
        self.base.request_redraw();
    }
    /// Confirms the dialog: emits `accepted`, then hides it.
    ///
    /// Unlike the file dialog this does not emit the value: the caller reads
    /// [`InputDialog::text_value`], [`InputDialog::int_value`],
    /// [`InputDialog::double_value`] or [`InputDialog::current_item`] according
    /// to [`InputDialog::mode`]. Pressing Enter (key code 13) on an enabled,
    /// visible dialog does the same.
    pub fn accept(&mut self) {
        self.accepted.emit();
        self.hide();
    }
    /// Cancels the dialog and hides it, emitting `rejected`.
    ///
    /// The values are **not** cleared, so a cancelled dialog still reports what
    /// was in its fields. Pressing Escape (key code 27) has the same effect.
    pub fn reject(&mut self) {
        self.rejected.emit();
        self.hide();
    }
}
impl Widget for InputDialog {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(350, 150)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `InputDialog` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "text_value_changed" => {
                Some(EventSignalRef::mapped("text_value_changed", &self.text_value_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "int_value_changed" => {
                Some(EventSignalRef::mapped("int_value_changed", &self.int_value_changed, |v| {
                    CapabilityValue::Int(*v)
                }))
            }
            "double_value_changed" => Some(EventSignalRef::mapped(
                "double_value_changed",
                &self.double_value_changed,
                |v| CapabilityValue::Float(*v),
            )),
            "accepted" => Some(EventSignalRef::unit("accepted", &self.accepted)),
            "rejected" => Some(EventSignalRef::unit("rejected", &self.rejected)),
            "focus_changed" => {
                Some(EventSignalRef::mapped("focus_changed", &self.focus_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `InputDialog`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` dispatch. Both properties are read-only: the old
/// write layer had no arm for this kind.
impl WidgetProperties for InputDialog {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title().to_string())),
            "label_text" => Ok(CapabilityValue::String(self.label_text().to_string())),
            // The four typed value properties. The schema in
            // `INPUT_DIALOG_PROPERTIES` has always declared them, but neither the
            // read nor the write arm existed, so the capability table published
            // three commands (`set_mode`, `set_text_value`, `set_int_value`,
            // `set_double_value`) against properties that answered
            // `UnknownProperty` for a control that really does support them.
            "mode" => Ok(CapabilityValue::String(mode_name(self.mode()).to_string())),
            "text_value" => Ok(CapabilityValue::String(self.text_value().to_string())),
            "int_value" => Ok(CapabilityValue::Int(self.int_value())),
            "double_value" => Ok(CapabilityValue::Float(self.double_value())),
            "int_step" => Ok(CapabilityValue::Int(self.int_step())),
            "current_item" => Ok(CapabilityValue::UInt(self.current_item() as u64)),
            "dropdown_open" => Ok(CapabilityValue::Bool(self.is_dropdown_open())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "label_text" => {
                self.set_label_text(expect_string(value)?);
                Ok(())
            }
            "mode" => {
                let requested = expect_string(value)?;
                let mode = mode_from_name(&requested)?;
                self.set_mode(mode);
                Ok(())
            }
            "text_value" => {
                let text = expect_string(value)?;
                self.set_text_value(text);
                Ok(())
            }
            "int_value" => {
                let number = match value {
                    CapabilityValue::Int(v) => v,
                    CapabilityValue::Float(v) => v as i64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_int_value(number);
                Ok(())
            }
            "double_value" => {
                let number = match value {
                    CapabilityValue::Float(v) => v,
                    CapabilityValue::Int(v) => v as f64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_double_value(number);
                Ok(())
            }
            "int_step" => {
                let step = match value {
                    CapabilityValue::Int(v) => v,
                    CapabilityValue::UInt(v) => v as i64,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                if step != self.int_step {
                    self.int_step = step;
                }
                Ok(())
            }
            "current_item" => {
                let index = match value {
                    CapabilityValue::UInt(v) => v as usize,
                    CapabilityValue::Int(v) if v >= 0 => v as usize,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                if index >= self.items.len() && !self.items.is_empty() {
                    return Err(CapabilityAccessError::OutOfRange);
                }
                if self.current_item != index {
                    self.current_item = index;
                    self.refresh_display_text();
                    self.base.request_redraw();
                }
                Ok(())
            }
            "dropdown_open" => {
                self.set_dropdown_open(expect_bool(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "label_text",
            "mode",
            "text_value",
            "int_value",
            "double_value",
            "int_step",
            "current_item",
            "dropdown_open",
            BASE_PROPERTY_NAMES
        ]
    }
}
impl EventHandler for InputDialog {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                // A press on an open drop-down row picks it before anything else can consume
                // the click; the rows are the topmost thing the dialog paints.
                if let Some((index, _)) =
                    self.dropdown_rows().iter().copied().find(|(_, rect)| rect.contains_point(*pos))
                {
                    self.current_item = index;
                    self.dropdown_open = false;
                    self.refresh_display_text();
                    self.base.request_redraw();
                    return;
                }
                if let Some(index) = self.button_at(*pos) {
                    self.focus = Some(index + 1);
                    if index == 0 {
                        self.accept();
                    } else {
                        self.reject();
                    }
                    return;
                }
                if self.input_band().contains_point(*pos) {
                    self.focus = Some(0);
                    if self.mode == InputMode::Item {
                        self.dropdown_open = !self.dropdown_open;
                        self.base.request_redraw();
                    } else if matches!(self.mode, InputMode::Text | InputMode::Password) {
                        let input = self.input_band();
                        self.text_focused = true;
                        let advance =
                            effective_font(self.base.style()).size().ceil().max(1.0) as u32;
                        let length = self.text_value.chars().count();
                        let caret = (((pos.x - input.x - 4).max(0) as u32) / advance) as usize;
                        self.move_text_caret(caret.min(length));
                    }
                } else if self.dropdown_open {
                    self.dropdown_open = false;
                    self.base.request_redraw();
                }
            }
            Event::TextInput { text } | Event::ImeCommit { text } => {
                self.text_input_seen = true;
                let filtered: String =
                    text.chars().filter(|character| !character.is_control()).collect();
                if !filtered.is_empty() {
                    self.insert_at_caret(&filtered);
                }
            }
            Event::KeyPress { key, modifiers } => {
                let length = self.text_value.chars().count();
                match *key {
                    key_codes::TAB => self.move_focus(),
                    key_codes::ENTER => {
                        // In a multi-line text field Enter is a newline; the chorded form
                        // (Ctrl/Cmd+Enter) is the submit gesture (DLG-14).
                        if matches!(self.mode, InputMode::Text)
                            && self.line_count > 1
                            && *modifiers & 0b1010 == 0
                        {
                            self.insert_at_caret("\n");
                            return;
                        }
                        if self.mode == InputMode::Item {
                            self.dropdown_open = false;
                        }
                        if self.focus == Some(2) {
                            self.reject();
                        } else {
                            self.accept();
                        }
                    }
                    key_codes::ESCAPE => {
                        if self.dropdown_open {
                            self.dropdown_open = false;
                            self.base.request_redraw();
                        } else {
                            self.reject();
                        }
                    }
                    key_codes::UP => {
                        if self.mode == InputMode::Item {
                            if self.current_item > 0 {
                                self.current_item -= 1;
                                self.refresh_display_text();
                                self.base.request_redraw();
                            }
                        } else {
                            self.step_value(1);
                        }
                    }
                    key_codes::DOWN => {
                        if self.mode == InputMode::Item {
                            if self.current_item + 1 < self.items.len() {
                                self.current_item += 1;
                                self.refresh_display_text();
                                self.base.request_redraw();
                            }
                        } else {
                            self.step_value(-1);
                        }
                    }
                    key_codes::LEFT => self.move_text_caret(self.text_caret.saturating_sub(1)),
                    key_codes::RIGHT => self.move_text_caret((self.text_caret + 1).min(length)),
                    key_codes::HOME => self.move_text_caret(0),
                    key_codes::END => self.move_text_caret(length),
                    key_codes::BACKSPACE => self.backspace_text(),
                    key_codes::DELETE => self.delete_text(),
                    _ => {
                        // Printable ASCII is the fallback for a host that reports only key
                        // codes; committed text wins once it has arrived.
                        if !self.text_input_seen && *key >= 32 && *key <= 126 {
                            if let Some(ch) = char::from_u32(*key) {
                                self.insert_at_caret(&ch.to_string());
                            }
                        }
                    }
                }
            }
            _ => { /* Other events are not relevant */ }
        }
    }
}
impl InputDialog {
    /// The frame the dialog actually paints: at most its intrinsic size, centred in the
    /// area it was given.
    ///
    /// # Why the frame is not the caller's rectangle
    ///
    /// `rect` is the area the dialog is **offered**. Painting it verbatim drew the 240x120
    /// census cell as a 240x120 frame whose title bar, label, field and buttons were then
    /// pinned to literals (`48`, `60`, `rect.height - 40`) written for the 400 px default
    /// size — so the rows sat in the top half and the buttons floated away from them.
    /// [`ControlMetrics::painted_box`] caps each axis at the dialog's own intrinsic size
    /// and centres what is left; every band below is derived from this one rect.
    fn frame_rect(&self) -> Rect {
        ControlMetrics::painted_box(
            self.base.geometry(),
            Size::new(dimensions::DIALOG_MIN_WIDTH, dimensions::DIALOG_MIN_HEIGHT),
        )
    }

    /// The entry field's band, derived from the same rows the painter stacks.
    fn input_band(&self) -> Rect {
        let rect = self.frame_rect();
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let font = effective_font(self.base.style());
        let label_line_h = context.measure_text("M", font).height.max(1);
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let field_band = ControlMetrics::content_below_top_band(body, label_line_h);
        // A multi-line field grows by whole rows, bounded by the room the label leaves.
        let wanted = (26 * self.line_count.max(1) as u32).max(26);
        ControlMetrics::top_band(field_band, field_band.height.min(wanted))
    }

    /// The number of text lines the field offers (DLG-14).
    pub fn line_count(&self) -> usize {
        self.line_count
    }

    /// Sets how many lines the text field offers; a zero is raised to one (DLG-14).
    pub fn set_line_count(&mut self, lines: usize) {
        let lines = lines.max(1);
        if self.line_count == lines {
            return;
        }
        self.line_count = lines;
        self.base.request_redraw();
    }

    /// The height of one drop-down row.
    const DROPDOWN_ROW_HEIGHT: u32 = 22;

    /// The item-mode drop-down's rectangle, when it is open and has room.
    fn dropdown_rect(&self) -> Option<Rect> {
        if !self.dropdown_open || self.mode != InputMode::Item || self.items.is_empty() {
            return None;
        }
        let input = self.input_band();
        let rect = self.frame_rect();
        let button_top = ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT).y;
        let top = input.y + input.height as i32 + 2;
        let available = (button_top - top - 4).max(0) as u32;
        let height = (self.items.len() as u32 * Self::DROPDOWN_ROW_HEIGHT).min(available);
        (height >= Self::DROPDOWN_ROW_HEIGHT).then(|| Rect::new(input.x, top, input.width, height))
    }

    /// The visible drop-down rows and their item indices, served from a cache (G-P14).
    fn dropdown_rows(&self) -> Rc<Vec<(usize, Rect)>> {
        let key = (self.dropdown_rect(), self.items.len());
        if let Some((cached_key, rows)) = self.dropdown_cache.borrow().as_ref() {
            if *cached_key == key {
                return Rc::clone(rows);
            }
        }
        let rows = Rc::new(match key.0 {
            Some(rect) => {
                let visible = (rect.height / Self::DROPDOWN_ROW_HEIGHT) as usize;
                (0..visible.min(self.items.len()))
                    .map(|index| {
                        (
                            index,
                            Rect::new(
                                rect.x,
                                rect.y + index as i32 * Self::DROPDOWN_ROW_HEIGHT as i32,
                                rect.width,
                                Self::DROPDOWN_ROW_HEIGHT,
                            ),
                        )
                    })
                    .collect()
            }
            None => Vec::new(),
        });
        *self.dropdown_cache.borrow_mut() = Some((key, Rc::clone(&rows)));
        rows
    }

    /// The action labels this dialog paints, in row order.
    ///
    /// `draw` and [`InputDialog::button_at`] both call this, so a press can never resolve
    /// against a different label set than the one that was measured and painted.
    fn action_labels() -> Vec<String> {
        // One buffer per label, moved into the row: the label is appended by the catalogue
        // instead of being materialised twice by `localized_label` (LYT-X04).
        let mut labels = Vec::with_capacity(2);
        let mut label = crate::compat::String::new();
        super::localized_label_into("common.button.ok", "OK", &mut label);
        labels.push(label);
        let mut label = crate::compat::String::new();
        super::localized_label_into("common.button.cancel", "Cancel", &mut label);
        labels.push(label);
        labels
    }

    /// The action button rects, rebuilt from the same frame and bottom band `draw` uses.
    ///
    /// The press path has no render context of its own, so it builds a measuring one — the
    /// pattern `MessageBox::action_button_at` established. Rebuilding rather than caching is
    /// deliberate: a cached rect is a second copy of the geometry that a resize can leave stale.
    fn action_buttons(&self) -> Vec<Rect> {
        let size = Size::new(self.base.geometry().width.max(1), self.base.geometry().height.max(1));
        let mut backend = crate::render::SoftwarePaintBackend::new(size, 1.0);
        let context = RenderContext::new(&mut backend);
        let rect = self.frame_rect();
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        );
        let labels = Self::action_labels();
        super::message_box::action_row_geometry(
            &context,
            effective_font(self.base.style()),
            &labels,
            button_band,
            true,
        )
        .buttons
    }

    /// The index of the action button under `pos`, or `None`.
    fn button_at(&self, pos: Point) -> Option<usize> {
        self.action_buttons().iter().position(|button| button.contains_point(pos))
    }

    /// The focus-target count: the field plus the two action buttons.
    fn focus_count(&self) -> usize {
        1 + Self::action_labels().len()
    }

    /// Moves the focus ring and announces the move.
    fn move_focus(&mut self) {
        let next = super::move_focus(self.focus, self.focus_count(), true);
        if next != self.focus {
            self.focus = next;
            self.focus_changed.emit(self.id());
            self.base.request_redraw();
        }
    }
}

impl Draw for InputDialog {
    fn draw(&mut self, context: &mut RenderContext) {
        // A modal dialog dims the page behind it: that dimming is the whole visual difference
        // between "this dialog is up and the page is inert" and "a panel was drawn on the page".
        // The `modal` flag was recorded and enforced by the modal stack, and nothing painted it.
        if self.modal {
            super::draw_modal_scrim(context, self.id(), self.geometry(), self.base.parent());
        }
        // The **frame**, not the control's rectangle: see `frame_rect`.
        let rect = self.frame_rect();
        let style = self.style();
        // The **effective font** — the resolved theme/caller font — for the title, label, entry
        // value and buttons (D09-STYLE-01).
        let font = effective_font(style);

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. The style step alone
        // was not enough: `WidgetStyle` carries no title-bar or accent field, so the two
        // most visible pixels of the dialog — the title band and the accept button —
        // stayed a hardcoded blue in either appearance, and the render census reported
        // the whole control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let theme = crate::style::resolved_theme_style_shared("input_dialog");
        // The window fill and the accent come from the shared palette snapshot, so no manager
        // guard is held while the dialog is drawn.
        let (window_fill, accent) = {
            match crate::style::palette_shared() {
                Some(colors) => (colors.background, colors.primary),
                None => (Color::WHITE, Color::rgb(0, 120, 215)),
            }
        };
        let accent_ink = accent.contrast_color();

        // A dialog is a `Surface`-role control and `Surface` resolves to the window's own
        // fill, which would leave the frame invisible against the window. A resolved
        // surface equal to the window fill is therefore re-derived one step toward the
        // ink, the same distinction `Colors::input_background` draws for a field.
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let surface = match style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
        {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.06),
        };
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(160, 160, 160));
        // The entry field is the editable interior: one step *away* from the dialog
        // surface, so on a light theme darker and on a dark one lighter rather than the
        // forced white it used to be.
        let field = if surface.is_dark() {
            surface.blend(&Color::WHITE, 0.08)
        } else {
            surface.blend(&Color::BLACK, 0.06)
        };

        // Rounded by [`dimensions::DIALOG_RADIUS`] so the dialog reads as the same class of
        // object as its neighbours; the radius is clamped to the frame so a box smaller
        // than its own corner is not drawn with an inverted one.
        let radius = dimensions::DIALOG_RADIUS.min(rect.width / 2).min(rect.height / 2);
        if radius > 0 {
            context.fill_rounded_rect(rect, radius, surface);
            context.draw_rounded_rect_stroke(rect, radius, border, 1);
        } else {
            context.face_with_gradient_corners(
                rect,
                surface,
                self.style().background_gradient.as_ref(),
                self.style().surface.unwrap_or_default(),
                self.style().resolved_corner_radii().to_array(),
                Color::BLACK,
            );
            context.draw_rect(rect, border);
        }
        // Title bar: a separate region from the dialog surface, derived from it one visible step
        // rather than filled with the accent. Every other dialog in the crate uses a neutral
        // title strip; the accent fill made this one look like a different class of window (and a
        // blue header is not macOS chrome). The accent still fills the accept button below, where
        // a call to action belongs. The label is fitted to the bar, so a long title truncates at
        // the bar's edge instead of running past the frame.
        let title_bar = surface.blend(&ink, 0.08);
        let title_bar_band = ControlMetrics::top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        context.fill_rect(title_bar_band, title_bar);
        if !self.title.is_empty() {
            let title_font = font;
            let title_line = context.text_line(title_bar_band, title_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    rect.x + 8,
                    title_line.y,
                    rect.width.saturating_sub(16),
                    title_line.height.max(1),
                ),
                &self.title,
                title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // The body is everything the title bar leaves. Its rows are derived from the body
        // rather than from the frame's top edge plus a literal, so the label sits above the
        // field it names at any frame size and the two cannot drift apart.
        let body =
            ControlMetrics::content_below_top_band(rect, dimensions::DIALOG_TITLE_BAR_HEIGHT);
        let label_font = font;
        let label_line_h = context.measure_text("M", label_font).height.max(1);
        // Label. Its box is the row above the entry field, which is what bounds a label
        // longer than the dialog instead of the dialog's own width. Guarded on the text
        // being non-empty so a labelless dialog emits no `<text …></text>`.
        let label_band = ControlMetrics::top_band(body, label_line_h);
        if !self.label_text.is_empty() {
            let label_line = context.text_line(label_band, label_font);
            context.draw_text_fitted_cow(
                Rect::new(
                    body.x + 10,
                    label_line.y,
                    body.width.saturating_sub(20),
                    label_line.height.max(1),
                ),
                &self.label_text,
                label_font,
                ink,
                HorizontalAlignment::Left,
            );
        }
        // Input field: the band below the label, its height the field's own rather than a
        // literal `26` written at the frame's top plus `60`.
        let field_band = ControlMetrics::content_below_top_band(body, label_line_h);
        let input_band = ControlMetrics::top_band(field_band, field_band.height.min(26));
        context.fill_rect(input_band, field);
        context.draw_rect(input_band, border);
        if self.focus == Some(0) {
            context.draw_rounded_rect_stroke(input_band, 2, accent, 2);
        }
        // The painted text comes from the cache the value writers maintain (G-P15).
        // Guarded: an unguarded draw of an empty value emits `<text …></text>`, an element
        // the rasteriser never produces. The line box is centred on the field through the
        // shared primitive, which the old `input_y + ((26 - h) / 2)` re-derived by hand.
        if !self.display_cache.is_empty() {
            let input_font = font;
            let multiline = self.line_count > 1
                && matches!(self.mode, InputMode::Text | InputMode::Password)
                && self.text_value.contains('\n');
            if multiline {
                // One row per explicit line; Enter inserts `\n` in this mode (DLG-14).
                let line_h = context.text_line(input_band, input_font).height.max(1);
                let rows = self.display_cache.split('\n');
                let lines: Vec<&str> = rows.collect();
                for (row_index, line_text) in lines.iter().enumerate() {
                    let y = input_band.y + 2 + row_index as i32 * line_h as i32;
                    if y + line_h as i32 > input_band.y + input_band.height as i32 {
                        break;
                    }
                    context.draw_text_fitted_cow(
                        Rect::new(input_band.x + 4, y, input_band.width.saturating_sub(8), line_h),
                        line_text,
                        input_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
            } else {
                let input_line = context.text_line(input_band, input_font);
                context.draw_text_fitted_cow(
                    Rect::new(
                        input_band.x + 4,
                        input_line.y,
                        input_band.width.saturating_sub(8),
                        input_line.height.max(1),
                    ),
                    &self.display_cache,
                    input_font,
                    ink,
                    HorizontalAlignment::Left,
                );
            }
        }
        // The caret is what makes the field look editable: a one-pixel line at the insertion
        // point. It is drawn in the text mode only, because the numeric and item modes are
        // stepped rather than typed into this band.
        if matches!(self.mode, InputMode::Text | InputMode::Password) && self.text_focused {
            let advance = font.size().ceil().max(1.0) as i32;
            let caret_x = input_band.x + 4 + self.text_caret as i32 * advance;
            if caret_x < input_band.x + input_band.width as i32 - 2 {
                context.draw_line(
                    Point::new(caret_x, input_band.y + 3),
                    Point::new(caret_x, input_band.y + input_band.height as i32 - 3),
                    ink,
                );
            }
        }
        // The item-mode drop-down: the chosen item is the field's value, and the open list is
        // bounded by what the button row leaves.
        if let Some(dropdown) = self.dropdown_rect() {
            context.fill_rect(dropdown, field);
            context.draw_rect(dropdown, border);
            context.push_clip(dropdown.x, dropdown.y, dropdown.width, dropdown.height);
            for (index, row) in self.dropdown_rows().iter().copied() {
                if index == self.current_item {
                    context.fill_rect(row, accent);
                }
                let label = self.items.get(index).map(|item| item.as_str()).unwrap_or("");
                if !label.is_empty() {
                    context.draw_text_fitted_cow(
                        Rect::new(row.x + 6, row.y, row.width.saturating_sub(12), row.height),
                        label,
                        font,
                        if index == self.current_item { accent_ink } else { ink },
                        HorizontalAlignment::Left,
                    );
                }
            }
            context.pop_clip();
        }
        // OK/Cancel. The pair is the frame's bottom band, right-aligned inside it through the
        // shared [`action_row_geometry`] derivation — the same function `message_box`,
        // `file_dialog`, `font_dialog`, `color_dialog` and `progress_dialog` draw their pair
        // with, so the six cannot disagree about the button width, the gap or the row's left
        // edge. Taking the row from `bottom_band` is what stops the buttons floating up at a
        // tall frame (the old `rect.height - 40`) or clipping at a short one.
        let button_band = super::message_box::inset_action_band(
            ControlMetrics::bottom_band(rect, dimensions::DIALOG_BUTTON_HEIGHT),
            rect.y,
        );
        let labels = Self::action_labels();
        let row =
            super::message_box::action_row_geometry(context, font, &labels, button_band, true);
        let ok_rect = row.buttons[0];
        context.fill_rect(ok_rect, accent);
        context.draw_text_line_cow(
            ok_rect,
            &labels[0],
            font,
            accent_ink,
            HorizontalAlignment::Center,
        );
        let cancel_rect = row.buttons[1];
        context.fill_rect(cancel_rect, surface.blend(&ink, 0.1));
        context.draw_rect(cancel_rect, border);
        context.draw_text_line_cow(cancel_rect, &labels[1], font, ink, HorizontalAlignment::Center);
        if let Some(focused) = self
            .focus
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| row.buttons.get(index))
        {
            context.draw_rounded_rect_stroke(*focused, 2, accent, 2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;
    use std::sync::{Arc, Mutex};

    #[test]
    fn get_int_clamps_and_sets_integer_mode() {
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 500, 0, 100, 5);
        assert_eq!(dialog.mode(), InputMode::Integer);
        assert_eq!(dialog.int_value(), 100);
    }

    /// Regression: `min`/`max` are `get_int` *parameters*, so a caller can cross
    /// them. `i64::clamp` panics on `min > max`, and it did so here during
    /// construction — before the dialog existed to be inspected. The pair is now
    /// ordered as it is stored, so a descending range behaves like its ascending
    /// twin.
    #[test]
    fn get_int_with_crossed_bounds_does_not_panic() {
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 500, 100, 0, 5);
        assert_eq!(dialog.int_value(), 100, "500 clamps into the ordered span [0, 100]");
        assert_eq!(dialog.mode(), InputMode::Integer);

        // A value below both bounds clamps up, in either written order.
        let dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", -50, 100, 0, 5);
        assert_eq!(dialog.int_value(), 0);
    }

    /// The same defect through the mutator, which clamps against whatever the
    /// dialog is currently holding.
    #[test]
    fn set_int_value_survives_a_collapsed_range() {
        let mut dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 0, 0, 0, 1);
        dialog.set_int_value(42);
        assert_eq!(dialog.int_value(), 0, "the collapsed span [0, 0] pulls the value to 0");

        let mut dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 0, 10, 10, 1);
        dialog.set_int_value(-5);
        assert_eq!(dialog.int_value(), 10, "the collapsed span [10, 10] pulls the value to 10");
    }

    /// `set_double_value` rejects `NaN` outright rather than clamping it. Keep
    /// that pinned: `NaN` compares false against every bound, so "clamping" it
    /// would store a value the control cannot place inside its own range.
    #[test]
    fn set_double_value_rejects_nan_and_clamps_otherwise() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        dialog.set_double_value(2.5);
        assert_eq!(dialog.double_value(), 2.5);
        dialog.set_double_value(f64::NAN);
        assert_eq!(dialog.double_value(), 2.5, "NaN must leave the stored value untouched");
    }

    #[test]
    fn clicking_ok_accepts_and_clicking_cancel_rejects() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        let accepted = Arc::new(Mutex::new(0usize));
        let rejected = Arc::new(Mutex::new(0usize));

        let a = Arc::clone(&accepted);
        dialog.accepted.connect(move || {
            if let Ok(mut n) = a.lock() {
                *n += 1;
            }
        });
        let r = Arc::clone(&rejected);
        dialog.rejected.connect(move || {
            if let Ok(mut n) = r.lock() {
                *n += 1;
            }
        });

        dialog.show();
        let ok = dialog.action_buttons()[0].center();
        dialog.handle_event(&Event::mouse_press(ok.x, ok.y, 1));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        let cancel = dialog.action_buttons()[1].center();
        dialog.handle_event(&Event::mouse_press(cancel.x, cancel.y, 1));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn enter_and_escape_emit_accept_reject() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        let accepted = Arc::new(Mutex::new(0usize));
        let rejected = Arc::new(Mutex::new(0usize));

        let a = Arc::clone(&accepted);
        dialog.accepted.connect(move || {
            if let Ok(mut n) = a.lock() {
                *n += 1;
            }
        });

        let r = Arc::clone(&rejected);
        dialog.rejected.connect(move || {
            if let Ok(mut n) = r.lock() {
                *n += 1;
            }
        });

        dialog.show();
        dialog.handle_event(&Event::key_press(13, 0));
        assert_eq!(*accepted.lock().expect("accepted lock"), 1);
        assert!(!dialog.is_visible());

        dialog.show();
        dialog.handle_event(&Event::key_press(27, 0));
        assert_eq!(*rejected.lock().expect("rejected lock"), 1);
        assert!(!dialog.is_visible());
    }

    #[test]
    fn the_text_mode_edits_at_the_caret() {
        let mut dialog = InputDialog::get_text(Rect::new(0, 0, 320, 180), "T", "L", "abc");
        dialog.move_text_caret(1);
        dialog.handle_event(&Event::TextInput { text: "Z".to_string() });
        assert_eq!(dialog.text_value(), "aZbc");
        assert_eq!(dialog.text_caret(), 2);
        dialog.handle_event(&Event::key_press(key_codes::BACKSPACE, 0));
        assert_eq!(dialog.text_value(), "abc");
        assert_eq!(dialog.text_caret(), 1);
        dialog.handle_event(&Event::key_press(key_codes::DELETE, 0));
        assert_eq!(dialog.text_value(), "ac");
    }

    #[test]
    fn clicking_the_field_positions_the_caret() {
        let mut dialog = InputDialog::get_text(Rect::new(0, 0, 320, 180), "T", "L", "abcdef");
        let band = dialog.input_band();
        let advance = effective_font(dialog.base.style()).size().ceil().max(1.0) as i32;
        dialog.handle_event(&Event::mouse_press(band.x + 4 + advance * 3, band.y + 4, 1));
        assert_eq!(dialog.text_caret(), 3);
    }

    #[test]
    fn the_integer_step_keys_use_the_configured_step() {
        let mut dialog = InputDialog::get_int(Rect::new(0, 0, 320, 180), "T", "L", 10, 0, 100, 5);
        dialog.handle_event(&Event::key_press(key_codes::UP, 0));
        assert_eq!(dialog.int_value(), 15);
        dialog.handle_event(&Event::key_press(key_codes::DOWN, 0));
        assert_eq!(dialog.int_value(), 10);
    }

    #[test]
    fn the_item_mode_dropdown_opens_and_selects() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 240));
        dialog.set_mode(InputMode::Item);
        dialog.set_items(vec!["One".to_string(), "Two".to_string(), "Three".to_string()]);
        let band = dialog.input_band();
        dialog.handle_event(&Event::mouse_press(band.x + 4, band.y + 4, 1));
        assert!(dialog.is_dropdown_open());
        let rows = dialog.dropdown_rows();
        assert_eq!(rows.len(), 3);
        let (_, second) = rows[1];
        dialog.handle_event(&Event::mouse_press(second.x + 2, second.y + 2, 1));
        assert_eq!(dialog.current_item(), 1);
        assert!(!dialog.is_dropdown_open());
        assert_eq!(dialog.current_item_text(), Some("Two"));
    }

    #[test]
    fn the_numeric_range_step_and_decimals_are_configurable() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        dialog.set_int_range(100, 0);
        assert_eq!(dialog.int_range(), (0, 100), "a descending pair is ordered as stored");
        dialog.set_int_value(500);
        assert_eq!(dialog.int_value(), 100, "the value clamps into the configured range");
        dialog.set_int_step(7);
        assert_eq!(dialog.int_step(), 7);
        dialog.set_mode(InputMode::Integer);
        dialog.handle_event(&Event::key_press(key_codes::UP, 0));
        assert_eq!(dialog.int_value(), 100, "the step still respects the range");

        dialog.set_mode(InputMode::Double);
        dialog.set_double_range(-1.0, 1.0);
        assert_eq!(dialog.double_range(), (-1.0, 1.0));
        dialog.set_double_value(5.0);
        assert_eq!(dialog.double_value(), 1.0);
        dialog.set_double_step(0.25);
        dialog.handle_event(&Event::key_press(key_codes::DOWN, 0));
        assert_eq!(dialog.double_value(), 0.75, "the configured step drives the arrow keys");
        dialog.set_double_decimals(3);
        assert_eq!(dialog.double_decimals(), 3);
    }

    #[test]
    fn tab_moves_the_focus_ring_and_announces_it() {
        let mut dialog = InputDialog::new(Rect::new(0, 0, 320, 180));
        let seen = Arc::new(Mutex::new(0usize));
        let sink = Arc::clone(&seen);
        dialog.focus_changed.connect(move |_| {
            *sink.lock().expect("lock") += 1;
        });
        dialog.handle_event(&Event::key_press(key_codes::TAB, 0));
        assert_eq!(*seen.lock().expect("lock"), 1);
    }
}
