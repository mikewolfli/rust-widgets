use crate::core::{Color, Rect};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Text,
    Checkbox,
    Radio,
    ListBox,
    ComboBox,
    Button,
    Signature,
}

#[derive(Debug, Clone)]
pub struct FormField {
    pub id: String,
    pub name: String,
    pub field_type: FieldType,
    pub page: u32,
    pub rect: Rect,
    pub value: String,
    pub default_value: String,
    pub is_read_only: bool,
    pub is_required: bool,
    pub is_hidden: bool,
    pub tooltip: String,
    pub font_name: String,
    pub font_size: f32,
    pub text_color: Color,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
    pub options: Vec<String>,
    pub selected_indices: Vec<usize>,
    pub max_length: Option<u32>,
    pub is_multiline: bool,
    pub is_password: bool,
    pub is_file_select: bool,
    pub is_spell_check_enabled: bool,
    pub is_scrollable: bool,
    pub is_comb: bool,
    pub rich_text_value: String,
    pub custom_data: HashMap<String, String>,
}

impl FormField {
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

    pub fn with_value(mut self, value: String) -> Self {
        self.value = value;
        self
    }

    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.options = options;
        self
    }

    pub fn with_tooltip(mut self, tooltip: String) -> Self {
        self.tooltip = tooltip;
        self
    }

    pub fn read_only(mut self) -> Self {
        self.is_read_only = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.is_required = true;
        self
    }

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

    pub fn get_display_value(&self) -> String {
        if self.is_password {
            "*".repeat(self.value.len())
        } else {
            self.value.clone()
        }
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

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

    pub fn deselect_option(&mut self, index: usize) {
        if !self.is_read_only {
            self.selected_indices.retain(|&i| i != index);
        }
    }

    pub fn clear_selection(&mut self) {
        if !self.is_read_only {
            self.selected_indices.clear();
            self.value.clear();
        }
    }

    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        self.rect.contains_point(crate::core::Point::new(x, y))
    }
}

#[derive(Debug, Clone)]
pub struct Form {
    pub id: String,
    pub name: String,
    pub fields: Vec<FormField>,
    pub need_appearances: bool,
    pub sig_flags: u32,
    pub co: Option<String>,
    pub default_resources: HashMap<String, String>,
}

impl Form {
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

    pub fn add_field(&mut self, field: FormField) {
        self.fields.push(field);
    }

    pub fn remove_field(&mut self, id: &str) -> Option<FormField> {
        if let Some(index) = self.fields.iter().position(|f| f.id == id) {
            Some(self.fields.remove(index))
        } else {
            None
        }
    }

    pub fn get_field(&self, id: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.id == id)
    }

    pub fn get_field_mut(&mut self, id: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.id == id)
    }

    pub fn get_field_by_name(&self, name: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn get_field_by_name_mut(&mut self, name: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.name == name)
    }

    pub fn get_fields_at_point(&self, page: u32, x: i32, y: i32) -> Vec<&FormField> {
        self.fields
            .iter()
            .filter(|f| f.page == page && f.contains_point(x, y))
            .collect()
    }

    pub fn get_all_values(&self) -> HashMap<String, String> {
        self.fields
            .iter()
            .map(|f| (f.name.clone(), f.value.clone()))
            .collect()
    }

    pub fn set_all_values(&mut self, values: HashMap<String, String>) {
        for (name, value) in values {
            if let Some(field) = self.get_field_by_name_mut(&name) {
                field.set_value(value);
            }
        }
    }

    pub fn reset_to_defaults(&mut self) {
        for field in &mut self.fields {
            field.value = field.default_value.clone();
            field.selected_indices.clear();
        }
    }

    pub fn clear(&mut self) {
        self.fields.clear();
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }

    pub fn required_field_count(&self) -> usize {
        self.fields.iter().filter(|f| f.is_required).count()
    }

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
    fn default() -> Self {
        Self::new("default".to_string(), "Default Form".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field_id: String,
    pub field_name: String,
    pub message: String,
}

pub struct FormManager {
    forms: HashMap<String, Form>,
    current_form: Option<String>,
}

impl FormManager {
    pub fn new() -> Self {
        Self {
            forms: HashMap::new(),
            current_form: None,
        }
    }

    pub fn add_form(&mut self, form: Form) {
        let id = form.id.clone();
        self.forms.insert(id, form);
    }

    pub fn remove_form(&mut self, id: &str) -> Option<Form> {
        self.forms.remove(id)
    }

    pub fn get_form(&self, id: &str) -> Option<&Form> {
        self.forms.get(id)
    }

    pub fn get_form_mut(&mut self, id: &str) -> Option<&mut Form> {
        self.forms.get_mut(id)
    }

    pub fn set_current_form(&mut self, id: Option<String>) {
        self.current_form = id;
    }

    pub fn get_current_form(&self) -> Option<&Form> {
        self.current_form.as_ref()
            .and_then(|id| self.forms.get(id))
    }

    pub fn get_current_form_mut(&mut self) -> Option<&mut Form> {
        self.current_form.as_ref()
            .and_then(|id| self.forms.get_mut(id))
    }

    pub fn get_all_field_values(&self) -> HashMap<String, HashMap<String, String>> {
        self.forms
            .iter()
            .map(|(id, form)| (id.clone(), form.get_all_values()))
            .collect()
    }

    pub fn clear(&mut self) {
        self.forms.clear();
        self.current_form = None;
    }

    pub fn form_count(&self) -> usize {
        self.forms.len()
    }

    pub fn total_field_count(&self) -> usize {
        self.forms.values().map(|f| f.field_count()).sum()
    }
}

impl Default for FormManager {
    fn default() -> Self {
        Self::new()
    }
}

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
