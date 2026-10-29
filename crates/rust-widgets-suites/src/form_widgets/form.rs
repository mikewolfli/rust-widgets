// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `Form` — a validated container that lays out labelled fields.
//!
//! # What this control owns, and what it does not
//!
//! A form owns three things: the **row model** (which control belongs to which key, its label and
//! its rules), the **layout** (label position, columns, spacing, padding, the button row) and the
//! **validity aggregate** (required checks plus host-reported errors). It does **not** own the
//! value: a field's value is read from and written to the child control's own property contract,
//! so there is exactly one store for each fact (principle #101).
//!
//! # Layout
//!
//! The content area is the control's rectangle inset by `content_padding`, below the optional title
//! band and above the optional button row. Fields are packed row-major into `columns` equal cells
//! separated by `column_spacing`; rows are separated by `row_spacing` and are as tall as their
//! tallest field. A field's cell is either label-beside-control (`label_position = left`, label
//! width `label_width`) or label-above-control (`label_position = top`). An error line, or the help
//! text when there is no error, is reserved under the control. `field_layouts()` returns the exact
//! boxes the control paints and hit-tests, so a host can align its own companions with them.
//!
//! # Overflow
//!
//! The form does not scroll. `size_hint()` is the height the fields ask for, and a host that
//! gives the control less than that paints the overflow outside its own rectangle (the layout
//! keeps packing rows; nothing clips them). A host with an unbounded field count either gives the
//! form its `size_hint` or wraps it in a `ScrollArea`.
//!
//! # Enter submits through the focused field
//!
//! The keyboard router delivers a key to the focused control, not to its ancestors, and a form is
//! deliberately not focusable — its fields own focus. So a standalone `Form` only sees Enter when
//! something forwards it: a host binds the field's own `return_pressed` signal to `Form::submit`,
//! and a `FormSet` (or any registry-forwarding container) forwards the key to its current form.
//! The `Event::KeyPress` arm in `handle_event` is that forwarding path, not a claim that a form
//! mounted alone receives keys.
//!
//! # Events and wiring
//!
//! Every published name has an arm in [`Widget::event_signal_dyn`], which is what makes
//! `connect_event`/`invoke_command` reach a real signal rather than a valid-but-inert name:
//! `submitted`, `reset`, `submit_failed`, `validity_changed`, `dirty_changed`, `field_added`,
//! `field_removed` and `field_changed`.

use rust_widgets::core::{Color, HorizontalAlignment, ObjectId, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{GenericSignal, Signal1};
use rust_widgets::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, SimpleRegistry, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::model::FormField;

/// The minimum width a form lays out at when its own geometry is unhelpfully narrow.
pub const DEFAULT_WIDTH: u32 = 320;
/// Height reserved for a label drawn above its control.
const LABEL_ROW_H: u32 = 18;
/// Height reserved for an error or help line below a control.
const ERROR_ROW_H: u32 = 16;
/// The gap between a beside-the-control label and the control itself.
const LABEL_GAP: u32 = 8;
/// Height of the title band.
const TITLE_H: u32 = 24;
/// Height of one action button.
const BUTTON_H: u32 = 28;
/// The narrowest action button the form lays out.
const BUTTON_MIN_W: u32 = 72;

/// A host rule for one field's value: `None` means the value is acceptable.
type FieldValidator = Rc<dyn Fn(&str) -> Option<String>>;
/// `(revision, x, y, width, boxes)` of the last computed layout.
type LayoutCache = (u32, i32, i32, u32, Vec<FormFieldLayout>);

/// Where a field's label is drawn relative to its control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormLabelPosition {
    /// Label in a fixed-width column to the left of the control.
    Left,
    /// Label on its own row above the control.
    Top,
}

impl FormLabelPosition {
    /// The published token naming this position.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Top => "top",
        }
    }

    /// Parses a published token, or `None` for an unknown one.
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "left" => Some(Self::Left),
            "top" => Some(Self::Top),
            _ => None,
        }
    }
}

/// The boxes one field occupies: its label, its control and its error/help line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormFieldLayout {
    /// The field's stable key.
    pub key: String,
    /// The label box (zero-sized when labels are hidden).
    pub label: Rect,
    /// The box the child control is written to.
    pub control: Rect,
    /// The box the error — or the help text when there is no error — is drawn in.
    pub error: Rect,
}

/// A validated form container.
pub struct Form {
    base: BaseWidget,
    title: String,
    show_title: bool,
    label_position: FormLabelPosition,
    label_width: u32,
    row_height: u32,
    row_spacing: u32,
    column_spacing: u32,
    content_padding: u32,
    columns: u32,
    required_mark: String,
    required_error: String,
    show_labels: bool,
    show_errors: bool,
    show_buttons: bool,
    submit_label: String,
    reset_label: String,
    button_width: u32,
    fields: Vec<FormField>,
    registry: Option<Rc<RefCell<SimpleRegistry>>>,
    last_valid: bool,
    last_dirty: bool,
    /// Per-field rules installed by the host, keyed by field key.
    ///
    /// Stored here rather than on `FormField` so the field stays a plain comparable value — a
    /// closure cannot live in a type that derives `PartialEq`/`Eq`.
    validators: BTreeMap<String, FieldValidator>,
    /// The last observed value of each bound field: an edge detector for `field_changed` and the
    /// derived flags. The value of record stays in the child control.
    observed: Vec<(String, Option<String>)>,
    /// Bumped whenever the control asks for a layout; keys the layout cache.
    layout_revision: Arc<AtomicU32>,
    /// `(revision, x, y, width, boxes)` of the last computed layout.
    layout_cache: RefCell<Option<LayoutCache>>,
    /// Emitted after a successful submit.
    pub submitted: GenericSignal,
    /// Emitted after a reset restored the baseline values.
    pub reset_requested: GenericSignal,
    /// Emitted with the number of fields still in error when a submit was refused.
    pub submit_failed: Signal1<usize>,
    /// Emitted with the aggregate validity when it changes.
    pub validity_changed: Signal1<bool>,
    /// Emitted with the aggregate dirty flag when it changes.
    pub dirty_changed: Signal1<bool>,
    /// Emitted with the key of a field that was added.
    pub field_added: Signal1<String>,
    /// Emitted with the key of a field that was removed.
    pub field_removed: Signal1<String>,
    /// Emitted with the key of a field whose value was written through the form.
    pub field_changed: Signal1<String>,
}

impl Form {
    /// Creates an empty form.
    pub fn new(geometry: Rect) -> Self {
        let base = BaseWidget::new(WidgetKind::Form, geometry, "Form");
        let layout_revision = Arc::new(AtomicU32::new(0));
        {
            // Every layout-affecting setter already calls `request_layout`; watching that one
            // signal is what keeps the cached boxes from outliving their inputs without a call
            // site to miss.
            let revision = Arc::clone(&layout_revision);
            let _ = base.layout_requested.connect(move || {
                revision.fetch_add(1, Ordering::Relaxed);
            });
        }
        Self {
            base,
            title: String::new(),
            show_title: true,
            label_position: FormLabelPosition::Left,
            label_width: 120,
            row_height: 32,
            row_spacing: 8,
            column_spacing: 16,
            content_padding: 12,
            columns: 1,
            required_mark: "*".to_string(),
            required_error: "Required".to_string(),
            show_labels: true,
            show_errors: true,
            show_buttons: true,
            submit_label: "Submit".to_string(),
            reset_label: "Reset".to_string(),
            button_width: BUTTON_MIN_W,
            fields: Vec::new(),
            registry: None,
            last_valid: true,
            last_dirty: false,
            validators: BTreeMap::new(),
            observed: Vec::new(),
            layout_revision,
            layout_cache: RefCell::new(None),
            submitted: GenericSignal::new(),
            reset_requested: GenericSignal::new(),
            submit_failed: Signal1::new(),
            validity_changed: Signal1::new(),
            dirty_changed: Signal1::new(),
            field_added: Signal1::new(),
            field_removed: Signal1::new(),
            field_changed: Signal1::new(),
        }
    }

    // ── Configuration ────────────────────────────────────────────────────────

    /// The optional heading drawn above the fields.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Replaces the heading.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the heading is drawn when non-empty.
    pub fn show_title(&self) -> bool {
        self.show_title
    }

    /// Shows or hides the heading band.
    pub fn set_show_title(&mut self, show: bool) {
        self.show_title = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Where labels sit relative to their controls.
    pub fn label_position(&self) -> FormLabelPosition {
        self.label_position
    }

    /// Chooses the label position.
    pub fn set_label_position(&mut self, position: FormLabelPosition) {
        self.label_position = position;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The width of the label column in `left` mode.
    pub fn label_width(&self) -> u32 {
        self.label_width
    }

    /// Sets the label column width (clamped to at least 24).
    pub fn set_label_width(&mut self, width: u32) {
        self.label_width = width.max(24);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The minimum height of one control row.
    pub fn row_height(&self) -> u32 {
        self.row_height
    }

    /// Sets the minimum control-row height.
    pub fn set_row_height(&mut self, height: u32) {
        self.row_height = height.max(16);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The vertical gap between rows.
    pub fn row_spacing(&self) -> u32 {
        self.row_spacing
    }

    /// Sets the vertical gap between rows.
    pub fn set_row_spacing(&mut self, spacing: u32) {
        self.row_spacing = spacing;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The horizontal gap between columns.
    pub fn column_spacing(&self) -> u32 {
        self.column_spacing
    }

    /// Sets the horizontal gap between columns.
    pub fn set_column_spacing(&mut self, spacing: u32) {
        self.column_spacing = spacing;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The inset between the control's edge and its content.
    pub fn content_padding(&self) -> u32 {
        self.content_padding
    }

    /// Sets the content inset.
    pub fn set_content_padding(&mut self, padding: u32) {
        self.content_padding = padding;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// How many fields are placed side by side.
    pub fn columns(&self) -> u32 {
        self.columns
    }

    /// Sets the column count (at least one).
    pub fn set_columns(&mut self, columns: u32) {
        self.columns = columns.max(1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The mark appended to a required field's label.
    pub fn required_mark(&self) -> &str {
        &self.required_mark
    }

    /// Sets the required mark (`""` hides it).
    pub fn set_required_mark(&mut self, mark: impl Into<String>) {
        self.required_mark = mark.into();
        self.base.request_redraw();
    }

    /// The error text a required-but-empty field receives from `validate`.
    pub fn required_error(&self) -> &str {
        &self.required_error
    }

    /// Sets the required-field error text.
    pub fn set_required_error(&mut self, error: impl Into<String>) {
        self.required_error = error.into();
        self.base.request_redraw();
    }

    /// Whether labels are painted.
    pub fn show_labels(&self) -> bool {
        self.show_labels
    }

    /// Shows or hides every label.
    pub fn set_show_labels(&mut self, show: bool) {
        self.show_labels = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether error lines are painted.
    pub fn show_errors(&self) -> bool {
        self.show_errors
    }

    /// Shows or hides error lines.
    pub fn set_show_errors(&mut self, show: bool) {
        self.show_errors = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether the submit/reset row is painted and clickable.
    pub fn show_buttons(&self) -> bool {
        self.show_buttons
    }

    /// Shows or hides the action row.
    pub fn set_show_buttons(&mut self, show: bool) {
        self.show_buttons = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The submit button's caption.
    pub fn submit_label(&self) -> &str {
        &self.submit_label
    }

    /// Sets the submit caption.
    pub fn set_submit_label(&mut self, label: impl Into<String>) {
        self.submit_label = label.into();
        self.base.request_redraw();
    }

    /// The reset button's caption.
    pub fn reset_label(&self) -> &str {
        &self.reset_label
    }

    /// Sets the reset caption.
    pub fn set_reset_label(&mut self, label: impl Into<String>) {
        self.reset_label = label.into();
        self.base.request_redraw();
    }

    /// The width of one action button.
    pub fn button_width(&self) -> u32 {
        self.button_width
    }

    /// Sets the action-button width (clamped to at least `BUTTON_MIN_W`).
    pub fn set_button_width(&mut self, width: u32) {
        self.button_width = width.max(BUTTON_MIN_W);
        self.base.request_layout();
        self.base.request_redraw();
    }

    // ── Field model ──────────────────────────────────────────────────────────

    /// The fields in layout order.
    pub fn fields(&self) -> &[FormField] {
        &self.fields
    }

    /// The field with `key`.
    pub fn field(&self, key: &str) -> Option<&FormField> {
        self.fields.iter().find(|field| field.key == key)
    }

    /// The field with `key`, mutably.
    ///
    /// A direct edit through this reference does not know whether it changed the layout or the
    /// validity; call [`Self::refresh`] afterwards if it did.
    pub fn field_mut(&mut self, key: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|field| field.key == key)
    }

    /// How many fields the form holds.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Every field key, in layout order.
    pub fn field_keys(&self) -> Vec<String> {
        self.fields.iter().map(|field| field.key.clone()).collect()
    }

    /// Adds a field and links its control into this form.
    ///
    /// Returns `false` when a field with the same key already exists — identity is the key, so a
    /// duplicate would make `field(key)` ambiguous. The field's baseline is captured from its
    /// control immediately, which is what makes [`Self::reset`] restore the value the form opened
    /// with.
    pub fn add_field(&mut self, mut field: FormField) -> bool {
        if self.field(&field.key).is_some() {
            return false;
        }
        if let Some(widget) = field.widget {
            self.base.add_child_linked(widget);
            field.baseline = read_field_value(widget);
        }
        let key = field.key.clone();
        let initial = field.baseline.clone();
        self.fields.push(field);
        self.observed.push((key.clone(), initial));
        self.base.request_layout();
        self.base.request_redraw();
        self.field_added.emit(key);
        self.publish_derived_state();
        true
    }

    /// Adds a field bound to `widget` in one call.
    pub fn add_field_with_widget(
        &mut self,
        key: impl Into<String>,
        label: impl Into<String>,
        widget: ObjectId,
    ) -> bool {
        self.add_field(FormField::new(key, label).with_widget(widget))
    }

    /// Removes the field with `key`, returning it.
    ///
    /// The control itself is not destroyed: it is unlinked from this form's child list and stays
    /// alive for its owner. That is the honest split — the form never owned the control.
    pub fn remove_field(&mut self, key: &str) -> Option<FormField> {
        let index = self.fields.iter().position(|field| field.key == key)?;
        let field = self.fields.remove(index);
        if let Some(widget) = field.widget {
            self.base.remove_child_linked(widget);
        }
        self.observed.retain(|(existing, _)| existing != key);
        self.base.request_layout();
        self.base.request_redraw();
        self.field_removed.emit(key.to_string());
        self.publish_derived_state();
        Some(field)
    }

    /// Removes every field.
    pub fn clear_fields(&mut self) {
        while let Some(field) = self.fields.pop() {
            if let Some(widget) = field.widget {
                self.base.remove_child_linked(widget);
            }
            self.field_removed.emit(field.key);
        }
        self.observed.clear();
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
    }

    /// Reports a host error for `key`.
    pub fn set_field_error(&mut self, key: &str, error: impl Into<String>) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_error(Some(error.into()));
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
        true
    }

    /// Clears both error slots of `key`.
    pub fn clear_field_error(&mut self, key: &str) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.clear_error();
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
        true
    }

    /// Clears every field's errors.
    pub fn clear_errors(&mut self) {
        for field in &mut self.fields {
            field.clear_error();
        }
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
    }

    /// Sets a field's label.
    pub fn set_field_label(&mut self, key: &str, label: impl Into<String>) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_label(label);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Sets a field's required flag.
    pub fn set_field_required(&mut self, key: &str, required: bool) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_required(required);
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
        true
    }

    /// Sets a field's help text.
    pub fn set_field_help(&mut self, key: &str, help: impl Into<String>) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_help(help);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Shows or hides one row.
    pub fn set_field_visible(&mut self, key: &str, visible: bool) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_visible(visible);
        if let Some(widget) = field.widget {
            rust_widgets::set_widget_visible(widget, visible);
        }
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Installs `validator` as the rule for the field `key`.
    ///
    /// The validator receives the field's current value and returns the message to show, or
    /// `None` when the value is acceptable. It runs from [`Self::validate`] /
    /// [`Self::validate_field`] after the required check, so a host rule and the built-in rule
    /// are one pass. Returns `false` for an unknown key.
    pub fn set_field_validator<F>(&mut self, key: &str, validator: F) -> bool
    where
        F: Fn(&str) -> Option<String> + 'static,
    {
        if self.field(key).is_none() {
            return false;
        }
        self.validators.insert(key.to_string(), Rc::new(validator));
        true
    }

    /// Removes the rule installed for `key`, returning whether one was installed.
    pub fn clear_field_validator(&mut self, key: &str) -> bool {
        self.validators.remove(key).is_some()
    }

    // ── Values and validity ──────────────────────────────────────────────────

    /// The current value of `key`, read from its control's property contract.
    pub fn value_of(&self, key: &str) -> Option<String> {
        self.field(key)?.widget.and_then(read_field_value)
    }

    /// Every bound field's `(key, value)`, in layout order.
    ///
    /// This is the payload a host reads after `submitted`; the event itself stays payload-free
    /// because a form's value set is heterogeneous and already available here.
    pub fn field_values(&self) -> Vec<(String, String)> {
        self.fields
            .iter()
            .filter_map(|field| {
                let widget = field.widget?;
                Some((field.key.clone(), read_field_value(widget)?))
            })
            .collect()
    }

    /// Writes `value` to `key`'s control through its property contract.
    pub fn set_value_of(&mut self, key: &str, value: &str) -> bool {
        let Some(field) = self.field(key) else {
            return false;
        };
        let Some(widget) = field.widget else {
            return false;
        };
        if !write_field_value(widget, value) {
            return false;
        }
        self.base.request_redraw();
        self.note_observed(key);
        self.field_changed.emit(key.to_string());
        self.publish_derived_state();
        true
    }

    /// Records every bound field's current value as the baseline [`Self::reset`] restores.
    pub fn capture_baseline(&mut self) {
        for field in &mut self.fields {
            if let Some(widget) = field.widget {
                field.baseline = read_field_value(widget);
            }
        }
        self.publish_derived_state();
    }

    /// Whether any bound field differs from its baseline.
    pub fn dirty(&self) -> bool {
        self.fields.iter().any(|field| match (field.baseline.as_deref(), field.widget) {
            (Some(baseline), Some(widget)) => {
                read_field_value(widget).is_some_and(|current| current != baseline)
            }
            _ => false,
        })
    }

    /// Runs the required-field pass and publishes the new aggregate validity.
    ///
    /// A host-reported error is left alone: `validate` owns the *derived* error slot only, so a
    /// domain rule a caller installed survives a validation pass (and can only be cleared by
    /// [`Self::clear_field_error`]).
    pub fn validate(&mut self) -> bool {
        let required_error = self.required_error.clone();
        {
            let validators = &self.validators;
            for field in &mut self.fields {
                let value = field.widget.and_then(read_field_value);
                let missing =
                    field.required && value.as_deref().is_none_or(|value| value.trim().is_empty());
                field.validation_error = if missing {
                    Some(required_error.clone())
                } else {
                    value.and_then(|value| validators.get(&field.key).and_then(|rule| rule(&value)))
                };
            }
        }
        self.base.request_layout();
        let valid = self.is_valid();
        self.publish_validity(valid);
        valid
    }

    /// Runs the required-field pass for one field and publishes the aggregate edge.
    ///
    /// Returns `false` for an unknown key. A host that validates field by field still sees one
    /// `validity_changed` edge for the form as a whole, because the aggregate is republished after
    /// the single-field pass — the same rule [`Self::validate`] follows.
    pub fn validate_field(&mut self, key: &str) -> bool {
        let required_error = self.required_error.clone();
        let validators = &self.validators;
        let Some(field) = self.fields.iter_mut().find(|field| field.key == key) else {
            return false;
        };
        let value = field.widget.and_then(read_field_value);
        let missing =
            field.required && value.as_deref().is_none_or(|value| value.trim().is_empty());
        field.validation_error = if missing {
            Some(required_error)
        } else {
            value.and_then(|value| validators.get(&field.key).and_then(|rule| rule(&value)))
        };
        self.base.request_layout();
        self.publish_derived_state();
        true
    }

    /// Whether every field currently reports no error and every required field has a value.
    pub fn is_valid(&self) -> bool {
        self.fields.iter().all(|field| {
            if field.error().is_some() {
                return false;
            }
            if !field.required {
                return true;
            }
            field.widget.and_then(read_field_value).is_some_and(|value| !value.trim().is_empty())
        })
    }

    /// How many fields currently show an error.
    pub fn error_count(&self) -> usize {
        self.fields.iter().filter(|field| field.error().is_some()).count()
    }

    /// Submits the form: validates, then emits `submitted` or `submit_failed`.
    ///
    /// Returns whether the submit succeeded, so a host that only cares about the outcome does not
    /// need a subscription.
    pub fn submit(&mut self) -> bool {
        if self.validate() {
            self.submitted.emit();
            true
        } else {
            self.submit_failed.emit(self.error_count());
            false
        }
    }

    /// Moves keyboard focus to the first visible field that currently shows an error.
    ///
    /// Returns whether a control was focused. A rule-only row (no bound control) and a hidden
    /// row have nothing to focus and are skipped; `false` is the honest answer for "there is
    /// nowhere to put the caret".
    pub fn focus_first_invalid(&mut self) -> bool {
        let target = self
            .fields
            .iter()
            .find(|field| field.visible && field.error().is_some())
            .and_then(|field| field.widget);
        target.is_some_and(rust_widgets::widget::runtime::focus_widget)
    }

    /// Restores every baseline value and clears the errors, then emits `reset`.
    pub fn reset(&mut self) {
        for field in &mut self.fields {
            if let (Some(widget), Some(baseline)) = (field.widget, field.baseline.clone()) {
                let _ = write_field_value(widget, &baseline);
            }
            field.clear_error();
        }
        self.base.request_layout();
        self.base.request_redraw();
        self.sync_observed();
        self.reset_requested.emit();
        self.publish_derived_state();
    }

    /// Requests a layout pass and a repaint.
    ///
    /// Call this after a direct [`Self::field_mut`] edit that changed a label, the required flag,
    /// the help text or a visibility flag.
    pub fn refresh(&self) {
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Re-reads every bound field's value and publishes the edges a direct edit produced.
    ///
    /// The form is a container: the user's keystrokes reach the child control directly, so the
    /// form only learns about them on the next event it forwards. Comparing against the last
    /// observed value turns that into `field_changed` plus the `dirty`/`valid` edges. The snapshot
    /// is an edge detector, not a second value store — the value of record stays in the child.
    fn resync_derived(&mut self) {
        let mut changed = false;
        for field in &self.fields {
            let Some(widget) = field.widget else {
                continue;
            };
            let current = read_field_value(widget);
            match self.observed.iter_mut().find(|(existing, _)| *existing == field.key) {
                Some((_, observed)) => {
                    if *observed != current {
                        *observed = current;
                        changed = true;
                        self.field_changed.emit(field.key.clone());
                    }
                }
                None => self.observed.push((field.key.clone(), current)),
            }
        }
        if changed {
            self.base.request_redraw();
            self.publish_derived_state();
        }
    }

    /// Records the current values as the new observation baseline (no events).
    fn sync_observed(&mut self) {
        self.observed.clear();
        for field in &self.fields {
            self.observed.push((field.key.clone(), field.widget.and_then(read_field_value)));
        }
    }

    /// Records one field's current value as observed, so a form-driven write is not reported
    /// again as a user edit on the next resync.
    fn note_observed(&mut self, key: &str) {
        let current = self.field(key).and_then(|field| field.widget).and_then(read_field_value);
        match self.observed.iter_mut().find(|(existing, _)| existing == key) {
            Some((_, observed)) => *observed = current,
            None => self.observed.push((key.to_string(), current)),
        }
    }

    // ── Layout ───────────────────────────────────────────────────────────────

    /// The box available to the fields: inset by the padding, below the title and above the
    /// button row.
    pub fn content_rect(&self) -> Rect {
        let geometry = self.geometry();
        let padding = self.content_padding as i32;
        let top = geometry.y + padding + self.title_band_height() as i32;
        let bottom = geometry.bottom() - padding - self.button_band_height() as i32;
        let width = geometry.width.saturating_sub(self.content_padding.saturating_mul(2));
        Rect::new(geometry.x + padding, top, width, (bottom - top).max(0) as u32)
    }

    /// The exact box of each field's label, control and error/help line.
    pub fn field_layouts(&self) -> Vec<FormFieldLayout> {
        self.with_layouts(|layouts| layouts.to_vec())
    }

    /// Whether the fields need more height than the control currently has.
    ///
    /// The form does not scroll: a host reads this to decide between giving the control its
    /// `size_hint()` height and wrapping it in a `ScrollArea`. The boxes keep packing past the
    /// bottom either way, so nothing is silently dropped from the layout.
    pub fn overflows(&self) -> bool {
        self.size_hint().height > self.geometry().height
    }

    /// Recomputes the boxes when the layout inputs changed, and leaves them alone otherwise.
    ///
    /// The revision is bumped by the control's own `request_layout` signal, and the content
    /// origin/width are part of the key, so a geometry change invalidates without extra calls.
    fn ensure_layouts(&self) {
        let content = self.content_rect();
        let revision = self.layout_revision.load(Ordering::Relaxed);
        let fresh = matches!(
            self.layout_cache.borrow().as_ref(),
            Some((cached, x, y, width, _))
                if *cached == revision
                    && *x == content.x
                    && *y == content.y
                    && *width == content.width
        );
        if fresh {
            return;
        }
        let layouts = self.compute_layouts(content.x, content.y, content.width).0;
        *self.layout_cache.borrow_mut() =
            Some((revision, content.x, content.y, content.width, layouts));
    }

    /// Runs `f` over the current boxes without cloning them.
    fn with_layouts<R>(&self, f: impl FnOnce(&[FormFieldLayout]) -> R) -> R {
        self.ensure_layouts();
        let cache = self.layout_cache.borrow();
        let (_, _, _, _, layouts) = cache.as_ref().expect("ensure_layouts stored the boxes");
        f(layouts)
    }

    /// The control box of `key`, if the field is visible.
    pub fn control_rect_of(&self, key: &str) -> Option<Rect> {
        self.with_layouts(|layouts| {
            layouts.iter().find(|layout| layout.key == key).map(|layout| layout.control)
        })
    }

    /// The submit button's box for the current geometry and configuration.
    pub fn submit_rect(&self) -> Rect {
        let geometry = self.geometry();
        let padding = self.content_padding as i32;
        let width = self.button_width.max(BUTTON_MIN_W);
        Rect::new(
            geometry.right() - padding - width as i32,
            geometry.bottom() - padding - BUTTON_H as i32,
            width,
            BUTTON_H,
        )
    }

    /// The reset button's box for the current geometry and configuration.
    pub fn reset_rect(&self) -> Rect {
        let submit = self.submit_rect();
        let width = self.button_width.max(BUTTON_MIN_W);
        Rect::new(submit.x - self.row_spacing as i32 - width as i32, submit.y, width, BUTTON_H)
    }

    fn title_band_height(&self) -> u32 {
        if self.show_title && !self.title.is_empty() {
            TITLE_H
        } else {
            0
        }
    }

    fn button_band_height(&self) -> u32 {
        if self.show_buttons {
            BUTTON_H + self.row_spacing
        } else {
            0
        }
    }

    /// The row-and-column arithmetic, shared by `field_layouts` and `size_hint`.
    ///
    /// Returns the layouts and the total height they occupy. The x/y origin is a parameter because
    /// `size_hint` asks the same question at a synthetic origin.
    fn compute_layouts(&self, x0: i32, y0: i32, width: u32) -> (Vec<FormFieldLayout>, u32) {
        let columns = self.columns.max(1) as usize;
        let gap = self.column_spacing as i32;
        let total_gap = gap.saturating_mul(columns as i32 - 1).max(0);
        let usable = (width as i32 - total_gap).max(0) as u32;
        let column_width = usable / columns as u32;
        let visible: Vec<&FormField> = self.fields.iter().filter(|field| field.visible).collect();
        let mut layouts = Vec::with_capacity(visible.len());
        let mut y = y0;
        let mut index = 0;
        while index < visible.len() {
            let end = visible.len().min(index + columns);
            let row = &visible[index..end];
            let mut row_height = 0u32;
            for field in row {
                row_height = row_height.max(self.field_height(field));
            }
            for (column, field) in row.iter().enumerate() {
                let x = x0 + column as i32 * (column_width as i32 + gap);
                let line_height = self.line_height(field);
                let top_height = row_height.saturating_sub(line_height);
                let (label, control) = match self.label_position {
                    // A hidden label takes no column *and* no gap: `FormFieldLayout::label`
                    // promises a zero-sized box, so the control gets the whole width.
                    FormLabelPosition::Left => {
                        let label_width = if self.show_labels {
                            self.label_width.min(column_width.saturating_sub(LABEL_GAP + 16))
                        } else {
                            0
                        };
                        let gap = if label_width > 0 { LABEL_GAP } else { 0 };
                        let label_height = if self.show_labels { top_height } else { 0 };
                        (
                            Rect::new(x, y, label_width, label_height),
                            Rect::new(
                                x + label_width as i32 + gap as i32,
                                y,
                                column_width.saturating_sub(label_width + gap),
                                top_height,
                            ),
                        )
                    }
                    FormLabelPosition::Top => {
                        let label_height =
                            if self.show_labels { LABEL_ROW_H.min(top_height) } else { 0 };
                        (
                            Rect::new(x, y, column_width, label_height),
                            Rect::new(
                                x,
                                y + label_height as i32,
                                column_width,
                                top_height.saturating_sub(label_height),
                            ),
                        )
                    }
                };
                layouts.push(FormFieldLayout {
                    key: field.key.clone(),
                    label,
                    control,
                    error: Rect::new(x, y + top_height as i32, column_width, line_height),
                });
            }
            y += row_height as i32 + self.row_spacing as i32;
            index += columns;
        }
        let used = (y - y0).max(0) as u32;
        (layouts, used)
    }

    /// The total height one field asks for.
    fn field_height(&self, field: &FormField) -> u32 {
        let control = self.row_height.max(1);
        let lines = self.line_height(field);
        let label = if self.show_labels { LABEL_ROW_H } else { 0 };
        match self.label_position {
            // A side label shares its row with the control, so it only raises a short row.
            FormLabelPosition::Left => control.max(label) + lines,
            FormLabelPosition::Top => label + control + lines,
        }
    }

    /// The reserved error/help line height for one field.
    fn line_height(&self, field: &FormField) -> u32 {
        let shows_error = self.show_errors && field.error().is_some();
        if shows_error || !field.help().is_empty() {
            ERROR_ROW_H
        } else {
            0
        }
    }

    /// Writes every bound field's control box into the child control.
    ///
    /// The tree painter draws a linked child at the child's own geometry, so this is the form's
    /// answer to "where does that child live". `draw` passes the boxes it already computed, so a
    /// frame costs one layout pass rather than two; `place_field_widgets` is the wrapper for the
    /// callers that do not hold them (`set_geometry`).
    fn place_layouts(&self, layouts: &[FormFieldLayout]) {
        for layout in layouts {
            if let Some(widget) = self.field(&layout.key).and_then(FormField::widget_id) {
                rust_widgets::set_widget_geometry(
                    widget,
                    layout.control.x,
                    layout.control.y,
                    layout.control.width,
                    layout.control.height,
                );
            }
        }
    }

    fn place_field_widgets(&self) {
        self.place_layouts(&self.field_layouts());
    }

    /// Binds the shared forwarding table used to deliver events to field controls.
    pub fn set_registry(&mut self, registry: Rc<RefCell<SimpleRegistry>>) {
        self.registry = Some(registry);
    }

    /// The bound forwarding table, if any.
    pub fn registry(&self) -> Option<&Rc<RefCell<SimpleRegistry>>> {
        self.registry.as_ref()
    }

    fn publish_validity(&mut self, valid: bool) {
        if self.last_valid != valid {
            self.last_valid = valid;
            self.validity_changed.emit(valid);
        }
    }

    fn publish_dirty(&mut self) {
        let dirty = self.dirty();
        if self.last_dirty != dirty {
            self.last_dirty = dirty;
            self.dirty_changed.emit(dirty);
        }
    }

    /// Publishes both derived edges after a mutation that can change them.
    fn publish_derived_state(&mut self) {
        let valid = self.is_valid();
        self.publish_validity(valid);
        self.publish_dirty();
    }

    fn draw_button(&self, context: &mut RenderContext, rect: Rect, label: &str, ink: Color) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Hover and press are tracked by `BaseWidget` from the runtime's hover/press transitions;
        // a disabled form paints the row faded instead of leaving it looking live.
        let enabled = self.base.is_enabled();
        let fill_alpha = if !enabled {
            8
        } else if self.base.is_pressed() {
            30
        } else if self.base.is_hovered() {
            24
        } else {
            18
        };
        let stroke_alpha = if enabled { 90 } else { 40 };
        let ink = if enabled { ink } else { ink.with_alpha(120) };
        context.fill_rounded_rect(rect, 4, ink.with_alpha(fill_alpha));
        context.draw_rounded_rect_stroke(rect, 4, ink.with_alpha(stroke_alpha), 1);
        let font = role_font(self.base.style(), 12.0);
        context.draw_text_fitted_cow(rect, label, &font, ink, HorizontalAlignment::Center);
    }
}

/// Reads a field control's value from its own property contract.
///
/// `text` is what every text entry publishes; `value` is the numeric/selection family's spelling;
/// `current_text` covers the editable combo boxes. The first property that answers wins, so a
/// control is never asked to be one particular kind.
fn read_field_value(widget: ObjectId) -> Option<String> {
    for name in ["text", "value", "current_text"] {
        match rust_widgets::read_widget_property_by_id(widget, name) {
            Ok(CapabilityValue::String(value)) => return Some(value),
            Ok(CapabilityValue::Int(value)) => return Some(value.to_string()),
            Ok(CapabilityValue::UInt(value)) => return Some(value.to_string()),
            Ok(CapabilityValue::Float(value)) => return Some(value.to_string()),
            Ok(CapabilityValue::Bool(value)) => return Some(value.to_string()),
            _ => {}
        }
    }
    None
}

/// Writes a value through a control's property contract.
///
/// The probe mirrors [`read_field_value`]: `text` first, then the editable combo boxes' spelling,
/// then the numeric `value` in the kinds the read path converts. Writing only `text`/`value` left
/// a bound editable combo readable but not writable, and an Int-only `value` field unsettable
/// because the only write attempt was a float.
fn write_field_value(widget: ObjectId, value: &str) -> bool {
    let write = |name: &str, value: CapabilityValue| {
        rust_widgets::write_widget_property_by_id(widget, name, value).is_ok()
    };
    if write("text", CapabilityValue::String(value.to_string())) {
        return true;
    }
    if write("current_text", CapabilityValue::String(value.to_string())) {
        return true;
    }
    if let Ok(number) = value.parse::<i64>() {
        if write("value", CapabilityValue::Int(number)) {
            return true;
        }
    }
    if let Ok(number) = value.parse::<u64>() {
        if write("value", CapabilityValue::UInt(number)) {
            return true;
        }
    }
    if let Ok(number) = value.parse::<f64>() {
        if write("value", CapabilityValue::Float(number)) {
            return true;
        }
    }
    false
}

impl Widget for Form {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }

    /// Reports the form's meaning: `Error` while any field carries a message.
    ///
    /// The interaction channel stays at rest — a refusal is not a hover or a press — and the
    /// border this control draws resolves from the state, exactly as `LineEdit` does.
    fn semantic_state(&self) -> rust_widgets::style::SemanticState {
        if self.error_count() > 0 {
            return rust_widgets::style::SemanticState::Error;
        }
        rust_widgets::style::SemanticState::None
    }

    /// The name a screen reader announces: the heading when there is one.
    ///
    /// The default falls back to the tooltip, which a form may never set; its `title` is the text
    /// a sighted user reads as the form's name, so it is the honest answer here.
    fn accessible_name(&self) -> String {
        let title = self.title.trim();
        if title.is_empty() {
            return self.base.tooltip().to_string();
        }
        title.to_string()
    }

    fn set_geometry(&mut self, geometry: Rect) {
        self.base.set_geometry(geometry);
        self.place_field_widgets();
        self.base.request_layout();
        self.base.request_redraw();
    }

    fn size_hint(&self) -> Size {
        let width = self.geometry().width.max(DEFAULT_WIDTH);
        let (_, content) = self.compute_layouts(0, 0, width);
        Size::new(
            width,
            self.title_band_height()
                + content
                + self.content_padding.saturating_mul(2)
                + self.button_band_height(),
        )
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves every published event name to the signal that emits it.
    ///
    /// `connect_event` validates a name against the capability table; only this method joins that
    /// name to a real signal, so a published-but-inert name (the silent failure the project's
    /// wiring gate exists to prevent) is impossible here.
    fn event_signal_dyn(&self, name: &str) -> Option<rust_widgets::signal::EventSignalRef> {
        use rust_widgets::signal::EventSignalRef;
        match name {
            "submitted" => Some(EventSignalRef::unit("submitted", &self.submitted)),
            "reset" => Some(EventSignalRef::unit("reset", &self.reset_requested)),
            "submit_failed" => {
                Some(EventSignalRef::mapped("submit_failed", &self.submit_failed, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            "validity_changed" => {
                Some(EventSignalRef::mapped("validity_changed", &self.validity_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            "dirty_changed" => {
                Some(EventSignalRef::mapped("dirty_changed", &self.dirty_changed, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            "field_added" => {
                Some(EventSignalRef::mapped("field_added", &self.field_added, |value| {
                    CapabilityValue::String(value.clone())
                }))
            }
            "field_removed" => {
                Some(EventSignalRef::mapped("field_removed", &self.field_removed, |value| {
                    CapabilityValue::String(value.clone())
                }))
            }
            "field_changed" => {
                Some(EventSignalRef::mapped("field_changed", &self.field_changed, |value| {
                    CapabilityValue::String(value.clone())
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for Form {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "title" => Ok(CapabilityValue::String(self.title.clone())),
            "show_title" => Ok(CapabilityValue::Bool(self.show_title)),
            "label_position" => {
                Ok(CapabilityValue::String(self.label_position.token().to_string()))
            }
            "label_width" => Ok(CapabilityValue::UInt(self.label_width as u64)),
            "row_height" => Ok(CapabilityValue::UInt(self.row_height as u64)),
            "row_spacing" => Ok(CapabilityValue::UInt(self.row_spacing as u64)),
            "column_spacing" => Ok(CapabilityValue::UInt(self.column_spacing as u64)),
            "content_padding" => Ok(CapabilityValue::UInt(self.content_padding as u64)),
            "columns" => Ok(CapabilityValue::UInt(self.columns as u64)),
            "required_mark" => Ok(CapabilityValue::String(self.required_mark.clone())),
            "required_error" => Ok(CapabilityValue::String(self.required_error.clone())),
            "show_labels" => Ok(CapabilityValue::Bool(self.show_labels)),
            "show_errors" => Ok(CapabilityValue::Bool(self.show_errors)),
            "show_buttons" => Ok(CapabilityValue::Bool(self.show_buttons)),
            "submit_label" => Ok(CapabilityValue::String(self.submit_label.clone())),
            "reset_label" => Ok(CapabilityValue::String(self.reset_label.clone())),
            "button_width" => Ok(CapabilityValue::UInt(self.button_width as u64)),
            "field_count" => Ok(CapabilityValue::UInt(self.field_count() as u64)),
            "valid" => Ok(CapabilityValue::Bool(self.is_valid())),
            "error_count" => Ok(CapabilityValue::UInt(self.error_count() as u64)),
            "dirty" => Ok(CapabilityValue::Bool(self.dirty())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "title" => {
                self.set_title(expect_string(value)?);
                Ok(())
            }
            "show_title" => {
                self.set_show_title(expect_bool(value)?);
                Ok(())
            }
            "label_position" => {
                let token = expect_string(value)?;
                let position = FormLabelPosition::from_token(&token)
                    .ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_label_position(position);
                Ok(())
            }
            "label_width" => {
                self.set_label_width(expect_u32(value)?);
                Ok(())
            }
            "row_height" => {
                self.set_row_height(expect_u32(value)?);
                Ok(())
            }
            "row_spacing" => {
                self.set_row_spacing(expect_u32(value)?);
                Ok(())
            }
            "column_spacing" => {
                self.set_column_spacing(expect_u32(value)?);
                Ok(())
            }
            "content_padding" => {
                self.set_content_padding(expect_u32(value)?);
                Ok(())
            }
            "columns" => {
                self.set_columns(expect_u32(value)?);
                Ok(())
            }
            "required_mark" => {
                self.set_required_mark(expect_string(value)?);
                Ok(())
            }
            "required_error" => {
                self.set_required_error(expect_string(value)?);
                Ok(())
            }
            "show_labels" => {
                self.set_show_labels(expect_bool(value)?);
                Ok(())
            }
            "show_errors" => {
                self.set_show_errors(expect_bool(value)?);
                Ok(())
            }
            "show_buttons" => {
                self.set_show_buttons(expect_bool(value)?);
                Ok(())
            }
            "submit_label" => {
                self.set_submit_label(expect_string(value)?);
                Ok(())
            }
            "reset_label" => {
                self.set_reset_label(expect_string(value)?);
                Ok(())
            }
            "button_width" => {
                self.set_button_width(expect_u32(value)?);
                Ok(())
            }
            // A disabled container disables the controls it hosts: the fields are separate
            // widgets and the router can dispatch to them directly, so writing this control's
            // own flag alone would leave them interactive (FM-06).
            "enabled" => {
                let enabled = expect_bool(value)?;
                self.base.set_enabled(enabled);
                for field in &self.fields {
                    if let Some(widget) = field.widget {
                        rust_widgets::set_widget_enabled(widget, enabled);
                    }
                }
                Ok(())
            }
            // Derived from the field list and the controls' values; a writer would be a second
            // store for a fact the form already derives.
            "field_count" | "valid" | "error_count" | "dirty" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "title",
            "show_title",
            "label_position",
            "label_width",
            "row_height",
            "row_spacing",
            "column_spacing",
            "content_padding",
            "columns",
            "required_mark",
            "required_error",
            "show_labels",
            "show_errors",
            "show_buttons",
            "submit_label",
            "reset_label",
            "button_width",
            "field_count",
            "valid",
            "error_count",
            "dirty",
            BASE_PROPERTY_NAMES
        ]
    }

    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "submit" => {
                let _ = self.submit();
                Ok(())
            }
            "reset" => {
                self.reset();
                Ok(())
            }
            "validate" => {
                let _ = self.validate();
                Ok(())
            }
            "clear_errors" => {
                self.clear_errors();
                Ok(())
            }
            // Payload-carrying assignments use the property route.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl Draw for Form {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(40, 40, 40, 255));
        let surface = style.background_color.unwrap_or(Color::rgba(255, 255, 255, 255));
        // A themed error border wins over the control's own border; a process with no theme keeps
        // the style's border, the same `Some(..) => themed, None => literal` ladder the calendar
        // and the badge use.
        let border = rust_widgets::style::resolved_semantic_border("form", self.semantic_state())
            .or(style.border_color)
            .unwrap_or_else(|| ink.with_alpha(60));
        let radius = style.border_radius.unwrap_or(0);
        context.fill_rounded_rect(rect, radius, surface);
        context.draw_rounded_rect_stroke(rect, radius, border, 1);

        let padding = self.content_padding as i32;
        if self.title_band_height() > 0 {
            let band = Rect::new(
                rect.x + padding,
                rect.y + padding,
                rect.width.saturating_sub(self.content_padding.saturating_mul(2)),
                TITLE_H,
            );
            let title_font = role_font(style, 14.0);
            context.draw_text_fitted_cow(
                band,
                &self.title,
                &title_font,
                ink,
                HorizontalAlignment::Left,
            );
        }

        let label_font = role_font(style, 12.0);
        // One token read per frame: the severity is the theme's `error` colour, and the literal
        // remains the fallback for a process with no theme installed.
        let error_ink =
            rust_widgets::style::semantic_color(rust_widgets::style::SemanticColor::Error)
                .unwrap_or_else(|| Color::rgba(190, 40, 40, 255));
        self.with_layouts(|layouts| {
            for layout in layouts {
                let Some(field) = self.field(&layout.key) else {
                    continue;
                };
                if self.show_labels {
                    let mut label = field.label().to_string();
                    if field.is_required() && !self.required_mark.is_empty() {
                        label.push_str(&self.required_mark);
                    }
                    context.draw_text_fitted_cow(
                        layout.label,
                        &label,
                        &label_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if let Some(error) = field.error() {
                    if self.show_errors {
                        context.draw_text_fitted_cow(
                            layout.error,
                            error,
                            &label_font,
                            error_ink,
                            HorizontalAlignment::Left,
                        );
                    }
                } else if !field.help().is_empty() {
                    context.draw_text_fitted_cow(
                        layout.error,
                        field.help(),
                        &label_font,
                        ink.with_alpha(130),
                        HorizontalAlignment::Left,
                    );
                }
            }
        });

        if self.show_buttons {
            self.draw_button(context, self.submit_rect(), &self.submit_label, ink);
            self.draw_button(context, self.reset_rect(), &self.reset_label, ink);
        }

        // The children are drawn by the tree painter; this only has to keep their boxes current
        // for a frame that served the layout from cache. The boxes come from the same cache, so
        // the frame pays for one layout pass and clones nothing.
        self.with_layouts(|layouts| self.place_layouts(layouts));
    }
}

impl EventHandler for Form {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            if self.show_buttons {
                if self.submit_rect().contains_point(*pos) {
                    let _ = self.submit();
                    return;
                }
                if self.reset_rect().contains_point(*pos) {
                    self.reset();
                    return;
                }
            }
        }
        // Enter submits when the form itself receives the key. A text field that consumes Enter
        // first keeps its own behaviour, which is what a multi-line or dialog-driving field wants.
        if let Event::KeyPress { key, .. } = event {
            if *key == rust_widgets::event::key_codes::ENTER {
                let _ = self.submit();
                return;
            }
        }
        let Some(registry) = self.registry.as_ref() else {
            return;
        };
        match event {
            Event::MousePress { pos, .. }
            | Event::MouseRelease { pos, .. }
            | Event::MouseMove { pos }
            | Event::MouseDoubleClick { pos, .. }
            | Event::ContextMenu { pos } => {
                let target: Option<ObjectId> = self.with_layouts(|layouts| {
                    layouts
                        .iter()
                        .find(|layout| layout.control.contains_point(*pos))
                        .and_then(|layout| self.field(&layout.key).and_then(FormField::widget_id))
                });
                if let Some(widget) = target {
                    let _ = registry.borrow_mut().forward_event(widget, event);
                }
            }
            // A coordinate-free event cannot name one field; every bound control receives it, the
            // same rule the group box documents for a multi-child container.
            _ => {
                for field in &self.fields {
                    if let Some(widget) = field.widget_id() {
                        let _ = registry.borrow_mut().forward_event(widget, event);
                    }
                }
            }
        }
        // The forwarded event may have changed a child's value; publish the edges it produced
        // (`field_changed`, `dirty_changed`, `validity_changed`) before returning.
        self.resync_derived();
    }
}
