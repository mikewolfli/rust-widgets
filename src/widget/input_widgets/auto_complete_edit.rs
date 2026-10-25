// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AutoCompleteEdit widget — a text input with an auto-completion dropdown.
//!
//! The AutoCompleteEdit widget provides a text entry field that filters and
//! displays a dropdown list of suggestions as the user types. The user can
//! select a suggestion with the keyboard (Enter) or by clicking.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::key_codes;
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::shortcut::Key;
use crate::signal::Signal1;
use crate::undo::{UndoStack, ValueCommand};
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, expect_usize,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::input_widgets::shortcut_editor::primary_shortcut_pressed;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::rc::Rc;

/// A text input field with auto-completion dropdown support.
///
/// As the user types, the widget filters its suggestions list and shows
/// matching entries in a dropdown below the text field. The user can select
/// a highlighted suggestion via Enter key or by clicking.
/// How the typed text is matched against a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompletionMatchMode {
    /// The suggestion contains the typed text anywhere. The historical behaviour.
    #[default]
    Contains,
    /// The suggestion starts with the typed text.
    StartsWith,
    /// The suggestion ends with the typed text.
    EndsWith,
    /// The suggestion contains the typed text only as a whole word.
    FixedString,
    /// The typed text is a pattern, using the same grammar as
    /// [`PatternValidator`](super::input_validator::PatternValidator).
    ///
    /// # Supported syntax
    ///
    /// The engine is the crate's own subset (there is no regex dependency): literals and `\`
    /// escapes, `.`, classes `[abc]` / ranges `[a-z]` / negation `[^abc]`, groups `(...)`,
    /// alternation `a|b`, the quantifiers `*`, `+`, `?`, and the top-level anchors `^` and `$`.
    /// Back-references and look-around are not supported. Grouping and alternation are what
    /// make the commonest candidate filter — `"png|jpg|webp"` — expressible at all (I-05).
    RegExp,
}

/// What selecting a suggestion does to the drop-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompletionClosePolicy {
    /// The list closes as soon as a suggestion is accepted.
    #[default]
    Always,
    /// The list stays open; the host decides when to dismiss it.
    Never,
    /// The list stays open after an inline completion and closes on an explicit commit.
    OnCommit,
}

/// The published token naming a [`CompletionMatchMode`].
fn match_mode_token(mode: CompletionMatchMode) -> &'static str {
    match mode {
        CompletionMatchMode::Contains => "contains",
        CompletionMatchMode::StartsWith => "starts_with",
        CompletionMatchMode::EndsWith => "ends_with",
        CompletionMatchMode::FixedString => "fixed_string",
        CompletionMatchMode::RegExp => "regexp",
    }
}

/// The match mode a published token names, or `None` for an unknown token.
fn match_mode_from_token(token: &str) -> Option<CompletionMatchMode> {
    match token {
        "contains" => Some(CompletionMatchMode::Contains),
        "starts_with" => Some(CompletionMatchMode::StartsWith),
        "ends_with" => Some(CompletionMatchMode::EndsWith),
        "fixed_string" => Some(CompletionMatchMode::FixedString),
        "regexp" => Some(CompletionMatchMode::RegExp),
        _ => None,
    }
}

/// The published token naming a [`CompletionClosePolicy`].
fn close_policy_token(policy: CompletionClosePolicy) -> &'static str {
    match policy {
        CompletionClosePolicy::Always => "always",
        CompletionClosePolicy::Never => "never",
        CompletionClosePolicy::OnCommit => "on_commit",
    }
}

/// The close policy a published token names, or `None` for an unknown token.
fn close_policy_from_token(token: &str) -> Option<CompletionClosePolicy> {
    match token {
        "always" => Some(CompletionClosePolicy::Always),
        "never" => Some(CompletionClosePolicy::Never),
        "on_commit" => Some(CompletionClosePolicy::OnCommit),
        _ => None,
    }
}

/// A text field that filters a suggestion list as the user types.
pub struct AutoCompleteEdit {
    base: BaseWidget,
    text: String,
    suggestions: Vec<String>,
    filtered_suggestions: Vec<String>,
    show_dropdown: bool,
    selected_suggestion: Option<usize>,
    max_visible: usize,
    /// The insertion point, in bytes from the start of the value.
    cursor: usize,
    /// The other end of a selection, in bytes. `None` means no selection.
    selection_anchor: Option<usize>,
    /// How the typed text is matched against the suggestion list.
    match_mode: CompletionMatchMode,
    /// Whether matching distinguishes case.
    case_sensitive: bool,
    /// When true, the completion is shown inline after the caret instead of only in the list.
    inline_completion: bool,
    /// What accepting a suggestion does to the list.
    close_policy: CompletionClosePolicy,
    /// The suffix an inline completion would add, derived from the selection and the text.
    inline_suffix: String,
    /// How the field's text is aligned within its own box.
    ///
    /// Horizontal only: the text is centred vertically in the field, so a `top`/`bottom` value
    /// would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted when the text content changes.
    pub text_changed: Signal1<String>,
    /// Emitted when a suggestion is selected from the dropdown.
    pub suggestion_selected: Signal1<String>,
    undo_stack: UndoStack,
    history_target: Rc<RefCell<String>>,
    restoring_history: bool,
    /// The hint painted while the field is empty, or `""` for no hint (I-04).
    ///
    /// Defaults to empty: the field used to hardcode "Type to search...", which a host could
    /// neither replace nor switch off.
    placeholder: String,
    /// Bumped whenever the value's bytes change, so the memoised measurement below is keyed on
    /// something that cannot go stale (I-P03).
    text_revision: u64,
    /// `(revision, font, advance, characters)` for the current value.
    ///
    /// The pointer hit test used to construct a zero-sized paint backend and shape the whole
    /// value on every press (I-P03). It only needs the total advance and the character count —
    /// pure functions of the text and the font — so they are measured once per revision here.
    measure_cache: RefCell<(u64, Option<Font>, i32, usize)>,
}

impl AutoCompleteEdit {
    /// Creates a new AutoCompleteEdit widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AutoCompleteEdit, geometry, "AutoCompleteEdit"),
            text: String::new(),
            suggestions: Vec::new(),
            filtered_suggestions: Vec::new(),
            show_dropdown: false,
            selected_suggestion: None,
            max_visible: 5,
            cursor: 0,
            selection_anchor: None,
            match_mode: CompletionMatchMode::Contains,
            case_sensitive: false,
            inline_completion: false,
            close_policy: CompletionClosePolicy::Always,
            inline_suffix: String::new(),
            alignment: crate::core::Alignment::Left,
            text_changed: Signal1::new(),
            suggestion_selected: Signal1::new(),
            undo_stack: UndoStack::new(),
            history_target: Rc::new(RefCell::new(String::new())),
            restoring_history: false,
            placeholder: String::new(),
            text_revision: 0,
            measure_cache: RefCell::new((0, None, 0, 0)),
        }
    }

    /// Returns the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// How the field's text is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's text is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the text is centred
    /// vertically in the field by the field's own layout — the property route refuses it through
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }

    /// Sets the text content, updates the filtered suggestions list,
    /// and emits the `text_changed` signal when the value actually moved.
    ///
    /// The new value is moved in rather than cloned three times, and the signal is a single
    /// clone emitted only on a real change (I-P02).
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        let before = core::mem::replace(&mut self.text, text);
        self.publish_text_change(before);
        self.cursor = self.text.len();
        self.selection_anchor = None;
    }

    /// Publishes a mutation the caller already applied to `self.text`.
    ///
    /// `before` is the pre-edit value, moved in so the undo record does not need another copy
    /// of the string (I-P02). Re-filtering and redrawing happen here because every edit path
    /// owes them.
    fn publish_text_change(&mut self, before: String) {
        let changed = before != self.text;
        self.text_revision = self.text_revision.wrapping_add(1);
        if !self.restoring_history {
            *self.history_target.borrow_mut() = self.text.clone();
            self.undo_stack.push(Box::new(ValueCommand::new(
                self.history_target.clone(),
                before,
                self.text.clone(),
                "Edit text",
                "auto_complete_text",
            )));
        }
        self.filter_suggestions();
        if changed {
            self.text_changed.emit(self.text.clone());
        }
        self.base.request_redraw();
    }

    /// The hint painted while the field is empty, or `""` for none (I-04).
    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    /// Sets the hint painted while the field is empty.
    ///
    /// An empty string paints nothing, which is what makes the hint optional rather than a
    /// hardcoded sentence (I-04).
    pub fn set_placeholder(&mut self, placeholder: &str) {
        if self.placeholder == placeholder {
            return;
        }
        self.placeholder.clear();
        self.placeholder.push_str(placeholder);
        self.base.request_redraw();
    }

    /// The caret's byte offset in the value.
    pub fn cursor_position(&self) -> usize {
        self.cursor
    }

    /// Moves the caret, clearing the selection.
    pub fn set_cursor_position(&mut self, position: usize) {
        let position = crate::widget::text_utils::floor_char_boundary(
            &self.text,
            position.min(self.text.len()),
        );
        if self.cursor == position && self.selection_anchor.is_none() {
            return;
        }
        self.cursor = position;
        self.selection_anchor = None;
        self.base.request_redraw();
    }

    /// How the typed text is matched against suggestions.
    pub fn match_mode(&self) -> CompletionMatchMode {
        self.match_mode
    }

    /// Sets the match mode and re-filters the list.
    pub fn set_match_mode(&mut self, mode: CompletionMatchMode) {
        if self.match_mode == mode {
            return;
        }
        self.match_mode = mode;
        self.filter_suggestions();
        self.base.request_redraw();
    }

    /// Whether matching distinguishes case.
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Sets case sensitivity and re-filters the list.
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive == case_sensitive {
            return;
        }
        self.case_sensitive = case_sensitive;
        self.filter_suggestions();
        self.base.request_redraw();
    }

    /// Whether completions are offered inline at the caret.
    pub fn is_inline_completion(&self) -> bool {
        self.inline_completion
    }

    /// Sets whether completions are offered inline at the caret.
    pub fn set_inline_completion(&mut self, inline: bool) {
        if self.inline_completion == inline {
            return;
        }
        self.inline_completion = inline;
        self.filter_suggestions();
        self.base.request_redraw();
    }

    /// What accepting a suggestion does to the list.
    pub fn close_policy(&self) -> CompletionClosePolicy {
        self.close_policy
    }

    /// Sets what accepting a suggestion does to the list.
    pub fn set_close_policy(&mut self, policy: CompletionClosePolicy) {
        if self.close_policy == policy {
            return;
        }
        self.close_policy = policy;
    }

    /// The suffix an inline completion would add, empty when none is offered.
    pub fn inline_suffix(&self) -> &str {
        &self.inline_suffix
    }

    /// Appends committed text to the field and re-runs completion (D09-INPUT-01).
    ///
    /// The single append entry point shared by the `KeyPress` printable arm and the
    /// `TextInput`/`ImeCommit` branch: control characters are filtered, and the result drives
    /// `set_text` so the filtered suggestions, the undo push and the `text_changed` signal all
    /// behave exactly as they do for a typed character.
    fn append_committed_text(&mut self, text: &str) {
        let filtered: String = text.chars().filter(|c| !c.is_control()).collect();
        if filtered.is_empty() {
            return;
        }
        let (start, end) = self.selection_anchor.map_or((self.cursor, self.cursor), |anchor| {
            if anchor <= self.cursor {
                (anchor, self.cursor)
            } else {
                (self.cursor, anchor)
            }
        });
        // Edited in place: the value is not cloned into a scratch buffer for every key (I-P02).
        let before = self.text.clone();
        self.text.replace_range(start..end, &filtered);
        self.cursor = start + filtered.len();
        self.selection_anchor = None;
        self.publish_text_change(before);
    }

    /// Adds a single suggestion to the suggestion list.
    pub fn add_suggestion(&mut self, suggestion: String) {
        self.suggestions.push(suggestion);
        self.base.request_redraw();
    }

    /// Removes a suggestion by value. Returns true if the suggestion was found and removed.
    pub fn remove_suggestion(&mut self, suggestion: &str) -> bool {
        let idx = self.suggestions.iter().position(|s| s == suggestion);
        if let Some(pos) = idx {
            self.suggestions.remove(pos);
            self.filter_suggestions();
            self.base.request_redraw();
            true
        } else {
            false
        }
    }

    /// Replaces the entire suggestion list with the given vector.
    pub fn set_suggestions(&mut self, suggestions: Vec<String>) {
        self.suggestions = suggestions;
        self.filter_suggestions();
        self.base.request_redraw();
    }

    /// Returns a reference to the full suggestion list.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Removes all suggestions and hides the dropdown.
    pub fn clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.filtered_suggestions.clear();
        self.show_dropdown = false;
        self.selected_suggestion = None;
        self.base.request_redraw();
    }

    /// Steps back one text edit and returns `true`, or `false` when there is
    /// nothing to undo.
    ///
    /// Replaying history emits `text_changed` but deliberately does not push a
    /// new undo entry. The undo stack is **not** cleared, so a redo of the
    /// undone change remains available.
    pub fn undo(&mut self) -> bool {
        if self.undo_stack.undo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Steps forward one undone edit and returns `true`, or `false` when there is
    /// nothing to redo. Signal behaviour matches
    /// [`AutoCompleteEdit::undo`].
    pub fn redo(&mut self) -> bool {
        if self.undo_stack.redo().is_err() {
            return false;
        }
        self.restore_history_text();
        true
    }

    /// Returns `true` if [`AutoCompleteEdit::undo`] would change the text.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }
    /// Returns `true` if [`AutoCompleteEdit::redo`] would change the text.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    fn restore_history_text(&mut self) {
        self.restoring_history = true;
        self.text = self.history_target.borrow().clone();
        self.restoring_history = false;
        self.text_revision = self.text_revision.wrapping_add(1);
        self.filter_suggestions();
        self.text_changed.emit(self.text.clone());
        self.base.request_redraw();
    }

    /// Returns whether the dropdown is currently visible.
    pub fn is_showing_dropdown(&self) -> bool {
        self.show_dropdown
    }

    /// Returns the number of suggestions currently offered for the typed text.
    ///
    /// Counts the filtered list the dropdown draws, not the full suggestion set,
    /// so the number matches what the user can actually pick right now.
    pub fn suggestion_count(&self) -> usize {
        self.filtered_suggestions.len()
    }

    /// Shows the dropdown (if there are filtered suggestions).
    pub fn show_dropdown(&mut self) {
        if !self.filtered_suggestions.is_empty() {
            self.show_dropdown = true;
            self.selected_suggestion = Some(0);
            self.base.request_redraw();
        }
    }

    /// Hides the dropdown.
    pub fn hide_dropdown(&mut self) {
        self.show_dropdown = false;
        self.selected_suggestion = None;
        self.base.request_redraw();
    }

    /// Toggles the dropdown visibility.
    pub fn toggle_dropdown(&mut self) {
        if self.show_dropdown {
            self.hide_dropdown();
        } else {
            self.show_dropdown();
        }
    }

    /// Filters the suggestion list based on the current text.
    ///
    /// # Cost
    ///
    /// The query is folded **once** per keystroke and every candidate is compared byte-wise
    /// (`text_match`), so a pass allocates no per-candidate string (I-P05). The pattern mode
    /// compiles its pattern once and reuses it for the whole pass, instead of re-parsing it per
    /// candidate.
    fn filter_suggestions(&mut self) {
        self.inline_suffix.clear();
        if self.text.is_empty() {
            self.filtered_suggestions.clear();
            self.show_dropdown = false;
            self.selected_suggestion = None;
            return;
        }
        // Completion works on the word at the caret: a sentence's earlier words are context,
        // not part of what is being completed.
        let prefix = completion_prefix(&self.text);
        let needle = if self.case_sensitive {
            crate::compat::String::from(prefix)
        } else {
            crate::widget::input_widgets::text_match::fold(prefix)
        };
        // One compile for the whole pass; a malformed pattern matches nothing, exactly as the
        // per-candidate call it replaces did.
        let compiled = match self.match_mode {
            CompletionMatchMode::RegExp => {
                crate::widget::input_widgets::input_validator::CompiledPattern::compile(&needle)
            }
            _ => None,
        };
        self.filtered_suggestions.clear();
        for suggestion in &self.suggestions {
            let candidate: &str = suggestion;
            let matched = if self.case_sensitive {
                match self.match_mode {
                    CompletionMatchMode::Contains => candidate.contains(prefix),
                    CompletionMatchMode::StartsWith => candidate.starts_with(prefix),
                    CompletionMatchMode::EndsWith => candidate.ends_with(prefix),
                    CompletionMatchMode::FixedString => {
                        candidate.split(|ch: char| !ch.is_alphanumeric()).any(|word| word == prefix)
                    }
                    // The suggestion is the subject and the typed prefix is the pattern.
                    CompletionMatchMode::RegExp => {
                        compiled.as_ref().is_some_and(|pattern| pattern.is_match(candidate))
                    }
                }
            } else {
                use crate::widget::input_widgets::text_match as fold;
                match self.match_mode {
                    CompletionMatchMode::Contains => fold::contains(candidate, &needle),
                    CompletionMatchMode::StartsWith => fold::starts_with(candidate, &needle),
                    CompletionMatchMode::EndsWith => fold::ends_with(candidate, &needle),
                    CompletionMatchMode::FixedString => fold::contains_word(candidate, &needle),
                    CompletionMatchMode::RegExp => {
                        compiled.as_ref().is_some_and(|pattern| pattern.is_match(candidate))
                    }
                }
            };
            if matched {
                self.filtered_suggestions.push(suggestion.clone());
            }
        }
        if let (true, Some(first)) = (self.inline_completion, self.filtered_suggestions.first()) {
            // The inline hint is only the part the user has not typed, and only when the
            // suggestion actually extends the prefix at the caret.
            if let Some(rest) = first.strip_prefix(prefix) {
                self.inline_suffix.push_str(rest);
            }
        }
        if self.filtered_suggestions.is_empty() {
            self.show_dropdown = false;
            self.selected_suggestion = None;
        } else {
            self.show_dropdown = true;
            self.selected_suggestion = Some(0);
        }
    }

    /// Accepts the highlighted suggestion.
    pub fn accept_highlighted(&mut self) -> bool {
        if self.selected_suggestion.is_none() {
            return false;
        }
        self.select_highlighted();
        true
    }

    /// The selected byte range, or `None` when nothing is selected.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_anchor?;
        let (start, end) =
            if anchor <= self.cursor { (anchor, self.cursor) } else { (self.cursor, anchor) };
        (start != end).then_some((start, end))
    }

    /// The byte offset a horizontal pixel points at.
    ///
    /// Reads the memoised advance and character count for the current value instead of building
    /// a zero-sized paint backend and shaping the whole string on every press (I-P03).
    fn byte_index_at_x(&self, x: i32) -> usize {
        if self.text.is_empty() {
            return 0;
        }
        let rect = self.geometry();
        let font = effective_font(self.style());
        let (advance, chars) = self.measured_value(font);
        let origin = rect.x + 6;
        let dx = (x - origin).clamp(0, advance.max(1));
        let chars = chars.max(1) as f32;
        let boundary =
            ((dx as f32 * chars / advance.max(1) as f32).round() as usize).min(chars as usize);
        self.text.char_indices().nth(boundary).map(|(index, _)| index).unwrap_or(self.text.len())
    }

    /// The value's total advance and character count, memoised per revision and font.
    fn measured_value(&self, font: &Font) -> (i32, usize) {
        if let Ok(mut cache) = self.measure_cache.try_borrow_mut() {
            if cache.0 == self.text_revision && cache.1.as_ref() == Some(font) {
                return (cache.2, cache.3);
            }
            let advance = crate::widget::metrics::estimate_text_width(&self.text, font, 1.0) as i32;
            let chars = self.text.chars().count();
            cache.0 = self.text_revision;
            cache.1 = Some(font.clone());
            cache.2 = advance;
            cache.3 = chars;
            return (advance, chars);
        }
        (
            crate::widget::metrics::estimate_text_width(&self.text, font, 1.0) as i32,
            self.text.chars().count(),
        )
    }

    /// Moves the caret by `delta` characters, extending the selection when asked.
    fn move_caret(&mut self, delta: i32, extend: bool) {
        let mut target = self.cursor;
        if delta < 0 {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::previous_char_boundary(&self.text, target);
            }
        } else {
            for _ in 0..delta.unsigned_abs() {
                target = crate::widget::text_utils::next_char_boundary(&self.text, target);
            }
        }
        target = crate::widget::text_utils::floor_char_boundary(&self.text, target);
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        self.cursor = target;
        self.base.request_redraw();
    }

    /// Selects the highlighted suggestion and commits it as the current text.
    fn select_highlighted(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if let Some(suggestion) = self.filtered_suggestions.get(idx) {
                let selected = suggestion.clone();
                self.set_text(selected.clone());
                self.suggestion_selected.emit(selected);
                if self.close_policy != CompletionClosePolicy::Never {
                    self.hide_dropdown();
                }
                self.base.request_redraw();
            }
        }
    }

    /// Moves the selection highlight up (towards the first item).
    fn select_previous(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if idx > 0 {
                self.selected_suggestion = Some(idx - 1);
                self.base.request_redraw();
            }
        }
    }

    /// Moves the selection highlight down (towards the last item).
    fn select_next(&mut self) {
        if let Some(idx) = self.selected_suggestion {
            if idx + 1 < self.filtered_suggestions.len() {
                self.selected_suggestion = Some(idx + 1);
                self.base.request_redraw();
            }
        }
    }

    /// Returns the index of the highlighted suggestion, or `None` when none is.
    ///
    /// # Why this exists as a public accessor
    ///
    /// The dropdown draws a highlight and the arrow keys move it, but nothing outside the control could
    /// ask *where* it was — so the property contract could only publish `suggestion_count`. A driver
    /// that cannot read the highlight cannot confirm its own navigation worked.
    pub fn selected_suggestion(&self) -> Option<usize> {
        self.selected_suggestion
    }

    /// Highlights the suggestion at `index`, or clears the highlight if it is out of range.
    ///
    /// Out of range clears rather than being ignored: the caller asked for a highlight that cannot be
    /// shown, and leaving the previous one up would silently select a different suggestion than the one
    /// requested.
    pub fn set_selected_suggestion(&mut self, index: usize) {
        let next = if index < self.filtered_suggestions.len() { Some(index) } else { None };
        if self.selected_suggestion != next {
            self.selected_suggestion = next;
            self.base.request_redraw();
        }
    }

    /// Clears the highlight, leaving the dropdown open.
    ///
    /// Distinct from `hide_dropdown`, which also closes the list — the two are different facts and the
    /// property contract publishes them separately.
    pub fn clear_selected_suggestion(&mut self) {
        if self.selected_suggestion.is_some() {
            self.selected_suggestion = None;
            self.base.request_redraw();
        }
    }

    /// Returns how many rows the dropdown will show at once.
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// Sets how many rows the dropdown shows at once, floored at one.
    ///
    /// A zero would draw an open dropdown with no rows in it, which reads as a glitch rather than as a
    /// setting, so the floor is applied here rather than left to the caller.
    pub fn set_max_visible(&mut self, max_visible: usize) {
        let next = max_visible.max(1);
        if self.max_visible != next {
            self.max_visible = next;
            self.base.request_redraw();
        }
    }
}

impl Widget for AutoCompleteEdit {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "text_changed" => {
                Some(EventSignalRef::mapped("text_changed", &self.text_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "suggestion_selected" => Some(EventSignalRef::mapped(
                "suggestion_selected",
                &self.suggestion_selected,
                |v| CapabilityValue::String(v.clone()),
            )),
            _ => None,
        }
    }
}

/// `AutoCompleteEdit`'s property contract.
///
/// # The defect this replaces
///
/// The control published `suggestion_count` and nothing else — a *derived* number. A consumer driving
/// it could count the suggestions but could not read which one was highlighted, could not ask whether
/// the dropdown was actually open, and could not ask whether an undo was available, even though the
/// control answers all three (`selected_suggestion`, `show_dropdown`, `can_undo`). Those are the
/// facts a driver or a test needs; a count alone describes the list but not the control's *state*.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `suggestion_count` is derived
/// from the filtered list, so it is refused as read-only rather than reported as a
/// name this control does not know.
impl WidgetProperties for AutoCompleteEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder" => Ok(CapabilityValue::String(self.placeholder().to_string())),
            "suggestion_count" => Ok(CapabilityValue::UInt(self.suggestion_count() as u64)),
            // `Null` for "nothing highlighted", the same encoding `hovered_index`-style properties
            // use elsewhere in the crate: index 0 is a real selection and must not stand in for
            // "none".
            "selected_index" => match self.selected_suggestion() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_suggestion" => Ok(match self.selected_suggestion() {
                Some(index) => match self.filtered_suggestions.get(index) {
                    Some(text) => CapabilityValue::String(text.clone()),
                    None => CapabilityValue::Null,
                },
                None => CapabilityValue::Null,
            }),
            // `dropdown_visible` is the *state*; `suggestion_count` is the size of what it would show.
            // A consumer that only reads `suggestion_count > 0` is guessing.
            "dropdown_visible" => Ok(CapabilityValue::Bool(self.is_showing_dropdown())),
            "max_visible" => Ok(CapabilityValue::UInt(self.max_visible as u64)),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            // Publishing undo/redo availability is what lets a toolbar button bind to it; the
            // commands exist regardless, so a driver that cannot ask ends up issuing a no-op.
            "can_undo" => Ok(CapabilityValue::Bool(self.can_undo())),
            "can_redo" => Ok(CapabilityValue::Bool(self.can_redo())),
            "match_mode" => {
                Ok(CapabilityValue::String(match_mode_token(self.match_mode()).to_string()))
            }
            "case_sensitive" => Ok(CapabilityValue::Bool(self.is_case_sensitive())),
            "inline_completion" => Ok(CapabilityValue::Bool(self.is_inline_completion())),
            "close_policy" => {
                Ok(CapabilityValue::String(close_policy_token(self.close_policy()).to_string()))
            }
            // The word being completed: the text after the last whitespace. Derived from the
            // value, so it is published read-only right beside the mode that decides how it is
            // matched.
            "completion_prefix" => {
                Ok(CapabilityValue::String(completion_prefix(&self.text).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder" => {
                let text = expect_string(value)?;
                self.set_placeholder(&text);
                Ok(())
            }
            // Selecting an index is a real write the control already supported through
            // `set_selected_suggestion`; publishing it makes keyboard-style navigation drivable.
            "selected_index" => match value {
                CapabilityValue::Null => {
                    self.clear_selected_suggestion();
                    Ok(())
                }
                other => {
                    self.set_selected_suggestion(expect_usize(other)?);
                    Ok(())
                }
            },
            "max_visible" => {
                self.set_max_visible(expect_usize(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "match_mode" => {
                let token = expect_string(value)?;
                let mode =
                    match_mode_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_match_mode(mode);
                Ok(())
            }
            "case_sensitive" => {
                self.set_case_sensitive(expect_bool(value)?);
                Ok(())
            }
            "inline_completion" => {
                self.set_inline_completion(expect_bool(value)?);
                Ok(())
            }
            "close_policy" => {
                let token = expect_string(value)?;
                let policy =
                    close_policy_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_close_policy(policy);
                Ok(())
            }
            // Derived or read-only: the two counts describe the list, and the two booleans describe
            // what the control has already done.
            "suggestion_count"
            | "selected_suggestion"
            | "dropdown_visible"
            | "can_undo"
            | "can_redo"
            | "completion_prefix" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "text",
            "placeholder",
            "suggestion_count",
            "selected_index",
            "selected_suggestion",
            "dropdown_visible",
            "max_visible",
            "alignment",
            "can_undo",
            "can_redo",
            "match_mode",
            "case_sensitive",
            "inline_completion",
            "close_policy",
            "completion_prefix",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `auto_complete_edit` publishes.
    ///
    /// `set_text` assigns the edit contents and needs a payload, so it is answered
    /// through the property route — the capability publishes no zero-argument action.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_text" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for AutoCompleteEdit {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // Chrome colours resolve explicit style first, then the theme's resolved style
        // for this control, and only then fall back to a literal. Without the theme step
        // a light/dark switch would change nothing on screen, because the field fill and
        // its outline were previously hardcoded.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global
        // manager's mutex is not re-entrant.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("auto_complete_edit");
        let field_background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgba(180, 180, 180, 255));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The surface the field sits on, for the *disabled* rules below: a disabled ink recedes
        // toward the field's contrast colour and a disabled fill toward the window it is painted on.
        let window_fill = crate::style::resolved_theme_style_shared("window")
            .and_then(|theme| theme.background_color)
            .unwrap_or(field_background);
        // The list is chrome of the same family as the field it drops from: its fill
        // and its ink are the resolved field colours, so both move with the appearance.
        let dropdown_background = if style.background_color.is_some() || theme.is_some() {
            field_background
        } else {
            Color::rgba(255, 255, 255, 255)
        };
        let dropdown_text = text_color;

        // Background
        // A disabled field recedes toward the window rather than snapping to a fixed near-white
        // slab, which on a dark theme made a disabled field the brightest thing on screen.
        let bg_color = self.base.disabled_surface_near(field_background, window_fill);
        context.fill_rounded_rect(rect, 4, bg_color);

        // Border
        context.draw_rounded_rect_stroke(rect, 4, border_color, 1);

        // Draw text
        // The **effective font** — the resolved theme/caller font — so the value and the
        // suggestion rows honour the theme body font and the user's text scale (D09-STYLE-01).
        let font = effective_font(style);
        let padding = 6i32;
        let text_x = rect.x + padding;
        // The glyph origin is the line box's top edge, so the vertical anchor comes from
        // `text_line` rather than the fixed `padding + 13` that assumed a 13 px face.
        let input_line = context.text_line(rect, font);
        let text_y = input_line.y;

        // The hint is the host's string and may be empty, in which case an empty field paints
        // nothing beyond its chrome (I-04).
        let display_text =
            if self.text.is_empty() { self.placeholder.as_str() } else { &self.text };
        // Empty field = placeholder: the resolved ink damped toward the fill, so the
        // hint stays legible on either appearance.
        let input_text_color = if self.text.is_empty() {
            text_color.blend(&field_background, 0.4)
        } else {
            // Both branches step off the surface; the disabled one used a fixed grey that never
            // moved with the appearance.
            self.base.disabled_ink_on(text_color, bg_color)
        };
        context.draw_text(
            Point::new(text_x, text_y),
            display_text,
            font,
            input_text_color,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );

        // Inline completion: the completed remainder follows the caret, dimmed, so the
        // suggestion is readable at the insertion point rather than only in the list.
        if !self.text.is_empty() {
            let font_width = context.measure_text(&self.text, font).width as i32;
            if !self.inline_suffix.is_empty() {
                context.draw_text(
                    Point::new(text_x + font_width, text_y),
                    &self.inline_suffix,
                    font,
                    input_text_color.blend(&bg_color, 0.45),
                    HorizontalAlignment::Left,
                );
            }
            // The caret at its own byte offset, for the same reason the other fields draw one:
            // a focused text entry with no visible insertion point does not say where typing lands.
            if self.base.draws_focus_ring() {
                let caret = crate::widget::text_utils::floor_char_boundary(&self.text, self.cursor);
                let caret_x = text_x + context.measure_text(&self.text[..caret], font).width as i32;
                context.draw_line(
                    Point::new(caret_x, text_y),
                    Point::new(caret_x, text_y + input_line.height as i32),
                    text_color,
                );
            }
        }

        // Draw dropdown if visible
        if !self.show_dropdown || self.filtered_suggestions.is_empty() {
            return;
        }

        let drop_down_y = rect.y + rect.height as i32;
        let item_height = 24u32;
        let visible_count = self.filtered_suggestions.len().min(self.max_visible);
        let drop_down_height = item_height * visible_count as u32;
        let drop_rect = Rect::new(rect.x, drop_down_y, rect.width, drop_down_height);

        // Dropdown background
        context.fill_rounded_rect(drop_rect, 2, dropdown_background);
        context.draw_rounded_rect_stroke(
            drop_rect,
            2,
            border_color.blend(&dropdown_background, 0.3),
            1,
        );

        for i in 0..visible_count {
            let item_rect = Rect::new(
                rect.x + 1,
                drop_down_y + (i as i32) * (item_height as i32),
                rect.width.saturating_sub(2),
                item_height,
            );

            if Some(i) == self.selected_suggestion {
                context.fill_rounded_rect(
                    item_rect,
                    2,
                    dropdown_text.blend(&dropdown_background, 0.85),
                );
            }

            if let Some(suggestion) = self.filtered_suggestions.get(i) {
                let item_text_x = item_rect.x + 4;
                let item_line = context.text_line(item_rect, font);
                // Fitted to the row: a long suggestion used to paint past the popup's
                // own edge.
                let item_text_width = item_rect.width.saturating_sub(8).max(1);
                context.draw_text_fitted_cow(
                    Rect::new(item_text_x, item_line.y, item_text_width, item_line.height.max(1)),
                    suggestion,
                    font,
                    dropdown_text,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for AutoCompleteEdit {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } if *button == 1 => {
                let rect = self.geometry();
                // Check if click is inside the text field area
                if rect.contains_point(*pos) {
                    // Place the caret where the press landed, then offer the list.
                    self.cursor = self.byte_index_at_x(pos.x);
                    self.selection_anchor = None;
                    self.show_dropdown();
                }
                // Check if click is on a dropdown item
                if self.show_dropdown {
                    let item_height = 24i32;
                    let drop_down_y = rect.y + rect.height as i32;
                    let visible_count = self.filtered_suggestions.len().min(self.max_visible);
                    let drop_down_height = item_height * visible_count as i32;
                    let drop_rect =
                        Rect::new(rect.x, drop_down_y, rect.width, drop_down_height as u32);

                    if drop_rect.contains_point(*pos) {
                        let rel_y = pos.y - drop_down_y;
                        let idx = (rel_y / item_height) as usize;
                        if idx < self.filtered_suggestions.len() {
                            self.selected_suggestion = Some(idx);
                            self.select_highlighted();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                if *key == key_codes::ENTER {
                    if self.show_dropdown {
                        self.select_highlighted();
                    }
                } else if *key == 9 {
                    // Tab accepts an inline completion; with no inline hint it accepts the
                    // highlighted row, which is the keyboard path a completer needs.
                    self.accept_highlighted();
                } else if *key == key_codes::ESCAPE {
                    // Escape
                    if self.show_dropdown {
                        self.hide_dropdown();
                    }
                } else if *key == key_codes::UP && *modifiers == 0 && self.show_dropdown {
                    // Up arrow
                    self.select_previous();
                } else if *key == key_codes::DOWN && *modifiers == 0 && self.show_dropdown {
                    // Down arrow
                    self.select_next();
                } else if *key == 37 {
                    let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                        .contains(crate::shortcut::Modifiers::SHIFT);
                    self.move_caret(-1, shift);
                } else if *key == 39 {
                    let shift = crate::shortcut::Modifiers::from_event_bits(*modifiers)
                        .contains(crate::shortcut::Modifiers::SHIFT);
                    self.move_caret(1, shift);
                } else if *key == 36 {
                    self.set_cursor_position(0);
                } else if *key == 35 {
                    self.set_cursor_position(self.text.len());
                } else if *key == 127 {
                    let (start, end) = self.selection_range().unwrap_or_else(|| {
                        (
                            self.cursor,
                            crate::widget::text_utils::next_char_boundary(&self.text, self.cursor),
                        )
                    });
                    if start != end {
                        // In place, with the pre-edit value captured for the undo record only
                        // (I-P02).
                        let before = self.text.clone();
                        self.text.replace_range(start..end, "");
                        self.cursor = start;
                        self.selection_anchor = None;
                        self.publish_text_change(before);
                    }
                } else if primary_shortcut_pressed(*key, *modifiers, Key::Z) {
                    let _ = self.undo();
                } else if primary_shortcut_pressed(*key, *modifiers, Key::Y) {
                    let _ = self.redo();
                } else if *key >= 32 && *key <= 126 {
                    // Printable ASCII — append through the shared committed-text entry point so the
                    // keyboard path and the platform's `TextInput` path stay in lockstep.
                    if let Some(c) = char::from_u32(*key) {
                        self.append_committed_text(&c.to_string());
                    }
                } else if *key == key_codes::BACKSPACE {
                    // Backspace removes the selection, or the character before the caret.
                    let (start, end) = self.selection_range().unwrap_or_else(|| {
                        (
                            crate::widget::text_utils::previous_char_boundary(
                                &self.text,
                                self.cursor,
                            ),
                            self.cursor,
                        )
                    });
                    if start != end {
                        let before = self.text.clone();
                        self.text.replace_range(start..end, "");
                        self.cursor = start;
                        self.selection_anchor = None;
                        self.publish_text_change(before);
                    }
                }
            }
            // Platform-committed text (D09-INPUT-01). A printable character or an IME commit reaches
            // the widget as `TextInput`/`ImeCommit`, not as a `KeyPress`, so this branch is what
            // actually receives typing from the desktop backends. It funnels into the shared
            // `append_committed_text`, which filters control characters and drives `set_text`, the
            // same entry point the printable key arm uses. The `is_enabled` guard above already ran.
            Event::TextInput { text } | Event::ImeCommit { text } => {
                self.append_committed_text(text);
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// The word completion matches against: the text after the last whitespace.
fn completion_prefix(text: &str) -> &str {
    text.rsplit(|ch: char| ch.is_whitespace()).next().unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::svg::render_to_svg;
    use std::sync::{Arc, Mutex};

    #[test]
    fn auto_complete_edit_default_creation() {
        let edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(edit.kind(), WidgetKind::AutoCompleteEdit);
        assert!(edit.text().is_empty());
        assert!(edit.suggestions().is_empty());
        assert!(!edit.is_showing_dropdown());
        assert_eq!(edit.geometry(), Rect::new(0, 0, 200, 30));
    }

    #[test]
    fn auto_complete_edit_add_and_remove_suggestion() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Apple".to_string());
        edit.add_suggestion("Banana".to_string());
        edit.add_suggestion("Cherry".to_string());
        assert_eq!(edit.suggestions().len(), 3);

        assert!(edit.remove_suggestion("Banana"));
        assert_eq!(edit.suggestions().len(), 2);

        assert!(!edit.remove_suggestion("NonExistent"));
        assert_eq!(edit.suggestions().len(), 2);
    }

    #[test]
    fn auto_complete_edit_set_text_filters_suggestions() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec![
            "Apple".to_string(),
            "Banana".to_string(),
            "Apricot".to_string(),
            "Cherry".to_string(),
        ]);
        assert_eq!(edit.suggestions().len(), 4);

        edit.set_text("Ap".to_string());
        assert_eq!(edit.text(), "Ap");
        assert!(edit.is_showing_dropdown());

        edit.set_text("XYZ".to_string());
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_text_changed_signal() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        let captured = Arc::new(Mutex::new(None::<String>));
        edit.text_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        edit.set_text("Hello".to_string());
        assert_eq!(captured.lock().unwrap().as_deref(), Some("Hello"));
    }

    #[test]
    fn auto_complete_edit_suggestion_selected_signal() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec!["Option 1".to_string(), "Option 2".to_string()]);
        edit.set_text("Opt".to_string());

        let captured = Arc::new(Mutex::new(None::<String>));
        edit.suggestion_selected.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<String>| {
                *captured.lock().unwrap() = Some(val.to_string());
            }
        });

        // Simulate Enter key to select highlighted suggestion
        edit.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert_eq!(captured.lock().unwrap().as_deref(), Some("Option 1"));
    }

    #[test]
    fn auto_complete_edit_clear_suggestions() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Test".to_string());
        assert_eq!(edit.suggestions().len(), 1);

        edit.clear_suggestions();
        assert!(edit.suggestions().is_empty());
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_toggle_dropdown() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_suggestions(vec!["Item".to_string()]);
        edit.set_text("It".to_string());

        assert!(edit.is_showing_dropdown());
        edit.hide_dropdown();
        assert!(!edit.is_showing_dropdown());
        edit.show_dropdown();
        assert!(edit.is_showing_dropdown());
        edit.toggle_dropdown();
        assert!(!edit.is_showing_dropdown());
    }

    #[test]
    fn auto_complete_edit_svg_output() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Suggestion".to_string());
        edit.set_text("Sug".to_string());
        let svg = render_to_svg(&mut edit);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    /// The contract describes the control's *state*, not just the size of its list.
    ///
    /// # The defect this pins
    ///
    /// `suggestion_count` was the only property, and it is derived: a consumer could count the rows
    /// but could not ask whether the dropdown was open, which row was highlighted, or whether an undo
    /// was available — all three of which the control already knew.
    #[test]
    fn the_contract_reports_the_dropdown_and_highlight_state() {
        use crate::widget::capability::WidgetProperties;

        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        assert_eq!(edit.get("dropdown_visible").unwrap().as_bool(), Some(false));
        assert_eq!(edit.get("selected_index").unwrap(), CapabilityValue::Null);
        assert_eq!(edit.get("selected_suggestion").unwrap(), CapabilityValue::Null);
        assert_eq!(edit.get("max_visible").unwrap().as_u64(), Some(5));
        assert_eq!(edit.get("can_undo").unwrap().as_bool(), Some(false));

        edit.add_suggestion("Alpha".to_string());
        edit.add_suggestion("Alpine".to_string());
        edit.set_text("Al".to_string());

        // Typing opens the dropdown and highlights the first row, which the contract must now show.
        assert_eq!(edit.get("suggestion_count").unwrap().as_u64(), Some(2));
        assert_eq!(edit.get("dropdown_visible").unwrap().as_bool(), Some(true));
        assert_eq!(edit.get("selected_index").unwrap().as_u64(), Some(0));
        assert_eq!(edit.get("selected_suggestion").unwrap().as_str(), Some("Alpha"));

        // `selected_suggestion` is the *text* of the highlighted row; it follows the index.
        edit.set("selected_index", CapabilityValue::UInt(1)).unwrap();
        assert_eq!(edit.get("selected_suggestion").unwrap().as_str(), Some("Alpine"));

        // And `Null` index clears the highlight without closing the list, which is a different fact.
        edit.set("selected_index", CapabilityValue::Null).unwrap();
        assert_eq!(edit.get("selected_index").unwrap(), CapabilityValue::Null);
        assert_eq!(edit.get("dropdown_visible").unwrap().as_bool(), Some(true));
    }

    /// An index out of range clears the highlight rather than silently selecting a different row.
    #[test]
    fn selecting_an_index_out_of_range_clears_the_highlight() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Alpha".to_string());
        edit.set_text("Al".to_string());
        assert_eq!(edit.selected_suggestion(), Some(0));

        edit.set_selected_suggestion(99);
        assert_eq!(
            edit.selected_suggestion(),
            None,
            "an unreachable index must not leave the previous row highlighted"
        );
    }

    /// `max_visible` floors at one, so an open dropdown can never be asked to draw no rows.
    #[test]
    fn max_visible_floors_at_one_row() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_max_visible(0);
        assert_eq!(edit.max_visible(), 1);
        edit.set("max_visible", CapabilityValue::UInt(3)).unwrap();
        assert_eq!(edit.max_visible(), 3);
    }

    // ─── D09-INPUT-01: platform-committed text reaches the value ───

    /// A `TextInput` from the platform must be appended to the field.
    ///
    /// # The defect this pins (D09-INPUT-01)
    ///
    /// The desktop backends deliver a printable character or an IME commit as `Event::TextInput`,
    /// not as a `KeyPress`; the handler only matched `KeyPress`, so committed text was dropped. The
    /// test feeds `TextInput` to prove that path itself is wired.
    #[test]
    fn text_input_is_appended_to_the_value() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.handle_event(&Event::TextInput { text: "hé".to_string() });
        assert_eq!(edit.text(), "hé");
    }

    /// An IME commit is appended the same way.
    #[test]
    fn ime_commit_is_appended_to_the_value() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.handle_event(&Event::ime_commit("你好"));
        assert_eq!(edit.text(), "你好");
    }

    /// Committed text re-runs completion and emits `text_changed`.
    #[test]
    fn text_input_emits_text_changed_and_filters() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.add_suggestion("Apple".to_string());
        edit.add_suggestion("Banana".to_string());
        let last = std::sync::Arc::new(Mutex::new(String::new()));
        edit.text_changed.connect({
            let last = std::sync::Arc::clone(&last);
            move |text| *last.lock().unwrap() = text.to_string()
        });
        edit.handle_event(&Event::TextInput { text: "Ap".to_string() });
        assert_eq!(edit.text(), "Ap");
        assert_eq!(*last.lock().unwrap(), "Ap");
        assert!(edit.is_showing_dropdown(), "the committed text must re-run completion");
    }

    /// A disabled field ignores committed text.
    #[test]
    fn disabled_ignores_text_input() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 30));
        edit.set_enabled(false);
        edit.handle_event(&Event::TextInput { text: "X".to_string() });
        assert_eq!(edit.text(), "");
    }

    /// Match modes and case sensitivity decide which suggestions are offered.
    #[test]
    fn match_modes_choose_the_filter() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 28));
        edit.set_suggestions(vec!["alpha".to_string(), "beta".to_string(), "alphabet".to_string()]);
        edit.set_match_mode(CompletionMatchMode::StartsWith);
        edit.set_text("alph");
        assert_eq!(edit.filtered_suggestions.len(), 2);

        edit.set_match_mode(CompletionMatchMode::EndsWith);
        assert_eq!(edit.filtered_suggestions.len(), 0);
        edit.set_case_sensitive(true);
        edit.set_text("ALPH");
        assert!(edit.filtered_suggestions.is_empty(), "case sensitivity is enforced");
    }

    /// The pattern mode treats the typed text as a pattern over each suggestion.
    #[test]
    fn the_pattern_mode_matches_patterns() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 28));
        edit.set_suggestions(vec!["alpha".to_string(), "alpine".to_string(), "beta".to_string()]);
        edit.set_match_mode(CompletionMatchMode::RegExp);
        assert_eq!(edit.get("match_mode").unwrap(), CapabilityValue::String("regexp".to_string()));

        edit.set_text("e$");
        assert_eq!(edit.filtered_suggestions, vec!["alpine".to_string()]);
        edit.set_text("^b[a-z]ta$");
        assert_eq!(edit.filtered_suggestions, vec!["beta".to_string()]);
        edit.set_text("[invalid");
        assert!(
            edit.filtered_suggestions.is_empty(),
            "a malformed pattern matches nothing rather than everything"
        );
    }

    /// The caret edits mid-value, and an inline completion exposes its suffix.
    #[test]
    fn the_caret_and_inline_completion_work() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 28));
        edit.set_suggestions(vec!["alpha".to_string()]);
        edit.set_text("alp");
        assert_eq!(edit.inline_suffix(), "");
        edit.set_inline_completion(true);
        assert_eq!(edit.inline_suffix(), "ha");

        edit.set_text("helo");
        edit.handle_event(&Event::KeyPress { key: 37, modifiers: 0 });
        edit.handle_event(&Event::KeyPress { key: 108, modifiers: 0 });
        assert_eq!(edit.text(), "hello");
        assert_eq!(edit.cursor_position(), 4);
    }

    /// Accepting a suggestion honours the close policy.
    #[test]
    fn the_close_policy_controls_the_popup() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 28));
        edit.set_suggestions(vec!["alpha".to_string()]);
        edit.set_text("al");
        edit.set_close_policy(CompletionClosePolicy::Never);
        assert!(edit.accept_highlighted());
        assert!(edit.is_showing_dropdown(), "Never keeps the list open");
        assert_eq!(edit.text(), "alpha");
    }

    /// The matching and popup knobs are published by token and round-trip.
    #[test]
    fn the_completion_contract_is_published() {
        let mut edit = AutoCompleteEdit::new(Rect::new(0, 0, 200, 28));
        edit.set_text("alpha al");
        assert_eq!(
            edit.get("match_mode").unwrap(),
            CapabilityValue::String("contains".to_string())
        );
        assert_eq!(
            edit.get("completion_prefix").unwrap(),
            CapabilityValue::String("al".to_string()),
            "the completed word is the text after the last whitespace"
        );

        edit.set("match_mode", CapabilityValue::String("starts_with".to_string())).unwrap();
        assert_eq!(edit.match_mode(), CompletionMatchMode::StartsWith);
        assert_eq!(
            edit.set("match_mode", CapabilityValue::String("fuzzy".to_string())),
            Err(CapabilityAccessError::OutOfRange),
            "an unsupported mode is refused, not approximated"
        );

        edit.set("case_sensitive", CapabilityValue::Bool(true)).unwrap();
        assert!(edit.is_case_sensitive());
        edit.set("inline_completion", CapabilityValue::Bool(true)).unwrap();
        assert!(edit.is_inline_completion());
        edit.set("close_policy", CapabilityValue::String("on_commit".to_string())).unwrap();
        assert_eq!(edit.close_policy(), CompletionClosePolicy::OnCommit);

        assert_eq!(
            edit.set("completion_prefix", CapabilityValue::String("x".to_string())),
            Err(CapabilityAccessError::ReadOnlyProperty),
            "the prefix follows the value"
        );
    }
}
