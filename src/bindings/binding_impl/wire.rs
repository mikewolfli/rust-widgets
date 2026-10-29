// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Wire encoding and decoding for capability values crossing the C ABI.
use super::*;
use core::ffi::c_char;
use core::ffi::c_int;
use core::ffi::CStr;

/// Splits a [`CapabilityValue`] into the `(kind, num, str)` triple the ABI uses.
///
/// # Why a triple instead of a union
///
/// A C union would need the caller to know the active member without reading it,
/// and `i64` is the smallest numeric representation both `Int` and `UInt` fit
/// losslessly. Keeping `out_num` and `out_str` independent means a caller can
/// ignore whichever it does not need.
#[cfg(not(stripped_widgets))]
/// Encodes a [`CapabilityValue`](crate::widget::capability::CapabilityValue) into
/// the ABI's `(kind, num, text)` triple.
///
/// `want_text` says whether the caller has a place to receive the text. When it is
/// `false`, the string-carrying kinds return `None` **without allocating**: the
/// owning `CString` is only produced when there is an output pointer to receive it,
/// so a kind-only read (`out_str == null`) cannot leak. The `kind`/`num` half is
/// computed unconditionally, so a caller that suppresses the text still gets the
/// discriminator and the numeric payload.
pub(crate) fn encode_capability_value(
    value: crate::widget::capability::CapabilityValue,
    want_text: bool,
) -> (c_int, i64, Option<*mut c_char>) {
    use crate::widget::capability::CapabilityValue;
    match value {
        CapabilityValue::Null => (RW_VALUE_NULL, 0, None),
        CapabilityValue::Bool(flag) => (RW_VALUE_BOOL, i64::from(flag), None),
        CapabilityValue::Int(number) => (RW_VALUE_INT, number, None),
        CapabilityValue::UInt(number) => (RW_VALUE_UINT, number as i64, None),
        CapabilityValue::Float(number) => {
            // Reinterpret rather than truncate: a caller reading a float property
            // must get the real value, so the bit pattern travels in the same i64.
            (RW_VALUE_FLOAT, number.to_bits() as i64, None)
        }
        CapabilityValue::String(text) => {
            if !want_text {
                return (RW_VALUE_STRING, 0, None);
            }
            let c_text = to_c_string_or_empty(text);
            (RW_VALUE_STRING, 0, Some(c_text))
        }
        // Colours and rectangles travel as their CSS-style string form, which is the
        // spelling the style layer already accepts. They keep distinct `kind`s so a
        // caller can tell a colour from free text without guessing, and so the value
        // is re-parseable into a real `Color`/`Rect` rather than a loose string.
        //
        // `to_hex_rgba` always emits `#RRGGBBAA`, which `CssParser::parse_color`
        // accepts, so encode/decode round-trips exactly (see
        // `capability_values_round_trip_through_the_abi`).
        CapabilityValue::Color(color) => {
            if !want_text {
                return (RW_VALUE_COLOR, 0, None);
            }
            (RW_VALUE_COLOR, 0, Some(to_c_string_or_empty(color.to_hex_rgba())))
        }
        CapabilityValue::Rect(rect) => {
            if !want_text {
                return (RW_VALUE_RECT, 0, None);
            }
            let text = alloc::format!(
                "{},{},{},{}",
                rect.x,
                rect.y,
                rect.width as i32,
                rect.height as i32
            );
            let c_text = to_c_string_or_empty(text);
            (RW_VALUE_RECT, 0, Some(c_text))
        }
        CapabilityValue::IconSpec(spec) => {
            if !want_text {
                return (RW_VALUE_ICON_SPEC, 0, None);
            }
            let c_text = to_c_string_or_empty(encode_icon_spec(&spec));
            (RW_VALUE_ICON_SPEC, 0, Some(c_text))
        }
        // A tuple travels as a compact `kind:value;kind:value` string with its own kind, so a C
        // caller can tell a tuple from free text and can recover each component with its type.
        // The components keep their own kinds because the whole point of the variant is that a
        // composite payload is not flattened to one scalar (BLUE19 #95).
        CapabilityValue::Tuple(items) => {
            if !want_text {
                return (RW_VALUE_TUPLE, items.len() as i64, None);
            }
            let mut text = alloc::string::String::new();
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    text.push(';');
                }
                encode_tuple_component(item, &mut text);
            }
            let c_text = to_c_string_or_empty(text);
            (RW_VALUE_TUPLE, items.len() as i64, Some(c_text))
        }
        // A record list travels as `name=kind:value;…` per record with `|` between records, so a C
        // caller can see each field's name and each value's own kind. `num` is the record count, which
        // is what a caller needs to size its array before reading the text.
        CapabilityValue::Records(list) => {
            if !want_text {
                return (RW_VALUE_RECORDS, list.len() as i64, None);
            }
            let c_text = to_c_string_or_empty(encode_record_list(&list));
            (RW_VALUE_RECORDS, list.len() as i64, Some(c_text))
        }
    }
}

/// Writes a record list as `name=kind:value;…` per record, with records separated by `|`.
///
/// The field name is escaped with the same separator-aware escaper the values use, so a name
/// containing a separator cannot be mistaken for one.
#[cfg(not(stripped_widgets))]
fn encode_record_list(list: &crate::widget::capability::RecordList) -> String {
    use core::fmt::Write as _;
    let mut out = alloc::string::String::new();
    for (record_index, record) in list.items().iter().enumerate() {
        if record_index > 0 {
            out.push('|');
        }
        for (field_index, (name, value)) in record.fields().iter().enumerate() {
            if field_index > 0 {
                out.push(';');
            }
            let _ = write!(out, "{}=", escape_wire_separators(name));
            encode_tuple_component(value, &mut out);
        }
    }
    out
}

/// Escapes every separator a record wire string uses, so a name cannot be read as structure.
#[cfg(not(stripped_widgets))]
fn escape_wire_separators(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if matches!(ch, '\\' | ';' | ':' | '|' | '=') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Appends one tuple component to `out` as `kind:value`, the wire form
/// [`decode_capability_value`] reads back for [`RW_VALUE_TUPLE`].
#[cfg(not(stripped_widgets))]
fn encode_tuple_component(value: &crate::widget::capability::CapabilityValue, out: &mut String) {
    use crate::widget::capability::CapabilityValue;
    use core::fmt::Write as _;
    match value {
        CapabilityValue::Null => {
            let _ = write!(out, "0:");
        }
        CapabilityValue::Bool(flag) => {
            let _ = write!(out, "1:{}", u8::from(*flag));
        }
        CapabilityValue::Int(number) => {
            let _ = write!(out, "2:{number}");
        }
        CapabilityValue::UInt(number) => {
            let _ = write!(out, "3:{number}");
        }
        CapabilityValue::Float(number) => {
            let _ = write!(out, "4:{number}");
        }
        CapabilityValue::String(text) => {
            // The separator characters are escaped so a component containing `;` or `:` round-trips.
            // `|` and `=` are escaped too, because a record list uses them as structure and a string
            // field carrying one would otherwise be read as a record or a field boundary. Escaping a
            // character that needs no escaping is harmless: the decoder strips a backslash before any
            // character, so the value reads back unchanged.
            let _ = write!(out, "5:{}", escape_wire_separators(text));
        }
        // A nested colour/rect/tuple/record-list is not produced by any event payload today; encoding
        // it as its announcement-free debug form would be a silent lossy path, so it is written as an
        // empty-string component and the decoder refuses it. This is stated rather than silently
        // emitted.
        CapabilityValue::Color(_)
        | CapabilityValue::Rect(_)
        | CapabilityValue::IconSpec(_)
        | CapabilityValue::Tuple(_)
        | CapabilityValue::Records(_) => {
            let _ = write!(out, "9:");
        }
    }
}

#[cfg(not(stripped_widgets))]
fn encode_icon_spec(spec: &crate::widget::icon_source::IconSpec) -> String {
    use crate::widget::icon_source::{IconColorMode, IconSpec};
    match spec {
        IconSpec::Builtin(name) => alloc::format!("builtin|{}", name.as_str()),
        IconSpec::Resource { id, color_mode } => {
            let mode = match color_mode {
                IconColorMode::Monochrome => "monochrome",
                IconColorMode::Original => "original",
            };
            alloc::format!("resource|{mode}|{}", escape_icon_resource_id(id))
        }
    }
}

#[cfg(not(stripped_widgets))]
fn escape_icon_resource_id(id: &str) -> String {
    let mut out = String::new();
    for ch in id.chars() {
        match ch {
            '%' => out.push_str("%25"),
            '|' => out.push_str("%7C"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(not(stripped_widgets))]
fn decode_icon_spec(text: &str) -> Option<crate::widget::icon_source::IconSpec> {
    use crate::widget::icon_source::{IconColorMode, IconSpec};
    let (kind, rest) = text.split_once('|')?;
    match kind {
        "builtin" => {
            crate::widget::display_widgets::icon::IconName::from_name(rest).map(IconSpec::Builtin)
        }
        "resource" => {
            let (mode, encoded_id) = rest.split_once('|')?;
            if encoded_id.is_empty() {
                return None;
            }
            let color_mode = match mode {
                "monochrome" => IconColorMode::Monochrome,
                "original" => IconColorMode::Original,
                _ => return None,
            };
            let mut id = String::new();
            let mut chars = encoded_id.chars();
            while let Some(ch) = chars.next() {
                if ch != '%' {
                    id.push(ch);
                    continue;
                }
                let high = chars.next()?;
                let low = chars.next()?;
                match (high, low) {
                    ('2', '5') => id.push('%'),
                    ('7', 'C') => id.push('|'),
                    _ => return None,
                }
            }
            if id.trim().is_empty() {
                return None;
            }
            Some(IconSpec::Resource { id, color_mode })
        }
        _ => None,
    }
}

/// Rebuilds a [`CapabilityValue`] from the ABI's `(kind, num, str)` triple.
///
/// Returns `None` for an unrecognised `kind`, which the caller reports as a type
/// mismatch rather than silently writing a zero.
#[cfg(not(stripped_widgets))]
pub(crate) fn decode_capability_value(
    kind: c_int,
    num: i64,
    str_value: *const c_char,
) -> Option<crate::widget::capability::CapabilityValue> {
    use crate::widget::capability::CapabilityValue;
    match kind {
        RW_VALUE_NULL => Some(CapabilityValue::Null),
        RW_VALUE_BOOL => Some(CapabilityValue::Bool(num != 0)),
        RW_VALUE_INT => Some(CapabilityValue::Int(num)),
        RW_VALUE_UINT => Some(CapabilityValue::UInt(u64::try_from(num).ok()?)),
        RW_VALUE_FLOAT => Some(CapabilityValue::Float(f64::from_bits(num as u64))),
        // SAFETY: `str_value` is null or a NUL-terminated string supplied by the ABI caller.
        RW_VALUE_STRING => Some(CapabilityValue::String(unsafe {
            if str_value.is_null() {
                String::new()
            } else {
                CStr::from_ptr(str_value).to_string_lossy().into_owned()
            }
        })),
        // Parsing happens here rather than being deferred, so a malformed string is
        // refused at the ABI boundary. `None` becomes a type mismatch at the caller,
        // which is the honest answer: the caller passed something this property's
        // declared kind cannot represent.
        RW_VALUE_COLOR => Some(CapabilityValue::Color(
            // SAFETY: `str_value` follows the ABI contract: null or a NUL-terminated string valid for the call.
            crate::style::CssParser::parse_color(&unsafe { read_c_string(str_value) }).ok()?,
        )),
        RW_VALUE_RECT => {
            // SAFETY: `str_value` follows the ABI contract: null or a NUL-terminated string valid for the call.
            Some(CapabilityValue::Rect(parse_rect_string(&unsafe { read_c_string(str_value) })?))
        }
        RW_VALUE_ICON_SPEC => {
            // SAFETY: `str_value` follows the ABI contract: null or a NUL-terminated string valid for the call.
            Some(CapabilityValue::IconSpec(decode_icon_spec(&unsafe { read_c_string(str_value) })?))
        }
        RW_VALUE_TUPLE => {
            // SAFETY: `str_value` follows the ABI contract: null or a NUL-terminated string valid for the call.
            let text = unsafe { read_c_string(str_value) };
            let components = decode_tuple_components(&text)?;
            // `num` is the advertised component count; trusting it over the parsed length would let a
            // truncated string look complete, so a disagreement is refused rather than resolved.
            if components.len() as i64 != num {
                return None;
            }
            Some(CapabilityValue::Tuple(components))
        }
        RW_VALUE_RECORDS => {
            // SAFETY: `str_value` follows the ABI contract: null or a NUL-terminated string valid for the call.
            let text = unsafe { read_c_string(str_value) };
            let records = decode_record_list(&text)?;
            // Same reasoning as the tuple arm: the advertised record count must agree with what the
            // text actually held, so a truncated list is refused instead of looking complete.
            if records.len() as i64 != num {
                return None;
            }
            // The decoded list carries no schema: this boundary cannot name a `&'static` one, and
            // inventing one here would be policy the property never declared. The property the value
            // is written to supplies the schema — that is where the record shape, the identity field
            // and the uniqueness rule are checked.
            Some(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                &crate::widget::capability::EMPTY_RECORD_SCHEMA,
                records,
            )))
        }
        _ => None,
    }
}

/// Parses the `name=kind:value;…` per record, `|`-separated, wire form written by
/// [`encode_record_list`].
///
/// Returns `None` when any record, field or value is malformed — including a nested value, which the
/// encoder marks `9:` because the wire form cannot carry one.
#[cfg(not(stripped_widgets))]
fn decode_record_list(
    text: &str,
) -> Option<crate::compat::Vec<crate::widget::capability::RecordValue>> {
    use crate::widget::capability::RecordValue;
    // An empty string is an empty list, which is what the encoder writes for one (`num` is then 0).
    if text.is_empty() {
        return Some(crate::compat::Vec::new());
    }
    let mut out = crate::compat::Vec::new();
    for record in split_unescaped_on(text, b'|') {
        let mut value = RecordValue::new();
        for field in split_unescaped_on(record, b';') {
            let (name, component) = split_unescaped_once(field, '=')?;
            let (kind, raw) = split_unescaped_once(component, ':')?;
            let decoded = match kind {
                "0" => crate::widget::capability::CapabilityValue::Null,
                "1" => crate::widget::capability::CapabilityValue::Bool(raw != "0"),
                "2" => crate::widget::capability::CapabilityValue::Int(raw.parse().ok()?),
                "3" => crate::widget::capability::CapabilityValue::UInt(raw.parse().ok()?),
                "4" => crate::widget::capability::CapabilityValue::Float(raw.parse().ok()?),
                "5" => crate::widget::capability::CapabilityValue::String(
                    unescape_tuple_component(raw)?,
                ),
                // A nested value cannot be carried, so a `9:` marker is refused rather than decoded
                // into something the caller did not send.
                _ => return None,
            };
            value = value.with(unescape_tuple_component(name)?, decoded);
        }
        out.push(value);
    }
    Some(out)
}

/// Splits `text` at the first `separator` not preceded by a backslash.
///
/// The `|`/`;`/`=` separators a record wire string uses are all single characters, and a backslash
/// escapes the next character whatever it is — so one scanner serves all three and the escaping rule
/// stays the one [`unescape_tuple_component`] already implements.
#[cfg(not(stripped_widgets))]
fn split_unescaped_once(text: &str, separator: char) -> Option<(&str, &str)> {
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == separator {
            return Some((&text[..index], &text[index + ch.len_utf8()..]));
        }
    }
    None
}

/// Splits `text` on every unescaped `separator`, keeping the escapes in the returned pieces.
#[cfg(not(stripped_widgets))]
fn split_unescaped_on(text: &str, separator: u8) -> crate::compat::Vec<&str> {
    let mut parts = crate::compat::Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == separator => {
                parts.push(&text[start..index]);
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    parts.push(&text[start..]);
    parts
}

/// Parses the `kind:value;kind:value` wire form written by [`encode_tuple_component`], or `None`
/// when any component is malformed.
#[cfg(not(stripped_widgets))]
fn decode_tuple_components(
    text: &str,
) -> Option<crate::compat::Vec<crate::widget::capability::CapabilityValue>> {
    use crate::widget::capability::CapabilityValue;
    let mut out = crate::compat::Vec::new();
    for component in split_unescaped(text) {
        let (kind, raw) = component.split_once(':')?;
        let value = match kind {
            "0" => CapabilityValue::Null,
            "1" => CapabilityValue::Bool(raw != "0"),
            "2" => CapabilityValue::Int(raw.parse().ok()?),
            "3" => CapabilityValue::UInt(raw.parse().ok()?),
            "4" => CapabilityValue::Float(raw.parse().ok()?),
            "5" => CapabilityValue::String(unescape_tuple_component(raw)?),
            // `9` is the encoder's "nested value, not representable here" marker; refusing it keeps
            // a lossy write from round-tripping as a silent empty value.
            _ => return None,
        };
        out.push(value);
    }
    Some(out)
}

/// Splits a tuple wire string on **unescaped** `;`, leaving `\;` inside a component intact.
#[cfg(not(stripped_widgets))]
fn split_unescaped(text: &str) -> crate::compat::Vec<&str> {
    split_unescaped_on(text, b';')
}

/// Undoes the escaping [`encode_tuple_component`] applies to a string component.
#[cfg(not(stripped_widgets))]
fn unescape_tuple_component(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            out.push(chars.next()?);
        } else {
            out.push(ch);
        }
    }
    Some(out)
}

/// The `rw_value_kind` discriminants, named so the Rust side and the generated
/// header cannot drift apart.
pub(crate) const RW_VALUE_NULL: c_int = 0;

pub(crate) const RW_VALUE_BOOL: c_int = 1;

pub(crate) const RW_VALUE_INT: c_int = 2;

pub(crate) const RW_VALUE_UINT: c_int = 3;

pub(crate) const RW_VALUE_FLOAT: c_int = 4;

pub(crate) const RW_VALUE_STRING: c_int = 5;

// Appended after `RW_VALUE_STRING` so the earlier discriminants keep the values any
// existing binding already uses; a C caller that knows only 0..=5 is unaffected.
pub(crate) const RW_VALUE_COLOR: c_int = 6;

pub(crate) const RW_VALUE_RECT: c_int = 7;

/// A composite payload (a tuple/list of values). Appended after `RW_VALUE_RECT`; `num` carries the
/// component count and the string holds `kind:value;…`.
pub(crate) const RW_VALUE_TUPLE: c_int = 8;

/// A record list (BLUE26 B26.09). Appended after `RW_VALUE_TUPLE` so the earlier discriminants keep
/// the values any existing binding already uses; a C caller that knows only 0..=8 is unaffected.
///
/// `num` carries the record count and the string holds `name=kind:value;…` per record, with records
/// separated by `|`. The field names travel because a decoder at this boundary has no schema: the
/// property the value is written to is what supplies identity and uniqueness policy, and the record
/// shape is checked against it there.
pub(crate) const RW_VALUE_RECORDS: c_int = 9;

/// A persistable icon descriptor. `str` carries the canonical `builtin|name` or
/// `resource|color-mode|escaped-id` spelling.
pub(crate) const RW_VALUE_ICON_SPEC: c_int = 10;
