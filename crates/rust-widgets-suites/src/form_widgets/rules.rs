// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Validation vocabulary: cross-field rules, typed bridges and async tickets.
//!
//! The form container owns the field list and the record of observed values.
//! These types own the *rules* and the evaluation of one field's derived error,
//! which keeps `form.rs` focused on layout, the value contract and the signals.
//! The split also keeps the file within the project's size ceiling.

use rust_widgets::signal::{GenericSignal, Signal1};
use rust_widgets::widget::input_widgets::input_validator::{
    InputValidator, LengthValidator, RangeValidator, Validator,
};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use super::form::Form;
use super::model::FieldKind;

/// Zero-payload signal alias (the suite's spelling for a unit notification).
pub type Signal0 = GenericSignal;
/// Two-payload signal alias bundling `(A, B)` in one emit.
pub type Signal2<A, B> = Signal1<(A, B)>;

/// A host rule for one field's value: `None` means the value is acceptable.
pub(crate) type FieldValidator = Rc<dyn Fn(&str) -> Option<String>>;
/// An asynchronous rule, run only when a ticket is resolved.
pub(crate) type AsyncFieldValidator = Rc<dyn Fn(&str) -> Option<String>>;

/// A rule that relates one field's value to another field's value.
///
/// The rule is attached to the field it *judges* (`set_form_rule(key, …)`), and
/// the other field is named by key. It runs from [`Form::validate`] after the
/// field-local required, typed and host rules, so a cross-field error is still
/// stored in the judged field's own error slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormRule {
    /// The judged field must equal `other`.
    SameAs {
        /// The field compared against.
        other: String,
        /// Overrides the default message.
        message: Option<String>,
    },
    /// The judged field must differ from `other`.
    DifferentFrom {
        /// The field compared against.
        other: String,
        /// Overrides the default message.
        message: Option<String>,
    },
    /// The judged field must be non-empty when `other` holds one of `values`.
    ///
    /// An empty `values` list means "whenever `other` is non-empty".
    RequiredWhen {
        /// The field whose value triggers the requirement.
        other: String,
        /// Trigger values; empty means "any non-empty value".
        values: Vec<String>,
        /// Overrides the default message.
        message: Option<String>,
    },
}

impl FormRule {
    /// Requires the judged field to equal `other`.
    pub fn same_as(other: impl Into<String>) -> Self {
        Self::SameAs { other: other.into(), message: None }
    }

    /// Requires the judged field to differ from `other`.
    pub fn different_from(other: impl Into<String>) -> Self {
        Self::DifferentFrom { other: other.into(), message: None }
    }

    /// Requires the judged field when `other` holds one of `values`.
    pub fn required_when<I, S>(other: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::RequiredWhen {
            other: other.into(),
            values: values.into_iter().map(Into::into).collect(),
            message: None,
        }
    }

    /// Replaces the rule's default message.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        let message = Some(message.into());
        match &mut self {
            Self::SameAs { message: slot, .. }
            | Self::DifferentFrom { message: slot, .. }
            | Self::RequiredWhen { message: slot, .. } => *slot = message,
        }
        self
    }

    /// The field this rule compares against.
    pub fn other(&self) -> &str {
        match self {
            Self::SameAs { other, .. }
            | Self::DifferentFrom { other, .. }
            | Self::RequiredWhen { other, .. } => other,
        }
    }

    /// Evaluates the rule against the current value map.
    pub(crate) fn evaluate(
        &self,
        value: &str,
        values: &BTreeMap<String, String>,
    ) -> Option<String> {
        match self {
            Self::SameAs { other, message } => {
                let expected = values.get(other).map(String::as_str).unwrap_or("");
                if expected == value {
                    None
                } else {
                    Some(message.clone().unwrap_or_else(|| format!("Must match {other}")))
                }
            }
            Self::DifferentFrom { other, message } => {
                let other_value = values.get(other).map(String::as_str).unwrap_or("");
                if other_value != value {
                    None
                } else {
                    Some(message.clone().unwrap_or_else(|| format!("Must differ from {other}")))
                }
            }
            Self::RequiredWhen { other, values: triggers, message } => {
                let trigger = values.get(other).map(String::as_str).unwrap_or("");
                let active = if triggers.is_empty() {
                    !trigger.trim().is_empty()
                } else {
                    triggers.iter().any(|candidate| candidate == trigger)
                };
                if active && value.trim().is_empty() {
                    Some(message.clone().unwrap_or_else(|| format!("Required when {other} is set")))
                } else {
                    None
                }
            }
        }
    }
}

/// A pending asynchronous validation.
///
/// The host starts the request, does the work off the UI thread and resolves the
/// ticket when it finishes. The ticket captures the **value** and the form's
/// validation revision at issue time, so a result computed for text that has
/// since changed is refused rather than applied to different content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationTicket {
    pub(crate) form: u64,
    pub(crate) id: u64,
    pub(crate) key: String,
    pub(crate) value: String,
    pub(crate) revision: u64,
    pub(crate) resolved: bool,
    pub(crate) cancelled: bool,
}

impl ValidationTicket {
    /// The ticket's process-unique id, for correlating worker results.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The field the ticket validates.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The value the ticket was issued for.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Whether a result has already been applied.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Whether the ticket was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Cancels the request, so a late result is refused.
    ///
    /// Returns `false` when the ticket was already resolved or cancelled, which is
    /// the honest answer for a second cancel.
    pub fn cancel(&mut self) -> bool {
        if self.resolved || self.cancelled {
            return false;
        }
        self.cancelled = true;
        true
    }

    /// Whether the result is no longer valid for `form`.
    ///
    /// A ticket goes stale when it was issued by another form, when the form's
    /// values changed since issue, or when it was already resolved/cancelled.
    pub fn is_stale(&self, form: &Form) -> bool {
        self.cancelled
            || self.resolved
            || self.form != form.form_serial
            || self.revision != form.validation_revision
            || form.value_of(&self.key).unwrap_or_default() != self.value
    }
}

/// Computes one field's derived error from every installed rule.
///
/// The order is deliberate: the required check is the form's own contract; the
/// typed rule is the shared vocabulary a field control also enforces; the closure
/// rule is a host's local extension; and the cross-field rules come last because
/// they judge the value against its siblings, which the other three never see.
/// The first failure wins, matching the single error slot the row paints.
#[allow(clippy::too_many_arguments)]
pub(crate) fn field_validation_error(
    key: &str,
    required: bool,
    value: Option<&str>,
    values: &BTreeMap<String, String>,
    required_error: &str,
    validators: &BTreeMap<String, FieldValidator>,
    input_validators: &BTreeMap<String, Arc<dyn Validator>>,
    rules: &BTreeMap<String, Vec<FormRule>>,
) -> Option<String> {
    let text = value.unwrap_or("");
    if required && text.trim().is_empty() {
        return Some(required_error.to_string());
    }
    // An unbound field has no value to judge; the existing contract reports no
    // derived error for it beyond the required check above.
    let value = value?;
    if !value.trim().is_empty() {
        if let Some(rule) = input_validators.get(key) {
            if !rule.state(value).is_acceptable() {
                return Some(rule.message_for(value).into_owned());
            }
        }
    }
    if let Some(error) = validators.get(key).and_then(|rule| rule(value)) {
        return Some(error);
    }
    if let Some(attached) = rules.get(key) {
        for rule in attached {
            if let Some(error) = rule.evaluate(value, values) {
                return Some(error);
            }
        }
    }
    None
}

/// The input validator a declared field kind installs.
///
/// `Text` has none: unconstrained text is the absence of a rule, not an empty one.
pub(crate) fn validator_for_kind(kind: FieldKind) -> Option<Arc<dyn Validator>> {
    match kind {
        FieldKind::Text => None,
        FieldKind::Email => Some(Arc::new(InputValidator::Email)),
        FieldKind::Phone => Some(Arc::new(InputValidator::Phone)),
        FieldKind::Integer { minimum, maximum } => {
            Some(Arc::new(RangeValidator::int(minimum, maximum)))
        }
        FieldKind::Length { minimum, maximum } => {
            Some(Arc::new(LengthValidator::new(minimum, maximum)))
        }
    }
}
