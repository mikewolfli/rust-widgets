// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The data model shared by `Form` and `FormSet`.
//!
//! A form is a *layout and validity* container: the actual editors are ordinary controls the host
//! already knows how to create. These types are therefore data rather than widgets — a field names
//! one child by id, and a page names one form by id. Keeping them as plain values is what lets the
//! container report a complete, inspectable model (`fields()`, `pages()`) without owning a second
//! copy of any control's state.

use rust_widgets::core::ObjectId;

/// The value grammar a field is expected to hold.
///
/// The kind is suite metadata, not a second value store: [`Form::add_field`](crate::form_widgets::Form::add_field)
/// uses it to install the matching `rust_widgets` input validator, and a host
/// can read it to choose an editor control. `Text` is the default and imposes no
/// rule of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FieldKind {
    /// Unconstrained text.
    #[default]
    Text,
    /// An e-mail address (`InputValidator::Email`).
    Email,
    /// A phone number (`InputValidator::Phone`).
    Phone,
    /// An inclusive whole-number range.
    Integer {
        /// Lowest accepted value.
        minimum: i64,
        /// Highest accepted value.
        maximum: i64,
    },
    /// An inclusive character-count range.
    Length {
        /// Lowest accepted character count.
        minimum: usize,
        /// Highest accepted character count.
        maximum: usize,
    },
}

/// One row of a [`Form`](crate::form_widgets::Form).
///
/// # Identity
///
/// `key` is the field's stable identity. Layout, error lookup and the `field_*` signals all use it,
/// so renaming a label never moves a caller's handle. Two fields in one form may not share a key.
///
/// # Value, baseline and errors
///
/// The *value* lives in the child control (its `text`/`value` property), not here — two stores for
/// one fact is the drift this project's rules forbid. The field keeps only what the control cannot:
/// its label, its required flag, its help text, and two error slots. `baseline` is the value a
/// [`Form::reset`](crate::form_widgets::Form::reset) restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) widget: Option<ObjectId>,
    pub(crate) required: bool,
    pub(crate) help: String,
    /// An error a host reported for this field (server-side validation, a domain rule).
    pub(crate) external_error: Option<String>,
    /// The result of an asynchronous rule resolved through a `ValidationTicket`.
    pub(crate) async_error: Option<String>,
    /// The error the form's own required/validity pass produced.
    pub(crate) validation_error: Option<String>,
    /// The value at the last baseline capture, restored by `Form::reset`.
    pub(crate) baseline: Option<String>,
    pub(crate) visible: bool,
    /// The grammar this field is expected to hold.
    pub(crate) kind: FieldKind,
}

impl FormField {
    /// A field with `key` and a display `label`, not yet bound to a control.
    pub fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            widget: None,
            required: false,
            help: String::new(),
            external_error: None,
            async_error: None,
            validation_error: None,
            baseline: None,
            visible: true,
            kind: FieldKind::Text,
        }
    }

    /// Binds this field to the mounted control `widget`.
    ///
    /// The form links the id into its own child list, so the tree painter draws the control in the
    /// field's row and the runtime routes pointer input to it.
    pub fn with_widget(mut self, widget: ObjectId) -> Self {
        self.widget = Some(widget);
        self
    }

    /// Marks the field required (or optional).
    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Sets the help text drawn under the control when there is no error.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Declares the value grammar this field expects.
    ///
    /// [`Form::add_field`](crate::form_widgets::Form::add_field) installs the
    /// matching input validator, so the rule runs from the form's validity pass
    /// rather than from the child control.
    pub fn with_kind(mut self, kind: FieldKind) -> Self {
        self.kind = kind;
        self
    }

    /// The field's stable identity.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The label drawn above or beside the control.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The control this field addresses, if one is bound.
    pub fn widget_id(&self) -> Option<ObjectId> {
        self.widget
    }

    /// Whether a value is required.
    pub fn is_required(&self) -> bool {
        self.required
    }

    /// The value grammar this field expects.
    pub fn kind(&self) -> FieldKind {
        self.kind
    }

    /// The help text shown when the field has no error.
    pub fn help(&self) -> &str {
        &self.help
    }

    /// The error currently shown: a host-reported one wins over an asynchronous
    /// result, which in turn wins over the form's own validity pass.
    ///
    /// The order is the order of authority: an explicit host refusal is a fact the
    /// form cannot overrule, a resolved asynchronous rule is newer than the local
    /// pass, and the local pass is the fallback.
    pub fn error(&self) -> Option<&str> {
        self.external_error
            .as_deref()
            .or(self.async_error.as_deref())
            .or(self.validation_error.as_deref())
    }

    /// Whether this field is laid out and painted.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Renames the field.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Sets or clears the required flag.
    pub fn set_required(&mut self, required: bool) {
        self.required = required;
    }

    /// Changes the declared value grammar.
    ///
    /// Call [`Form::set_field_kind`](crate::form_widgets::Form::set_field_kind)
    /// instead when the kind-derived validator should be installed too.
    pub fn set_kind(&mut self, kind: FieldKind) {
        self.kind = kind;
    }

    /// Sets or clears the help text.
    pub fn set_help(&mut self, help: impl Into<String>) {
        self.help = help.into();
    }

    /// Shows or hides the row.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Reports a host error (`None` clears it).
    pub fn set_error(&mut self, error: Option<impl Into<String>>) {
        self.external_error = error.map(Into::into);
    }

    /// Stores a resolved asynchronous rule's result.
    pub fn set_async_error(&mut self, error: Option<impl Into<String>>) {
        self.async_error = error.map(Into::into);
    }

    /// The error a resolved asynchronous ticket produced, if any.
    pub fn async_error(&self) -> Option<&str> {
        self.async_error.as_deref()
    }

    /// Clears both error slots.
    pub fn clear_error(&mut self) {
        self.external_error = None;
        self.async_error = None;
        self.validation_error = None;
    }

    /// The value this field's control held when the baseline was captured.
    pub fn baseline(&self) -> Option<&str> {
        self.baseline.as_deref()
    }
}

/// One page of a [`FormSet`](crate::form_widgets::FormSet).
///
/// A page is a title plus the form it hosts. A page with no form is legal — it is a step that the
/// host fills in later — and it never blocks the set's aggregate validity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormPage {
    pub(crate) key: String,
    pub(crate) title: String,
    pub(crate) form: Option<ObjectId>,
    pub(crate) completed: bool,
    pub(crate) enabled: bool,
}

impl FormPage {
    /// A page with `key` and a display `title`, not yet bound to a form.
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self { key: key.into(), title: title.into(), form: None, completed: false, enabled: true }
    }

    /// Binds the page to the mounted `Form` control `form`.
    pub fn with_form(mut self, form: ObjectId) -> Self {
        self.form = Some(form);
        self
    }

    /// Marks the page completed (or not).
    pub fn with_completed(mut self, completed: bool) -> Self {
        self.completed = completed;
        self
    }

    /// The page's stable identity.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The title drawn in the header and the tab.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The form this page hosts, if one is bound.
    pub fn form_id(&self) -> Option<ObjectId> {
        self.form
    }

    /// Whether the host marked this page complete.
    pub fn is_completed(&self) -> bool {
        self.completed
    }

    /// Whether the page can be navigated to.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Renames the page.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Enables or disables navigation to this page.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}
