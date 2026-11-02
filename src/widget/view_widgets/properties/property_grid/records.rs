// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The property row's declared record shape, its record conversions, and the numeric bounds.
//!
//! Kept out of `property_grid.rs` so that file's production part stays under the structure limit.
//! Every item here is about *describing* a row rather than painting or editing one: the
//! [`PROPERTY_ITEM_SCHEMA`] a designer and the `properties` capability share, the two directions of
//! the record conversion, and [`NumberBounds`] — the limits a number row enforces and exposes to
//! its editor (RW-13).

use super::*;

/// The numeric bounds a number row enforces (RW-13).
///
/// `None` on any field means "unbounded in that respect", which is the historical behaviour for a
/// plain number row. The bounds are exposed to a custom editor through
/// [`PropertyItem::number_bounds`], so a slider or spin box can start from the same limits the
/// commit path enforces instead of inventing its own.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberBounds {
    /// The smallest accepted value.
    pub min: Option<f64>,
    /// The largest accepted value.
    pub max: Option<f64>,
    /// The increment a committed value must land on, measured from `min` (or zero).
    pub step: Option<f64>,
}

impl NumberBounds {
    /// Bounds with no limits at all.
    pub fn unbounded() -> Self {
        Self { min: None, max: None, step: None }
    }

    /// Bounds with a minimum and a maximum.
    pub fn range(min: f64, max: f64) -> Self {
        Self { min: Some(min), max: Some(max), step: None }
    }

    /// Sets the step increment.
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = Some(step);
        self
    }

    /// Whether `value` lies inside the bounds and on the step grid.
    ///
    /// Returns `Ok(())` or a human-readable reason, which the grid reports through
    /// `cell_change_rejected` rather than swallowing.
    pub fn validate(&self, value: f64) -> Result<(), String> {
        if let Some(min) = self.min {
            if value < min {
                return Err(format!("value {value} is below the minimum {min}"));
            }
        }
        if let Some(max) = self.max {
            if value > max {
                return Err(format!("value {value} is above the maximum {max}"));
            }
        }
        if let Some(step) = self.step.filter(|step| *step > 0.0 && step.is_finite()) {
            let origin = self.min.unwrap_or(0.0);
            let steps = (value - origin) / step;
            if (steps - steps.round()).abs() > 1.0e-9 {
                return Err(format!("value {value} is not a multiple of the step {step}"));
            }
        }
        Ok(())
    }
}

/// The published token for a row's editor kind.
pub(super) fn property_item_kind_token(kind: &PropertyItemKind) -> &'static str {
    match kind {
        PropertyItemKind::Text => "text",
        PropertyItemKind::Number => "number",
        PropertyItemKind::Bool => "bool",
        PropertyItemKind::Color => "color",
        PropertyItemKind::Choice(_) => "choice",
        PropertyItemKind::File => "file",
    }
}

/// The declared shape of one property row, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `properties` property, and the control owns both ends of it: the
/// field names are [`PropertyItem`]'s own fields. `name` and `value` are required because
/// [`PropertyItem::new`] requires them; the rest are optional because a row that omits them is an
/// editable text property with no category. `kind` is the editor token and `choices` carries a
/// [`PropertyItemKind::Choice`]'s option list (the only kind with a payload). `min`/`max`/`step`
/// carry a number row's bounds (RW-13), so a designer round-trip keeps them.
pub const PROPERTY_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "name",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "value",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "editable",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "kind",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "choices",
                crate::widget::capability::PropertyValueKind::StringList,
            ),
            crate::widget::capability::RecordField::optional(
                "category",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "mixed",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "placeholder",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "min",
                crate::widget::capability::PropertyValueKind::Float,
            ),
            crate::widget::capability::RecordField::optional(
                "max",
                crate::widget::capability::PropertyValueKind::Float,
            ),
            crate::widget::capability::RecordField::optional(
                "step",
                crate::widget::capability::PropertyValueKind::Float,
            ),
        ],
        // A row has no separate identity field; its name is what a document spells it by, and two
        // rows may legitimately share a name across categories.
        id_field: "name",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`PROPERTY_ITEM_SCHEMA`].
pub(super) fn property_item_record(item: &PropertyItem) -> crate::widget::capability::RecordValue {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("name", CapabilityValue::String(item.name.clone()))
        .with("value", CapabilityValue::String(item.value.clone()))
        .with("editable", CapabilityValue::Bool(item.editable))
        .with("kind", CapabilityValue::String(property_item_kind_token(&item.kind).to_string()));
    if let PropertyItemKind::Choice(choices) = &item.kind {
        record = record.with(
            "choices",
            CapabilityValue::Tuple(
                choices.iter().map(|choice| CapabilityValue::String(choice.clone())).collect(),
            ),
        );
    }
    if let Some(category) = &item.category {
        record = record.with("category", CapabilityValue::String(category.clone()));
    }
    if item.mixed {
        record = record.with("mixed", CapabilityValue::Bool(true));
    }
    if let Some(placeholder) = &item.placeholder {
        record = record.with("placeholder", CapabilityValue::String(placeholder.clone()));
    }
    if let Some(bounds) = item.number_bounds {
        if let Some(min) = bounds.min {
            record = record.with("min", CapabilityValue::Float(min));
        }
        if let Some(max) = bounds.max {
            record = record.with("max", CapabilityValue::Float(max));
        }
        if let Some(step) = bounds.step {
            record = record.with("step", CapabilityValue::Float(step));
        }
    }
    record
}

/// Reads an optional numeric field, accepting both float and integer spellings.
fn optional_float(
    record: &crate::widget::capability::RecordValue,
    name: &str,
) -> Result<Option<f64>, CapabilityAccessError> {
    match record.get(name) {
        None | Some(CapabilityValue::Null) => Ok(None),
        Some(CapabilityValue::Float(value)) => Ok(Some(*value)),
        Some(CapabilityValue::Int(value)) => Ok(Some(*value as f64)),
        Some(CapabilityValue::UInt(value)) => Ok(Some(*value as f64)),
        Some(_) => Err(CapabilityAccessError::TypeMismatch),
    }
}

/// Rebuilds one [`PropertyItem`] from a row of [`PROPERTY_ITEM_SCHEMA`].
pub(super) fn property_item_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<PropertyItem, CapabilityAccessError> {
    let name = match record.get("name") {
        Some(CapabilityValue::String(name)) => name.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let value = match record.get("value") {
        Some(CapabilityValue::String(value)) => value.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let editable = match record.get("editable") {
        None | Some(CapabilityValue::Null) => true,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let choices = match record.get("choices") {
        None | Some(CapabilityValue::Null) => Vec::new(),
        Some(CapabilityValue::Tuple(items)) => items
            .iter()
            .map(|item| match item {
                CapabilityValue::String(choice) => Ok(choice.clone()),
                _ => Err(CapabilityAccessError::TypeMismatch),
            })
            .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let kind = match record.get("kind") {
        None | Some(CapabilityValue::Null) => PropertyItemKind::Text,
        Some(CapabilityValue::String(token)) => match token.as_str() {
            "text" => PropertyItemKind::Text,
            "number" => PropertyItemKind::Number,
            "bool" => PropertyItemKind::Bool,
            "color" => PropertyItemKind::Color,
            "choice" => PropertyItemKind::Choice(choices),
            "file" => PropertyItemKind::File,
            _ => return Err(CapabilityAccessError::OutOfRange),
        },
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let category = match record.get("category") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(category)) => Some(category.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let mixed = match record.get("mixed") {
        None | Some(CapabilityValue::Null) => false,
        Some(CapabilityValue::Bool(mixed)) => *mixed,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let placeholder = match record.get("placeholder") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(placeholder)) => Some(placeholder.clone()),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let min = optional_float(record, "min")?;
    let max = optional_float(record, "max")?;
    let step = optional_float(record, "step")?;
    let number_bounds = (min.is_some() || max.is_some() || step.is_some())
        .then_some(NumberBounds { min, max, step });
    Ok(PropertyItem { name, value, editable, mixed, placeholder, kind, category, number_bounds })
}
