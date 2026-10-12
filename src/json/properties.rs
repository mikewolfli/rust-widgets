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
/// # Why this is a re-export and not a copy
///
/// It used to be a private copy, justified by "the capability helper is crate-private".
/// [`normalize_key`] is in fact `pub` — the crate's own `capability` module re-exports it —
/// so that premise was false, and the copy was a second implementation of one rule. The two
/// happened to agree when this was written, but nothing kept them agreeing: the loader and
/// the capability layer decide "is this key a property?" independently, and the day one
/// spelling changed the answer would differ between the two front ends (principle #101).
///
/// Re-exported under the local name so every call site in this module reads the same, and so
/// there is exactly one implementation to change.
use crate::widget::capability::coercion::normalize_key as normalize;

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
/// Exposed crate-wide because it is the crate's single JSON→[`CapabilityValue`] reading: the
/// loader's write and the generator's schema pre-check both need it, and a second copy would be a
/// second answer to "what value does this document mean" (BLUE26 B26.02).
///
/// Without a schema the JSON type selects the variant directly; that fallback is
/// what lets the loader work before a control's schema is registered.
///
/// Gated on its callers — the generator's scalar pre-check, which only exists with the designer,
/// and this module's own tests. It is not dead code, but a build with neither compiled it and said
/// so, so the condition is stated where the function is defined rather than where it is used.
#[cfg(any(test, feature = "designer"))]
pub(crate) fn to_capability_value(
    value: &Value,
    declared: Option<PropertyValueKind>,
) -> Option<CapabilityValue> {
    to_capability_value_with_record_schema(value, declared, None)
}

/// [`to_capability_value`], with the row schema a `Records` property declared.
///
/// A record row may carry a **structured** field — today only an icon descriptor — and reading one
/// needs the row's declared shape, so the caller that looked the property's schema up passes it
/// here. Callers that have no schema (and every caller before the schema was known) get the plain
/// conversion, which refuses compound row values rather than guessing at them.
pub(crate) fn to_capability_value_with_record_schema(
    value: &Value,
    declared: Option<PropertyValueKind>,
    record_schema: Option<&crate::widget::capability::RecordSchema>,
) -> Option<CapabilityValue> {
    match declared {
        Some(PropertyValueKind::Bool) => value.as_bool().map(CapabilityValue::Bool),
        Some(PropertyValueKind::Int) => value.as_i64().map(CapabilityValue::Int),
        Some(PropertyValueKind::UInt) => value.as_u64().map(CapabilityValue::UInt),
        Some(PropertyValueKind::Float) => value.as_f64().map(CapabilityValue::Float),
        // A `Number` property decides its own carrier: an integer-mode spin box must
        // keep answering `Int` for `{"value": 3}`, so the JSON number's own shape picks
        // the variant. Accepting a decimal here is deliberate — `{"value": 3.0}` is a
        // legitimate way to spell three, and rejecting it would make the JSON layer
        // harder to use than the Rust API it wraps.
        Some(PropertyValueKind::Number) => {
            let n = value.as_f64()?;
            // `n <= i64::MAX as f64` admitted exactly `2^63`, whose `as i64` saturates to
            // `i64::MAX` — losing the one that distinguishes them and making the round trip
            // `Int -> JSON -> Int` asymmetric. `2^63` is not an `i64`, so the strict `<` is the
            // correct predicate for "this fits in the integer carrier".
            if n.fract() == 0.0 && n >= i64::MIN as f64 && n < i64::MAX as f64 {
                Some(CapabilityValue::Int(n as i64))
            } else {
                Some(CapabilityValue::Float(n))
            }
        }
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
        // A record list is written as a JSON array of objects — the shape a document naturally uses
        // for rows and the shape the manifest exports (BLUE26 B26.09). Field names and values are read
        // **structurally**, not as a stringified blob: a typo in a field name then stays visible to
        // the property's own schema check instead of becoming a value the setter accepts.
        Some(PropertyValueKind::Records) => {
            records_from_json(value, record_schema).map(CapabilityValue::Records)
        }
        Some(PropertyValueKind::IconSpec) => icon_spec_from_json(value)
            .map(CapabilityValue::IconSpec)
            .or_else(|| value.is_null().then_some(CapabilityValue::Null)),
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

fn icon_spec_from_json(value: &Value) -> Option<crate::widget::icon_source::IconSpec> {
    use crate::widget::icon_source::{IconColorMode, IconSpec};

    let object = value.as_object()?;
    let kind = object.get("kind")?.as_str()?;
    match kind {
        "builtin" if object.len() == 2 => {
            let name = object.get("name")?.as_str()?;
            crate::widget::display_widgets::icon::IconName::from_name(name).map(IconSpec::Builtin)
        }
        "resource" if object.len() == 3 => {
            let id = object.get("id")?.as_str()?;
            if id.trim().is_empty() {
                return None;
            }
            let color_mode = match object.get("color_mode")?.as_str()? {
                "monochrome" => IconColorMode::Monochrome,
                "original" => IconColorMode::Original,
                _ => return None,
            };
            Some(IconSpec::Resource { id: id.to_string(), color_mode })
        }
        _ => None,
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
    // The origin gets the same treatment as the extents: `as i32` silently wraps an
    // out-of-range coordinate (`4294967296` -> `0`, `2147483648` -> `-2147483648`), so a
    // document with a bad position loaded as a *wrong on-screen position* with no error —
    // contradicting the "refused rather than defaulted" contract the extents already follow.
    let x = i32::try_from(components[0]).ok()?;
    let y = i32::try_from(components[1]).ok()?;
    Some(crate::core::Rect::new(x, y, width, height))
}

/// Converts a JSON value into a record list for a `Records`-declared property (BLUE26 B26.09).
///
/// # What it accepts, and what it refuses
///
/// An array of objects, each object becoming one record whose fields are the object's own keys in the
/// document's order. That is the only accepted shape:
///
/// * a non-array is refused, rather than read as a one-record list — a document that forgot the array
///   is a document whose intent the library should not guess;
/// * a non-object element is refused, because a record's fields have to be named;
/// * a nested array or object as a *field* value is refused, because a record field is one value of a
///   declared kind and a nested structure is not one of them (a nested record list would also mean
///   the property's schema check could not see inside it);
/// * a JSON `null` field is kept as [`CapabilityValue::Null`], which is a value the schema check can
///   judge — unlike a *missing* field, which the same check reports when the field is required.
///
/// # Why the shape is not checked here
///
/// Which fields are required, which kind each one holds, and whether ids must be unique are the
/// declaring control's business, and its schema is the single place those answers live. This function
/// only turns the document's structure into the value's structure; `validate_property_value` judges it.
///
/// The returned list carries [`EMPTY_RECORD_SCHEMA`](crate::widget::capability::EMPTY_RECORD_SCHEMA):
/// at this point no `&'static` schema is known, and inventing one would be policy the property never
/// declared. The caller that *does* know the schema adopts the list — see
/// [`RecordList::with_schema`](crate::widget::capability::RecordList::with_schema).
fn records_from_json(
    value: &Value,
    schema: Option<&crate::widget::capability::RecordSchema>,
) -> Option<crate::widget::capability::RecordList> {
    use crate::widget::capability::{CapabilityValue, RecordList, RecordValue};
    let Value::Array(items) = value else {
        return None;
    };
    let mut records = crate::compat::Vec::new();
    for item in items {
        let Value::Object(fields) = item else {
            return None;
        };
        let mut record = RecordValue::new();
        for (name, field) in fields {
            let field_value = match field {
                Value::Null => CapabilityValue::Null,
                Value::Bool(flag) => CapabilityValue::Bool(*flag),
                Value::Number(number) => {
                    match (number.as_u64(), number.as_i64(), number.as_f64()) {
                        (Some(unsigned), _, _) => CapabilityValue::UInt(unsigned),
                        (None, Some(signed), _) => CapabilityValue::Int(signed),
                        (None, None, Some(float)) => CapabilityValue::Float(float),
                        _ => return None,
                    }
                }
                Value::String(text) => CapabilityValue::String(text.clone()),
                // A nested object is read **only** where the row's declared schema says the field is
                // a structured icon descriptor — the one compound kind a row declares today. The
                // kind decides, not the JSON shape, so a typo cannot smuggle an object into a field
                // that promises text (and a field the schema does not declare is still refused here
                // and named by the row validator one layer up).
                Value::Array(_) | Value::Object(_) if declares_icon_spec_field(schema, name) => {
                    CapabilityValue::IconSpec(icon_spec_from_json(field)?)
                }
                // Any other compound value inside a record would be a second grammar to keep in
                // step; it is refused rather than guessed at.
                Value::Array(_) | Value::Object(_) => return None,
            };
            record = record.with(name.clone(), field_value);
        }
        records.push(record);
    }
    Some(RecordList::from_items(&crate::widget::capability::EMPTY_RECORD_SCHEMA, records))
}

/// Whether the row schema declares `name` as a structured icon descriptor.
fn declares_icon_spec_field(
    schema: Option<&crate::widget::capability::RecordSchema>,
    name: &str,
) -> bool {
    let Some(schema) = schema else {
        return false;
    };
    schema.fields.iter().any(|field| {
        field.name == name && field.kind == crate::widget::capability::PropertyValueKind::IconSpec
    })
}

/// Look up the full declared schema for a property from the factory registry.
///
/// Returns `None` when the schema does not describe the property, which is the honest answer for a
/// control shipped in a profile without the capability registry: the write then proceeds by JSON
/// type rather than being blocked on metadata that is not compiled in.
fn declared_schema(
    widget: &dyn crate::widget::Widget,
    name: &str,
) -> Option<crate::widget::capability::PropertySchema> {
    let factory = crate::json::schema_factory();
    let capability = factory.capability_for_kind_instance(widget)?;
    let wanted = normalize(name);
    capability.properties.iter().find(|schema| normalize(schema.name) == wanted).copied()
}

/// Whether `widget` declares `name` as a record-list property (BLUE26 B26.09).
///
/// # Why the loader asks this before honouring "loader-owned"
///
/// A loader-owned key is skipped by the name-driven pass because a construction arm or the layout
/// machinery consumes it. `items` is one of those keys — `combobox` and `listbox` read it as an array
/// of *strings* — so a control whose item list is a record list had no route at all: its value was
/// skipped by the pass and ignored by the construction arm (`as_str()` on an object is `None`), which
/// is the silent drop this release exists to remove.
///
/// The exception is deliberately as narrow as it can be: it fires only for a name the control's own
/// capability declares as [`PropertyValueKind::Records`]. Widening it to "any published property"
/// would make the name-driven pass apply keys the arms already consumed (`text`, `tooltip`, `id`) a
/// second time. `children` — an array of objects, like a record list — is not published by any
/// control as this kind, so it keeps its old route.
pub(crate) fn declares_record_list(widget: &dyn crate::widget::Widget, name: &str) -> bool {
    let Some(published) = resolve_property_name(widget, name) else {
        return false;
    };
    declared_schema(widget, published)
        .is_some_and(|schema| schema.value_kind == PropertyValueKind::Records)
}

/// Apply one JSON `key: value` pair to a widget through its property contract.
///
/// The key is matched against the names the control publishes, so this works for
/// every control the contract covers rather than a fixed table. Returns
/// [`ApplyOutcome::NotAProperty`] when the name is not a property — the caller
/// may then try a construction-time key.
///
/// A value the declared schema's constraints refuse is `Rejected`, through the *same* validator
/// the capability write path uses (`widget::capability::validate_property_value`): a document may
/// not load a value a generated program's `.prop(..)` could not write, or the two front ends would
/// describe different controls from one input (BLUE26 B26.02).
#[cfg(test)]
pub(crate) fn apply_widget_property(
    widget: &mut dyn crate::widget::Widget,
    key: &str,
    value: &Value,
) -> ApplyOutcome {
    apply_widget_property_with_icon_resolver(widget, key, value, None)
}

/// Apply a property like [`apply_widget_property`], resolving resource icon specs through
/// the caller's explicit resource context when one is supplied.
pub(crate) fn apply_widget_property_with_icon_resolver(
    widget: &mut dyn crate::widget::Widget,
    key: &str,
    value: &Value,
    resolver: Option<&dyn crate::widget::icon_source::IconResourceResolver>,
) -> ApplyOutcome {
    let Some(published) = resolve_property_name(widget, key) else {
        return ApplyOutcome::NotAProperty;
    };

    let declared = declared_schema(widget, published);
    let Some(capability_value) = to_capability_value_with_record_schema(
        value,
        declared.as_ref().map(|schema| schema.value_kind),
        declared.as_ref().and_then(|schema| schema.record_schema()),
    ) else {
        log::warn!(
            "JSON layout: property {published:?} rejected value {value}: it could not be \
             converted to the declared capability value kind"
        );
        return ApplyOutcome::Rejected;
    };

    if let Some(schema) = declared {
        if let Err(violation) = crate::widget::capability::validate_property_value(
            published,
            &schema,
            &capability_value,
        ) {
            // Named, not silent: the reason carries the property and the bound it broke, so a
            // document author can see *why* the value was dropped rather than wondering.
            log::warn!(
                "JSON layout: property {published:?} rejected the value {value}: {violation}"
            );
            return ApplyOutcome::Rejected;
        }
    }

    let result = match (&capability_value, resolver) {
        (
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                id,
                color_mode,
            }),
            Some(resolver),
        ) => resolver.resolve_icon(id, *color_mode).and_then(|source| {
            crate::widget::capability::properties_trait::widget_set_resolved_icon_source(
                widget,
                Some(source),
            )
            .map_err(|error| format!("{error:?}"))
        }),
        _ => widget_property_set(widget, published, capability_value)
            .map_err(|error| format!("{error:?}")),
    };
    match result {
        Ok(()) => ApplyOutcome::Applied,
        // A control that publishes the name but refuses this particular value is
        // a real rejection, not "not a property": reporting `NotAProperty` here
        // would make the loader fall through and silently ignore the value.
        Err(error) => {
            log::warn!("JSON layout: property {published:?} rejected the value {value}: {error}");
            ApplyOutcome::Rejected
        }
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

    /// B26.09: a JSON array of objects becomes a typed record list, not a stringified blob.
    ///
    /// The field names and the values are read structurally, so the value a caller receives can be
    /// inspected field by field — which is the difference between a typed collection and a string that
    /// happens to hold JSON.
    #[test]
    fn to_capability_value_reads_a_record_list_structurally() {
        let json = serde_json::json!([
            {"id": "a", "label": "Alpha", "selected": true},
            {"id": "b", "label": "Beta"}
        ]);
        let Some(CapabilityValue::Records(list)) =
            to_capability_value(&json, Some(PropertyValueKind::Records))
        else {
            panic!("an array of objects must read as a record list");
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list.get(0).expect("first").text("id"), "a");
        assert_eq!(list.get(0).expect("first").text("label"), "Alpha");
        assert!(matches!(
            list.get(0).expect("first").get("selected"),
            Some(CapabilityValue::Bool(true))
        ));
        // An absent optional field is absent, not invented.
        assert!(list.get(1).expect("second").get("selected").is_none());
        // The list carries no schema: it is the *property* that supplies one.
        assert!(list.schema().fields.is_empty());
    }

    /// Every shape that is not an array of objects is refused rather than guessed at.
    #[test]
    fn to_capability_value_refuses_a_shape_that_is_not_a_record_list() {
        let kind = Some(PropertyValueKind::Records);
        // Not an array at all.
        assert_eq!(to_capability_value(&serde_json::json!({"id": "a"}), kind), None);
        assert_eq!(to_capability_value(&serde_json::json!("[]"), kind), None);
        assert_eq!(to_capability_value(&serde_json::json!(3), kind), None);
        // An array whose elements are not objects.
        assert_eq!(to_capability_value(&serde_json::json!(["a", "b"]), kind), None);
        // A nested structure as a field value: a record field is one value of a declared kind.
        assert_eq!(to_capability_value(&serde_json::json!([{"id": {"nested": 1}}]), kind), None);
        assert_eq!(to_capability_value(&serde_json::json!([{"id": [1, 2]}]), kind), None);
        // An empty array **is** an empty list, which is a legitimate value.
        let Some(CapabilityValue::Records(empty)) =
            to_capability_value(&serde_json::json!([]), kind)
        else {
            panic!("an empty array is an empty record list");
        };
        assert!(empty.is_empty());
        // And with no declared kind, a list stays refused: without a schema nothing can judge it.
        assert_eq!(to_capability_value(&serde_json::json!([{"id": "a"}]), None), None);
    }

    #[test]
    fn icon_spec_json_is_structured_and_strict() {
        let kind = Some(PropertyValueKind::IconSpec);
        let built_in = serde_json::json!({"kind": "builtin", "name": "info"});
        assert_eq!(
            to_capability_value(&built_in, kind),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::display_widgets::icon::IconName::Info
            )))
        );
        let resource = serde_json::json!({
            "kind": "resource",
            "id": "project/icons/open",
            "color_mode": "monochrome"
        });
        assert_eq!(
            to_capability_value(&resource, kind),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                id: "project/icons/open".into(),
                color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
            }))
        );

        for invalid in [
            serde_json::json!({"kind": "builtin", "name": "not-a-built-in"}),
            serde_json::json!({"kind": "builtin", "name": "info", "id": "conflict"}),
            serde_json::json!({"kind": "resource", "id": " ", "color_mode": "original"}),
            serde_json::json!({"kind": "resource", "id": "icons/open", "color_mode": "tint"}),
            serde_json::json!({"kind": "resource", "id": "icons/open", "color_mode": "original", "name": "conflict"}),
        ] {
            assert_eq!(to_capability_value(&invalid, kind), None, "accepted {invalid}");
        }
        assert_eq!(to_capability_value(&Value::Null, kind), Some(CapabilityValue::Null));
    }

    #[test]
    fn icon_spec_property_apply_preserves_builtin_and_rejects_unresolved_resources() {
        let mut button = Button::new("Save".into(), Rect::new(0, 0, 100, 30));
        assert_eq!(
            apply_widget_property(
                &mut button,
                "icon_source",
                &serde_json::json!({"kind": "builtin", "name": "save"})
            ),
            ApplyOutcome::Applied
        );
        assert_eq!(
            button.icon_source(),
            Some(crate::widget::IconSource::Builtin(
                crate::widget::display_widgets::icon::IconName::Save
            ))
        );
        assert_eq!(
            apply_widget_property(
                &mut button,
                "icon_source",
                &serde_json::json!({
                    "kind": "resource",
                    "id": "project/icons/save",
                    "color_mode": "monochrome"
                })
            ),
            ApplyOutcome::Rejected
        );
        assert_eq!(
            button.icon_source(),
            Some(crate::widget::IconSource::Builtin(
                crate::widget::display_widgets::icon::IconName::Save
            )),
            "a rejected resource must not replace the last valid source"
        );
        assert_eq!(
            apply_widget_property(&mut button, "icon_source", &Value::Null),
            ApplyOutcome::Applied
        );
        assert_eq!(button.icon_source(), None);
    }

    /// The loader refuses a record list the control's own schema rejects, and leaves the control alone.
    #[test]
    fn a_record_list_the_schema_refuses_is_rejected_not_applied() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};
        let mut chip = crate::widget::special_widgets::chip::Chip::new(Rect::new(0, 0, 200, 36));
        // A well-formed document reaches the control.
        assert_eq!(
            apply_widget_property(
                &mut chip,
                "items",
                &serde_json::json!([{"id": "a", "label": "Alpha"}])
            ),
            ApplyOutcome::Applied
        );
        assert_eq!(chip.items().len(), 1);
        assert_eq!(chip.items()[0].id, "a");

        // A malformed one is rejected *and* changes nothing — the list is checked in full before any
        // item is replaced, so there is no partial application to undo.
        let mut order = RecordList::new(&EMPTY_RECORD_SCHEMA);
        order.push(
            RecordValue::new()
                .with("id", CapabilityValue::String(String::from("x")))
                .with("label", CapabilityValue::String(String::from("X"))),
        );
        assert_eq!(
            order.validate_against(&crate::widget::special_widgets::chip::CHIP_ITEM_SCHEMA),
            Ok(())
        );
        // `label` is required, so a record without it is refused.
        assert_eq!(
            apply_widget_property(&mut chip, "items", &serde_json::json!([{"id": "b"}])),
            ApplyOutcome::Rejected
        );
        assert_eq!(chip.items().len(), 1, "a refused list must not replace the live items");
        assert_eq!(chip.items()[0].id, "a");
    }

    /// `declares_record_list` is the narrow exception that lets a loader-owned key reach the property
    /// pass — and it answers only for a control that declares a `Records` property.
    #[test]
    fn only_a_record_list_property_claims_a_loader_owned_key() {
        let chip = crate::widget::special_widgets::chip::Chip::new(Rect::new(0, 0, 200, 36));
        assert!(declares_record_list(&chip, "items"));
        // A control that reads `items` as an array of *strings* keeps the loader-owned route.
        let combo = crate::widget::ComboBox::new(Rect::new(0, 0, 200, 36));
        assert!(!declares_record_list(&combo, "items"));
        // And a structure key no control publishes as this kind is unaffected.
        assert!(!declares_record_list(&chip, "children"));
        assert!(!declares_record_list(&chip, "not_a_property"));
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

    /// A `Number` property picks the carrier from the JSON number's own shape, which is
    /// what keeps `{"value": 3}` valid after a spin box has been given decimals
    /// (principle #21).
    #[test]
    fn to_capability_value_number_keeps_the_json_shape() {
        assert_eq!(
            to_capability_value(&serde_json::json!(3), Some(PropertyValueKind::Number)),
            Some(CapabilityValue::Int(3)),
            "a whole number must stay an Int, so integer callers are unaffected"
        );
        assert_eq!(
            to_capability_value(&serde_json::json!(3.5), Some(PropertyValueKind::Number)),
            Some(CapabilityValue::Float(3.5))
        );
        // `3.0` is a decimal spelling of a whole number, so it is accepted as an `Int`
        // rather than rejected for having a fractional part that happens to be zero.
        assert_eq!(
            to_capability_value(&serde_json::json!(3.0), Some(PropertyValueKind::Number)),
            Some(CapabilityValue::Int(3))
        );
        // Text is still refused: parsing can fail, and a failure has no defined result.
        assert_eq!(
            to_capability_value(&serde_json::json!("3"), Some(PropertyValueKind::Number)),
            None
        );
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

    /// B26.02: a JSON value the declared schema's constraints refuse must be rejected here too, by
    /// the *same* validator the capability write path uses.
    ///
    /// The two front ends must not disagree about one document: a value that `write_property`
    /// refuses cannot be one the loader happily applies. `progress_circle.value` declares
    /// `[0.0, 1.0]`, so `1.5` is the sharpest case — the control would have coerced it to `1.0`,
    /// and the loader would have reported a value the document never gets.
    #[test]
    fn a_constraint_violation_is_rejected_not_applied() {
        use crate::widget::capability::WidgetFactory;

        let factory = WidgetFactory::new_with_defaults();
        // `1.5` is outside the declared `[0.0, 1.0]` bound.
        let Some(mut circle) = factory.create("progress_circle", Rect::new(0, 0, 64, 64), "")
        else {
            // A profile without the control has no schema either, so there is nothing to check;
            // the test would prove nothing, and `full_widgets` covers this module's profiles.
            return;
        };
        assert_eq!(
            apply_widget_property(circle.as_mut(), "value", &serde_json::json!(1.5)),
            ApplyOutcome::Rejected,
            "the loader must refuse a value the schema's bound excludes"
        );
        // The boundaries themselves are legal, so the refusal is about the bound and not the name.
        assert_eq!(
            apply_widget_property(circle.as_mut(), "value", &serde_json::json!(1.0)),
            ApplyOutcome::Applied
        );
        assert_eq!(
            apply_widget_property(circle.as_mut(), "value", &serde_json::json!(0.25)),
            ApplyOutcome::Applied
        );
    }
}
