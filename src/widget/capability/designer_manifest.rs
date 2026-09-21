// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The designer's view of a capability, as JSON.
//!
//! # What this is for
//!
//! A designer that lets a user pick a control, see what it can do and wire an event to a property
//! needs the capability table in a form it can **save and reload**. That means JSON, and it means
//! the JSON has to be *round-trippable*: what is written to a file must come back as the same
//! description, or every reopening of a project silently loses information.
//!
//! [`capability_manifest_json`] writes that document; [`DesignerManifest::from_json`] reads it
//! back. The pair is asserted by `tools/check_designer_manifest_roundtrip.py`, which exports,
//! re-loads through the parser here, exports again, and requires the two strings to be
//! **byte-identical** — and requires the sentinels to be present, so two empty documents cannot
//! satisfy it.
//!
//! # Why the serialisation is written by hand
//!
//! `serde` is a device-profile feature (`desktop`/`tablet`/`mobile`), not a universal one: it is
//! absent from both `mini` and `embedded`. Deriving `Serialize` on the capability types would
//! therefore make the capability layer's shape depend on the profile, and the alternative — a
//! second, `cfg`-gated description of the same data — is the duplication rule #54 forbids.
//!
//! The value set here is small and closed: strings, integers, booleans and `null`. Writing the
//! escaping and the key order by hand is a few dozen lines, and it buys two things a derive does
//! not: the output is **stable** (field order is the order in this file, not the order a macro
//! happened to emit) and it is **readable in a diff**, which matters because these files are
//! checked in by designers.
//!
//! # Stability is a contract, not an accident
//!
//! [`capability_manifest_json`] sorts controls by name and events by name, so the same capability
//! table always produces the same bytes. Without that, a round-trip test would pass and a
//! designer's diff would not: re-saving a project would reorder the file.

use super::types::{
    CapabilityValue, EventManifest, EventPayloadShape, PropertySchema, PropertyValueKind,
};
use crate::compat::{format, String, Vec};

/// The key names in the exported document.
///
/// Named constants rather than literals because the writer and the reader must agree exactly, and
/// a typo in one of them is a key that silently reads back as absent.
mod keys {
    pub(crate) const CONTROL: &str = "control";
    pub(crate) const NAME: &str = "name";
    pub(crate) const ALIASES: &str = "aliases";
    pub(crate) const PROPERTIES: &str = "properties";
    pub(crate) const EVENTS: &str = "events";
    pub(crate) const COMMANDS: &str = "commands";
    pub(crate) const KIND: &str = "kind";
    pub(crate) const READABLE: &str = "readable";
    pub(crate) const WRITABLE: &str = "writable";
    pub(crate) const TOKENS: &str = "tokens";
    pub(crate) const DEFAULT: &str = "default";
    pub(crate) const PAYLOAD: &str = "payload";
    pub(crate) const SHAPE: &str = "shape";
}

/// The designer-facing spelling of a [`PropertyValueKind`].
///
/// Deliberately a token rather than a number: a designer reading the JSON, or a human reading a
/// diff, needs to see `"int"`, and an ordinal would change meaning the moment a variant is added
/// in the middle of the enum.
pub fn value_kind_token(kind: PropertyValueKind) -> &'static str {
    match kind {
        PropertyValueKind::Bool => "bool",
        PropertyValueKind::Int => "int",
        PropertyValueKind::UInt => "uint",
        PropertyValueKind::Float => "float",
        PropertyValueKind::Number => "number",
        PropertyValueKind::String => "string",
        PropertyValueKind::Enum => "enum",
        PropertyValueKind::Color => "color",
        PropertyValueKind::Rect => "rect",
    }
}

/// The inverse of [`value_kind_token`]; `None` for an unrecognised spelling.
///
/// Returning `None` rather than a default is what makes a corrupt document an error instead of a
/// document that quietly describes the wrong types.
pub fn value_kind_from_token(token: &str) -> Option<PropertyValueKind> {
    match token {
        "bool" => Some(PropertyValueKind::Bool),
        "int" => Some(PropertyValueKind::Int),
        "uint" => Some(PropertyValueKind::UInt),
        "float" => Some(PropertyValueKind::Float),
        "number" => Some(PropertyValueKind::Number),
        "string" => Some(PropertyValueKind::String),
        "enum" => Some(PropertyValueKind::Enum),
        "color" => Some(PropertyValueKind::Color),
        "rect" => Some(PropertyValueKind::Rect),
        _ => None,
    }
}

/// The designer-facing spelling of an [`EventPayloadShape`].
pub fn shape_token(shape: EventPayloadShape) -> &'static str {
    match shape {
        EventPayloadShape::Scalar => "scalar",
        EventPayloadShape::OptionalScalar => "optional",
        EventPayloadShape::ListScalar => "list",
        EventPayloadShape::Tuple2 => "tuple2",
        EventPayloadShape::Tuple3 => "tuple3",
        EventPayloadShape::Tuple4 => "tuple4",
        EventPayloadShape::OptionalTuple2 => "optional_tuple2",
        EventPayloadShape::Mixed => "mixed",
    }
}

/// The inverse of [`shape_token`]; `None` for an unrecognised spelling.
pub fn shape_from_token(token: &str) -> Option<EventPayloadShape> {
    match token {
        "scalar" => Some(EventPayloadShape::Scalar),
        "optional" => Some(EventPayloadShape::OptionalScalar),
        "list" => Some(EventPayloadShape::ListScalar),
        "tuple2" => Some(EventPayloadShape::Tuple2),
        "tuple3" => Some(EventPayloadShape::Tuple3),
        "tuple4" => Some(EventPayloadShape::Tuple4),
        "optional_tuple2" => Some(EventPayloadShape::OptionalTuple2),
        "mixed" => Some(EventPayloadShape::Mixed),
        _ => None,
    }
}

/// One property as the designer sees it: its declared type, accessibility and default value.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerProperty {
    /// The property's name.
    pub name: String,
    /// The declared value kind.
    pub kind: PropertyValueKind,
    /// Whether a read is allowed.
    pub readable: bool,
    /// Whether a write is allowed.
    pub writable: bool,
    /// The accepted spellings of an enum property, empty for every other kind.
    pub tokens: Vec<String>,
    /// The value the property holds on a freshly created control.
    pub default_value: CapabilityValue,
}

/// One event as the designer sees it: its name and the type of the value it carries.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerEvent {
    /// The published name, as `connect_event` accepts it.
    pub name: String,
    /// The declared payload kind, or `None` when the event carries no value.
    pub payload: Option<PropertyValueKind>,
    /// The arrangement of the payload.
    pub shape: Option<EventPayloadShape>,
}

/// One control's full designer-facing description.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerManifest {
    /// The canonical control name.
    pub control: String,
    /// Every name the factory also accepts for this control.
    pub aliases: Vec<String>,
    /// The properties it publishes.
    pub properties: Vec<DesignerProperty>,
    /// The events it publishes, with payload types.
    pub events: Vec<DesignerEvent>,
    /// The commands it accepts.
    pub commands: Vec<String>,
}

/// Why a designer document could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestParseError {
    /// What was wrong, positioned by key path where the parser can tell.
    pub detail: String,
}

impl core::fmt::Display for ManifestParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.detail)
    }
}

/// Why a manifest could not be produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestExportError {
    /// No control is registered under the requested name.
    UnknownControl,
    /// The control exists but its property default values could not all be read, so the document
    /// would describe fewer properties than the control publishes.
    ///
    /// Reported rather than skipped: a designer that silently omits a property shows the user a
    /// control with capabilities it does not have.
    IncompleteDefaults,
}

impl core::fmt::Display for ManifestExportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownControl => f.write_str("no control registered under that name"),
            Self::IncompleteDefaults => f.write_str(
                "a property default value could not be read, so the export is incomplete",
            ),
        }
    }
}

// ── Writing ────────────────────────────────────────────────────────────────────────

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
        CapabilityValue::Int(inner) => out.push_str(&format!("{inner}")),
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
fn write_manifest(out: &mut String, manifest: &DesignerManifest) {
    out.push_str("{\n");
    out.push_str("  ");
    push_json_string(out, keys::CONTROL);
    out.push_str(": ");
    push_json_string(out, &manifest.control);
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
        keys::COMMANDS,
        &manifest.commands,
        |out, command| {
            push_json_string(out, command);
        },
        false,
    );

    out.push_str("}\n");
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

// ── Reading ────────────────────────────────────────────────────────────────────────

/// A minimal JSON reader for the documents this module writes.
///
/// # Why this is not `serde_json`
///
/// `src/json/` already depends on `serde_json`, but that module is `full_widgets`-gated and the
/// capability layer is not the JSON layer's owner. Depending on it here would make the round-trip
/// untestable in any profile without it, and the document this parser reads is one this module
/// wrote — a closed grammar with no numbers in exponent form, no duplicate keys and no nesting
/// beyond arrays of objects.
///
/// It is still a real parser, not a substring search: every key is matched as a key, strings are
/// unescaped, and anything unexpected is an error. A parser that guessed would make the round-trip
/// test unable to fail, which is the one property this module exists to have.
struct JsonReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> JsonReader<'a> {
    fn new(text: &'a str) -> Self {
        Self { bytes: text.as_bytes(), cursor: 0 }
    }

    /// The error to report for the current position.
    fn fail<T>(&self, detail: &str) -> Result<T, ManifestParseError> {
        Err(ManifestParseError { detail: format!("{} at byte {}", detail, self.cursor) })
    }

    /// Advances past whitespace.
    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.bytes.get(self.cursor) {
            if byte.is_ascii_whitespace() {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }

    /// Consumes `expected`, or fails.
    fn expect(&mut self, expected: u8, what: &str) -> Result<(), ManifestParseError> {
        self.skip_whitespace();
        match self.bytes.get(self.cursor) {
            Some(byte) if *byte == expected => {
                self.cursor += 1;
                Ok(())
            }
            _ => self.fail(what),
        }
    }

    /// Reports whether the next non-space byte is `byte`, without consuming it.
    fn peek(&mut self, byte: u8) -> bool {
        self.skip_whitespace();
        self.bytes.get(self.cursor) == Some(&byte)
    }

    /// Reads the literal `null`, or fails.
    fn read_null(&mut self) -> Result<(), ManifestParseError> {
        self.skip_whitespace();
        if self.bytes[self.cursor..].starts_with(b"null") {
            self.cursor += 4;
            return Ok(());
        }
        self.fail("expected `null`")
    }

    /// Reads `true` or `false`, or fails.
    fn read_bool(&mut self) -> Result<bool, ManifestParseError> {
        self.skip_whitespace();
        if self.bytes[self.cursor..].starts_with(b"true") {
            self.cursor += 4;
            return Ok(true);
        }
        if self.bytes[self.cursor..].starts_with(b"false") {
            self.cursor += 5;
            return Ok(false);
        }
        self.fail("expected `true` or `false`")
    }

    /// Reads a JSON string, unescaping it.
    fn read_string(&mut self) -> Result<String, ManifestParseError> {
        self.expect(b'"', "expected a string")?;
        let mut out = String::new();
        loop {
            let Some(byte) = self.bytes.get(self.cursor).copied() else {
                return self.fail("unterminated string");
            };
            self.cursor += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let Some(escape) = self.bytes.get(self.cursor).copied() else {
                        return self.fail("unterminated escape");
                    };
                    self.cursor += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let code = self.read_hex4()?;
                            // A surrogate pair is two `\u` escapes; both arrive here, and
                            // `char::from_u32` rejects an unpaired half rather than producing a
                            // replacement character that would not re-serialize identically.
                            let Some(ch) = char::from_u32(code) else {
                                return self.fail("invalid \\u escape");
                            };
                            out.push(ch);
                        }
                        _ => return self.fail("unknown escape"),
                    }
                }
                _ => {
                    // The document is UTF-8 and this reader walks bytes, so a multi-byte character
                    // must be copied whole. Re-decoding from the cursor position is what keeps a
                    // label in any script intact through the round trip.
                    let start = self.cursor - 1;
                    let width = utf8_width(byte);
                    if width == 1 {
                        out.push(byte as char);
                    } else {
                        let end = start + width;
                        if end > self.bytes.len() {
                            return self.fail("truncated UTF-8 sequence");
                        }
                        let Some(text) = core::str::from_utf8(&self.bytes[start..end]).ok() else {
                            return self.fail("invalid UTF-8 sequence");
                        };
                        out.push_str(text);
                        self.cursor = end;
                    }
                }
            }
        }
    }

    /// Reads the four hex digits of a `\u` escape.
    fn read_hex4(&mut self) -> Result<u32, ManifestParseError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.bytes.get(self.cursor).copied() else {
                return self.fail("truncated \\u escape");
            };
            self.cursor += 1;
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u32,
                b'a'..=b'f' => (byte - b'a') as u32 + 10,
                b'A'..=b'F' => (byte - b'A') as u32 + 10,
                _ => return self.fail("non-hex digit in \\u escape"),
            };
            value = value * 16 + digit;
        }
        Ok(value)
    }

    /// Reads an integer or float, reporting whether it had a fractional part.
    fn read_number(&mut self) -> Result<(f64, bool), ManifestParseError> {
        self.skip_whitespace();
        let start = self.cursor;
        while let Some(byte) = self.bytes.get(self.cursor) {
            if byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E') {
                self.cursor += 1;
            } else {
                break;
            }
        }
        if start == self.cursor {
            return self.fail("expected a number");
        }
        let Some(text) = core::str::from_utf8(&self.bytes[start..self.cursor]).ok() else {
            return self.fail("number is not ASCII");
        };
        let is_float = text.contains('.') || text.contains('e') || text.contains('E');
        match text.parse::<f64>() {
            Ok(value) => Ok((value, is_float)),
            Err(_) => self.fail("malformed number"),
        }
    }

    /// Reads the key at the cursor and the colon after it.
    fn read_key(&mut self) -> Result<String, ManifestParseError> {
        let key = self.read_string()?;
        self.expect(b':', "expected `:` after a key")?;
        Ok(key)
    }
}

/// How many bytes the UTF-8 sequence starting with `first` occupies.
fn utf8_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// Reads an array of strings.
fn read_string_array(reader: &mut JsonReader<'_>) -> Result<Vec<String>, ManifestParseError> {
    reader.expect(b'[', "expected `[`")?;
    let mut items = Vec::new();
    if reader.peek(b']') {
        reader.expect(b']', "expected `]`")?;
        return Ok(items);
    }
    loop {
        items.push(reader.read_string()?);
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b']', "expected `]` or `,` in a string array")?;
        return Ok(items);
    }
}

/// Reads one property object.
fn read_property(reader: &mut JsonReader<'_>) -> Result<DesignerProperty, ManifestParseError> {
    reader.expect(b'{', "expected `{` to start a property")?;
    let mut name = None;
    let mut kind = None;
    let mut readable = None;
    let mut writable = None;
    let mut tokens = Vec::new();
    let mut default_value = CapabilityValue::Null;
    let mut saw_default = false;

    loop {
        let key = reader.read_key()?;
        match key.as_str() {
            // Matching the literal would allocate a key we already own; the constants are `&str`
            // and the parsed key is a `String`, so the comparison is against the token.
            k if k == keys::NAME => name = Some(reader.read_string()?),
            k if k == keys::KIND => {
                let token = reader.read_string()?;
                kind = value_kind_from_token(&token);
                if kind.is_none() {
                    return Err(ManifestParseError {
                        detail: format!("unknown property kind `{token}`"),
                    });
                }
            }
            k if k == keys::READABLE => readable = Some(reader.read_bool()?),
            k if k == keys::WRITABLE => writable = Some(reader.read_bool()?),
            k if k == keys::TOKENS => tokens = read_string_array(reader)?,
            k if k == keys::DEFAULT => {
                default_value = read_value(reader)?;
                saw_default = true;
            }
            // An unknown key is an error rather than a skip: a document this writer did not produce
            // is a document whose round trip cannot be reasoned about.
            other => {
                return Err(ManifestParseError {
                    detail: format!("unexpected key `{other}` in a property"),
                })
            }
        }
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b'}', "expected `}` or `,` in a property")?;
        break;
    }

    Ok(DesignerProperty {
        name: name.ok_or(ManifestParseError { detail: "a property has no name".into() })?,
        kind: kind.ok_or(ManifestParseError { detail: "a property has no kind".into() })?,
        readable: readable.unwrap_or(false),
        writable: writable.unwrap_or(false),
        tokens,
        default_value: if saw_default { default_value } else { CapabilityValue::Null },
    })
}

/// Reads one event object.
fn read_event(reader: &mut JsonReader<'_>) -> Result<DesignerEvent, ManifestParseError> {
    reader.expect(b'{', "expected `{` to start an event")?;
    let mut name = None;
    let mut payload = None;
    let mut shape = None;

    loop {
        let key = reader.read_key()?;
        match key.as_str() {
            k if k == keys::NAME => name = Some(reader.read_string()?),
            k if k == keys::PAYLOAD => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    let token = reader.read_string()?;
                    payload = Some(value_kind_from_token(&token).ok_or(ManifestParseError {
                        detail: format!("unknown event payload kind `{token}`"),
                    })?);
                }
            }
            k if k == keys::SHAPE => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    let token = reader.read_string()?;
                    shape = Some(shape_from_token(&token).ok_or(ManifestParseError {
                        detail: format!("unknown event shape `{token}`"),
                    })?);
                }
            }
            other => {
                return Err(ManifestParseError {
                    detail: format!("unexpected key `{other}` in an event"),
                })
            }
        }
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b'}', "expected `}` or `,` in an event")?;
        break;
    }

    Ok(DesignerEvent {
        name: name.ok_or(ManifestParseError { detail: "an event has no name".into() })?,
        payload,
        shape,
    })
}

/// Reads a JSON value into a [`CapabilityValue`].
///
/// Strings are taken verbatim: `"#DCDCDCFF"` and `"0,0,0,0"` are how the capability table spells a
/// colour default and a geometry default, so recovering a `Color`/`Rect` here would produce a value
/// the table does not hold and the two exports would disagree. The schema's `kind` is what says how
/// to interpret the string.
fn read_value(reader: &mut JsonReader<'_>) -> Result<CapabilityValue, ManifestParseError> {
    if reader.peek(b'n') {
        reader.read_null()?;
        return Ok(CapabilityValue::Null);
    }
    // `true`/`false` must be tested before the number arm: a `t` is not a digit, so the number arm
    // would report a malformed number for a valid boolean. The order of these guards is the whole
    // reason `read_value` is not a `match` on the first byte — `t` and `f` have no other meaning in
    // this grammar, but relying on that would make the reader silently wrong if one were added.
    if reader.peek(b't') || reader.peek(b'f') {
        return Ok(CapabilityValue::Bool(reader.read_bool()?));
    }
    if reader.peek(b'"') {
        return Ok(CapabilityValue::String(reader.read_string()?));
    }
    let (number, is_float) = reader.read_number()?;
    if is_float {
        return Ok(CapabilityValue::Float(number));
    }
    // An integral number carries no record of which arm wrote it. `UInt` is chosen for a
    // non-negative value because it is the only kind that holds it without a lossy cast, and every
    // non-negative integer default in the capability table is declared `UInt`.
    if number < 0.0 {
        return Ok(CapabilityValue::Int(number as i64));
    }
    Ok(CapabilityValue::UInt(number as u64))
}

impl DesignerManifest {
    /// Reads a document written by [`capability_manifest_json`].
    ///
    /// Every key this writer emits is required, and an unknown key is an error: a document that
    /// this reader half-understands is one whose next export would silently differ from its input,
    /// which is exactly the failure the round-trip check exists to catch.
    pub fn from_json(text: &str) -> Result<Self, ManifestParseError> {
        let mut reader = JsonReader::new(text);
        reader.expect(b'{', "expected `{` to start a document")?;

        let mut control = None;
        let mut aliases = Vec::new();
        let mut properties = Vec::new();
        let mut events = Vec::new();
        let mut commands = Vec::new();

        loop {
            let key = reader.read_key()?;
            match key.as_str() {
                k if k == keys::CONTROL => control = Some(reader.read_string()?),
                k if k == keys::ALIASES => aliases = read_string_array(&mut reader)?,
                k if k == keys::COMMANDS => commands = read_string_array(&mut reader)?,
                k if k == keys::PROPERTIES => {
                    reader.expect(b'[', "expected `[` to start the property list")?;
                    if !reader.peek(b']') {
                        loop {
                            properties.push(read_property(&mut reader)?);
                            if reader.peek(b',') {
                                reader.expect(b',', "expected `,`")?;
                                continue;
                            }
                            break;
                        }
                    }
                    reader.expect(b']', "expected `]` after the property list")?;
                }
                k if k == keys::EVENTS => {
                    reader.expect(b'[', "expected `[` to start the event list")?;
                    if !reader.peek(b']') {
                        loop {
                            events.push(read_event(&mut reader)?);
                            if reader.peek(b',') {
                                reader.expect(b',', "expected `,`")?;
                                continue;
                            }
                            break;
                        }
                    }
                    reader.expect(b']', "expected `]` after the event list")?;
                }
                other => {
                    return Err(ManifestParseError {
                        detail: format!("unexpected key `{other}` in a manifest"),
                    })
                }
            }
            if reader.peek(b',') {
                reader.expect(b',', "expected `,`")?;
                continue;
            }
            reader.expect(b'}', "expected `}` or `,` in a manifest")?;
            break;
        }

        Ok(Self {
            control: control.ok_or(ManifestParseError {
                detail: "the document does not name its control".into(),
            })?,
            aliases,
            properties,
            events,
            commands,
        })
    }
}

/// Serialises one control's designer-facing description.
///
/// This is the export half of the round trip. It reads the capability table for the structure and
/// the factory for each property's default value, sorts both lists by name, and writes the document
/// [`DesignerManifest::from_json`] reads.
///
/// # Why defaults are read rather than declared
///
/// The default value is what a designer shows for a freshly dropped control. Taking it from the
/// same call the property API uses means the panel and the runtime cannot disagree; a table of
/// defaults kept beside the schema would be a second answer to the same question.
///
/// # Errors
///
/// * [`ManifestExportError::UnknownControl`] — no control is registered under `control`.
/// * [`ManifestExportError::IncompleteDefaults`] — some property's default could not be read, so
///   the document would describe fewer properties than the control publishes.
#[cfg(full_widgets)]
pub fn designer_manifest(
    factory: &super::WidgetFactory,
    control: &str,
) -> Result<DesignerManifest, ManifestExportError> {
    let capability = factory.capability(control).ok_or(ManifestExportError::UnknownControl)?;

    let mut properties = Vec::with_capacity(capability.properties.len());
    for schema in capability.properties {
        let default_value = factory
            .schema_default_value(capability.kind, schema.name)
            .ok_or(ManifestExportError::IncompleteDefaults)?;
        properties.push(DesignerProperty {
            name: schema.name.into(),
            kind: schema.value_kind,
            readable: schema.readable,
            writable: schema.writable,
            tokens: schema.accepted_tokens.iter().map(|token| String::from(*token)).collect(),
            default_value,
        });
    }
    properties.sort_by(|left, right| left.name.cmp(&right.name));

    let mut events: Vec<DesignerEvent> = capability
        .events
        .iter()
        .map(|schema| DesignerEvent {
            name: schema.name.into(),
            payload: schema.payload,
            shape: schema.shape,
        })
        .collect();
    events.sort_by(|left, right| left.name.cmp(&right.name));

    let mut commands: Vec<String> =
        capability.commands.iter().map(|command| String::from(*command)).collect();
    commands.sort();

    let mut aliases: Vec<String> =
        capability.aliases.iter().map(|alias| String::from(*alias)).collect();
    aliases.sort();

    Ok(DesignerManifest {
        control: capability.canonical_name.into(),
        aliases,
        properties,
        events,
        commands,
    })
}

/// Serialises one control's designer-facing description as a JSON document.
///
/// The document is stable: the same capability table produces the same bytes, which is what makes
/// it usable as a checked-in artifact and what lets the round-trip check compare strings rather
/// than parse trees.
#[cfg(full_widgets)]
pub fn capability_manifest_json(
    factory: &super::WidgetFactory,
    control: &str,
) -> Result<String, ManifestExportError> {
    let manifest = designer_manifest(factory, control)?;
    let mut out = String::new();
    write_manifest(&mut out, &manifest);
    Ok(out)
}

/// Serialises an already-built manifest, for a caller that has one in hand.
pub fn manifest_to_json(manifest: &DesignerManifest) -> String {
    let mut out = String::new();
    write_manifest(&mut out, manifest);
    out
}

/// Serialises every control the factory knows, as a JSON array.
///
/// # Why an array and not an object keyed by name
///
/// Order is part of the contract: the entries are sorted by control name, and a designer building a
/// palette reads them in that order. An object keyed by name would make the order incidental and
/// the document harder to diff.
#[cfg(full_widgets)]
pub fn all_capability_manifests_json(
    factory: &super::WidgetFactory,
) -> Result<String, ManifestExportError> {
    let mut names: Vec<&str> =
        factory.capabilities().iter().map(|capability| capability.canonical_name).collect();
    names.sort_unstable();

    let mut out = String::from("[\n");
    for (index, name) in names.iter().enumerate() {
        let manifest = designer_manifest(factory, name)?;
        let mut body = String::new();
        write_manifest(&mut body, &manifest);
        // Indented by one level so the array reads as a list of documents rather than a run of
        // top-level objects.
        for line in body.lines() {
            if line.is_empty() {
                continue;
            }
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
        if index + 1 != names.len() {
            out.push_str("  ,\n");
        }
    }
    out.push(']');
    out.push('\n');
    Ok(out)
}

/// The events a control publishes, as manifest entries.
///
/// Exposed so a consumer that already holds a manifest can reach the event list without going back
/// through the factory.
pub fn events_of_manifest(manifest: &DesignerManifest) -> &[DesignerEvent] {
    &manifest.events
}

impl From<&EventManifest> for DesignerEvent {
    fn from(manifest: &EventManifest) -> Self {
        Self { name: manifest.name.clone(), payload: manifest.payload, shape: manifest.shape }
    }
}

impl From<&PropertySchema> for (String, PropertyValueKind) {
    fn from(schema: &PropertySchema) -> Self {
        (schema.name.into(), schema.value_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Rect};

    fn sample() -> DesignerManifest {
        DesignerManifest {
            control: String::from("slider"),
            aliases: vec![String::from("range")],
            properties: vec![DesignerProperty {
                name: String::from("value"),
                kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
                tokens: Vec::new(),
                default_value: CapabilityValue::Int(42),
            }],
            events: vec![DesignerEvent {
                name: String::from("value_changed"),
                payload: Some(PropertyValueKind::Int),
                shape: Some(EventPayloadShape::Scalar),
            }],
            commands: vec![String::from("set_value")],
        }
    }

    /// The round trip must reproduce the document byte for byte.
    #[test]
    fn a_manifest_round_trips_through_json() {
        let manifest = sample();
        let first = manifest_to_json(&manifest);
        let reloaded = DesignerManifest::from_json(&first).expect("the document this wrote parses");
        let second = manifest_to_json(&reloaded);
        assert_eq!(first, second, "export -> load -> export changed the document");
    }

    /// A payload-free event keeps its absence of a payload through the round trip.
    #[test]
    fn a_payload_free_event_round_trips_as_null() {
        let mut manifest = sample();
        manifest.events.push(DesignerEvent {
            name: String::from("slider_pressed"),
            payload: None,
            shape: None,
        });
        let json = manifest_to_json(&manifest);
        assert!(json.contains("\"payload\": null"), "a payload-free event must declare null");
        let reloaded = DesignerManifest::from_json(&json).expect("parses");
        assert_eq!(reloaded.events[1].payload, None);
        assert_eq!(reloaded.events[1].shape, None);
    }

    /// Every `CapabilityValue` variant survives, including the two structured ones.
    ///
    /// `Color` and `Rect` are written as the strings the capability table uses, so they come back
    /// as `String` — the round trip is over the *document*, and the schema's `kind` is what tells a
    /// consumer to interpret that string as a colour. Asserting `String` here rather than
    /// `Color`/`Rect` is the check that the writer did not invent a second representation.
    #[test]
    fn every_value_variant_round_trips() {
        let cases: [(CapabilityValue, CapabilityValue); 9] = [
            (CapabilityValue::Null, CapabilityValue::Null),
            (CapabilityValue::Bool(true), CapabilityValue::Bool(true)),
            (CapabilityValue::Bool(false), CapabilityValue::Bool(false)),
            (CapabilityValue::UInt(7), CapabilityValue::UInt(7)),
            (CapabilityValue::Float(1.5), CapabilityValue::Float(1.5)),
            (CapabilityValue::Int(-3), CapabilityValue::Int(-3)),
            (
                CapabilityValue::String(String::from("a \"quoted\" \\ value\nwith a newline")),
                CapabilityValue::String(String::from("a \"quoted\" \\ value\nwith a newline")),
            ),
            (
                CapabilityValue::String(String::from("标签值")),
                CapabilityValue::String(String::from("标签值")),
            ),
            (
                CapabilityValue::Color(Color::rgba(1, 2, 3, 4)),
                CapabilityValue::String(String::from("#01020304")),
            ),
        ];
        for (written, expected) in cases {
            let mut manifest = sample();
            manifest.properties[0].default_value = written.clone();
            let json = manifest_to_json(&manifest);
            let reloaded = DesignerManifest::from_json(&json)
                .unwrap_or_else(|error| panic!("{written:?} failed to parse: {error}"));
            assert_eq!(
                reloaded.properties[0].default_value, expected,
                "{written:?} did not survive the round trip"
            );
        }
    }

    /// A structured value must be written in the spelling the capability table uses.
    #[test]
    fn structured_values_use_the_property_api_spelling() {
        let mut manifest = sample();
        manifest.properties[0].default_value = CapabilityValue::Rect(Rect::new(-5, 6, 7, 8));
        let json = manifest_to_json(&manifest);
        assert!(
            json.contains("\"-5,6,7,8\""),
            "a rectangle must be written as the `x,y,w,h` string the property API accepts: {json}"
        );

        manifest.properties[0].default_value =
            CapabilityValue::Color(Color::rgba(0xdc, 0xdc, 0xdc, 0xff));
        let json = manifest_to_json(&manifest);
        assert!(json.contains("\"#dcdcdcff\""), "a colour must be written as `#rrggbbaa`: {json}");
    }

    /// A whole float must not come back as an integer.
    ///
    /// `format!("{}", 1.0f64)` is `"1"`, so a writer that does not add the fractional part produces
    /// a document that reads back as a different kind.
    #[test]
    fn a_whole_float_keeps_its_fractional_part() {
        let mut manifest = sample();
        manifest.properties[0].default_value = CapabilityValue::Float(1.0);
        let json = manifest_to_json(&manifest);
        assert!(json.contains("1.0"), "a float must be written so it reads back as a float");
        let reloaded = DesignerManifest::from_json(&json).expect("parses");
        assert_eq!(reloaded.properties[0].default_value, CapabilityValue::Float(1.0));
    }

    /// An unknown kind or shape must be refused rather than defaulted.
    ///
    /// The sentinel is `"quaternion"`, which is not a token this reader knows and is not
    /// about to be one. It used to be `"number"`, which stopped being a valid sentinel the
    /// moment `PropertyValueKind::Number` was added — the test then failed for the right
    /// reason (the token is accepted now) and had to move to a spelling that stays unknown.
    #[test]
    fn an_unrecognised_kind_is_refused() {
        let json =
            manifest_to_json(&sample()).replace("\"kind\": \"int\"", "\"kind\": \"quaternion\"");
        assert_ne!(json, manifest_to_json(&sample()), "the sentinel must actually appear");
        let result = DesignerManifest::from_json(&json);
        assert!(result.is_err(), "a kind this reader does not know must not be accepted");
    }

    /// A key this writer never emits must be refused, so a half-understood file cannot be read.
    #[test]
    fn an_unknown_key_is_refused() {
        let json = manifest_to_json(&sample()).replace("\"control\":", "\"controller\":");
        assert!(DesignerManifest::from_json(&json).is_err());
    }

    /// Truncated input must fail rather than return a partial document.
    #[test]
    fn a_truncated_document_is_refused() {
        let json = manifest_to_json(&sample());
        assert!(DesignerManifest::from_json(&json[..json.len() / 2]).is_err());
    }

    /// Every `PropertyValueKind` and `EventPayloadShape` token must survive its own mapping.
    #[test]
    fn every_token_maps_both_ways() {
        for kind in [
            PropertyValueKind::Bool,
            PropertyValueKind::Int,
            PropertyValueKind::UInt,
            PropertyValueKind::Float,
            PropertyValueKind::Number,
            PropertyValueKind::String,
            PropertyValueKind::Enum,
            PropertyValueKind::Color,
            PropertyValueKind::Rect,
        ] {
            assert_eq!(value_kind_from_token(value_kind_token(kind)), Some(kind));
        }
        for shape in [
            EventPayloadShape::Scalar,
            EventPayloadShape::OptionalScalar,
            EventPayloadShape::ListScalar,
            EventPayloadShape::Tuple2,
            EventPayloadShape::Tuple3,
            EventPayloadShape::Tuple4,
            EventPayloadShape::OptionalTuple2,
            EventPayloadShape::Mixed,
        ] {
            assert_eq!(shape_from_token(shape_token(shape)), Some(shape));
        }
        assert_eq!(value_kind_from_token("no_such_kind"), None);
        assert_eq!(shape_from_token("no_such_shape"), None);
    }
}
