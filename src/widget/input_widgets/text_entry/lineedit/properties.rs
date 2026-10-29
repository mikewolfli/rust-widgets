// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! [`LineEdit`]'s validation plumbing, decoration setters and property contract.

use super::model::*;

use crate::compat::{Box, String, ToString};
use crate::platform::EchoMode;

use crate::property_names_of;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_string, expect_usize,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::decorations::{DecorationLayout, DecorationSlots};
use crate::widget::input_widgets::input_validator::{InputValidator, ValidationState, Validator};
use crate::widget::input_widgets::validation_bubble::{ValidationBubble, ValidationSeverity};

/// The published token naming a [`ValidationState`].
pub(super) fn validation_state_token(state: ValidationState) -> &'static str {
    match state {
        ValidationState::Invalid => "invalid",
        ValidationState::Intermediate => "intermediate",
        ValidationState::Acceptable => "acceptable",
    }
}

/// The [`EchoMode`] a published token names, or `None` for an unknown token.
pub(super) fn echo_mode_from_token(token: &str) -> Option<EchoMode> {
    match token {
        "normal" => Some(EchoMode::Normal),
        "password" => Some(EchoMode::Password),
        "no_echo" => Some(EchoMode::NoEcho),
        _ => None,
    }
}

impl LineEdit {
    /// The rule this field enforces, or `None` for a free-form field.
    pub fn validator(&self) -> Option<&dyn Validator> {
        self.validator.as_deref()
    }

    /// Installs a host rule, replacing any current one, and revalidates the value.
    ///
    /// This is the extension point for a domain rule the library cannot enumerate. A built-in
    /// grammar assigned through [`Self::set_validator`] still works: both paths store the rule
    /// in the same box, so the field consults exactly one validator at a time.
    pub fn set_dyn_validator(&mut self, validator: Option<Box<dyn Validator>>) {
        // A built-in assigned twice is the one case that can be recognised without downcasting,
        // so re-applying it does not revalidate and repaint for nothing.
        let unchanged = match (self.validator.as_deref(), validator.as_deref()) {
            (Some(current), Some(next)) => {
                current.builtin().is_some() && current.builtin() == next.builtin()
            }
            _ => false,
        };
        if unchanged {
            return;
        }
        self.validator = validator;
        // The rule changed, so a memoised answer for the current revision no longer applies.
        self.validation_revision = None;
        self.revalidate();
        self.base.request_redraw();
    }

    /// Sets the built-in grammar, revalidating the current value.
    ///
    /// The `phone_input` / `email_input` constructors are the usual callers; a host that wants
    /// one control to switch grammars can use it directly.
    pub fn set_validator(&mut self, validator: Option<InputValidator>) {
        match (self.validator.as_deref(), validator) {
            (None, None) => return,
            (Some(current), Some(next)) if current.builtin() == Some(next) => return,
            _ => {}
        }
        self.validator = validator.map(|rule| Box::new(rule) as Box<dyn Validator>);
        self.validation_revision = None;
        self.revalidate();
        self.base.request_redraw();
    }

    /// Whether the current value satisfies the field's rule (true without a validator).
    pub fn is_valid(&self) -> bool {
        self.validation_state.is_acceptable()
    }

    /// The three-way outcome for the current value.
    ///
    /// `Intermediate` is not a failure: it is a value the user is still assembling, and a form
    /// can keep its submit affordance disabled without accusing the user. `Acceptable` is the
    /// state a field without a validator always reports.
    pub fn validation_state(&self) -> ValidationState {
        self.validation_state
    }

    /// The message for the current non-acceptable value, or `""` when it is acceptable.
    pub fn validation_message(&self) -> &'static str {
        self.validation_message
    }

    /// The message for the current non-acceptable value, or `""` when it is acceptable.
    ///
    /// Prefers the rule's value-specific text ([`Validator::message_for`]), so a range refusal can
    /// name the bound and the value that missed it (I-27).
    pub fn validation_message_text(&self) -> &str {
        match self.validation_message_detail.as_deref() {
            Some(detail) => detail,
            None => self.validation_message,
        }
    }

    /// The field-anchored message bubble this control keeps in step with its rule.
    ///
    /// The bubble is mounted by the host (it floats outside the field's own rectangle), and the
    /// field writes its message, severity and anchor. Reading it is how a host that lays out its
    /// own validation chrome knows what to show (I-029).
    pub fn validation_bubble(&self) -> &ValidationBubble {
        &self.validation_bubble
    }

    /// The field-anchored message bubble, for placement by the host.
    pub fn validation_bubble_mut(&mut self) -> &mut ValidationBubble {
        &mut self.validation_bubble
    }

    /// Writes the current rule outcome into the bubble.
    ///
    /// An invalid value shows the rule's message as an error; an unfinished or acceptable value
    /// hides the bubble, matching the field's own rule that an incomplete value is not an
    /// accusation. The anchor is the field's own rectangle, so the bubble floats beside the
    /// control it accuses.
    pub(super) fn sync_validation_bubble(&mut self) {
        let anchor = self.base.geometry();
        self.validation_bubble.set_anchor(anchor);
        match self.validation_state {
            ValidationState::Invalid => {
                let text = self.validation_message_text().to_string();
                self.validation_bubble.show_message(&text, ValidationSeverity::Error);
            }
            _ => self.validation_bubble.hide(),
        }
    }

    /// Applies the validator's own repair to the current value.
    ///
    /// Returns whether the value changed. The repair runs through [`Self::set_text`], so it is
    /// undoable and emits `text_changed` like any other programmatic write. A rule with no
    /// repair, or a value already in its nearest form, returns `false`.
    pub fn fixup(&mut self) -> bool {
        let Some(validator) = self.validator.as_deref() else {
            return false;
        };
        let Some(repaired) = validator.fixup(&self.text) else {
            return false;
        };
        if repaired == self.text {
            return false;
        }
        // A repair is an edit the field performs for the user, so it stays undoable: undoing it
        // must put back the value the user actually typed.
        self.apply_text(repaired, EditOrigin::User);
        true
    }

    /// Recomputes validity and mirrors it into the `error` decoration.
    ///
    /// Only a **definite** refusal is painted as an error. An intermediate value is one the
    /// user is still typing, and painting it red turns every entry into an accusation — the
    /// three-way state exists precisely so the two cases can be told apart.
    ///
    /// Only a message the validator wrote is cleared here: a caller's own refusal text must
    /// survive a value that happens to satisfy the grammar.
    pub(super) fn revalidate(&mut self) {
        // One rule evaluation per value: re-entrant calls for the same revision reuse the answer
        // (I-008). A validator swap clears the marker so the new rule runs immediately.
        if self.validation_revision == Some(self.text_revision) {
            return;
        }
        self.validation_revision = Some(self.text_revision);
        // The rule is consulted in two short borrows and its answers copied out, so the
        // decoration writes below never overlap the borrow of `self.validator`.
        let state = match self.validator.as_deref() {
            Some(validator) => validator.state(&self.text),
            None => ValidationState::Acceptable,
        };
        let mut message = "";
        let mut detail: Option<String> = None;
        if let Some(validator) = self.validator.as_deref() {
            if !state.is_acceptable() {
                message = validator.message();
                // A value-specific message is held only when it actually says something else, so
                // a rule that inherits the static text costs no allocation (I-27).
                let quoted = validator.message_for(&self.text);
                if quoted.as_ref() != message {
                    detail = Some(quoted.into_owned());
                }
            }
        }
        self.validation_state = state;
        self.validation_message = message;
        self.validation_message_detail = detail;
        self.sync_validation_bubble();
        if state.is_invalid() {
            let shown = self.validation_message_text();
            if self.decorations.error.as_str() != shown {
                let mut next = self.decorations.clone();
                next.error = shown.to_string();
                self.set_decorations(next);
            }
            self.validation_error_active = true;
        } else if self.validation_error_active {
            self.validation_error_active = false;
            if !self.decorations.error.is_empty() {
                let mut next = self.decorations.clone();
                next.error.clear();
                self.set_decorations(next);
            }
        }
    }
}

impl LineEdit {
    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the value is centred
    /// vertically in the field band by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this setter
    /// matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
}

impl LineEdit {
    /// Whether the selected text may be dragged out of the field.
    pub fn drag_enabled(&self) -> bool {
        self.drag_enabled
    }

    /// Enables or disables dragging the selection out of the field.
    ///
    /// On by default, which is the behaviour every field had before the switch existed: a press
    /// and drag inside the field selects, and a drag that leaves the box starts moving the
    /// selected text. A host that wants the gesture to stay inside the field turns it off. The
    /// switch is not draw-visible, so it queues no frame.
    pub fn set_drag_enabled(&mut self, enabled: bool) {
        if self.drag_enabled == enabled {
            return;
        }
        self.drag_enabled = enabled;
    }
}

impl LineEdit {
    // ---------------------------------------------------------------------------
    // Decoration slots
    // ---------------------------------------------------------------------------

    /// Returns the five decoration strings this field shows.
    pub fn decorations(&self) -> &DecorationSlots {
        &self.decorations
    }

    /// Replaces the whole decoration set at once.
    ///
    /// Prefer the individual setters when only one slot changes; this exists because a caller
    /// assembling a form from a document has all five at once and five separate calls would repaint
    /// five times.
    pub fn set_decorations(&mut self, decorations: DecorationSlots) {
        let changed = self.decorations != decorations;
        self.decorations = decorations;
        if changed {
            // The slots decide the value's box and the support row, so the memoised layout is
            // stale as soon as they change (I-P13).
            self.invalidate_text_advances();
            self.base.request_layout();
            self.base.request_redraw();
        }
    }

    /// Returns the unit marker written before the value.
    pub fn prefix(&self) -> &str {
        &self.decorations.prefix
    }

    /// Sets the unit marker written before the value.
    ///
    /// # Why this does not touch `text`
    ///
    /// The prefix is **chrome**, not content: it is drawn in its own box, so the caret can still sit at
    /// the start of what the user is editing and a select-all copies the value alone. Folding it into
    /// `text` would make all three of those wrong while looking the same on screen.
    pub fn set_prefix(&mut self, prefix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.prefix = prefix.into();
        self.set_decorations(next);
    }

    /// Returns the unit marker written after the value.
    pub fn suffix(&self) -> &str {
        &self.decorations.suffix
    }

    /// Sets the unit marker written after the value, anchored to the field's trailing edge.
    pub fn set_suffix(&mut self, suffix: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.suffix = suffix.into();
        self.set_decorations(next);
    }

    /// Returns the quiet hint shown below the field.
    pub fn helper_text(&self) -> &str {
        &self.decorations.helper
    }

    /// Sets the quiet hint shown below the field, displaced by any error.
    pub fn set_helper_text(&mut self, helper: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.helper = helper.into();
        self.set_decorations(next);
    }

    /// Returns the refusal message shown below the field.
    pub fn error_text(&self) -> &str {
        &self.decorations.error
    }

    /// Sets the refusal message, which displaces the helper and paints in the theme's error colour.
    ///
    /// Pass an empty string to clear it. This does not *validate* anything — it is the caller's report
    /// — but [`Self::set_max_length`] does drive the counter and the over-limit state on its own,
    /// because the field knows both numbers itself.
    pub fn set_error_text(&mut self, error: impl Into<String>) {
        let mut next = self.decorations.clone();
        next.error = error.into();
        self.set_decorations(next);
    }

    /// Returns the usage counter shown at the field's trailing lower edge, if any.
    ///
    /// Derived from the value's length and `max_length` rather than stored, so it cannot go stale: a
    /// counter the caller has to keep in step with the text is a counter that will disagree with it.
    pub fn counter_text(&self) -> Option<String> {
        self.value_readings().1
    }

    /// Returns whether the current value exceeds `max_length`.
    ///
    /// The field's own answer, so an over-long value is *reported* rather than silently truncated.
    pub fn is_over_limit(&self) -> bool {
        DecorationLayout::over_limit(self.char_count(), self.max_length) > 0
    }
}

/// `LineEdit`'s property contract.
///
/// # The decoration slots
///
/// `prefix`, `suffix`, `helper`, `error` and `counter` are all published here. They are the five strings
/// a text entry shows that are **not** its value, and before they existed the control could only be
/// announced and driven by its text and placeholder.
///
/// `counter` is published read-only: it is derived from the value's own length and `max_length`, so a
/// caller that could write it would be able to make the count disagree with the text it counts.
///
/// `echo_mode` is published as one of the tokens `normal`, `password` or `no_echo`, which is
/// the same spelling the platform layer reads back, so a form binding can round-trip it.
impl WidgetProperties for LineEdit {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "text" => Ok(CapabilityValue::String(self.text().to_string())),
            "placeholder_text" => Ok(CapabilityValue::String(self.placeholder_text().to_string())),
            "max_length" => match self.max_length() {
                Some(len) => Ok(CapabilityValue::UInt(len as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "read_only" => Ok(CapabilityValue::Bool(self.is_read_only())),
            "cursor_position" => Ok(CapabilityValue::UInt(self.cursor_position() as u64)),
            "echo_mode" => {
                Ok(CapabilityValue::String(Self::echo_mode_token(self.echo_mode()).to_string()))
            }
            "prefix" => Ok(CapabilityValue::String(self.prefix().to_string())),
            "suffix" => Ok(CapabilityValue::String(self.suffix().to_string())),
            "helper" => Ok(CapabilityValue::String(self.helper_text().to_string())),
            "error" => Ok(CapabilityValue::String(self.error_text().to_string())),
            "counter" => match self.counter_text() {
                Some(text) => Ok(CapabilityValue::String(text)),
                None => Ok(CapabilityValue::Null),
            },
            "over_limit" => Ok(CapabilityValue::Bool(self.is_over_limit())),
            "valid" => Ok(CapabilityValue::Bool(self.is_valid())),
            "validation_state" => Ok(CapabilityValue::String(
                validation_state_token(self.validation_state()).to_string(),
            )),
            "validation_message" => {
                Ok(CapabilityValue::String(self.validation_message_text().to_string()))
            }
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            "modified" => Ok(CapabilityValue::Bool(self.is_modified())),
            "clear_button_enabled" => Ok(CapabilityValue::Bool(self.clear_button_enabled())),
            "password_echo_on_edit" => Ok(CapabilityValue::Bool(self.password_echo_on_edit())),
            "drag_enabled" => Ok(CapabilityValue::Bool(self.drag_enabled())),
            "scroll_x" => Ok(CapabilityValue::Int(self.scroll_x() as i64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(expect_string(value)?);
                Ok(())
            }
            "placeholder_text" => {
                self.set_placeholder_text(expect_string(value)?);
                Ok(())
            }
            "max_length" => {
                match value {
                    CapabilityValue::Null => self.set_max_length(None),
                    other => self.set_max_length(Some(expect_usize(other)?)),
                }
                Ok(())
            }
            "read_only" => {
                self.set_read_only(expect_bool(value)?);
                Ok(())
            }
            "echo_mode" => {
                let token = expect_string(value)?;
                let mode = echo_mode_from_token(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_echo_mode(mode);
                Ok(())
            }
            "prefix" => {
                self.set_prefix(expect_string(value)?);
                Ok(())
            }
            "suffix" => {
                self.set_suffix(expect_string(value)?);
                Ok(())
            }
            "helper" => {
                self.set_helper_text(expect_string(value)?);
                Ok(())
            }
            "error" => {
                self.set_error_text(expect_string(value)?);
                Ok(())
            }
            // Derived from the value and the limit: a writer would be a second way to say what the
            // text already determines, and one of the two would be able to disagree.
            "counter" | "over_limit" => Err(CapabilityAccessError::ReadOnlyProperty),
            // Derived from the value and the field's grammar, exactly like the two above.
            "valid" | "validation_state" | "validation_message" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "cursor_position" => {
                self.set_cursor_position(expect_usize(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "modified" => {
                self.set_modified(expect_bool(value)?);
                Ok(())
            }
            "clear_button_enabled" => {
                self.set_clear_button_enabled(expect_bool(value)?);
                Ok(())
            }
            "password_echo_on_edit" => {
                self.set_password_echo_on_edit(expect_bool(value)?);
                Ok(())
            }
            "drag_enabled" => {
                self.set_drag_enabled(expect_bool(value)?);
                Ok(())
            }
            "scroll_x" => {
                let offset = match value {
                    CapabilityValue::Int(v) => v as i32,
                    CapabilityValue::UInt(v) => v as i32,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_scroll_x(offset);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `LINE_EDIT_PROPERTIES`.
        property_names_of![
            "text",
            "placeholder_text",
            "max_length",
            "read_only",
            "echo_mode",
            "cursor_position",
            "prefix",
            "suffix",
            "helper",
            "error",
            "counter",
            "over_limit",
            "valid",
            "validation_state",
            "validation_message",
            "alignment",
            "modified",
            "clear_button_enabled",
            "password_echo_on_edit",
            "drag_enabled",
            "scroll_x",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `line_edit` publishes.
    ///
    /// Both are zero-argument actions with a direct method on the control, so both
    /// run rather than being routed through the property path: `clear` assigns the
    /// empty text and `select_all` selects without changing it.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear" => {
                self.clear();
                Ok(())
            }
            "select_all" => {
                self.select_all();
                Ok(())
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl LineEdit {
    /// The host actions this field publishes for its context menu.
    pub fn actions(&self) -> &[LineEditAction] {
        &self.actions
    }

    /// Adds an action descriptor to the field's context-menu surface.
    pub fn add_action(&mut self, action: LineEditAction) {
        if self.actions.iter().any(|existing| existing.id == action.id) {
            if let Some(existing) =
                self.actions.iter_mut().find(|existing| existing.id == action.id)
            {
                *existing = action;
            }
            return;
        }
        self.actions.push(action);
    }

    /// Removes an action by id.
    pub fn remove_action(&mut self, id: &str) -> bool {
        let before = self.actions.len();
        self.actions.retain(|action| action.id != id);
        self.actions.len() != before
    }

    /// Reports that the host activated `id`, emitting [`Self::action_triggered`].
    pub fn activate_action(&self, id: &str) -> bool {
        if !self.actions.iter().any(|action| action.id == id) {
            return false;
        }
        self.action_triggered.emit(id.to_string());
        true
    }

    /// The candidates the completer slot offers, or an empty slice when unset.
    pub fn completer(&self) -> &[String] {
        &self.completion_source
    }

    /// Installs the completer's candidate list. An empty list removes the completer.
    pub fn set_completer(&mut self, candidates: Vec<String>) {
        self.completion_source = candidates;
    }

    /// The candidates matching the word at the caret, in source order.
    ///
    /// The match is a case-insensitive prefix test against the text after the last whitespace —
    /// the completion unit every editable control in this family uses. An empty completer, or an
    /// empty word with no candidates starting empty, yields an empty list.
    pub fn completion_candidates(&self) -> Vec<String> {
        if self.completion_source.is_empty() {
            return Vec::new();
        }
        let word = self.text.rsplit(|ch: char| ch.is_whitespace()).next().unwrap_or("");
        if word.is_empty() {
            return Vec::new();
        }
        self.completion_source
            .iter()
            .filter(|candidate| {
                crate::widget::input_widgets::text_match::starts_with(candidate, word)
            })
            .cloned()
            .collect()
    }

    /// Replaces the word at the caret with `candidate`, as one undoable edit.
    pub fn accept_completion(&mut self, candidate: &str) -> bool {
        if self.read_only || candidate.is_empty() {
            return false;
        }
        let word = self.text.rsplit(|ch: char| ch.is_whitespace()).next().unwrap_or("");
        let start = self.text.len().saturating_sub(word.len());
        if word == candidate {
            return false;
        }
        let before = self.needs_full_before_image().then(|| self.text.clone());
        let removed = self.text[start..].to_string();
        self.text.replace_range(start.., candidate);
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.commit_splice(start, &removed, candidate, before, EditOrigin::User);
        self.modified = true;
        true
    }

    /// Rewrites the value through the installed mask from `raw` (I-011).
    pub(super) fn rewrite_masked(&mut self, raw: String) -> bool {
        let Some(engine) = self.mask_engine.as_ref() else {
            return false;
        };
        let formatted = engine.format(&engine.sanitize(&raw));
        if formatted == self.text {
            return false;
        }
        let before = self.text.clone();
        self.text = formatted;
        self.cursor_position = self.text.len();
        self.selection_start = None;
        self.commit_in_place(before, EditOrigin::User);
        self.modified = true;
        true
    }
}
