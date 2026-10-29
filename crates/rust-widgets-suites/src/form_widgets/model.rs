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
    /// The error the form's own required/validity pass produced.
    pub(crate) validation_error: Option<String>,
    /// The value at the last baseline capture, restored by `Form::reset`.
    pub(crate) baseline: Option<String>,
    pub(crate) visible: bool,
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
            validation_error: None,
            baseline: None,
            visible: true,
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

    /// The help text shown when the field has no error.
    pub fn help(&self) -> &str {
        &self.help
    }

    /// The error currently shown: a host-reported one wins over the form's own validity pass.
    pub fn error(&self) -> Option<&str> {
        self.external_error.as_deref().or(self.validation_error.as_deref())
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

    /// Clears both error slots.
    pub fn clear_error(&mut self) {
        self.external_error = None;
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
