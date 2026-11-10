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
//! something forwards it, and the usual path is the field's own `return_pressed` signal bound to
//! `Form::submit`; the `Event::KeyPress` arm in `handle_event` handles a form that is handed the
//! key directly.
//!
//! # Events and wiring
//!
//! Every published name has an arm in `Widget::event_signal_dyn`, which is what makes
//! `connect_event`/`invoke_command` reach a real signal rather than a valid-but-inert name:
//! `submitted`, `reset`, `cancel`, `submit_failed`, `validity_changed`, `dirty_changed`,
//! `field_added`, `field_removed`, `field_changed` and `field_validated`.

use rust_widgets::core::{Color, HorizontalAlignment, ObjectId, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{GenericSignal, Signal1};
use rust_widgets::widget::capability::coercion::{expect_bool, expect_string, expect_u32};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::input_widgets::input_validator::Validator;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use super::model::{FieldKind, FormField};
use super::rules::{
    field_validation_error, validator_for_kind, AsyncFieldValidator, FieldValidator, FormRule,
    Signal0, Signal2, ValidationTicket,
};

/// Process-unique identity source for forms.
///
/// A `ValidationTicket` must only resolve against the form that issued it; a
/// per-form serial is cheaper and stricter than comparing field contents.
static NEXT_FORM_SERIAL: AtomicU64 = AtomicU64::new(1);

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
/// The largest accepted column count.
///
/// A form is a row/column layout, not a grid of pixels: this bound keeps `columns` inside what a
/// window can show and keeps the layout arithmetic away from integer overflow. `set_columns`
/// clamps to it.
const MAX_COLUMNS: u32 = 256;

/// The last computed layout: the boxes, the field each belongs to, and the content origin
/// they were computed for.
///
/// `field_indices` stores the index into `Form::fields` of the field each box describes. It is
/// captured with the boxes, so a frame pairs them in O(1) instead of scanning `fields` once per
/// row (the old `field(&layout.key)` was O(n) per box). A structural edit (add/remove/clear)
/// requests a layout, which rebuilds both halves together; a direct `field_mut` visibility edit
/// leaves the indices pointing at the same fields the stale boxes describe, which is exactly the
/// pairing the boxes themselves already assume.
struct LayoutCache {
    revision: u32,
    x: i32,
    y: i32,
    width: u32,
    layouts: Vec<FormFieldLayout>,
    field_indices: Vec<usize>,
}

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

/// The action row's buttons, for per-button hover feedback.
///
/// The base tracks hover and press for the control as a whole; a form's buttons are drawn by the
/// control itself, so the button under the pointer is the extra fact that keeps the highlight on
/// one button instead of all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormAction {
    Submit,
    Reset,
    Cancel,
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
    /// The label text painted on the current row, reused across fields and frames.
    label_scratch: RefCell<String>,
    last_valid: bool,
    last_dirty: bool,
    /// Per-field rules installed by the host, keyed by field key.
    ///
    /// Stored here rather than on `FormField` so the field stays a plain comparable value — a
    /// closure cannot live in a type that derives `PartialEq`/`Eq`.
    validators: BTreeMap<String, FieldValidator>,
    /// Typed rules bridged from `rust_widgets`' input validators.
    ///
    /// A separately typed map rather than a second closure shape: the trait's
    /// three-way state is richer than `Option<String>`, so flattening it into the
    /// closure map would lose the distinction between "invalid" and "incomplete".
    input_validators: BTreeMap<String, Arc<dyn Validator>>,
    /// Cross-field rules attached to the field they judge.
    form_rules: BTreeMap<String, Vec<FormRule>>,
    /// Asynchronous rules, keyed by field. Resolving a ticket runs the rule only
    /// through [`Form::resolve_async`]; [`Form::resolve`] applies a host result.
    async_validators: BTreeMap<String, AsyncFieldValidator>,
    /// Bumped whenever a value changes, so outstanding tickets go stale.
    pub(crate) validation_revision: u64,
    /// Identity of this form, stamped into every ticket it issues.
    pub(crate) form_serial: u64,
    /// Source of ticket ids.
    next_ticket_id: u64,
    /// The label of the optional cancel button; empty hides it.
    cancel_label: String,
    /// The last observed value of each bound field: an edge detector for `field_changed` and the
    /// derived flags. The value of record stays in the child control.
    observed: Vec<(String, Option<String>)>,
    /// Bumped whenever the control asks for a layout; keys the layout cache.
    layout_revision: Arc<AtomicU32>,
    /// The boxes, their field indices and the content origin of the last computed layout.
    layout_cache: RefCell<Option<LayoutCache>>,
    /// Where the pointer last entered or moved inside this control; `None` after it leaves.
    ///
    /// Hover is derived from this position and the same rectangles hit-testing uses, so moving a
    /// button (a new geometry or padding) cannot leave a highlight behind on a stale box.
    pointer: Cell<Option<Point>>,
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
    /// Emitted with `(key, valid)` when an asynchronous ticket is resolved.
    pub field_validated: Signal2<String, bool>,
    /// Emitted when the user abandons the form without restoring its baseline.
    ///
    /// Distinct from [`Self::reset_requested`]: reset writes the baseline values
    /// back, cancel leaves the edits in place and only clears the verdicts.
    pub cancel_requested: Signal0,
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
            label_scratch: RefCell::new(String::new()),
            last_valid: true,
            last_dirty: false,
            validators: BTreeMap::new(),
            input_validators: BTreeMap::new(),
            form_rules: BTreeMap::new(),
            async_validators: BTreeMap::new(),
            validation_revision: 0,
            form_serial: NEXT_FORM_SERIAL.fetch_add(1, Ordering::Relaxed),
            next_ticket_id: 0,
            cancel_label: String::new(),
            observed: Vec::new(),
            layout_revision,
            layout_cache: RefCell::new(None),
            pointer: Cell::new(None),
            submitted: GenericSignal::new(),
            reset_requested: GenericSignal::new(),
            submit_failed: Signal1::new(),
            validity_changed: Signal1::new(),
            dirty_changed: Signal1::new(),
            field_added: Signal1::new(),
            field_removed: Signal1::new(),
            field_changed: Signal1::new(),
            field_validated: Signal1::new(),
            cancel_requested: GenericSignal::new(),
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

    /// Sets the column count (at least one, at most 256).
    pub fn set_columns(&mut self, columns: u32) {
        self.columns = columns.clamp(1, MAX_COLUMNS);
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

    /// The cancel button's caption, or an empty string when there is none.
    pub fn cancel_label(&self) -> &str {
        &self.cancel_label
    }

    /// Sets the cancel caption. An empty caption hides the button.
    ///
    /// The button is additive: the default is empty, so the action row is
    /// exactly the submit/reset pair it has always been.
    pub fn set_cancel_label(&mut self, label: impl Into<String>) {
        self.cancel_label = label.into();
        self.base.request_layout();
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
            // Adoption must restore the presentation flags a detached child kept: `remove_field`
            // and `clear_fields` hide a removed control (C2-D1), so a row added again would sit in
            // the layout while its control stayed invisible. `enabled` follows the same rule the
            // container applies to every hosted field (FM-06), which also keeps a field added to a
            // disabled form from staying interactive.
            rust_widgets::set_widget_visible(widget, field.visible);
            rust_widgets::set_widget_enabled(widget, self.base.is_enabled());
            field.baseline = read_field_value(widget);
        }
        let key = field.key.clone();
        let initial = field.baseline.clone();
        let kind = field.kind;
        self.fields.push(field);
        if let Some(validator) = validator_for_kind(kind) {
            self.input_validators.insert(key.clone(), validator);
        }
        self.observed.push((key.clone(), initial));
        self.bump_validation_revision();
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
            // C2-D1: a detached field must stop claiming to be visible (family contract).
            rust_widgets::set_widget_visible(widget, false);
        }
        self.observed.retain(|(existing, _)| existing != key);
        self.validators.remove(key);
        self.form_rules.remove(key);
        // A rule attached to *another* field may name this one as its comparison target. Keeping
        // it would make the surviving row report "Must match <gone field>" against a value nobody
        // can set, so the incoming rules go with the field they depend on — and the verdicts they
        // produced are retired with them.
        let mut judged = Vec::new();
        for (target, rules) in self.form_rules.iter_mut() {
            let before = rules.len();
            rules.retain(|rule| rule.other() != key);
            if rules.len() != before {
                judged.push(target.clone());
            }
        }
        // `validation_error` carries no provenance, so a blind clear would also destroy a verdict
        // the row's own closure/typed rule still produces. Re-deriving each judged row keeps the
        // remaining rules honest (and repaints/publishes once per row).
        for target in judged {
            self.revalidate_field(&target);
        }
        // The detached field keeps its error slots otherwise, so re-adding the value a host kept
        // (the doc says the control stays alive for its owner) resurrected a message from a rule
        // that no longer exists and published `validity_changed(false)` for it.
        let mut field = field;
        field.validation_error = None;
        field.external_error = None;
        field.async_error = None;
        self.input_validators.remove(key);
        self.async_validators.remove(key);
        self.bump_validation_revision();
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
                // C2-D1: same rule as `remove_field` — the detached field is hidden.
                rust_widgets::set_widget_visible(widget, false);
            }
            self.field_removed.emit(field.key);
        }
        self.observed.clear();
        self.validators.clear();
        self.form_rules.clear();
        self.input_validators.clear();
        self.async_validators.clear();
        self.bump_validation_revision();
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
        // An in-flight async ticket for this value still matches the form, the value and the
        // revision, so without this bump it would land its verdict straight back on the next frame
        // — the rule `cancel` and `reset` already follow.
        self.bump_validation_revision();
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
        // Same invariant as `clear_field_error`: a ticket issued before this call must not land
        // after it.
        self.bump_validation_revision();
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
        // Turning the flag off can retire a "Required" verdict the field is still holding; a
        // fresh pass re-derives the row from the rules that remain. Turning it on adds a possible
        // error rather than invalidating the current verdict, so it does not force one now.
        if required {
            self.publish_derived_state();
        } else {
            self.revalidate_field(key);
        }
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
    ///
    /// A hidden row leaves the layout, the aggregate validity and `focus_first_invalid` together:
    /// it paints nothing and cannot be focused, so it must not be the reason a submit is refused.
    /// Its derived validation verdict is retired with it (a host-reported or asynchronous error is
    /// the value's own fact and is kept), so showing it again cannot resurrect an error no pass
    /// re-derived. Showing it again puts it back under all three rules, and the visibility edge
    /// publishes the aggregate change it produced.
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
        // The value did not change, but the aggregate did: a hidden invalid row stops blocking
        // and a shown one starts again. A hidden row cannot be corrected, so its frozen verdict is
        // retired here rather than re-weighted on the next show.
        if !visible {
            if let Some(field) = self.field_mut(key) {
                field.validation_error = None;
            }
            // A rule attached to *another* row may name this one as its comparison target, and
            // `field_validation_error` skips such a rule once the target is hidden. The row's stored
            // verdict was produced while the target was visible, so it has to be re-derived — the
            // same rule `remove_field` applies — or a visible row keeps an error the reader can no
            // longer see the cause of.
            let judged: Vec<String> = self
                .form_rules
                .iter()
                .filter(|(_, rules)| rules.iter().any(|rule| rule.other() == key))
                .map(|(target, _)| target.clone())
                .collect();
            for target in judged {
                if target != key {
                    self.revalidate_field(&target);
                }
            }
        }
        self.publish_derived_state();
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
        if self.validators.remove(key).is_none() {
            return false;
        }
        // The verdict on `key` was produced by a rule that no longer exists; the row is re-derived
        // from the rules that remain instead of leaving the old message painted and counted.
        self.revalidate_field(key);
        true
    }

    /// Installs a typed rule from `rust_widgets`' input-validator family.
    ///
    /// The rule is the same value a `LineEdit`/`SpinBox` enforces, so a form and
    /// its fields cannot disagree about what an e-mail address or a range is. It
    /// runs after the required check and before the closure rule; a value that the
    /// validator calls `Invalid` (or `Intermediate` while non-empty) becomes this
    /// field's error with the validator's own message.
    ///
    /// Returns `false` for an unknown key.
    pub fn set_field_input_validator<V>(&mut self, key: &str, validator: V) -> bool
    where
        V: Validator + 'static,
    {
        if self.field(key).is_none() {
            return false;
        }
        self.input_validators.insert(key.to_string(), Arc::new(validator));
        self.base.request_redraw();
        true
    }

    /// Removes the typed rule for `key`, returning whether one was installed.
    pub fn clear_field_input_validator(&mut self, key: &str) -> bool {
        if self.input_validators.remove(key).is_none() {
            return false;
        }
        self.revalidate_field(key);
        true
    }

    /// Declares a field's value grammar and installs its validator.
    ///
    /// This is the method form of [`FormField::with_kind`]; it allows a host to
    /// change the kind after mounting. A previous typed rule is replaced, so an
    /// explicit [`Self::set_field_input_validator`] call after this one wins.
    /// `FieldKind::Text` removes the kind-derived rule.
    pub fn set_field_kind(&mut self, key: &str, kind: FieldKind) -> bool {
        let Some(field) = self.field_mut(key) else {
            return false;
        };
        field.set_kind(kind);
        match validator_for_kind(kind) {
            Some(validator) => {
                self.input_validators.insert(key.to_string(), validator);
            }
            None => {
                self.input_validators.remove(key);
            }
        }
        // The typed rule that produced the row's message was just replaced or removed, so the old
        // verdict cannot stand; re-derive it from the rules now installed.
        self.revalidate_field(key);
        true
    }

    /// Re-runs validation for one field and publishes the result.
    ///
    /// Used after a rule, a kind or a required flag changed: `validation_error` carries no
    /// provenance, so clearing the slot is not enough (it could hold a failure of a rule that
    /// still exists) and leaving it is wrong (it could hold a failure of the rule just removed).
    fn revalidate_field(&mut self, key: &str) {
        self.apply_validation(Some(key));
        self.bump_validation_revision();
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
    }

    /// Attaches a cross-field rule to the field it judges.
    ///
    /// Several rules may attach to one field; they run in insertion order and the
    /// first failure is the error shown. Returns `false` for an unknown key.
    pub fn set_form_rule(&mut self, key: &str, rule: FormRule) -> bool {
        if self.field(key).is_none() {
            return false;
        }
        self.form_rules.entry(key.to_string()).or_default().push(rule);
        self.base.request_redraw();
        true
    }

    /// Removes every cross-field rule attached to `key`.
    ///
    /// Returns whether any rule was removed. Rules are attached to one field, so
    /// this is the whole lifetime of that field's cross-field rules.
    pub fn remove_form_rule(&mut self, key: &str) -> bool {
        if self.form_rules.remove(key).is_none() {
            return false;
        }
        // Re-derive rather than clear: `validation_error` carries no provenance, so a blind clear
        // also destroyed a verdict the row's own closure rule, typed rule or required flag still
        // produces — the form then reported valid until the next `validate()` put the message back.
        self.revalidate_field(key);
        true
    }

    /// The cross-field rules attached to `key`, in evaluation order.
    pub fn form_rules(&self, key: &str) -> &[FormRule] {
        self.form_rules.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Registers an asynchronous rule and issues a ticket for the current value.
    ///
    /// The host runs its work wherever it likes and then either resolves the
    /// ticket with [`Self::resolve`] (a result it computed itself) or calls
    /// [`Self::resolve_async`] (runs the registered rule). The ticket records the
    /// value and the form's revision, so a late result for text that changed is
    /// refused. An unknown key returns a cancelled ticket rather than pretending
    /// a request exists.
    pub fn set_field_async_validator<F>(&mut self, key: &str, validator: F) -> ValidationTicket
    where
        F: Fn(&str) -> Option<String> + 'static,
    {
        let known = self.field(key).is_some();
        let value = self.value_of(key).unwrap_or_default();
        // Issuing a ticket must not invalidate a ticket for another field: the
        // revision tracks **value changes**, not request count.
        self.next_ticket_id = self.next_ticket_id.wrapping_add(1);
        if known {
            self.async_validators.insert(key.to_string(), Rc::new(validator));
        }
        ValidationTicket {
            form: self.form_serial,
            id: self.next_ticket_id,
            key: key.to_string(),
            value,
            revision: self.validation_revision,
            resolved: false,
            cancelled: !known,
        }
    }

    /// Applies an asynchronous result, if the ticket is still current.
    ///
    /// Returns `false` when the ticket is stale (another form, a newer value, a
    /// cancel). A successful resolve stores the error on the field, emits
    /// [`Self::field_validated`] with `(key, valid)` and republishes the form's
    /// aggregate validity.
    pub fn resolve(&mut self, ticket: &mut ValidationTicket, error: Option<String>) -> bool {
        if ticket.resolved || ticket.cancelled {
            return false;
        }
        if ticket.form != self.form_serial
            || self.field(&ticket.key).is_none()
            || ticket.revision != self.validation_revision
            || self.value_of(&ticket.key).unwrap_or_default() != ticket.value
        {
            ticket.resolved = true;
            return false;
        }
        let valid = error.is_none();
        if let Some(field) = self.field_mut(&ticket.key) {
            field.set_async_error(error);
        }
        ticket.resolved = true;
        self.base.request_layout();
        self.base.request_redraw();
        self.field_validated.emit((ticket.key.clone(), valid));
        self.publish_derived_state();
        true
    }

    /// Runs the registered asynchronous rule for `ticket` and resolves it.
    ///
    /// This is the convenience path for a rule that needs no I/O of its own (a
    /// server response is resolved through [`Self::resolve`]). Returns `false`
    /// when the ticket is stale or no rule is registered.
    pub fn resolve_async(&mut self, ticket: &mut ValidationTicket) -> bool {
        if ticket.is_stale(self) {
            ticket.resolved = true;
            return false;
        }
        let Some(rule) = self.async_validators.get(&ticket.key).cloned() else {
            ticket.resolved = true;
            return false;
        };
        let error = rule(&ticket.value);
        self.resolve(ticket, error)
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
        let before = read_field_value(widget);
        if !write_field_value(widget, value) {
            return false;
        }
        // A write of the value the field already holds is not a change: reporting it would make
        // `field_changed` fire from the form path only, while a child-driven write stays an edge
        // (see `resync_derived`).
        if read_field_value(widget) == before {
            self.note_observed(key);
            return true;
        }
        // A stored asynchronous verdict describes the *old* text, so a write
        // retires it in the same step that bumps the revision.
        let retired_verdict = match self.field_mut(key) {
            Some(field) => {
                let had_verdict = field.async_error().is_some();
                field.async_error = None;
                had_verdict
            }
            None => false,
        };
        self.bump_validation_revision();
        if retired_verdict {
            // The retired verdict owned an error line; the row has to give that height back.
            self.base.request_layout();
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

    /// Runs the required, typed, host and cross-field passes and publishes the
    /// new aggregate validity.
    ///
    /// A host-reported error is left alone: `validate` owns the *derived* error slot only, so a
    /// domain rule a caller installed survives a validation pass (and can only be cleared by
    /// [`Self::clear_field_error`]). The same is true of an asynchronous result
    /// already stored on the field.
    ///
    /// Hidden rows are not judged, because [`Self::is_valid`] does not count them; the two rules
    /// are one contract (see [`Self::set_field_visible`]).
    pub fn validate(&mut self) -> bool {
        self.apply_validation(None);
        self.base.request_layout();
        // The pass writes the derived error slots, so the error/help lines it produced (and the
        // lines it retired) are pixel changes. A layout request alone only invalidates the cached
        // boxes; without the repaint the row keeps painting the verdict from the previous pass.
        self.base.request_redraw();
        let valid = self.is_valid();
        self.publish_validity(valid);
        valid
    }

    /// Runs the validation pass for one field and publishes the aggregate edge.
    ///
    /// Returns `false` for an unknown key. A host that validates field by field still sees one
    /// `validity_changed` edge for the form as a whole, because the aggregate is republished after
    /// the single-field pass — the same rule [`Self::validate`] follows. A hidden field is a
    /// successful no-op: it owns no verdict while it is outside the aggregate.
    pub fn validate_field(&mut self, key: &str) -> bool {
        if self.field(key).is_none() {
            return false;
        }
        self.apply_validation(Some(key));
        self.base.request_layout();
        self.base.request_redraw();
        self.publish_derived_state();
        true
    }

    /// Runs one validation pass, for every field or for `only`.
    ///
    /// The value map is materialised once so a cross-field rule sees one consistent
    /// snapshot instead of re-reading every field per comparison.
    fn apply_validation(&mut self, only: Option<&str>) {
        let values = self.current_values();
        let required_error = self.required_error.clone();
        {
            let validators = &self.validators;
            let input_validators = &self.input_validators;
            let rules = &self.form_rules;
            for field in &mut self.fields {
                // A hidden row is outside the aggregate: it paints no error, `focus_first_invalid`
                // skips it and `is_valid` ignores it, so judging it here would produce a verdict
                // the user has no way to see or fix. Its old verdict is retired for the same
                // reason: keeping it resurfaced a stale error when the row was shown again.
                if !field.visible {
                    field.validation_error = None;
                    continue;
                }
                if only.is_some_and(|key| key != field.key.as_str()) {
                    continue;
                }
                let value = field.widget.and_then(read_field_value);
                field.validation_error = field_validation_error(
                    &field.key,
                    field.required,
                    value.as_deref(),
                    &values,
                    &required_error,
                    validators,
                    input_validators,
                    rules,
                );
            }
        }
    }

    /// Every visible field's current value, with unbound fields reported as empty.
    ///
    /// Hidden rows are omitted: a cross-field rule must not compare against a value the reader
    /// cannot see or edit, which is the same reason `apply_validation` skips them as the judged
    /// row.
    fn current_values(&self) -> BTreeMap<String, String> {
        self.fields
            .iter()
            .filter(|field| field.visible)
            .map(|field| {
                (field.key.clone(), field.widget.and_then(read_field_value).unwrap_or_default())
            })
            .collect()
    }

    /// Invalidates outstanding asynchronous tickets.
    fn bump_validation_revision(&mut self) {
        self.validation_revision = self.validation_revision.wrapping_add(1);
    }

    /// Whether every visible field currently reports no error and every required field has a
    /// value.
    ///
    /// A hidden row is outside the aggregate: it cannot be painted, focused or corrected, so it
    /// never blocks a submit. [`Self::set_field_visible`] publishes the resulting edge, which is
    /// what keeps the visible/hidden transition and this predicate from disagreeing.
    pub fn is_valid(&self) -> bool {
        self.fields.iter().filter(|field| field.visible).all(|field| {
            if field.error().is_some() {
                return false;
            }
            if !field.required {
                return true;
            }
            field.widget.and_then(read_field_value).is_some_and(|value| !value.trim().is_empty())
        })
    }

    /// How many visible fields currently show an error.
    pub fn error_count(&self) -> usize {
        self.fields.iter().filter(|field| field.visible && field.error().is_some()).count()
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
        for field in &self.fields {
            if !field.visible || field.error().is_none() {
                continue;
            }
            let Some(widget) = field.widget else {
                continue;
            };
            // `focus_widget` only checks mount and modals; a disabled control and a control that
            // reports `is_focusable() == false` must not receive the caret, so the row is skipped
            // (and the search moves on instead of answering `false` for the whole form).
            if !rust_widgets::widget::runtime::widget_wants_keyboard_focus(widget) {
                continue;
            }
            if rust_widgets::widget::runtime::has_focus(widget) {
                return true;
            }
            if rust_widgets::widget::runtime::focus_widget(widget) {
                return true;
            }
        }
        false
    }

    /// Restores every baseline value and clears the errors, then emits `reset`.
    ///
    /// Returns whether every baseline write succeeded. A bound control that publishes none of
    /// `text`/`current_text`/`value` cannot be restored; such a field keeps its edited value and
    /// its error, and the `false` tells the host the reset was partial rather than silent.
    pub fn reset(&mut self) -> bool {
        let mut restored = true;
        for field in &mut self.fields {
            match (field.widget, field.baseline.clone()) {
                (Some(widget), Some(baseline)) => {
                    if write_field_value(widget, &baseline) {
                        field.clear_error();
                    } else {
                        restored = false;
                    }
                }
                // Bound, but the control publishes none of `text`/`current_text`/`value`, so
                // `add_field` stored no baseline: this field was not restored, and the documented
                // `false` is what tells the host the reset was partial.
                (Some(_), None) => restored = false,
                (None, _) => field.clear_error(),
            }
        }
        self.bump_validation_revision();
        self.base.request_layout();
        self.base.request_redraw();
        self.sync_observed();
        self.reset_requested.emit();
        self.publish_derived_state();
        restored
    }

    /// Abandons the validation state without restoring the baseline.
    ///
    /// This is deliberately **not** [`Self::reset`]: reset writes the values the
    /// form opened with, while cancelling means "stop judging these edits" — the
    /// user's values stay in the child controls, every error is cleared and
    /// outstanding asynchronous tickets are invalidated. Emits
    /// [`Self::cancel_requested`].
    pub fn cancel(&mut self) {
        for field in &mut self.fields {
            field.clear_error();
        }
        // A ticket issued before the cancel must not land after it.
        self.bump_validation_revision();
        self.base.request_layout();
        self.base.request_redraw();
        self.cancel_requested.emit();
        self.publish_derived_state();
    }

    /// Re-applies the fields' presentation and requests a layout pass and a repaint.
    ///
    /// Call this after a direct [`Self::field_mut`] edit that changed a label, the required flag,
    /// the help text or a visibility flag. `refresh` is the *presentation* half: it writes each
    /// hidden/shown field's flag into its control and invalidates the cached boxes. It does not
    /// re-read values or publish events — [`Self::resync`] is the half that does.
    pub fn refresh(&self) {
        // A `field_mut` edit flips the field's own `visible` flag, while the child control keeps
        // its own; without this reconcile the row leaves the layout but the control stays painted,
        // which is exactly the draw/hit mismatch the documented visibility edit could produce.
        for field in &self.fields {
            if let Some(widget) = field.widget {
                rust_widgets::set_widget_visible(widget, field.visible);
            }
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Re-reads every bound field and publishes the edges a direct write produced.
    ///
    /// This is the mutable counterpart of [`Self::refresh`]. A user's keystrokes, or a host that
    /// writes a child behind the form's back, change the child's value without telling the form;
    /// `resync` re-reads every field, retires asynchronous verdicts for text that changed, and
    /// publishes `field_changed`, `dirty_changed` and `validity_changed`. The value of record stays
    /// in the child — the observation is an edge detector, not a second store.
    ///
    /// Call `refresh` for a presentation-only edit and `resync` when the signals matter too; a
    /// host that wants both calls both (the two are additive and idempotent).
    pub fn resync(&mut self) {
        self.resync_derived();
        // A direct edit can change validity without changing a value — the required flag and a
        // visibility flag both do — and `resync_derived` only publishes on a value edge.
        self.publish_derived_state();
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Re-reads every bound field's value and publishes the edges a direct edit produced.
    ///
    /// The form is a container: the user's keystrokes reach the child control directly, so the
    /// form only learns about them on the next event it receives. Comparing against the last
    /// observed value turns that into `field_changed` plus the `dirty`/`valid` edges. The snapshot
    /// is an edge detector, not a second value store — the value of record stays in the child.
    fn resync_derived(&mut self) {
        let mut changed = false;
        let mut changed_keys: Vec<String> = Vec::new();
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
                        changed_keys.push(field.key.clone());
                        self.field_changed.emit(field.key.clone());
                    }
                }
                None => self.observed.push((field.key.clone(), current)),
            }
        }
        if changed {
            // An asynchronous result for the previous text is no longer an
            // answer about the current one.
            let mut retired_verdict = false;
            for field in &mut self.fields {
                if changed_keys.contains(&field.key) && field.async_error.is_some() {
                    field.async_error = None;
                    retired_verdict = true;
                }
            }
            self.bump_validation_revision();
            if retired_verdict {
                // A retired verdict frees the error line it reserved, so the boxes move too.
                self.base.request_layout();
            }
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
        self.with_layouts(|layouts, _| layouts.to_vec())
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
            Some(cache)
                if cache.revision == revision
                    && cache.x == content.x
                    && cache.y == content.y
                    && cache.width == content.width
        );
        if fresh {
            return;
        }
        let (layouts, field_indices, _) = self.compute_layouts(content.x, content.y, content.width);
        *self.layout_cache.borrow_mut() = Some(LayoutCache {
            revision,
            x: content.x,
            y: content.y,
            width: content.width,
            layouts,
            field_indices,
        });
    }

    /// Runs `f` over the current boxes and their field indices without cloning them.
    fn with_layouts<R>(&self, f: impl FnOnce(&[FormFieldLayout], &[usize]) -> R) -> R {
        self.ensure_layouts();
        let cache = self.layout_cache.borrow();
        let cache = cache.as_ref().expect("ensure_layouts stored the boxes");
        f(&cache.layouts, &cache.field_indices)
    }

    /// The control box of `key`, if the field is visible.
    pub fn control_rect_of(&self, key: &str) -> Option<Rect> {
        self.with_layouts(|layouts, _| {
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

    /// The cancel button's box, or `None` when no cancel label is configured.
    ///
    /// It sits to the left of reset so adding it never moves the submit button,
    /// which is the anchor every host and test already relies on.
    pub fn cancel_rect(&self) -> Option<Rect> {
        if self.cancel_label.is_empty() {
            return None;
        }
        let reset = self.reset_rect();
        let width = self.button_width.max(BUTTON_MIN_W);
        Some(Rect::new(reset.x - self.row_spacing as i32 - width as i32, reset.y, width, BUTTON_H))
    }

    /// The action button under `pos`, or `None` when the row is hidden or the point misses.
    ///
    /// Draw and hit-testing both call this, so the button that is highlighted is exactly the one
    /// a press activates. The predicate also reads the live rectangles, which is what lets a
    /// geometry or padding change move the highlight with the button instead of leaving it on a
    /// remembered box.
    fn action_at(&self, pos: Point) -> Option<FormAction> {
        if !self.show_buttons {
            return None;
        }
        if self.submit_rect().contains_point(pos) {
            return Some(FormAction::Submit);
        }
        if self.reset_rect().contains_point(pos) {
            return Some(FormAction::Reset);
        }
        if self.cancel_rect().is_some_and(|rect| rect.contains_point(pos)) {
            return Some(FormAction::Cancel);
        }
        None
    }

    /// Records the pointer position the per-button hover is derived from.
    fn update_hover(&self, event: &Event) {
        match event {
            Event::MouseEnter { pos } | Event::MouseMove { pos } => self.pointer.set(Some(*pos)),
            Event::MouseLeave { .. } => self.pointer.set(None),
            _ => {}
        }
    }

    /// The action button the pointer is over, as its published-style token.
    ///
    /// Test observability for the per-button hover state; the capability surface does not expose
    /// hover, because it is pointer feedback rather than model state.
    #[cfg(test)]
    pub(crate) fn hovered_action(&self) -> Option<&'static str> {
        self.pointer.get().and_then(|pos| self.action_at(pos)).map(|action| match action {
            FormAction::Submit => "submit",
            FormAction::Reset => "reset",
            FormAction::Cancel => "cancel",
        })
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
    /// Returns the layouts, the `fields` index each layout belongs to, and the total height they
    /// occupy. The x/y origin is a parameter because `size_hint` asks the same question at a
    /// synthetic origin.
    fn compute_layouts(
        &self,
        x0: i32,
        y0: i32,
        width: u32,
    ) -> (Vec<FormFieldLayout>, Vec<usize>, u32) {
        // `i64` for the width arithmetic: the public column count is a `u32`, and casting it to
        // `i32` turned `2^31` into `i32::MIN`, whose `- 1` overflows (panic in debug, an inverted
        // gap in release). The clamped count keeps the row/index maths in `usize`.
        let columns = self.columns.clamp(1, MAX_COLUMNS) as usize;
        let gap = i64::from(self.column_spacing);
        let column_count = columns as i64;
        let total_gap = (gap * (column_count - 1)).max(0);
        let usable = (i64::from(width) - total_gap).max(0);
        let column_width = (usable / column_count) as u32;
        let visible: Vec<(usize, &FormField)> =
            self.fields.iter().enumerate().filter(|(_, field)| field.visible).collect();
        let mut layouts = Vec::with_capacity(visible.len());
        let mut field_indices = Vec::with_capacity(visible.len());
        let mut y = y0;
        let mut index = 0;
        while index < visible.len() {
            let end = visible.len().min(index + columns);
            let row = &visible[index..end];
            let mut row_height = 0u32;
            for (_, field) in row {
                row_height = row_height.max(self.field_height(field));
            }
            for (column, (field_index, field)) in row.iter().enumerate() {
                let x = i64::from(x0) + column as i64 * (i64::from(column_width) + gap);
                let x = x.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
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
                field_indices.push(*field_index);
            }
            y += row_height as i32 + self.row_spacing as i32;
            index += columns;
        }
        // The loop advances past the last row's gap as well, but the gap after the final row is
        // not height the fields ask for: counting it would make `size_hint` (and therefore
        // `overflows`) claim a form needs one row spacing more than it does.
        let trailing_gap = if layouts.is_empty() { 0 } else { self.row_spacing as i32 };
        let used = (y - y0 - trailing_gap).max(0) as u32;
        (layouts, field_indices, used)
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
        // Only reserve the line for text `draw` will actually paint. An error that `show_errors`
        // hides also shadows the help text (the error slot wins), so that combination paints
        // nothing and must not hold a blank line under the control.
        let shows_error = self.show_errors && field.error().is_some();
        let shows_help = field.error().is_none() && !field.help().is_empty();
        if shows_error || shows_help {
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
    fn place_layouts(&self, layouts: &[FormFieldLayout], field_indices: &[usize]) {
        for (layout, field_index) in layouts.iter().zip(field_indices) {
            // The index came from the same computation as the box; `get` keeps a broken invariant
            // from panicking rather than trusting it.
            let Some(widget) = self.fields.get(*field_index).and_then(FormField::widget_id) else {
                continue;
            };
            rust_widgets::set_widget_geometry(
                widget,
                layout.control.x,
                layout.control.y,
                layout.control.width,
                layout.control.height,
            );
        }
    }

    fn place_field_widgets(&self) {
        self.with_layouts(|layouts, field_indices| {
            self.place_layouts(layouts, field_indices);
        });
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

    fn draw_button(
        &self,
        context: &mut RenderContext,
        rect: Rect,
        label: &str,
        ink: Color,
        action: FormAction,
    ) {
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Hover and press are per button: the pointer over submit must not light reset and cancel
        // too. The base tracks them for the control as a whole, so the action under the pointer is
        // the discriminator; draw and hit-testing share `action_at`, so a highlighted button is
        // always the one a press activates.
        let enabled = self.base.is_enabled();
        let hovered =
            enabled && self.pointer.get().is_some_and(|pos| self.action_at(pos) == Some(action));
        let pressed = hovered && self.base.is_pressed();
        let fill_alpha = if !enabled {
            8
        } else if pressed {
            30
        } else if hovered {
            24
        } else {
            18
        };
        let stroke_alpha = if enabled { 90 } else { 40 };
        let ink = if enabled { ink } else { ink.with_alpha(120) };
        context.fill_rounded_rect(rect, 4, ink.with_alpha(fill_alpha));
        context.draw_rounded_rect_stroke(rect, 4, ink.with_alpha(stroke_alpha), 1);
        let font = role_font(self.base.style(), 12.0);
        context.draw_text_fitted_ellipsis(rect, label, &font, ink, HorizontalAlignment::Center);
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
    // A control that publishes its value only as a String under `value` (`OtpInput`, for one) was
    // readable but never writable: the read probe takes a String from any of the three names, while
    // this probe only ever wrote a String to `text`/`current_text`.
    if write("value", CapabilityValue::String(value.to_string())) {
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
    fn capability_name(&self) -> Option<&'static str> {
        Some("form")
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// Disables the form **and the fields it hosts**, whichever spelling of the call a host uses.
    ///
    /// The property route has always propagated (FM-06); the trait method is the other entry
    /// point, so it follows the same rule here. The container's own bookkeeping is the trait
    /// default, written out in `super::state::apply_enabled_state` because an override cannot
    /// call the implementation it replaces.
    fn set_enabled(&mut self, enabled: bool) {
        // Propagate only on a real transition: adoption already applies the container's flag
        // (FM-06), and a per-frame container sync that reasserts the same value must not descend
        // into every field again.
        let changed = self.base.is_enabled() != enabled;
        super::state::apply_enabled_state(self, enabled);
        if !changed {
            return;
        }
        if !enabled {
            // A disabled form paints no hover, and the remembered position would otherwise light
            // a button again the moment it is enabled.
            self.pointer.set(None);
        }
        for field in &self.fields {
            if let Some(widget) = field.widget {
                rust_widgets::set_widget_enabled(widget, enabled);
            }
        }
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
        let (_, _, content) = self.compute_layouts(0, 0, width);
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
            "field_validated" => Some(EventSignalRef::mapped(
                "field_validated",
                &self.field_validated,
                |(key, valid)| {
                    CapabilityValue::Tuple(rust_widgets::compat::Vec::from([
                        CapabilityValue::String(key.clone()),
                        CapabilityValue::Bool(*valid),
                    ]))
                },
            )),
            "cancel" => Some(EventSignalRef::unit("cancel", &self.cancel_requested)),
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
            "cancel_label" => Ok(CapabilityValue::String(self.cancel_label.clone())),
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
            "cancel_label" => {
                self.set_cancel_label(expect_string(value)?);
                Ok(())
            }
            "button_width" => {
                self.set_button_width(expect_u32(value)?);
                Ok(())
            }
            // A disabled container disables the controls it hosts: the fields are separate
            // widgets and the router can dispatch to them directly, so writing this control's
            // own flag alone would leave them interactive (FM-06). The trait override owns that
            // propagation, so the property route and `Widget::set_enabled` now agree.
            "enabled" => {
                self.set_enabled(expect_bool(value)?);
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
            "cancel_label",
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
            "cancel" => {
                self.cancel();
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
            // Payload-carrying assignments use the property route; an unpublished suffix is
            // honestly unknown.
            _ => self.default_command(name),
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
            context.draw_text_fitted_ellipsis(
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
        self.with_layouts(|layouts, field_indices| {
            for (layout, field_index) in layouts.iter().zip(field_indices) {
                let Some(field) = self.fields.get(*field_index) else {
                    continue;
                };
                if self.show_labels {
                    let mut label = self.label_scratch.borrow_mut();
                    label.clear();
                    label.push_str(field.label());
                    if field.is_required() && !self.required_mark.is_empty() {
                        label.push_str(&self.required_mark);
                    }
                    context.draw_text_fitted_ellipsis(
                        layout.label,
                        &label,
                        &label_font,
                        ink,
                        HorizontalAlignment::Left,
                    );
                }
                if let Some(error) = field.error() {
                    if self.show_errors {
                        context.draw_text_fitted_ellipsis(
                            layout.error,
                            error,
                            &label_font,
                            error_ink,
                            HorizontalAlignment::Left,
                        );
                    }
                } else if !field.help().is_empty() {
                    context.draw_text_fitted_ellipsis(
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
            self.draw_button(
                context,
                self.submit_rect(),
                &self.submit_label,
                ink,
                FormAction::Submit,
            );
            self.draw_button(context, self.reset_rect(), &self.reset_label, ink, FormAction::Reset);
            if let Some(rect) = self.cancel_rect() {
                self.draw_button(context, rect, &self.cancel_label, ink, FormAction::Cancel);
            }
        }

        // The children are drawn by the tree painter; this only has to keep their boxes current
        // for a frame that served the layout from cache. The boxes come from the same cache, so
        // the frame pays for one layout pass and clones nothing.
        self.with_layouts(|layouts, field_indices| {
            self.place_layouts(layouts, field_indices);
        });
    }
}

impl EventHandler for Form {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        // Record the pointer before the enabled guard: hover is a position, and draw decides
        // whether it is painted. MouseLeave clears it.
        self.update_hover(event);
        if !self.base.is_enabled() {
            return;
        }
        if let Event::MousePress { pos, button: 1, .. } = event {
            // One predicate for drawing and activation: the box the pointer highlights is the box
            // the press reaches.
            if let Some(action) = self.action_at(*pos) {
                match action {
                    FormAction::Submit => {
                        let _ = self.submit();
                    }
                    FormAction::Reset => {
                        self.reset();
                    }
                    FormAction::Cancel => self.cancel(),
                }
                return;
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
        // A direct edit of a child may have changed its value; publish the edges it produced
        // (`field_changed`, `dirty_changed`, `validity_changed`) before returning.
        self.resync_derived();
    }
}
