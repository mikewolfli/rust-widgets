// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::core::{Color, Rect};
use crate::pdf::types::PdfFormField;
use std::collections::HashMap;
/// The kind of an AcroForm field, controlling how the field's value is
/// interpreted and what control a viewer draws for it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    /// A single-line or multiline free-text entry.
    Text,
    /// A boolean toggle that may be checked or cleared.
    Checkbox,
    /// One choice out of a mutually exclusive group.
    Radio,
    /// A scrolling list from which one or more options may be selected.
    ListBox,
    /// A drop-down list from which exactly one option may be selected.
    ComboBox,
    /// A push button that carries an action rather than a value.
    Button,
    /// A digital signature placeholder.
    Signature,
}
/// An interactive form field, holding both the field's value and the
/// appearance and validation options that apply to it.
#[derive(Debug, Clone)]
pub struct FormField {
    /// Caller-assigned identifier, unique within a form. Used as the key by the
    /// lookup and removal helpers on [`Form`].
    pub id: String,
    /// The fully qualified field name, which is the key used when values are
    /// exported or imported as a set (see [`Form::get_all_values`]).
    pub name: String,
    /// The kind of field, which determines how the value is rendered and shown.
    pub field_type: FieldType,
    /// Zero-based index of the page the field appears on.
    pub page: u32,
    /// The field's rectangle on the page, used both for drawing and for hit
    /// testing via [`FormField::contains_point`].
    pub rect: Rect,
    /// The field's current value. For a checkbox or radio this is the selected
    /// option's text; for a button it is the label; for the list types the
    /// selection is held in [`FormField::selected_indices`] instead.
    pub value: String,
    /// The value the field reverts to when the form is reset (see
    /// [`Form::reset_to_defaults`]).
    pub default_value: String,
    /// Whether the user may change the value. Enforced by
    /// [`FormField::set_value`], [`FormField::select_option`],
    /// [`FormField::deselect_option`] and [`FormField::clear_selection`].
    pub is_read_only: bool,
    /// Whether the field must hold a value for the form to validate. Only a
    /// non-empty [`FormField::value`] is accepted; see [`Form::validate`].
    pub is_required: bool,
    /// Whether the field is excluded from display.
    pub is_hidden: bool,
    /// Short help text shown when the pointer hovers over the field.
    pub tooltip: String,
    /// PostScript name of the font used to draw the field's text. Defaults to
    /// `"Helvetica"`.
    pub font_name: String,
    /// Font size for the field's text, in points. Defaults to `12.0`.
    pub font_size: f32,
    /// Colour of the field's text. Defaults to [`Color::BLACK`].
    pub text_color: Color,
    /// Fill colour behind the field's text, or `None` for no fill.
    pub background_color: Option<Color>,
    /// Colour of the border drawn around the field, or `None` for no border.
    pub border_color: Option<Color>,
    /// Width of the field's border, in points. Defaults to `1.0` and is ignored
    /// when [`FormField::border_color`] is `None`.
    pub border_width: f32,
    /// The available choices for a list, combo or radio field, in display
    /// order. [`FormField::value`] and [`FormField::selected_indices`] refer to
    /// positions in this vector.
    pub options: Vec<String>,
    /// Indices into [`FormField::options`] that are currently selected. A radio
    /// or checkbox field keeps at most one entry; a list box may hold several.
    pub selected_indices: Vec<usize>,
    /// Maximum number of characters accepted, counted in `char`s rather than
    /// bytes, or `None` for unlimited. Enforced by [`FormField::set_value`],
    /// which truncates over-long input rather than rejecting it; a value written
    /// directly to the field is not subject to the limit.
    pub max_length: Option<u32>,
    /// Whether text entry spans multiple lines. Defaults to `false`.
    pub is_multiline: bool,
    /// Whether the typed text is obscured. [`FormField::get_display_value`]
    /// substitutes one `*` per character.
    pub is_password: bool,
    /// Whether the field accepts a filesystem path instead of a word value.
    pub is_file_select: bool,
    /// Whether the viewer should spell-check the field's text. Defaults to
    /// `true`.
    pub is_spell_check_enabled: bool,
    /// Whether long content may be scrolled within the field. Defaults to
    /// `true`.
    pub is_scrollable: bool,
    /// Whether the user should be required to fill the field with the field as
    /// the only visible hint, as in a segmented code entry. Defaults to `false`.
    pub is_comb: bool,
    /// The field's value with formatting applied, for viewers that display rich
    /// text. Kept alongside [`FormField::value`] rather than replacing it.
    pub rich_text_value: String,
    /// Arbitrary application-specific key/value pairs stored with the field.
    pub custom_data: HashMap<String, String>,
}
impl FormField {
    /// Creates a field of the given type on `page`, positioned at `rect`.
    ///
    /// `value` and `default_value` are empty, the field is editable and
    /// optional, the font is Helvetica at 12 points with black text and no
    /// background or border, options and selections are empty, and no length
    /// limit is set.
    pub fn new(id: String, name: String, field_type: FieldType, page: u32, rect: Rect) -> Self {
        Self {
            id,
            name,
            field_type,
            page,
            rect,
            value: String::new(),
            default_value: String::new(),
            is_read_only: false,
            is_required: false,
            is_hidden: false,
            tooltip: String::new(),
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            text_color: Color::BLACK,
            background_color: None,
            border_color: None,
            border_width: 1.0,
            options: Vec::new(),
            selected_indices: Vec::new(),
            max_length: None,
            is_multiline: false,
            is_password: false,
            is_file_select: false,
            is_spell_check_enabled: true,
            is_scrollable: true,
            is_comb: false,
            rich_text_value: String::new(),
            custom_data: HashMap::new(),
        }
    }
    /// Builder-style setter for the field's current value.
    pub fn with_value(mut self, value: String) -> Self {
        self.value = value;
        self
    }
    /// Builder-style setter replacing the field's available choices.
    ///
    /// Any indices already in [`FormField::selected_indices`] are left untouched
    /// and are not revalidated, so they may no longer address the new options.
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
    }
    /// Builder-style setter for the hover help text.
    pub fn with_tooltip(mut self, tooltip: String) -> Self {
        self.tooltip = tooltip;
        self
    }
    /// Builder-style setter marking the field read-only.
    pub fn read_only(mut self) -> Self {
        self.is_read_only = true;
        self
    }
    /// Builder-style setter marking the field required.
    pub fn required(mut self) -> Self {
        self.is_required = true;
        self
    }
    /// Sets the field's value, truncating it to [`FormField::max_length`]
    /// characters (counted as `char`s, so multi-byte text is cut on a character
    /// boundary) when that limit is set.
    ///
    /// A no-op when the field is read-only.
    pub fn set_value(&mut self, value: String) {
        if !self.is_read_only {
            if let Some(max) = self.max_length {
                let truncated: String = value.chars().take(max as usize).collect();
                self.value = truncated;
            } else {
                self.value = value;
            }
        }
    }
    /// The text a viewer should show for the field: the value with each
    /// character replaced by `*` when [`FormField::is_password`] is set, and the
    /// value itself otherwise.
    pub fn get_display_value(&self) -> String {
        if self.is_password {
            "*".repeat(self.value.len())
        } else {
            self.value.clone()
        }
    }
    /// Whether the option at `index` is currently selected. An out-of-range
    /// index is simply not selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }
    /// Selects the option at `index`, ignoring an index outside
    /// [`FormField::options`] and any call on a read-only field.
    ///
    /// A radio or checkbox field replaces the whole selection and copies the
    /// option's text into [`FormField::value`]; any other field type adds
    /// `index` to the selection if it is not already there.
    pub fn select_option(&mut self, index: usize) {
        if !self.is_read_only && index < self.options.len() {
            if self.field_type == FieldType::Radio || self.field_type == FieldType::Checkbox {
                self.selected_indices = vec![index];
                if let Some(option) = self.options.get(index) {
                    self.value = option.clone();
                }
            } else {
                if !self.selected_indices.contains(&index) {
                    self.selected_indices.push(index);
                }
            }
        }
    }
    /// Removes the option at `index` from the selection, ignoring an index that
    /// was not selected. A no-op on a read-only field.
    ///
    /// Unlike [`FormField::select_option`], this does not update
    /// [`FormField::value`].
    pub fn deselect_option(&mut self, index: usize) {
        if !self.is_read_only {
            self.selected_indices.retain(|&i| i != index);
        }
    }
    /// Clears both [`FormField::selected_indices`] and [`FormField::value`].
    /// A no-op on a read-only field.
    pub fn clear_selection(&mut self) {
        if !self.is_read_only {
            self.selected_indices.clear();
            self.value.clear();
        }
    }
    /// Convert this `FormField` to a `PdfFormField` for PDF serialization.
    pub fn to_pdf_form_field(&self) -> PdfFormField {
        match self.field_type {
            FieldType::Text => PdfFormField::TextField {
                name: self.name.clone(),
                rect: self.rect,
                value: self.value.clone(),
            },
            FieldType::Checkbox | FieldType::Radio => PdfFormField::CheckBox {
                name: self.name.clone(),
                rect: self.rect,
                checked: !self.value.is_empty() && self.value != "Off" && self.value != "false",
            },
            FieldType::Button => PdfFormField::Button {
                name: self.name.clone(),
                rect: self.rect,
                text: self.value.clone(),
            },
            FieldType::ComboBox => PdfFormField::ComboBox {
                name: self.name.clone(),
                rect: self.rect,
                value: self.value.clone(),
                options: self.options.clone(),
            },
            FieldType::ListBox => PdfFormField::ListBox {
                name: self.name.clone(),
                rect: self.rect,
                selected: self.selected_indices.clone(),
                options: self.options.clone(),
            },
            FieldType::Signature => {
                // Signatures fall back to text fields in the simplified model.
                PdfFormField::TextField {
                    name: self.name.clone(),
                    rect: self.rect,
                    value: self.value.clone(),
                }
            }
        }
    }

    /// Whether the point `(x, y)` falls inside the field's rectangle, in the
    /// page coordinate space the rectangle lives in. The lower and left edges
    /// count as inside, the upper and right edges do not.
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.rect.contains_point(crate::core::Point::from_f32(x as f32, y as f32))
    }
}
/// A PDF form: a collection of [`FormField`]s together with the document-level
/// options that govern how they are exported.
#[derive(Debug, Clone)]
pub struct Form {
    /// Caller-assigned identifier used as the key by [`FormManager`].
    pub id: String,
    /// Human-readable form name.
    pub name: String,
    /// The fields belonging to this form, in insertion order.
    pub fields: Vec<FormField>,
    /// Whether the viewer should regenerate the fields' appearances rather than
    /// relying on appearances stored in the file. Defaults to `true`.
    pub need_appearances: bool,
    /// The signature flags (`/SigFlags`) to write for the form. `0` means the
    /// form is not intended to be signed.
    pub sig_flags: u32,
    /// The calculation order (`/CO`) giving the field names in the order their
    /// computations should be evaluated, or `None` to leave it unset.
    pub co: Option<String>,
    /// Default resource names to resource descriptions, shared by the fields.
    pub default_resources: HashMap<String, String>,
}
impl Form {
    /// Creates an empty form with no fields, `need_appearances` set to `true`,
    /// no signature flags, no calculation order and no default resources.
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            fields: Vec::new(),
            need_appearances: true,
            sig_flags: 0,
            co: None,
            default_resources: HashMap::new(),
        }
    }
    /// Appends `field` to the form. The field's id is not checked for
    /// duplication.
    pub fn add_field(&mut self, field: FormField) {
        self.fields.push(field);
    }
    /// Removes and returns the first field whose id is `id`, or `None` if the
    /// form has no such field.
    pub fn remove_field(&mut self, id: &str) -> Option<FormField> {
        if let Some(index) = self.fields.iter().position(|f| f.id == id) {
            Some(self.fields.remove(index))
        } else {
            None
        }
    }
    /// Looks up the first field whose id is `id`.
    pub fn get_field(&self, id: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.id == id)
    }
    /// Looks up the first field whose id is `id` for modification.
    pub fn get_field_mut(&mut self, id: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.id == id)
    }
    /// Looks up the first field whose fully qualified name is `name`. Names are
    /// not required to be unique, so an earlier field wins.
    pub fn get_field_by_name(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name == name)
    }
    /// Looks up the first field whose fully qualified name is `name` for
    /// modification. Names are not required to be unique, so an earlier field
    /// wins.
    pub fn get_field_by_name_mut(&mut self, name: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.name == name)
    }
    /// Returns the fields on `page` whose rectangle contains `(x, y)`, in
    /// insertion order. Overlapping fields are all returned.
    pub fn get_fields_at_point(&self, page: u32, x: i32, y: i32) -> Vec<&FormField> {
        self.fields.iter().filter(|f| f.page == page && f.contains_point(x, y)).collect()
    }
    /// Collects every field's current value, keyed by field name. Fields sharing
    /// a name collapse into a single entry, and a nameless field yields an entry
    /// under the empty string.
    pub fn get_all_values(&self) -> HashMap<String, String> {
        self.fields.iter().map(|f| (f.name.clone(), f.value.clone())).collect()
    }
    /// Applies `values` by field name, going through [`FormField::set_value`] so
    /// that read-only fields and length limits are respected. Names with no
    /// matching field are ignored.
    pub fn set_all_values(&mut self, values: HashMap<String, String>) {
        for (name, value) in values {
            if let Some(field) = self.get_field_by_name_mut(&name) {
                field.set_value(value);
            }
        }
    }
    /// Restores every field to its default: the value becomes
    /// [`FormField::default_value`] and the selection is cleared.
    ///
    /// This bypasses [`FormField::set_value`], so it writes to read-only fields
    /// as well and does not apply [`FormField::max_length`]. The value it
    /// restores is also not checked against the field's options.
    pub fn reset_to_defaults(&mut self) {
        for field in &mut self.fields {
            field.value = field.default_value.clone();
            field.selected_indices.clear();
        }
    }
    /// Removes every field from the form.
    pub fn clear(&mut self) {
        self.fields.clear();
    }
    /// Number of fields in the form.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
    /// Number of fields marked [`FormField::is_required`].
    pub fn required_field_count(&self) -> usize {
        self.fields.iter().filter(|f| f.is_required).count()
    }
    /// Convert all fields in this form to `PdfFormField` entries suitable
    /// for serialization into the PDF `/Annots` array.
    ///
    /// Only fields on the given page are included.
    pub fn to_pdf_form_fields(&self, page: u32) -> Vec<PdfFormField> {
        self.fields.iter().filter(|f| f.page == page).map(|f| f.to_pdf_form_field()).collect()
    }

    /// Convert all fields across all pages to `PdfFormField` entries.
    pub fn to_pdf_form_fields_all(&self) -> Vec<(u32, PdfFormField)> {
        self.fields.iter().map(|f| (f.page, f.to_pdf_form_field())).collect()
    }

    /// Validates every field and returns the problems found, in field order. An
    /// empty result means the form is valid.
    ///
    /// The only rule checked at present is that a field marked
    /// [`FormField::is_required`] has a non-empty [`FormField::value`]; in
    /// particular a required list or combo box whose selection lives only in
    /// [`FormField::selected_indices`] is reported as missing.
    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        for field in &self.fields {
            if field.is_required && field.value.is_empty() {
                errors.push(ValidationError {
                    field_id: field.id.clone(),
                    field_name: field.name.clone(),
                    message: format!("Field '{}' is required", field.name),
                });
            }
        }
        errors
    }
}
impl Default for Form {
    /// Returns a form with the id `"default"` and the name `"Default Form"`.
    fn default() -> Self {
        Self::new("default".to_string(), "Default Form".to_string())
    }
}
/// A single validation failure reported by [`Form::validate`].
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Id of the field that failed, matching [`FormField::id`].
    pub field_id: String,
    /// Name of the field that failed, matching [`FormField::name`].
    pub field_name: String,
    /// Human-readable description of the failure, suitable for showing to the
    /// user.
    pub message: String,
}
/// An in-memory collection of forms, with one of them designated as current
/// for convenience lookups.
pub struct FormManager {
    /// All forms, keyed by [`Form::id`].
    forms: HashMap<String, Form>,
    /// Id of the form considered current, if any.
    current_form: Option<String>,
}
impl FormManager {
    /// Creates an empty manager with no forms and no current form.
    pub fn new() -> Self {
        Self { forms: HashMap::new(), current_form: None }
    }
    /// Stores `form` under its id, replacing any form already stored under that
    /// id. The current form is left unchanged, so replacing the current form
    /// keeps it current.
    pub fn add_form(&mut self, form: Form) {
        let id = form.id.clone();
        self.forms.insert(id, form);
    }
    /// Removes and returns the form with the given id, or `None` if no such form
    /// is stored.
    ///
    /// If the removed form was the current one, the manager is left pointing at
    /// an id that no longer resolves, so the current-form accessors return
    /// `None` until [`FormManager::set_current_form`] is called again.
    pub fn remove_form(&mut self, id: &str) -> Option<Form> {
        self.forms.remove(id)
    }
    /// Looks up the form with the given id.
    pub fn get_form(&self, id: &str) -> Option<&Form> {
        self.forms.get(id)
    }
    /// Looks up the form with the given id for modification.
    pub fn get_form_mut(&mut self, id: &str) -> Option<&mut Form> {
        self.forms.get_mut(id)
    }
    /// Designates which form is current. The id is not validated, so an unknown
    /// id simply makes [`FormManager::get_current_form`] return `None`.
    pub fn set_current_form(&mut self, id: Option<String>) {
        self.current_form = id;
    }
    /// Looks up the current form.
    pub fn get_current_form(&self) -> Option<&Form> {
        self.current_form.as_ref().and_then(|id| self.forms.get(id))
    }
    /// Looks up the current form for modification.
    pub fn get_current_form_mut(&mut self) -> Option<&mut Form> {
        self.current_form.as_ref().and_then(|id| self.forms.get_mut(id))
    }
    /// Returns every form's values as a map from form id to that form's
    /// name-to-value map.
    pub fn get_all_field_values(&self) -> HashMap<String, HashMap<String, String>> {
        self.forms.iter().map(|(id, form)| (id.clone(), form.get_all_values())).collect()
    }
    /// Removes every form and clears the current-form selection.
    pub fn clear(&mut self) {
        self.forms.clear();
        self.current_form = None;
    }
    /// Number of forms in the manager.
    pub fn form_count(&self) -> usize {
        self.forms.len()
    }
    /// Convert all forms to a flat list of `(page, PdfFormField)` entries.
    pub fn to_pdf_form_fields_all(&self) -> Vec<(u32, PdfFormField)> {
        let mut result = Vec::new();
        for form in self.forms.values() {
            result.extend(form.to_pdf_form_fields_all());
        }
        result
    }

    /// Convert all forms to `PdfFormField` entries for a specific page.
    pub fn to_pdf_form_fields(&self, page: u32) -> Vec<PdfFormField> {
        let mut result = Vec::new();
        for form in self.forms.values() {
            result.extend(form.to_pdf_form_fields(page));
        }
        result
    }

    /// Total number of fields across all the manager's forms. Forms are counted
    /// independently, so the same field present in two forms counts twice.
    pub fn total_field_count(&self) -> usize {
        self.forms.values().map(|f| f.field_count()).sum()
    }
}
crate::impl_default_via_new!(FormManager);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_form_field_creation() {
        let field = FormField::new(
            "field-1".to_string(),
            "username".to_string(),
            FieldType::Text,
            1,
            Rect::new(100, 100, 200, 30),
        )
        .with_value("John Doe".to_string())
        .with_tooltip("Enter your username".to_string());
        assert_eq!(field.id, "field-1");
        assert_eq!(field.name, "username");
        assert_eq!(field.value, "John Doe");
        assert_eq!(field.tooltip, "Enter your username");
    }
    #[test]
    fn test_form_field_password() {
        let mut field = FormField::new(
            "field-1".to_string(),
            "password".to_string(),
            FieldType::Text,
            1,
            Rect::new(100, 100, 200, 30),
        );
        field.is_password = true;
        field.value = "secret123".to_string();
        assert_eq!(field.get_display_value(), "*********");
    }
    #[test]
    fn test_form_creation() {
        let mut form = Form::new("form-1".to_string(), "Login Form".to_string());
        let field = FormField::new(
            "field-1".to_string(),
            "username".to_string(),
            FieldType::Text,
            1,
            Rect::new(100, 100, 200, 30),
        );
        form.add_field(field);
        assert_eq!(form.field_count(), 1);
        assert!(form.get_field("field-1").is_some());
    }
    #[test]
    fn test_form_validation() {
        let mut form = Form::new("form-1".to_string(), "Login Form".to_string());
        let field = FormField::new(
            "field-1".to_string(),
            "username".to_string(),
            FieldType::Text,
            1,
            Rect::new(100, 100, 200, 30),
        )
        .required();
        form.add_field(field);
        let errors = form.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("required"));
    }
    #[test]
    fn test_form_manager() {
        let mut manager = FormManager::new();
        let form = Form::new("form-1".to_string(), "Test Form".to_string());
        manager.add_form(form);
        manager.set_current_form(Some("form-1".to_string()));
        assert_eq!(manager.form_count(), 1);
        assert!(manager.get_current_form().is_some());
    }
}
