// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The JSON writer for the capability manifest.
//!
//! Split out of [`super::designer_manifest`] when that module crossed the 2000-line limit
//! (principle #9). The seam is the wire half: everything here turns a `DesignerManifest` into
//! document bytes, and nothing here decides what a manifest *is*. The reader stays beside the
//! structs it builds, so both halves of the round trip share one type definition.

use super::designer_manifest::keys;
use super::designer_manifest::{
    shape_token, value_kind_token, DesignerCommand, DesignerConstraint, DesignerManifest,
};
use super::types::CapabilityValue;
use crate::compat::{format, String};

/// Appends `text` as a JSON string, escaping exactly what RFC 8259 requires.
///
/// The escape set is the conservative one — quote, backslash and the C0 controls — rather than
/// also escaping non-ASCII. Capability names are ASCII today, but a property default may hold a
/// label in any script, and escaping those would make the file unreadable for no gain: JSON is
/// UTF-8 and needs no escaping there.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // The remaining C0 controls, plus DEL, which is not printable and has no shorthand.
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Appends `value` as a JSON value.
///
/// Every [`CapabilityValue`] variant maps to a distinct JSON form, including the ones that could
/// have shared one: `Int` and `UInt` both become numbers but are written from their own arms, so a
/// change to one cannot silently change the other. `Null` is the JSON `null`, which is why
/// `Option<usize>`-style payloads have a way to say "absent" without a second encoding.
///
/// # Why `Color` and `Rect` are written as their strings
///
/// The capability *table* declares every colour default as `CapabilityValue::String("#DCDCDCFF")`
/// and every geometry default as `CapabilityValue::String("0,0,0,0")` — see
/// `access::default_widget_property_default_value`. So a manifest that converted them into a
/// structured form would be inventing a representation the library does not actually store: the
/// re-loaded document would then hold a `Color` where the table holds a `String`, the two exports
/// would differ, and the designer's saved value would stop matching what a runtime read returns.
///
/// The declared `value_kind` in the schema still says `Color`/`Rect`, which is what a designer uses
/// to pick a colour picker; the *value* travels in the spelling the property API accepts. That
/// split is the same one the event side makes between `payload` and `shape`: the type says what a
/// value means, and the carrier says how it is written.
fn push_json_value(out: &mut String, value: &CapabilityValue) {
    match value {
        CapabilityValue::Null => out.push_str("null"),
        CapabilityValue::Bool(inner) => out.push_str(if *inner { "true" } else { "false" }),
        CapabilityValue::Int(inner) => push_json_int(out, *inner),
        CapabilityValue::UInt(inner) => out.push_str(&format!("{inner}")),
        CapabilityValue::Float(inner) => push_json_float(out, *inner),
        CapabilityValue::String(inner) => push_json_string(out, inner),
        CapabilityValue::Color(color) => {
            // `#rrggbbaa`. Reached only for a value the table typed as a colour; the spelling
            // matches what `access` writes so both forms re-read identically.
            out.push_str(&format!(
                "\"#{:02x}{:02x}{:02x}{:02x}\"",
                color.r, color.g, color.b, color.a
            ));
        }
        CapabilityValue::Rect(rect) => {
            // `"x,y,w,h"`, the same spelling `geometry_to_value` produces and
            // `access::...@parse_rect` reads, so a manifest value and a property value are one
            // representation rather than two that must be kept in step.
            out.push_str(&format!("\"{},{},{},{}\"", rect.x, rect.y, rect.width, rect.height));
        }
        CapabilityValue::IconSpec(spec) => match spec {
            crate::widget::icon_source::IconSpec::Builtin(name) => {
                out.push_str("{\"kind\":\"builtin\",\"name\":");
                push_json_string(out, name.as_str());
                out.push('}');
            }
            crate::widget::icon_source::IconSpec::Resource { id, color_mode } => {
                out.push_str("{\"kind\":\"resource\",\"id\":");
                push_json_string(out, id);
                out.push_str(",\"color_mode\":");
                push_json_string(
                    out,
                    match color_mode {
                        crate::widget::icon_source::IconColorMode::Monochrome => "monochrome",
                        crate::widget::icon_source::IconColorMode::Original => "original",
                    },
                );
                out.push('}');
            }
        },
        CapabilityValue::Tuple(items) => {
            // A composite payload exports as a JSON **array** of its components, each written by its
            // own kind. That is the natural JSON shape for an ordered group and keeps the round trip
            // lossless for the kinds a tuple can carry (a list of scalars), which is why a tuple
            // payload no longer exports as its debug spelling.
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                push_json_value(out, item);
            }
            out.push(']');
        }
        CapabilityValue::Records(list) => {
            // A record list exports as a JSON **array of objects**, each object's keys written by
            // `push_json_string` so a field name with a quote or a backslash stays valid JSON. This is
            // the shape a document declares, so the manifest form and the document form are one
            // representation rather than two (BLUE26 B26.09).
            out.push('[');
            for (record_index, record) in list.items().iter().enumerate() {
                if record_index > 0 {
                    out.push(',');
                }
                out.push('{');
                for (field_index, (name, value)) in record.fields().iter().enumerate() {
                    if field_index > 0 {
                        out.push(',');
                    }
                    push_json_string(out, name);
                    out.push(':');
                    push_json_value(out, value);
                }
                out.push('}');
            }
            out.push(']');
        }
    }
}

/// Appends one constraint object (BLUE26 B26.02).
///
/// Optional numeric fields are written as `null` when absent rather than omitted, so every
/// constraint object has the same keys. That is what makes the reader a fixed shape: a missing key
/// and a `null` value mean the same thing here, and writing one spelling keeps the round trip
/// byte-stable.
fn push_constraint(out: &mut String, constraint: &DesignerConstraint) {
    out.push_str("      {\n        ");
    push_json_string(out, keys::RULE);
    out.push_str(": ");
    push_json_string(out, constraint.kind);
    out.push_str(",\n        ");
    push_json_string(out, keys::MINIMUM);
    out.push_str(": ");
    push_optional_float(out, constraint.minimum);
    out.push_str(",\n        ");
    push_json_string(out, keys::MAXIMUM);
    out.push_str(": ");
    push_optional_float(out, constraint.maximum);
    out.push_str(",\n        ");
    push_json_string(out, keys::STEP);
    out.push_str(": ");
    push_optional_float(out, constraint.step);
    out.push_str(",\n        ");
    push_json_string(out, keys::PLACES);
    out.push_str(": ");
    match constraint.places {
        Some(places) => out.push_str(&format!("{places}")),
        None => out.push_str("null"),
    }
    out.push_str("\n      }");
}

/// Appends an optional float, writing `null` for `None`.
///
/// A present value goes through [`push_json_float`], so it keeps the integral-value marker that
/// distinguishes a float from an int on reload.
fn push_optional_float(out: &mut String, value: Option<f64>) {
    match value {
        Some(inner) => push_json_float(out, inner),
        None => out.push_str("null"),
    }
}

/// Appends a signed integer, ensuring the result re-reads as an `Int`.
///
/// # Why a non-negative `Int` needs more than its digits
///
/// JSON has one number type and no signed/unsigned distinction, so `Int(100)` and `UInt(100)` are
/// the same three characters. The reader has to pick one, and it picks `UInt` for a non-negative
/// value — `UInt` is the only arm that holds a non-negative integer without a lossy cast, and
/// every non-negative integer default in the capability table is declared `UInt`. That made the
/// round trip **partial**: `Int(100)` exported as `100` and reloaded as `UInt(100)`, so
/// export → load → export produced a different value kind from its input.
///
/// That path is reachable, not hypothetical: `Slider`'s capability declares `minimum` as `Int(0)`
/// and `maximum` as `Int(100)` (`access::...`), both non-negative.
///
/// A leading `+` is the marker. JSON's grammar allows it (`number = [ "-" ] int [ frac ] [ exp ]`,
/// and an unsigned int is `digit1-9 *digit`), the reader records whether it saw one, and no
/// capability default is written with a redundant `+` — so the two arms stay distinguishable
/// without inventing a second document shape (a wrapper object, a `"100i"` string) that a
/// consumer would have to learn. `UInt` keeps the bare spelling, so every existing document and
/// the whole rest of the table is unchanged.
fn push_json_int(out: &mut String, value: i64) {
    if value < 0 {
        out.push_str(&format!("{value}"));
    } else {
        out.push('+');
        out.push_str(&format!("{value}"));
    }
}

/// Appends a float, ensuring the result re-reads as a float.
///
/// `format!("{}", 1.0f64)` is `"1"`, which a reader would take for an `Int` — and the round-trip
/// would then produce a different value kind from the one declared. Non-finite values are written
/// as `null`, because JSON has no spelling for them and writing `NaN` would produce a document
/// that no conforming parser accepts.
fn push_json_float(out: &mut String, value: f64) {
    if !value.is_finite() {
        out.push_str("null");
        return;
    }
    let rendered = format!("{value}");
    if rendered.contains('.') || rendered.contains('e') || rendered.contains('E') {
        out.push_str(&rendered);
        return;
    }
    out.push_str(&rendered);
    out.push_str(".0");
}

/// Writes one control's description as a JSON document.
///
/// # Layout
///
/// One key per line, in a fixed order, so a saved project diffs by field rather than by offset.
/// The property list and the event list each carry their own object per entry, which is what lets a
/// designer patch a single entry instead of rewriting the file.
pub(super) fn write_manifest(out: &mut String, manifest: &DesignerManifest) {
    out.push_str("{\n");
    out.push_str("  ");
    push_json_string(out, keys::PROTOCOL);
    out.push_str(": ");
    out.push_str(&format!("{},\n", manifest.protocol));
    out.push_str("  ");
    push_json_string(out, keys::LIBRARY);
    out.push_str(": ");
    push_json_string(out, &manifest.library);
    out.push_str(",\n  ");
    push_json_string(out, keys::PROFILE);
    out.push_str(": ");
    push_json_string(out, &manifest.profile);
    out.push_str(",\n  ");
    push_json_string(out, keys::TRIPLE);
    out.push_str(": ");
    push_json_string(out, &manifest.triple);
    out.push_str(",\n  ");
    push_json_string(out, keys::CONTROL);
    out.push_str(": ");
    push_json_string(out, &manifest.control);
    out.push_str(",\n  ");
    push_json_string(out, keys::CATEGORY);
    out.push_str(": ");
    push_json_string(out, &manifest.category);
    out.push_str(",\n  ");
    push_json_string(out, keys::ALIAS_OF);
    out.push_str(": ");
    push_json_string(out, &manifest.alias_of);
    out.push_str(",\n  ");
    push_json_string(out, keys::COMPANION_OF);
    out.push_str(": ");
    push_token_array(out, &manifest.companion_of);
    out.push_str(",\n");

    push_array(
        out,
        keys::ALIASES,
        &manifest.aliases,
        |out, alias| {
            push_json_string(out, alias);
        },
        true,
    );

    out.push_str("  ");
    push_json_string(out, keys::PROPERTIES);
    out.push_str(": [\n");
    for (index, property) in manifest.properties.iter().enumerate() {
        out.push_str("    {\n      ");
        push_json_string(out, keys::NAME);
        out.push_str(": ");
        push_json_string(out, &property.name);
        out.push_str(",\n      ");
        push_json_string(out, keys::KIND);
        out.push_str(": ");
        push_json_string(out, value_kind_token(property.kind));
        out.push_str(",\n      ");
        push_json_string(out, keys::READABLE);
        out.push_str(&format!(": {},\n      ", property.readable));
        push_json_string(out, keys::WRITABLE);
        out.push_str(&format!(": {},\n      ", property.writable));
        push_json_string(out, keys::TOKENS);
        out.push_str(": ");
        push_token_array(out, &property.tokens);
        out.push_str(",\n      ");
        push_json_string(out, keys::DEFAULT);
        out.push_str(": ");
        push_json_value(out, &property.default_value);
        out.push_str(",\n      ");
        push_json_string(out, keys::CONSTRAINTS);
        out.push_str(": [");
        if !property.constraints.is_empty() {
            out.push('\n');
            for (c_index, constraint) in property.constraints.iter().enumerate() {
                push_constraint(out, constraint);
                out.push_str(if c_index + 1 == property.constraints.len() { "\n" } else { ",\n" });
            }
        }
        out.push_str("    ]");
        out.push_str("\n    }");
        out.push_str(if index + 1 == manifest.properties.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ],\n");

    out.push_str("  ");
    push_json_string(out, keys::EVENTS);
    out.push_str(": [\n");
    for (index, event) in manifest.events.iter().enumerate() {
        out.push_str("    {\n      ");
        push_json_string(out, keys::NAME);
        out.push_str(": ");
        push_json_string(out, &event.name);
        out.push_str(",\n      ");
        push_json_string(out, keys::PAYLOAD);
        out.push_str(": ");
        match event.payload {
            Some(kind) => push_json_string(out, value_kind_token(kind)),
            None => out.push_str("null"),
        }
        out.push_str(",\n      ");
        push_json_string(out, keys::SHAPE);
        out.push_str(": ");
        match event.shape {
            Some(shape) => push_json_string(out, shape_token(shape)),
            None => out.push_str("null"),
        }
        out.push_str("\n    }");
        out.push_str(if index + 1 == manifest.events.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ],\n");

    push_array(
        out,
        keys::CHILDREN,
        &manifest.children,
        |out, child| {
            push_json_string(out, child);
        },
        true,
    );

    out.push_str("  ");
    push_json_string(out, keys::COMMANDS);
    out.push_str(": [\n");
    for (index, command) in manifest.commands.iter().enumerate() {
        push_command(out, command);
        out.push_str(if index + 1 == manifest.commands.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ]\n");

    out.push_str("}\n");
}

/// Appends one command object, so a designer can validate a binding against its parameter shape.
///
/// A command used to be a bare string. A string is enough to *offer* a command but not to check a
/// wire to it: a designer must know whether it takes an argument, of what type, and whether its
/// effect warrants a warning (BLUE26 B26.03). The object form carries exactly that — and nothing
/// else — so the document describes the command the control actually dispatches.
fn push_command(out: &mut String, command: &DesignerCommand) {
    out.push_str("    {\n      ");
    push_json_string(out, keys::NAME);
    out.push_str(": ");
    push_json_string(out, &command.name);
    out.push_str(",\n      ");
    push_json_string(out, keys::SIGNATURE);
    out.push_str(": ");
    push_json_string(out, command.signature);
    out.push_str(",\n      ");
    push_json_string(out, keys::PARAMETER);
    out.push_str(": ");
    push_json_string(out, &command.parameter);
    out.push_str(",\n      ");
    push_json_string(out, keys::KIND);
    out.push_str(": ");
    match command.kind {
        Some(kind) => push_json_string(out, value_kind_token(kind)),
        None => out.push_str("null"),
    }
    out.push_str(",\n      ");
    push_json_string(out, keys::TOKENS);
    out.push_str(": ");
    push_token_array(out, &command.tokens);
    out.push_str(",\n      ");
    push_json_string(out, keys::ELEMENT);
    out.push_str(": ");
    match command.element {
        Some(kind) => push_json_string(out, value_kind_token(kind)),
        None => out.push_str("null"),
    }
    out.push_str(",\n      ");
    push_json_string(out, keys::ARITY);
    out.push_str(": ");
    match command.arity {
        Some(count) => out.push_str(&format!("{count}")),
        None => out.push_str("null"),
    }
    out.push_str(",\n      ");
    push_json_string(out, keys::EFFECT);
    out.push_str(": ");
    push_json_string(out, command.effect);
    out.push_str("\n    }");
}

/// Appends `"key": ["a", "b"]` for a list of strings that needs no nested objects.
///
/// The trailing comma is the **caller's** decision, because the last list in the document is
/// followed by `}` and must not carry one. An earlier revision always emitted it and relied on the
/// caller not to add another, which produced a comma before the closing brace — a syntax error the
/// round-trip test caught on its first run. Making the separator a parameter rather than a
/// convention is what keeps that from being reintroduced by the next list added here.
fn push_array<T, F>(out: &mut String, key: &str, items: &[T], mut write: F, trailing_comma: bool)
where
    F: FnMut(&mut String, &T),
{
    let sep = if trailing_comma { ",\n" } else { "\n" };
    out.push_str("  ");
    push_json_string(out, key);
    out.push_str(": [");
    if items.is_empty() {
        out.push(']');
        out.push_str(sep);
        return;
    }
    out.push('\n');
    for (index, item) in items.iter().enumerate() {
        out.push_str("    ");
        write(out, item);
        out.push_str(if index + 1 == items.len() { "\n" } else { ",\n" });
    }
    out.push_str("  ]");
    out.push_str(sep);
}

/// Appends a list of already-owned strings, one per line, without the trailing comma of a key.
fn push_token_array(out: &mut String, items: &[String]) {
    out.push('[');
    if items.is_empty() {
        out.push(']');
        return;
    }
    // Inline, because an enum's tokens are a short closed set and a vertical list would triple the
    // height of the document for no readability gain.
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_json_string(out, item);
    }
    out.push(']');
}
