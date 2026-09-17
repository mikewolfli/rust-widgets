// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Generic JSON → widget property application.
//!
//! # Why this module exists
//!
//! The JSON loader used to apply properties through one hand-written `if let`
//! block per key, plus one hand-written construction arm per widget type. That
//! approach cannot scale to the 167 `WidgetKind` variants the library ships:
//! every property added to any control would need a matching branch here, and a
//! control whose JSON name nobody remembered would load with silent defaults.
//!
//! This module instead drives the **per-control property contract**
//! ([`WidgetProperties`](crate::widget::capability::properties_trait::WidgetProperties))
//! that every control already implements. A JSON key is looked up by name on the
//! widget instance itself:
//!
//! 1. The key is matched against the control's own published property names
//!    (`widget_property_names`), using the same normalisation the capability
//!    layer uses (case-insensitive, `-`/`_`/space-insensitive).
//! 2. The JSON value is converted to a [`CapabilityValue`] of the kind the
//!    control declares for that name, using the schema published through
//!    [`WidgetFactory::property_schema`](crate::widget::capability::WidgetFactory::property_schema).
//! 3. The write goes through [`widget_property_set`], so a type error or a
//!    read-only property is reported rather than silently ignored.
//!
//! # Relationship to the hand-written keys
//!
//! A handful of JSON keys describe *construction* rather than *state* — an
//! `items` array for a list control, a `children` array for a container. Those
//! have no property equivalent and stay in the loader's construction path.
//! Everything else is now name-driven and therefore works for every control
//! whose property contract publishes the name, including controls this module
//! has never heard of.

use serde_json::Value;

use crate::widget::capability::{
    widget_property_names, widget_property_set, CapabilityValue, PropertyValueKind,
};

/// The outcome of applying one JSON key to a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApplyOutcome {
    /// The control publishes this name and the value was written.
    Applied,
    /// The control does not publish this name. The caller decides whether that
    /// is a typo (warn) or an unrelated JSON key (ignore).
    NotAProperty,
    /// The control publishes the name but the value could not be converted, or
    /// the control refused the write (read-only, type mismatch). Reported so
    /// the loader can surface it instead of dropping the value.
    Rejected,
}

/// Normalise a property name the way the capability layer does.
///
/// Mirrors `crate::widget::capability::coercion::normalize_key`: lower-case and
/// drop `-`, `_` and spaces. Kept here as a small private copy rather than
/// re-exported, because the capability helper is crate-private and this module
/// only needs the comparison, not the lookup tables it guards.
fn normalize(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '-' | '_' | ' ')).flat_map(char::to_lowercase).collect()
}

/// Whether `name` is published by the widget's property contract.
///
/// Public because it answers the question a caller of the JSON path needs: given
/// a JSON key and a live widget, is this key meaningful? The answer is the same
/// one the loader acts on, so a caller validating a layout ahead of time gets the
/// loader's verdict rather than a reimplementation of it.
pub fn is_widget_property(widget: &dyn crate::widget::Widget, name: &str) -> bool {
    let Some(published) = widget_property_names(widget) else {
        return false;
    };
    let wanted = normalize(name);
    published.iter().any(|candidate| normalize(candidate) == wanted)
}

/// Resolve the published property name that `name` refers to.
///
/// Returns the contract's own spelling, so a caller can report the name the
/// control actually publishes rather than the JSON spelling that was tried.
pub(crate) fn resolve_property_name(
    widget: &dyn crate::widget::Widget,
    name: &str,
) -> Option<&'static str> {
    let published = widget_property_names(widget)?;
    let wanted = normalize(name);
    published.iter().copied().find(|candidate| normalize(candidate) == wanted)
}

/// Convert a JSON value to the `CapabilityValue` variant a property declares.
///
/// The conversion is driven by the declared [`PropertyValueKind`] when a schema
/// is available, so `"50"` written to an `Int` property is a reported failure
/// rather than a silent coercion, and `true` written to a `String` property does
/// not become `"true"`.
///
/// Without a schema the JSON type selects the variant directly; that fallback is
/// what lets the loader work before a control's schema is registered.
fn to_capability_value(
    value: &Value,
    declared: Option<PropertyValueKind>,
) -> Option<CapabilityValue> {
    match declared {
        Some(PropertyValueKind::Bool) => value.as_bool().map(CapabilityValue::Bool),
        Some(PropertyValueKind::Int) => value.as_i64().map(CapabilityValue::Int),
        Some(PropertyValueKind::UInt) => value.as_u64().map(CapabilityValue::UInt),
        Some(PropertyValueKind::Float) => value.as_f64().map(CapabilityValue::Float),
        Some(PropertyValueKind::String) | Some(PropertyValueKind::Enum) => {
            value.as_str().map(|s| CapabilityValue::String(s.to_string()))
        }
        // A colour is written as its CSS spelling ("#rrggbb", "rgb(..)", a named
        // colour). A string that does not parse is refused rather than defaulted, so a
        // typo in a JSON tree is an error the author sees instead of a wrong colour.
        Some(PropertyValueKind::Color) => value
            .as_str()
            .and_then(|s| crate::style::CssParser::parse_color(s).ok())
            .map(CapabilityValue::Color),
        // A rectangle is written as "x,y,w,h", or as a 4-element array, which is the
        // natural shape in JSON and avoids asking authors to build a string.
        Some(PropertyValueKind::Rect) => rect_from_json(value).map(CapabilityValue::Rect),
        None => match value {
            Value::Null => Some(CapabilityValue::Null),
            Value::Bool(b) => Some(CapabilityValue::Bool(*b)),
            // Prefer the unsigned variant for non-negative JSON integers: counts
            // and indices dominate the property set, and a schema-less write of
            // a count as `Int` would be rejected by an unsigned property.
            Value::Number(n) => {
                if let Some(u) = n.as_u64() {
                    Some(CapabilityValue::UInt(u))
                } else if let Some(i) = n.as_i64() {
                    Some(CapabilityValue::Int(i))
                } else {
                    n.as_f64().map(CapabilityValue::Float)
                }
            }
            Value::String(s) => Some(CapabilityValue::String(s.clone())),
            Value::Array(_) | Value::Object(_) => None,
        },
    }
}

/// Converts a JSON value into a rectangle for a `Rect`-declared property.
///
/// Accepts both spellings the format allows, because they serve different authors: a
/// `"x,y,w,h"` string matches what the C ABI carries and what CSS-ish tooling emits,
/// while a 4-element array is what a JSON author naturally writes. Accepting only one
/// would make the same value legal in one entry point and illegal in the other.
fn rect_from_json(value: &Value) -> Option<crate::core::Rect> {
    let components: [i64; 4] = match value {
        Value::String(text) => {
            let mut parts = text.split(',');
            let mut parsed = [0i64; 4];
            for slot in parsed.iter_mut() {
                *slot = parts.next()?.trim().parse().ok()?;
            }
            if parts.next().is_some() {
                return None;
            }
            parsed
        }
        Value::Array(items) => {
            if items.len() != 4 {
                return None;
            }
            let mut parsed = [0i64; 4];
            for (slot, item) in parsed.iter_mut().zip(items) {
                *slot = item.as_i64()?;
            }
            parsed
        }
        _ => return None,
    };
    // Negative extents are refused rather than cast to a huge unsigned size, which is
    // what `as u32` alone would produce.
    let width = u32::try_from(components[2]).ok()?;
    let height = u32::try_from(components[3]).ok()?;
    Some(crate::core::Rect::new(components[0] as i32, components[1] as i32, width, height))
}

/// Look up the declared value kind for a property from the factory schema.
///
/// Returns `None` when the schema does not describe the property, which is the
/// honest answer for a control shipped in a profile without the capability
/// registry: the write still proceeds by JSON type rather than being blocked on
/// metadata that is not compiled in.
fn declared_kind(widget: &dyn crate::widget::Widget, name: &str) -> Option<PropertyValueKind> {
    let factory = crate::json::schema_factory();
    let capability = factory.capability_for_kind_instance(widget)?;
    let wanted = normalize(name);
    capability
        .properties
        .iter()
        .find(|schema| normalize(schema.name) == wanted)
        .map(|schema| schema.value_kind)
}

/// Apply one JSON `key: value` pair to a widget through its property contract.
///
/// The key is matched against the names the control publishes, so this works for
/// every control the contract covers rather than a fixed table. Returns
/// [`ApplyOutcome::NotAProperty`] when the name is not a property — the caller
/// may then try a construction-time key.
pub(crate) fn apply_widget_property(
    widget: &mut dyn crate::widget::Widget,
    key: &str,
    value: &Value,
) -> ApplyOutcome {
    let Some(published) = resolve_property_name(widget, key) else {
        return ApplyOutcome::NotAProperty;
    };

    let declared = declared_kind(widget, published);
    let Some(capability_value) = to_capability_value(value, declared) else {
        return ApplyOutcome::Rejected;
    };

    match widget_property_set(widget, published, capability_value) {
        Ok(()) => ApplyOutcome::Applied,
        // A control that publishes the name but refuses this particular value is
        // a real rejection, not "not a property": reporting `NotAProperty` here
        // would make the loader fall through and silently ignore the value.
        Err(_) => ApplyOutcome::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::widget::Button;

    #[test]
    fn normalize_folds_case_and_separators() {
        assert_eq!(normalize("Font-Size"), "fontsize");
        assert_eq!(normalize("font_size"), "fontsize");
        assert_eq!(normalize("font size"), "fontsize");
        assert_eq!(normalize("fontsize"), "fontsize");
    }

    #[test]
    fn to_capability_value_honours_declared_kind() {
        let json = serde_json::json!(50);
        assert_eq!(
            to_capability_value(&json, Some(PropertyValueKind::Int)),
            Some(CapabilityValue::Int(50))
        );
        assert_eq!(
            to_capability_value(&json, Some(PropertyValueKind::UInt)),
            Some(CapabilityValue::UInt(50))
        );
        // A number is not a string: the declared kind is enforced, not guessed.
        assert_eq!(to_capability_value(&json, Some(PropertyValueKind::String)), None);
        // A string is not an integer either.
        let quoted = serde_json::json!("50");
        assert_eq!(to_capability_value(&quoted, Some(PropertyValueKind::Int)), None);
    }

    #[test]
    fn to_capability_value_without_schema_prefers_unsigned_for_counts() {
        assert_eq!(
            to_capability_value(&serde_json::json!(7), None),
            Some(CapabilityValue::UInt(7))
        );
        assert_eq!(
            to_capability_value(&serde_json::json!(-7), None),
            Some(CapabilityValue::Int(-7))
        );
        assert_eq!(
            to_capability_value(&serde_json::json!(7.5), None),
            Some(CapabilityValue::Float(7.5))
        );
        // Containers have no scalar property representation.
        assert_eq!(to_capability_value(&serde_json::json!([1, 2]), None), None);
        assert_eq!(to_capability_value(&serde_json::json!({"a": 1}), None), None);
    }

    #[test]
    fn button_publishes_text_and_accepts_a_write_by_name() {
        let mut button = Button::new("before".to_string(), Rect::new(0, 0, 10, 10));
        assert!(is_widget_property(&button, "text"));
        assert!(is_widget_property(&button, "Text"), "lookup is case-insensitive");
        assert!(!is_widget_property(&button, "no_such_property"));

        let outcome = apply_widget_property(&mut button, "text", &serde_json::json!("after"));
        assert_eq!(outcome, ApplyOutcome::Applied);
        assert_eq!(button.text(), "after");
    }

    #[test]
    fn unknown_name_is_reported_as_not_a_property() {
        let mut button = Button::new("x".to_string(), Rect::new(0, 0, 10, 10));
        assert_eq!(
            apply_widget_property(&mut button, "colour", &serde_json::json!("#ff0000")),
            ApplyOutcome::NotAProperty
        );
    }

    #[test]
    fn wrong_json_type_is_rejected_not_ignored() {
        let mut button = Button::new("x".to_string(), Rect::new(0, 0, 10, 10));
        // `enabled` is a bool property; a string must not be silently accepted.
        assert_eq!(
            apply_widget_property(&mut button, "enabled", &serde_json::json!("yes")),
            ApplyOutcome::Rejected
        );
        assert_eq!(
            apply_widget_property(&mut button, "enabled", &serde_json::json!(false)),
            ApplyOutcome::Applied
        );
    }

    #[test]
    fn resolve_property_name_returns_the_published_spelling() {
        let button = Button::new("x".to_string(), Rect::new(0, 0, 10, 10));
        // The JSON spelling is "Text"; the contract publishes "text".
        assert_eq!(resolve_property_name(&button, "Text"), Some("text"));
        assert_eq!(resolve_property_name(&button, "not_there"), None);
    }
}
