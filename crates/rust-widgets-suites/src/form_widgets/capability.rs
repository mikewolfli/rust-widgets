// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability rows, property schemas and constructors of the form suite.
//!
//! These are the same public names a host uses through [`install`]: a canonical name, its aliases,
//! the property contract with a type and an accessibility flag per row, the published events with
//! their payload kinds, and the payload-free command names. A capability is the designer's entire
//! view of a control, so an event name without its payload type (or a property without a schema)
//! would be a half-published control — the two halves are declared together here.

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::types::{
    CapabilityValue, EventPayloadShape as S, EventSchema, PropertySchema, PropertyValueKind as K,
    RegistrationError, WidgetCapability, WidgetCtor,
};
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::{Widget, WidgetKind};

use super::form::Form;
use super::formset::FormSet;

/// `Form`'s published properties.
pub const FORM_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("title", K::String, true, true),
    PropertySchema::new("show_title", K::Bool, true, true),
    PropertySchema::enumerated("label_position", true, true, &["left", "top"]),
    PropertySchema::new("label_width", K::UInt, true, true),
    PropertySchema::new("row_height", K::UInt, true, true),
    PropertySchema::new("row_spacing", K::UInt, true, true),
    PropertySchema::new("column_spacing", K::UInt, true, true),
    PropertySchema::new("content_padding", K::UInt, true, true),
    PropertySchema::new("columns", K::UInt, true, true),
    PropertySchema::new("required_mark", K::String, true, true),
    PropertySchema::new("required_error", K::String, true, true),
    PropertySchema::new("show_labels", K::Bool, true, true),
    PropertySchema::new("show_errors", K::Bool, true, true),
    PropertySchema::new("show_buttons", K::Bool, true, true),
    PropertySchema::new("submit_label", K::String, true, true),
    PropertySchema::new("reset_label", K::String, true, true),
    PropertySchema::new("button_width", K::UInt, true, true),
    // Derived from the field list and the controls' values: readable, not writable, because a
    // write would be a second store for a fact the form already derives.
    PropertySchema::new("field_count", K::UInt, true, false),
    PropertySchema::new("valid", K::Bool, true, false),
    PropertySchema::new("error_count", K::UInt, true, false),
    PropertySchema::new("dirty", K::Bool, true, false),
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
];

/// `FormSet`'s published properties.
pub const FORM_SET_PROPERTIES: &[PropertySchema] = &[
    PropertySchema::new("current_index", K::UInt, true, true),
    PropertySchema::new("page_count", K::UInt, true, false),
    PropertySchema::new("valid", K::Bool, true, false),
    PropertySchema::new("dirty", K::Bool, true, false),
    PropertySchema::new("completed_count", K::UInt, true, false),
    PropertySchema::new("show_header", K::Bool, true, true),
    PropertySchema::new("header_height", K::UInt, true, true),
    PropertySchema::enumerated("header_style", true, true, &["tabs", "steps"]),
    PropertySchema::new("show_navigation", K::Bool, true, true),
    PropertySchema::new("navigation_height", K::UInt, true, true),
    PropertySchema::new("content_padding", K::UInt, true, true),
    PropertySchema::new("submit_label", K::String, true, true),
    PropertySchema::new("previous_label", K::String, true, true),
    PropertySchema::new("next_label", K::String, true, true),
    PropertySchema::new("current_key", K::String, true, false),
    PropertySchema::new("enabled", K::Bool, true, true),
    PropertySchema::new("visible", K::Bool, true, true),
    PropertySchema::new("tooltip", K::String, true, true),
    PropertySchema::new("geometry", K::String, false, false),
];

/// `Form`'s published events.
const FORM_EVENTS: &[EventSchema] = &[
    EventSchema::unit("submitted"),
    EventSchema::unit("reset"),
    EventSchema::scalar("submit_failed", K::UInt),
    EventSchema::scalar("validity_changed", K::Bool),
    EventSchema::scalar("dirty_changed", K::Bool),
    EventSchema::scalar("field_added", K::String),
    EventSchema::scalar("field_removed", K::String),
    EventSchema::scalar("field_changed", K::String),
];

/// `FormSet`'s published events.
const FORM_SET_EVENTS: &[EventSchema] = &[
    EventSchema::scalar("current_changed", K::UInt),
    EventSchema::scalar("page_added", K::String),
    EventSchema::scalar("page_removed", K::String),
    // `(key, completed)`: two different kinds in one payload, so the declared kind is how the whole
    // value travels and the shape is `Mixed` (there is no single-element kind to promise).
    EventSchema { name: "completed_changed", payload: Some(K::String), shape: Some(S::Mixed) },
    EventSchema::scalar("validity_changed", K::Bool),
    EventSchema::scalar("dirty_changed", K::Bool),
    EventSchema::unit("submitted"),
    EventSchema::scalar("submit_failed", K::UInt),
    EventSchema::unit("reset"),
];

/// Creates an empty form, using the constructor's `text` as its heading.
pub fn create_form(geometry: Rect, text: &str) -> Box<dyn Widget> {
    let mut form = Form::new(geometry);
    if !text.is_empty() {
        form.set_title(text);
    }
    Box::new(form)
}

/// Creates an empty form set.
///
/// The factory constructor's `text` has no meaning for a set: its headings are the page titles, and
/// there are no pages yet. The argument is accepted to satisfy the uniform constructor shape.
pub fn create_form_set(geometry: Rect, _text: &str) -> Box<dyn Widget> {
    Box::new(FormSet::new(geometry))
}

/// The `form` capability.
pub fn form_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::Form,
        canonical_name: "form",
        aliases: &["form_panel", "form_layout"],
        properties: FORM_PROPERTIES,
        events: FORM_EVENTS,
        commands: &["submit", "reset", "validate", "clear_errors"],
    }
}

/// The `form_set` capability.
pub fn form_set_capability() -> WidgetCapability {
    WidgetCapability {
        kind: WidgetKind::FormSet,
        canonical_name: "form_set",
        aliases: &["formset", "wizard_form", "form_pages"],
        properties: FORM_SET_PROPERTIES,
        events: FORM_SET_EVENTS,
        commands: &["next", "previous", "submit", "reset_all", "validate_all", "mark_completed"],
    }
}

/// A registered control: its capability row, its constructor, and its defaults provider.
type FormRow = (WidgetCapability, WidgetCtor, fn(&str) -> Option<CapabilityValue>);

/// Registers the form suite in `factory`.
///
/// Returns the first name that could not be claimed. A factory that already answers one of these
/// names (a host that installed twice, or a build that ships a control of the same name) is
/// reported rather than silently shadowed.
///
/// Each row also registers the kind's declared defaults with the factory, so the standard
/// `default_property_value` a host or designer calls answers for these kinds too (FM-04);
/// [`default_property_value`] remains the one table those answers come from.
pub fn install(factory: &mut WidgetFactory) -> Result<(), RegistrationError> {
    let rows: [FormRow; 2] = [
        (form_capability(), create_form, form_defaults),
        (form_set_capability(), create_form_set, form_set_defaults),
    ];
    for (capability, ctor, defaults) in rows {
        let name = capability.canonical_name;
        factory.try_register(capability, ctor)?;
        let _ = factory.register_property_defaults(name, defaults);
    }
    Ok(())
}

/// The `form` kind's declared defaults, in the shape the factory calls.
fn form_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("form", property)
}

/// The `form_set` kind's declared defaults, in the shape the factory calls.
fn form_set_defaults(property: &str) -> Option<CapabilityValue> {
    default_property_value("form_set", property)
}

/// Every property name the form suite publishes, for a quick completeness check.
pub fn published_property_names() -> (Vec<&'static str>, Vec<&'static str>) {
    (
        FORM_PROPERTIES.iter().map(|schema| schema.name).collect(),
        FORM_SET_PROPERTIES.iter().map(|schema| schema.name).collect(),
    )
}

/// Every event name the form suite publishes, for a quick completeness check.
pub fn published_event_names() -> (Vec<&'static str>, Vec<&'static str>) {
    (
        FORM_EVENTS.iter().map(|schema| schema.name).collect(),
        FORM_SET_EVENTS.iter().map(|schema| schema.name).collect(),
    )
}

/// A property's declared default, for the capability manifest.
///
/// The form suite does not implement the parent's per-kind default table (that table lives with the
/// parent's own controls); this helper is what a host or a designer reads instead, so the declared
/// default and the control's constructor stay one row apart.
pub fn default_property_value(kind: &str, name: &str) -> Option<CapabilityValue> {
    match (kind, name) {
        ("form", "title") => Some(CapabilityValue::String(String::new())),
        ("form", "show_title") => Some(CapabilityValue::Bool(true)),
        ("form", "label_position") => Some(CapabilityValue::String("left".to_string())),
        ("form", "label_width") => Some(CapabilityValue::UInt(120)),
        ("form", "row_height") => Some(CapabilityValue::UInt(32)),
        ("form", "row_spacing") => Some(CapabilityValue::UInt(8)),
        ("form", "column_spacing") => Some(CapabilityValue::UInt(16)),
        ("form", "content_padding") => Some(CapabilityValue::UInt(12)),
        ("form", "columns") => Some(CapabilityValue::UInt(1)),
        ("form", "required_mark") => Some(CapabilityValue::String("*".to_string())),
        ("form", "required_error") => Some(CapabilityValue::String("Required".to_string())),
        ("form", "show_labels" | "show_errors" | "show_buttons") => {
            Some(CapabilityValue::Bool(true))
        }
        ("form", "submit_label") => Some(CapabilityValue::String("Submit".to_string())),
        ("form", "reset_label") => Some(CapabilityValue::String("Reset".to_string())),
        ("form", "button_width") => Some(CapabilityValue::UInt(72)),
        ("form", "field_count" | "error_count") => Some(CapabilityValue::UInt(0)),
        ("form", "valid") => Some(CapabilityValue::Bool(true)),
        ("form", "dirty") => Some(CapabilityValue::Bool(false)),
        ("form_set", "current_index" | "page_count" | "completed_count") => {
            Some(CapabilityValue::UInt(0))
        }
        ("form_set", "valid") => Some(CapabilityValue::Bool(true)),
        ("form_set", "dirty") => Some(CapabilityValue::Bool(false)),
        ("form_set", "show_header" | "show_navigation") => Some(CapabilityValue::Bool(true)),
        ("form_set", "header_height") => Some(CapabilityValue::UInt(32)),
        ("form_set", "header_style") => Some(CapabilityValue::String("tabs".to_string())),
        ("form_set", "navigation_height") => Some(CapabilityValue::UInt(36)),
        ("form_set", "content_padding") => Some(CapabilityValue::UInt(12)),
        ("form_set", "submit_label") => Some(CapabilityValue::String("Submit".to_string())),
        ("form_set", "previous_label") => Some(CapabilityValue::String("Back".to_string())),
        ("form_set", "next_label") => Some(CapabilityValue::String("Next".to_string())),
        ("form_set", "current_key") => Some(CapabilityValue::Null),
        ("form" | "form_set", "enabled" | "visible") => Some(CapabilityValue::Bool(true)),
        ("form" | "form_set", "tooltip") => Some(CapabilityValue::String(String::new())),
        _ => None,
    }
}
