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
//! back. The pair is asserted by `tools/check_designer_manifest_roundtrip.sh`, which exports,
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
//! The value set here is closed: scalars, arrays of records, and the tagged IconSpec object. Writing
//! the escaping and key order by hand keeps the output **stable** (field order is the order in this
//! file, not the order a macro happened to emit) and **readable in a diff**, which matters because
//! these files are checked in by designers.
//!
//! # Stability is a contract, not an accident
//!
//! [`capability_manifest_json`] sorts controls by name and events by name, so the same capability
//! table always produces the same bytes. Without that, a round-trip test would pass and a
//! designer's diff would not: re-saving a project would reorder the file.

use super::designer_manifest_writer::write_manifest;
use super::types::{
    CapabilityValue, EventManifest, EventPayloadShape, PropertySchema, PropertyValueKind,
};

/// Re-exported at its historical path; the implementation lives with `CommandSchema`.
pub use super::command_schema::designer_command_from;
use crate::compat::{format, String, Vec};

/// The key names in the exported document.
///
/// Named constants rather than literals because the writer and the reader must agree exactly, and
/// a typo in one of them is a key that silently reads back as absent.
pub(super) mod keys {
    pub(crate) const CONTROL: &str = "control";
    /// The designer-palette grouping token for this control.
    pub(crate) const CATEGORY: &str = "category";
    pub(crate) const NAME: &str = "name";
    pub(crate) const ALIASES: &str = "aliases";
    /// The canonical control an alias row spells; empty for a canonical row.
    pub(crate) const ALIAS_OF: &str = "alias_of";
    /// The controls a companion is designed to sit beside (RW-107).
    pub(crate) const COMPANION_OF: &str = "companion_of";
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
    /// The protocol version this document is written in (BLUE26 B26.01).
    pub(crate) const PROTOCOL: &str = "protocol";
    /// The library version the document was exported from.
    pub(crate) const LIBRARY: &str = "library";
    /// The build profile the facts describe.
    pub(crate) const PROFILE: &str = "profile";
    /// The target triple the facts describe.
    pub(crate) const TRIPLE: &str = "triple";
    /// A command's parameter shape token (BLUE26 B26.03).
    pub(crate) const SIGNATURE: &str = "signature";
    /// A command's parameter name.
    pub(crate) const PARAMETER: &str = "parameter";
    /// A command's element kind.
    pub(crate) const ELEMENT: &str = "element";
    /// A command's payload arity.
    pub(crate) const ARITY: &str = "arity";
    /// A command's side-effect class.
    pub(crate) const EFFECT: &str = "effect";
    /// A property's constraint list (BLUE26 B26.02).
    pub(crate) const CONSTRAINTS: &str = "constraints";
    /// A constraint's inclusive minimum.
    pub(crate) const MINIMUM: &str = "minimum";
    /// A constraint's inclusive maximum.
    pub(crate) const MAXIMUM: &str = "maximum";
    /// A constraint's step.
    pub(crate) const STEP: &str = "step";
    /// A constraint's decimal places.
    pub(crate) const PLACES: &str = "places";
    /// A constraint's kind token.
    pub(crate) const RULE: &str = "rule";
    /// The child-kind constraints of a container control (RW-41).
    pub(crate) const CHILDREN: &str = "children";
    /// Whether a record field is mandatory.
    pub(crate) const REQUIRED: &str = "required";
    /// The field list of a record-list property, in the order a designer should offer them.
    pub(crate) const FIELDS: &str = "fields";
    /// A record schema's identity field name.
    pub(crate) const ID_FIELD: &str = "id_field";
    /// Whether two records in one list may share an id.
    pub(crate) const IDS_UNIQUE: &str = "ids_unique";
    /// Whether a record's id must be non-empty.
    pub(crate) const IDS_REQUIRED: &str = "ids_required";
}

/// The manifest protocol version this build writes and can read.
///
/// # Why this is a number of its own rather than the library version
///
/// The two move independently, and coupling them would make one of them lie. The library version
/// changes for any code change, including ones that do not touch this document; the protocol
/// version changes only when the document's *shape* does — a new key, a removed key, a changed
/// meaning. A reader uses the protocol version to decide whether it understands the document at
/// all, which is a question the library version cannot answer (2.8.5 and 2.9.0 could be the same
/// protocol, and two protocol versions could ship in one library version during a transition).
///
/// # Compatibility rule
///
/// A reader accepts a document whose protocol is **at or below** [`MANIFEST_PROTOCOL_VERSION`]:
/// older documents are readable by construction, because every version so far has only *added*
/// keys. A document with a **higher** protocol is refused explicitly
/// ([`ManifestParseError`]), never partially read — a partial read of a newer document is how a
/// designer silently drops a field it does not understand and re-saves a lossy file.
///
/// History:
/// * `1` — control, aliases, properties, events, commands.
/// * `2` — adds `protocol`, `library`, `profile` and `triple` (BLUE26 B26.01).
/// * `3` — adds structured IconSpec property defaults (BLUE27 U5).
/// * `4` — adds `alias_of` (COMP-27 alias identity). Absent in older documents and read as the
///   empty string, which means "this row is canonical".
/// * `5` — adds `category`, the designer-palette grouping token
///   ([`WidgetKind::designer_category`](crate::widget::WidgetKind::designer_category)). Absent in
///   older documents and read as the empty string, so a protocol-4 file still opens.
/// * `6` — adds `children`, the allowed-child-kind list for a container (RW-41). Absent in older
///   documents and read as an empty list. A new key is a change to the document's *shape*, which is
///   the stated criterion for a protocol bump: a v5 reader rejects the unknown key, so writing it
///   under v5 would make this build's output unreadable to itself without an explicit version.
/// * `7` — adds `commands[].tokens`, the accepted spellings of an enum command parameter (RW-103).
///   Absent in older documents and read as an empty list, so every v1–v6 file still opens; the key
///   is written under v7 for the same reason `children` was written under v6 — a v6 reader refuses
///   a key it does not know rather than dropping it on re-save.
/// * `8` — adds `companion_of`, the control(s) a companion is designed to sit beside (RW-107).
///   Absent in older documents and read as an empty list, which means "not a companion".
/// * `9` — adds a record property's `fields`/`id_field`/`ids_unique`/`ids_required`, so a
///   designer can render and validate a record-list default without consulting the control's
///   capability table. Absent in older documents and read as no schema.
pub const MANIFEST_PROTOCOL_VERSION: u32 = 9;

/// The oldest protocol version this reader still accepts.
///
/// Kept as an explicit number rather than implied by [`MANIFEST_PROTOCOL_VERSION`], because the
/// two answer different questions: one is "what do I write", the other is "what can I still
/// read". Dropping support for v1 would be a deliberate change here, not a side effect of
/// bumping the written version.
pub const MANIFEST_MIN_PROTOCOL_VERSION: u32 = 1;

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
        PropertyValueKind::IconSpec => "icon-spec",
        PropertyValueKind::Records => "records",
        PropertyValueKind::StringList => "string-list",
        PropertyValueKind::NumberList => "number-list",
        PropertyValueKind::ColorList => "color-list",
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
        "icon-spec" => Some(PropertyValueKind::IconSpec),
        "records" => Some(PropertyValueKind::Records),
        "string-list" => Some(PropertyValueKind::StringList),
        "number-list" => Some(PropertyValueKind::NumberList),
        "color-list" => Some(PropertyValueKind::ColorList),
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
    /// The runtime-enforced constraints, empty when the property has none (BLUE26 B26.02).
    ///
    /// A designer renders these directly: a `Range` becomes a bounded slider, a `Required` an
    /// asterisk. Only constraints the control actually enforces are present (see
    /// [`Constraint`](crate::widget::capability::Constraint)), so an editor built from them accepts
    /// exactly what the control accepts.
    pub constraints: Vec<DesignerConstraint>,
    /// The declared shape of one record when the property is a record list.
    ///
    /// `None` for every other kind. Without it the manifest can say `"kind": "records"` and
    /// nothing else, leaving a designer unable to render the fields or validate a hand-edited
    /// default, so protocol 9 publishes it.
    pub record_schema: Option<DesignerRecordSchema>,
}

/// One field of a record-list property's declared schema.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerRecordField {
    /// The field's name.
    pub name: String,
    /// The field's declared value kind.
    pub kind: PropertyValueKind,
    /// Whether every record must carry it.
    pub required: bool,
}

/// The declared shape of one record of a record-list property.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerRecordSchema {
    /// The fields, in declaration order.
    pub fields: Vec<DesignerRecordField>,
    /// The field that carries an item's identity.
    pub id_field: String,
    /// Whether two records in one list may share an id.
    pub ids_unique: bool,
    /// Whether a record's id must be non-empty.
    pub ids_required: bool,
}

/// One constraint as a designer sees it (BLUE26 B26.02).
///
/// A flat, tagged form rather than re-using the runtime [`Constraint`](crate::widget::capability::Constraint)
/// enum directly, because this is a *wire* type: it must round-trip through JSON with the rest of the
/// manifest, and a tagged object is the shape a reader can extend without renumbering an enum.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerConstraint {
    /// The constraint's kind: `"range"`, `"precision"`, or `"required"`.
    pub kind: &'static str,
    /// The inclusive minimum, for a `"range"` constraint.
    pub minimum: Option<f64>,
    /// The inclusive maximum, for a `"range"` constraint.
    pub maximum: Option<f64>,
    /// The step, for a `"range"` constraint that has one.
    pub step: Option<f64>,
    /// The decimal places, for a `"precision"` constraint.
    pub places: Option<u8>,
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

/// One command as the designer sees it: its name, its parameter shape and its effect
/// (BLUE26 B26.03).
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerCommand {
    /// The published command name.
    pub name: String,
    /// The parameter shape: `"none"`, `"value"`, or `"composite"`.
    pub signature: &'static str,
    /// The parameter name, empty for a payload-free command.
    pub parameter: String,
    /// The declared kind of a `"value"` parameter, empty for the other shapes.
    pub kind: Option<PropertyValueKind>,
    /// The accepted spellings of an `Enum` parameter, empty for every other kind.
    ///
    /// The command that writes a property and the property itself describe the same value, so a
    /// designer that can build a picker for `alignment` must be able to build the same picker for
    /// `set_alignment`'s argument. Protocol 6 carried only the kind, which left the panel with
    /// free text and a command that rejected illegal values (RW-103). The list is copied from the
    /// property's `accepted_tokens`, so the control's own declaration stays the single source
    /// (principles #54/#101).
    pub tokens: Vec<String>,
    /// How many values a `"composite"` payload carries, or `None` for a list.
    pub arity: Option<usize>,
    /// The declared kind of each composite element, when homogeneous.
    pub element: Option<PropertyValueKind>,
    /// The side-effect class: `"mutates-control"`, `"mutates-collection"`, `"side-effecting"`.
    pub effect: &'static str,
}

/// One control's full designer-facing description.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignerManifest {
    /// The capability-manifest protocol version this document is written in.
    ///
    /// Present on every manifest so a reader never has to guess: a document without it is
    /// protocol 1 by definition (see [`MANIFEST_PROTOCOL_VERSION`]), and one with a version
    /// above what this build writes is refused rather than half-read.
    pub protocol: u32,
    /// The library version that produced the document, for diagnostics and support.
    ///
    /// Not used to decide compatibility — that is [`Self::protocol`]'s job — but recorded so a
    /// bug report about a manifest can name the exact build it came from without a guess.
    pub library: String,
    /// The build profile the facts describe (BLUE26 B26.01/B26.06).
    ///
    /// The question this answers is "which build said so", which is exactly what is missing when
    /// a `desktop` build's `video_codec` support is read as a claim about a `mini` device.
    pub profile: String,
    /// The target triple the facts describe, as `build.rs` captured it.
    pub triple: String,
    /// The canonical control name.
    pub control: String,
    /// The designer-palette section this control belongs in.
    ///
    /// One token per control, from `WidgetKind::designer_category`. A palette that had only the
    /// control name could group by spelling (`l`, `li`, `lis`…), which is alphabetic rather than
    /// functional; this is the fact it needs instead. Empty when the document predates protocol 5
    /// — a reader that sees an empty string groups the row under "other" rather than guessing.
    pub category: String,
    /// Every name the factory also accepts for this control.
    pub aliases: Vec<String>,
    /// When this control is a second spelling of another, the canonical control it spells.
    ///
    /// Empty for a canonical row. Read from the crate's single `ALIAS_OF` registry so the
    /// manifest, the census and the converted-control gate answer the identity question from one
    /// table instead of three copies.
    pub alias_of: String,
    /// The properties it publishes.
    pub properties: Vec<DesignerProperty>,
    /// The events it publishes, with payload types.
    pub events: Vec<DesignerEvent>,
    /// The commands it accepts, with parameter shape and effect.
    pub commands: Vec<DesignerCommand>,
    /// The child kinds this control accepts, as canonical factory names (RW-41).
    ///
    /// Empty for a control that holds no children. For a container that accepts *any*
    /// constructible child the list is [`ANY_CHILD_WILDCARD`](super::types::ANY_CHILD_WILDCARD),
    /// because "unconstrained" is not the same fact as "no children" and an empty list cannot say
    /// both. A constrained container lists the **factory** names of its allowed kinds — a kind's
    /// factory spelling is the one a document can actually construct, so a dock widget lists
    /// `dock_widget` (the factory name of the dock-panel kind) rather than an unregistered variant
    /// spelling.
    pub children: Vec<String>,
    /// The canonical controls this control is designed to sit beside (RW-107).
    ///
    /// Empty for a control that stands alone. A companion — a transport bar, a chart data table, a
    /// map overview — has no meaning by itself, and a designer that knew only its category could
    /// insert it but not pair it. The list names the host(s) the companion was built for; it is a
    /// pairing fact, not proof that a specific `attach_*` method exists (see
    /// [`companion_hosts`]]).
    pub companion_of: Vec<String>,
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

impl core::error::Error for ManifestExportError {}

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
    /// The first non-whitespace byte after a `[`, without consuming anything.
    fn peek_after_open_bracket(&self) -> Option<u8> {
        let mut index = self.cursor + 1;
        while let Some(byte) = self.bytes.get(index) {
            if byte.is_ascii_whitespace() {
                index += 1;
            } else {
                return Some(*byte);
            }
        }
        None
    }

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

    /// Reads an integer or float, preserving an integer's full width.
    ///
    /// [`read_number`](Self::read_number) parses through `f64`, which cannot hold an integer above
    /// 2^53 exactly. A default is a value the document promises to restore, so it is read through
    /// `i64`/`u64` instead: `9007199254740993` must come back as itself rather than `…992`.
    fn read_number_value(&mut self) -> Result<JsonNumber, ManifestParseError> {
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
        if text.contains('.') || text.contains('e') || text.contains('E') {
            return match text.parse::<f64>() {
                Ok(value) => Ok(JsonNumber::Float(value)),
                Err(_) => self.fail("malformed number"),
            };
        }
        // A leading `+` was this writer's historical `Int` marker. It is accepted so a document
        // written by an older build still opens; the kind-based recovery below no longer needs it.
        let digits = text.strip_prefix('+').unwrap_or(text);
        if digits.starts_with('-') {
            return match digits.parse::<i64>() {
                Ok(value) => Ok(JsonNumber::Int(value)),
                Err(_) => self.fail("integer is out of range"),
            };
        }
        match digits.parse::<u64>() {
            Ok(value) => Ok(JsonNumber::UInt(value)),
            Err(_) => self.fail("integer is out of range"),
        }
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

    /// Reads a non-negative integer, for keys that are counts or versions.
    ///
    /// A dedicated reader rather than [`read_number`](Self::read_number) because a version is a
    /// `u32` and a parse through `f64` would lose precision above 2^53 and silently accept a
    /// fractional value (`2.5` would truncate). The digits are parsed directly, and anything that
    /// is not a plain run of digits — a sign, a fraction, an exponent — is a malformed document.
    fn read_uint(&mut self) -> Result<u32, ManifestParseError> {
        self.skip_whitespace();
        let start = self.cursor;
        while self.bytes.get(self.cursor).is_some_and(u8::is_ascii_digit) {
            self.cursor += 1;
        }
        if start == self.cursor {
            return self.fail("expected a non-negative integer");
        }
        let Some(text) = core::str::from_utf8(&self.bytes[start..self.cursor]).ok() else {
            return self.fail("number is not ASCII");
        };
        text.parse::<u32>().map_err(|_| ManifestParseError {
            detail: format!("`{text}` is not a valid version number"),
        })
    }

    /// Whether the number at the cursor is written with a **leading `+`**.
    ///
    /// # Why the sign is asked about separately
    ///
    /// [`read_number`](Self::read_number) returns a magnitude and nothing else, because that is
    /// all a `f64` can carry. But JSON has no signed/unsigned distinction, so the magnitude alone
    /// cannot say whether `100` means `Int(100)` or `UInt(100)` — and the writer emits the `UInt`
    /// spelling for every non-negative value in the capability table, so the reader must be able to
    /// tell the two apart or `Int(100)` reloads as a different kind than it was written as. This is
    /// the other half of [`push_json_int`]: the marker is written there and observed here.
    ///
    /// It looks ahead without consuming, so it can be asked before or after `read_number` as long
    /// as the cursor has not moved past the token.
    fn peek_leading_plus(&mut self) -> bool {
        self.skip_whitespace();
        self.bytes.get(self.cursor) == Some(&b'+')
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

/// A number token as the writer spelled it.
///
/// Integral values keep their exact magnitude; a float goes through `f64`. See
/// [`JsonReader::read_number_value`].
enum JsonNumber {
    Int(i64),
    UInt(u64),
    Float(f64),
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
    let mut constraints = Vec::new();
    let mut default_value = CapabilityValue::Null;
    let mut saw_default = false;
    let mut fields: Option<Vec<DesignerRecordField>> = None;
    let mut id_field = String::new();
    let mut ids_unique = false;
    let mut ids_required = false;

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
            k if k == keys::CONSTRAINTS => constraints = read_constraints(reader)?,
            k if k == keys::DEFAULT => {
                default_value = read_value(reader)?;
                saw_default = true;
            }
            k if k == keys::FIELDS => fields = Some(read_record_fields(reader)?),
            k if k == keys::ID_FIELD => id_field = reader.read_string()?,
            k if k == keys::IDS_UNIQUE => ids_unique = reader.read_bool()?,
            k if k == keys::IDS_REQUIRED => ids_required = reader.read_bool()?,
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

    let kind = kind.ok_or(ManifestParseError { detail: "a property has no kind".into() })?;
    let record_schema =
        fields.map(|fields| DesignerRecordSchema { fields, id_field, ids_unique, ids_required });
    // The writer no longer marks `Int` with a leading `+` (RFC 8259 has no such token), so the
    // declared kind is what separates the two carriers. Normalising here is what keeps
    // `Int(0)`/`Int(100)` — Slider's own defaults — from reloading as `UInt`.
    let default_value = if saw_default {
        normalize_default_value(default_value, kind, record_schema.as_ref())
    } else {
        CapabilityValue::Null
    };
    if !manifest_default_matches_kind(&default_value, kind) {
        return Err(ManifestParseError {
            detail: format!(
                "property `{}` declares `{}` but its default is not that kind",
                name.as_deref().unwrap_or(""),
                value_kind_token(kind)
            ),
        });
    }

    Ok(DesignerProperty {
        name: name.ok_or(ManifestParseError { detail: "a property has no name".into() })?,
        kind,
        readable: readable.unwrap_or(false),
        writable: writable.unwrap_or(false),
        tokens,
        constraints,
        default_value,
        record_schema,
    })
}

/// Reads a record schema's field list.
fn read_record_fields(
    reader: &mut JsonReader<'_>,
) -> Result<Vec<DesignerRecordField>, ManifestParseError> {
    reader.expect(b'[', "expected `[` to start a record field list")?;
    let mut fields = Vec::new();
    if reader.peek(b']') {
        reader.expect(b']', "expected `]`")?;
        return Ok(fields);
    }
    loop {
        reader.expect(b'{', "expected `{` to start a record field")?;
        let mut name = None;
        let mut kind = None;
        let mut required = false;
        loop {
            let key = reader.read_key()?;
            match key.as_str() {
                k if k == keys::NAME => name = Some(reader.read_string()?),
                k if k == keys::KIND => {
                    let token = reader.read_string()?;
                    kind = Some(value_kind_from_token(&token).ok_or(ManifestParseError {
                        detail: format!("unknown record field kind `{token}`"),
                    })?);
                }
                k if k == keys::REQUIRED => required = reader.read_bool()?,
                other => {
                    return Err(ManifestParseError {
                        detail: format!("unexpected key `{other}` in a record field"),
                    })
                }
            }
            if reader.peek(b',') {
                reader.expect(b',', "expected `,`")?;
                continue;
            }
            reader.expect(b'}', "expected `}` or `,` in a record field")?;
            break;
        }
        fields.push(DesignerRecordField {
            name: name.ok_or(ManifestParseError { detail: "a record field has no name".into() })?,
            kind: kind.ok_or(ManifestParseError { detail: "a record field has no kind".into() })?,
            required,
        });
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b']', "expected `]` or `,` in a record field list")?;
        return Ok(fields);
    }
}

/// Reads a constraint array (BLUE26 B26.02).
fn read_constraints(
    reader: &mut JsonReader<'_>,
) -> Result<Vec<DesignerConstraint>, ManifestParseError> {
    reader.expect(b'[', "expected `[` to start a constraint list")?;
    let mut constraints = Vec::new();
    if reader.peek(b']') {
        reader.expect(b']', "expected `]`")?;
        return Ok(constraints);
    }
    loop {
        constraints.push(read_constraint(reader)?);
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b']', "expected `]` or `,` in a constraint list")?;
        return Ok(constraints);
    }
}

/// Reads one constraint object (BLUE26 B26.02).
fn read_constraint(reader: &mut JsonReader<'_>) -> Result<DesignerConstraint, ManifestParseError> {
    reader.expect(b'{', "expected `{` to start a constraint")?;
    let mut kind: Option<&'static str> = None;
    let mut minimum = None;
    let mut maximum = None;
    let mut step = None;
    let mut places = None;

    loop {
        let key = reader.read_key()?;
        match key.as_str() {
            k if k == keys::RULE => {
                let token = reader.read_string()?;
                kind = Some(constraint_rule_token(&token).ok_or(ManifestParseError {
                    detail: format!("unknown constraint kind `{token}`"),
                })?);
            }
            k if k == keys::MINIMUM => minimum = read_optional_float(reader)?,
            k if k == keys::MAXIMUM => maximum = read_optional_float(reader)?,
            k if k == keys::STEP => step = read_optional_float(reader)?,
            k if k == keys::PLACES => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    let raw = reader.read_uint()?;
                    let count = u8::try_from(raw).map_err(|_| ManifestParseError {
                        detail: format!("`{raw}` is too large for a decimal-place count"),
                    })?;
                    places = Some(count);
                }
            }
            other => {
                return Err(ManifestParseError {
                    detail: format!("unexpected key `{other}` in a constraint"),
                })
            }
        }
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b'}', "expected `}` or `,` in a constraint")?;
        break;
    }

    Ok(DesignerConstraint {
        kind: kind.ok_or(ManifestParseError { detail: "a constraint has no kind".into() })?,
        minimum,
        maximum,
        step,
        places,
    })
}

/// Reads an optional float, consuming `null` as `None`.
fn read_optional_float(reader: &mut JsonReader<'_>) -> Result<Option<f64>, ManifestParseError> {
    if reader.peek(b'n') {
        reader.read_null()?;
        return Ok(None);
    }
    let (value, _is_float) = reader.read_number()?;
    Ok(Some(value))
}

/// The constraint-rule tokens a document may carry, and their inverse.
fn constraint_rule_token(token: &str) -> Option<&'static str> {
    match token {
        "range" => Some("range"),
        "precision" => Some("precision"),
        "required" => Some("required"),
        _ => None,
    }
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

/// Reads one command object (BLUE26 B26.03).
///
/// The signature token is validated against the closed set the writer emits, so a document naming
/// a signature this build does not understand is refused rather than read as a payload-free command
/// — which would tell a designer a wire to a value-taking command is legal.
fn read_command(reader: &mut JsonReader<'_>) -> Result<DesignerCommand, ManifestParseError> {
    reader.expect(b'{', "expected `{` to start a command")?;
    let mut name = None;
    let mut signature: Option<&'static str> = None;
    let mut parameter = String::new();
    let mut kind = None;
    let mut tokens: Vec<String> = Vec::new();
    let mut element = None;
    let mut arity = None;
    let mut effect: Option<&'static str> = None;

    loop {
        let key = reader.read_key()?;
        match key.as_str() {
            k if k == keys::NAME => name = Some(reader.read_string()?),
            k if k == keys::SIGNATURE => {
                let token = reader.read_string()?;
                signature = Some(command_signature_token(&token).ok_or(ManifestParseError {
                    detail: format!("unknown command signature `{token}`"),
                })?);
            }
            k if k == keys::PARAMETER => parameter = reader.read_string()?,
            k if k == keys::KIND => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    let token = reader.read_string()?;
                    kind = Some(value_kind_from_token(&token).ok_or(ManifestParseError {
                        detail: format!("unknown command parameter kind `{token}`"),
                    })?);
                }
            }
            k if k == keys::TOKENS => tokens = read_string_array(reader)?,
            k if k == keys::ELEMENT => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    let token = reader.read_string()?;
                    element = Some(value_kind_from_token(&token).ok_or(ManifestParseError {
                        detail: format!("unknown command element kind `{token}`"),
                    })?);
                }
            }
            k if k == keys::ARITY => {
                if reader.peek(b'n') {
                    reader.read_null()?;
                } else {
                    arity = Some(reader.read_uint()? as usize);
                }
            }
            k if k == keys::EFFECT => {
                let token = reader.read_string()?;
                effect = Some(command_effect_token(&token).ok_or(ManifestParseError {
                    detail: format!("unknown command effect `{token}`"),
                })?);
            }
            other => {
                return Err(ManifestParseError {
                    detail: format!("unexpected key `{other}` in a command"),
                })
            }
        }
        if reader.peek(b',') {
            reader.expect(b',', "expected `,`")?;
            continue;
        }
        reader.expect(b'}', "expected `}` or `,` in a command")?;
        break;
    }

    Ok(DesignerCommand {
        name: name.ok_or(ManifestParseError { detail: "a command has no name".into() })?,
        signature: signature
            .ok_or(ManifestParseError { detail: "a command has no signature".into() })?,
        parameter,
        kind,
        tokens,
        element,
        arity,
        effect: effect.ok_or(ManifestParseError { detail: "a command has no effect".into() })?,
    })
}

/// The signature tokens a command document may carry, and their inverse.
///
/// A `&'static str` rather than the `CommandSignature` enum because the two live in different
/// layers: this module owns the wire spelling, `command_schema` owns the semantics. The mapping is
/// the join between them, kept closed so an unknown token is an error.
fn command_signature_token(token: &str) -> Option<&'static str> {
    match token {
        "none" => Some("none"),
        "value" => Some("value"),
        "composite" => Some("composite"),
        _ => None,
    }
}

/// The effect tokens a command document may carry, and their inverse.
fn command_effect_token(token: &str) -> Option<&'static str> {
    match token {
        "mutates-control" => Some("mutates-control"),
        "mutates-collection" => Some("mutates-collection"),
        "side-effecting" => Some("side-effecting"),
        _ => None,
    }
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
    if reader.peek(b'{') {
        return read_icon_spec(reader).map(CapabilityValue::IconSpec);
    }
    // A JSON array is either a record list (`[{...}]`) or a plain scalar list (`["a"]`, `[1,2]`).
    // The writer emits both; deciding by the first element keeps the round trip for `StringList`
    // and `NumberList` defaults (swatch colours, chart-tooltip offsets, axis ranges).
    if reader.peek(b'[') {
        let first = reader.peek_after_open_bracket();
        if first == Some(b'{') {
            return Ok(CapabilityValue::Records(read_record_list(reader)?));
        }
        let items = read_scalar_list(reader)?;
        return Ok(CapabilityValue::Tuple(items.into_iter().collect()));
    }
    let leading_plus = reader.peek_leading_plus();
    match reader.read_number_value()? {
        JsonNumber::Float(number) => Ok(CapabilityValue::Float(number)),
        JsonNumber::Int(number) => Ok(CapabilityValue::Int(number)),
        // A non-negative integral value is `Int` only when the old writer's `+` marker says so.
        // The property's declared kind decides it otherwise — see `normalize_default_value`.
        JsonNumber::UInt(number) => {
            if leading_plus {
                match i64::try_from(number) {
                    Ok(number) => Ok(CapabilityValue::Int(number)),
                    Err(_) => reader.fail("an `Int` default does not fit in `i64`"),
                }
            } else {
                Ok(CapabilityValue::UInt(number))
            }
        }
    }
}

/// Recovers the carrier the property's declared kind implies.
///
/// The writer emits standard JSON, so a non-negative integer has one spelling for both `Int` and
/// `UInt`. The declared kind is the fact that separates them. `Number` deliberately keeps whichever
/// carrier the document spelled: it accepts both by definition, and the byte round trip is stable
/// either way.
fn normalize_default_value(
    value: CapabilityValue,
    kind: PropertyValueKind,
    record_schema: Option<&DesignerRecordSchema>,
) -> CapabilityValue {
    match (kind, value) {
        (PropertyValueKind::Int, CapabilityValue::UInt(number)) => match i64::try_from(number) {
            Ok(number) => CapabilityValue::Int(number),
            Err(_) => CapabilityValue::UInt(number),
        },
        (PropertyValueKind::UInt, CapabilityValue::Int(number)) if number >= 0 => {
            CapabilityValue::UInt(number as u64)
        }
        // An empty record list and an empty scalar list have the same JSON spelling (`[]`), so
        // the value reader cannot tell them apart. The declared kind is the fact that decides.
        (PropertyValueKind::Records, CapabilityValue::Tuple(items)) if items.is_empty() => {
            CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                &crate::widget::capability::EMPTY_RECORD_SCHEMA,
                crate::compat::Vec::new(),
            ))
        }
        (PropertyValueKind::Records, CapabilityValue::Records(list)) => {
            let Some(schema) = record_schema else {
                return CapabilityValue::Records(list);
            };
            let items = list
                .items()
                .iter()
                .map(|record| {
                    let mut normalized = crate::widget::capability::RecordValue::new();
                    for (name, value) in record.fields() {
                        let field_kind = schema
                            .fields
                            .iter()
                            .find(|field| field.name == *name)
                            .map(|field| field.kind);
                        let value = match field_kind {
                            Some(field_kind) => {
                                normalize_default_value(value.clone(), field_kind, None)
                            }
                            None => value.clone(),
                        };
                        normalized = normalized.with(name.clone(), value);
                    }
                    normalized
                })
                .collect();
            CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                &crate::widget::capability::EMPTY_RECORD_SCHEMA,
                items,
            ))
        }
        (PropertyValueKind::ColorList, CapabilityValue::Tuple(items)) => {
            let items = items
                .into_iter()
                .map(|item| match item {
                    CapabilityValue::String(text) => match crate::core::Color::parse_hex(&text) {
                        Some(color) => CapabilityValue::Color(color),
                        None => CapabilityValue::String(text),
                    },
                    other => other,
                })
                .collect();
            CapabilityValue::Tuple(items)
        }
        (_, value) => value,
    }
}

/// Whether a manifest default is a legal value for the declared kind.
///
/// The manifest spells a `Color` and a `Rect` as their wire strings (see [`read_value`]), so a
/// string default is the documented form for those kinds. Numeric widening is the same one
/// [`coercion`](super::coercion) defines: an integer fits a float, an in-range unsigned fits a
/// signed integer, and `Number` accepts both carriers. `Null` is the documented "not set" and is
/// accepted everywhere.
fn manifest_default_matches_kind(value: &CapabilityValue, kind: PropertyValueKind) -> bool {
    if matches!(value, CapabilityValue::Null) {
        return true;
    }
    match kind {
        PropertyValueKind::Bool => matches!(value, CapabilityValue::Bool(_)),
        PropertyValueKind::Int => match value {
            CapabilityValue::Int(_) => true,
            CapabilityValue::UInt(number) => i64::try_from(*number).is_ok(),
            _ => false,
        },
        PropertyValueKind::UInt => match value {
            CapabilityValue::UInt(_) => true,
            CapabilityValue::Int(number) => *number >= 0,
            _ => false,
        },
        PropertyValueKind::Float | PropertyValueKind::Number => matches!(
            value,
            CapabilityValue::Float(_) | CapabilityValue::Int(_) | CapabilityValue::UInt(_)
        ),
        PropertyValueKind::String | PropertyValueKind::Enum => {
            matches!(value, CapabilityValue::String(_))
        }
        PropertyValueKind::Color | PropertyValueKind::Rect => {
            matches!(
                value,
                CapabilityValue::Color(_) | CapabilityValue::Rect(_) | CapabilityValue::String(_)
            )
        }
        PropertyValueKind::IconSpec => matches!(value, CapabilityValue::IconSpec(_)),
        PropertyValueKind::Records => matches!(value, CapabilityValue::Records(_)),
        // `StringList` also carries the two-dimensional list the vocabulary has no kind for
        // (`chart_data_table.rows`, `axis_picker.edges`' `[edge, label, auto_range]` triples), so an
        // element is a string or one row of scalar cells. A bare number is still refused.
        PropertyValueKind::StringList => matches!(
            value,
            CapabilityValue::Tuple(items)
                if items.iter().all(|item| match item {
                    CapabilityValue::String(_) => true,
                    CapabilityValue::Tuple(row) => row.iter().all(|cell| !matches!(
                        cell,
                        CapabilityValue::Null
                            | CapabilityValue::Tuple(_)
                            | CapabilityValue::Records(_)
                            | CapabilityValue::IconSpec(_)
                    )),
                    _ => false,
                })
        ),
        PropertyValueKind::NumberList => matches!(
            value,
            CapabilityValue::Tuple(items)
                if items.iter().all(|item| matches!(
                    item,
                    CapabilityValue::Float(_) | CapabilityValue::Int(_) | CapabilityValue::UInt(_)
                ))
        ),
        PropertyValueKind::ColorList => matches!(
            value,
            CapabilityValue::Tuple(items)
                if items.iter().all(|item| matches!(item, CapabilityValue::Color(_)))
        ),
    }
}

/// Reads a JSON array of scalar values into a `Tuple`.
fn read_scalar_list(
    reader: &mut JsonReader<'_>,
) -> Result<Vec<CapabilityValue>, ManifestParseError> {
    reader.expect(b'[', "expected `[` to start a list")?;
    let mut items = Vec::new();
    if reader.peek(b']') {
        reader.expect(b']', "expected `]`")?;
        return Ok(items);
    }
    loop {
        items.push(read_value(reader)?);
        if reader.peek(b',') {
            reader.expect(b',', "expected `,` between list items")?;
            continue;
        }
        reader.expect(b']', "expected `]` to close a list")?;
        return Ok(items);
    }
}

fn read_icon_spec(
    reader: &mut JsonReader<'_>,
) -> Result<crate::widget::icon_source::IconSpec, ManifestParseError> {
    use crate::widget::icon_source::{IconColorMode, IconSpec};

    reader.expect(b'{', "expected `{` to start an icon source")?;
    let mut kind = None;
    let mut name = None;
    let mut id = None;
    let mut color_mode = None;
    loop {
        let key = reader.read_key()?;
        let slot = match key.as_str() {
            "kind" => &mut kind,
            "name" => &mut name,
            "id" => &mut id,
            "color_mode" => &mut color_mode,
            _ => {
                return reader.fail(&format!("unexpected icon source field `{key}`"));
            }
        };
        if slot.is_some() {
            return reader.fail(&format!("duplicate icon source field `{key}`"));
        }
        *slot = Some(reader.read_string()?);
        if reader.peek(b',') {
            reader.expect(b',', "expected `,` in icon source")?;
            continue;
        }
        reader.expect(b'}', "expected `}` or `,` in icon source")?;
        break;
    }
    match kind.as_deref() {
        Some("builtin") if id.is_none() && color_mode.is_none() => {
            let token = name.ok_or_else(|| ManifestParseError {
                detail: "a built-in icon source has no name".into(),
            })?;
            let icon = crate::widget::display_widgets::icon::IconName::from_name(&token)
                .ok_or_else(|| ManifestParseError {
                    detail: format!("unknown built-in icon `{token}`"),
                })?;
            Ok(IconSpec::Builtin(icon))
        }
        Some("resource") if name.is_none() => {
            let resource_id = id.ok_or_else(|| ManifestParseError {
                detail: "a resource icon source has no id".into(),
            })?;
            if resource_id.trim().is_empty() {
                return reader.fail("a resource icon source has an empty id");
            }
            let mode = match color_mode.as_deref() {
                Some("monochrome") => IconColorMode::Monochrome,
                Some("original") => IconColorMode::Original,
                _ => return reader.fail("a resource icon source has an invalid color_mode"),
            };
            Ok(IconSpec::Resource { id: resource_id, color_mode: mode })
        }
        Some("builtin" | "resource") => reader.fail("icon source fields conflict with its kind"),
        Some(other) => reader.fail(&format!("unknown icon source kind `{other}`")),
        None => reader.fail("icon source has no kind"),
    }
}

/// Reads a JSON array of objects as a record list (B26.09).
///
/// # What it accepts, and why it is strict
///
/// One object per record, each object's keys becoming that record's fields in document order, and each
/// field value read by [`read_value`] — so a nested array or object inside a record is refused by the
/// same reader that refuses one anywhere else. Strictness is the point: this reader's contract is that
/// the document it half-understands is one whose next export would silently differ, so an element that
/// is not an object is an error rather than a skipped element.
///
/// # Why the list carries no schema
///
/// The manifest describes properties, and this reader has no schema in hand — the same situation the
/// JSON loader and the C ABI are in. The list therefore carries
/// [`EMPTY_RECORD_SCHEMA`](crate::widget::capability::EMPTY_RECORD_SCHEMA) and the *property* is what
/// supplies the record shape; see
/// [`RecordList::validate_against`](crate::widget::capability::RecordList::validate_against).
fn read_record_list(
    reader: &mut JsonReader<'_>,
) -> Result<crate::widget::capability::RecordList, ManifestParseError> {
    use crate::widget::capability::{RecordList, RecordValue};
    reader.expect(b'[', "expected `[` to start a record list")?;
    let mut items = Vec::new();
    if reader.peek(b']') {
        reader.expect(b']', "expected `]`")?;
        return Ok(RecordList::from_items(&crate::widget::capability::EMPTY_RECORD_SCHEMA, items));
    }
    loop {
        reader.expect(b'{', "expected `{` to start a record")?;
        let mut record = RecordValue::new();
        if !reader.peek(b'}') {
            loop {
                let name = reader.read_string()?;
                reader.expect(b':', "expected `:` after a field name")?;
                record = record.with(name, read_value(reader)?);
                if reader.peek(b',') {
                    reader.expect(b',', "expected `,` between fields")?;
                    continue;
                }
                break;
            }
        }
        reader.expect(b'}', "expected `}` to end a record")?;
        items.push(record);
        if reader.peek(b',') {
            reader.expect(b',', "expected `,` between records")?;
            continue;
        }
        reader.expect(b']', "expected `]` to end a record list")?;
        break;
    }
    Ok(RecordList::from_items(&crate::widget::capability::EMPTY_RECORD_SCHEMA, items))
}

impl DesignerManifest {
    /// Reads a document written by [`capability_manifest_json`].    ///
    /// Every key this writer emits is required **except** `alias_of`, which was added in protocol 4
    /// and defaults to the empty string so a protocol-3 document still reads. An unknown key is an
    /// error: a document that this reader half-understands is one whose next export would silently
    /// differ from its input, which is exactly the failure the round-trip check exists to catch.
    pub fn from_json(text: &str) -> Result<Self, ManifestParseError> {
        let mut reader = JsonReader::new(text);
        reader.expect(b'{', "expected `{` to start a document")?;

        // Protocol first, but a writer may order keys freely, so the version check happens
        // **after** the loop: a document that names a future protocol anywhere is refused as a
        // whole rather than read until the offending key and then abandoned, which would leave a
        // caller holding a half-built manifest.
        let mut protocol: Option<u32> = None;
        let mut library = String::new();
        let mut profile = String::new();
        let mut triple = String::new();
        let mut control = None;
        let mut category = String::new();
        let mut aliases = Vec::new();
        let mut alias_of = String::new();
        let mut companion_of = Vec::new();
        let mut properties = Vec::new();
        let mut events = Vec::new();
        let mut commands = Vec::new();
        let mut children = Vec::new();

        loop {
            let key = reader.read_key()?;
            match key.as_str() {
                k if k == keys::PROTOCOL => protocol = Some(reader.read_uint()?),
                k if k == keys::LIBRARY => library = reader.read_string()?,
                k if k == keys::PROFILE => profile = reader.read_string()?,
                k if k == keys::TRIPLE => triple = reader.read_string()?,
                k if k == keys::CONTROL => control = Some(reader.read_string()?),
                // Optional so a protocol-4 document still reads; an absent category is the empty
                // string, which a palette renders as "other" instead of inventing a section.
                k if k == keys::CATEGORY => category = reader.read_string()?,
                k if k == keys::ALIASES => aliases = read_string_array(&mut reader)?,
                // Optional so a document written before the identity field existed still reads:
                // an absent `alias_of` means "this is a canonical row".
                k if k == keys::ALIAS_OF => alias_of = reader.read_string()?,
                // Optional so a protocol-7 document still reads: an absent list means "this
                // control is not a companion".
                k if k == keys::COMPANION_OF => companion_of = read_string_array(&mut reader)?,
                // Optional so a protocol-5 document still reads: an absent list is the empty one,
                // which is the same answer a leaf control gives.
                k if k == keys::CHILDREN => children = read_string_array(&mut reader)?,
                k if k == keys::COMMANDS => {
                    reader.expect(b'[', "expected `[` to start the command list")?;
                    if !reader.peek(b']') {
                        loop {
                            commands.push(read_command(&mut reader)?);
                            if reader.peek(b',') {
                                reader.expect(b',', "expected `,`")?;
                                continue;
                            }
                            break;
                        }
                    }
                    reader.expect(b']', "expected `]` after the command list")?;
                }
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

        // A document with no `protocol` key is protocol 1 by definition: every document written
        // before B26.01 lacked it, and reading them as v1 is what keeps an old project openable.
        let protocol = protocol.unwrap_or(MANIFEST_MIN_PROTOCOL_VERSION);
        if protocol > MANIFEST_PROTOCOL_VERSION {
            return Err(ManifestParseError {
                detail: format!(
                    "manifest protocol {protocol} is newer than this build understands \
                     ({MANIFEST_PROTOCOL_VERSION}); refusing to read it partially"
                ),
            });
        }

        // A duplicate name makes one entry unreachable by name while the document still exports
        // both, so a caller that edited one would silently keep the other. Refused here rather
        // than left for the first name-based lookup to resolve arbitrarily.
        reject_duplicate_names(&properties, "property", |entry| entry.name.as_str())?;
        reject_duplicate_names(&events, "event", |entry| entry.name.as_str())?;
        reject_duplicate_names(&commands, "command", |entry| entry.name.as_str())?;

        Ok(Self {
            protocol,
            library,
            profile,
            triple,
            control: control.ok_or(ManifestParseError {
                detail: "the document does not name its control".into(),
            })?,
            category,
            aliases,
            alias_of,
            companion_of,
            properties,
            events,
            commands,
            children,
        })
    }
}

/// Refuses two entries in one list that carry the same name.
fn reject_duplicate_names<T, F>(items: &[T], what: &str, name: F) -> Result<(), ManifestParseError>
where
    F: Fn(&T) -> &str,
{
    for (index, item) in items.iter().enumerate() {
        if items[..index].iter().any(|earlier| name(earlier) == name(item)) {
            return Err(ManifestParseError {
                detail: format!("duplicate {what} name `{}`", name(item)),
            });
        }
    }
    Ok(())
}

/// Serialises a record schema, so the manifest carries more than the `records` token.
#[cfg(full_widgets)]
fn designer_record_schema_from(
    schema: &crate::widget::capability::RecordSchema,
) -> DesignerRecordSchema {
    DesignerRecordSchema {
        fields: schema
            .fields
            .iter()
            .map(|field| DesignerRecordField {
                name: String::from(field.name),
                kind: field.kind,
                required: field.required,
            })
            .collect(),
        id_field: String::from(schema.id_field),
        ids_unique: schema.ids_unique,
        ids_required: schema.ids_required,
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
    let mut declared: crate::compat::Vec<&'static str> =
        crate::compat::Vec::with_capacity(capability.properties.len());
    for schema in capability.properties {
        // Resolved through `default_property_value` rather than the kind-keyed table: a control
        // that shares a `WidgetKind` with its owner answers some defaults by canonical name (the
        // popup host's `items`, the palette host's `entries`, and the COMP companions), and the
        // kind-keyed lookup cannot see them — which made those controls fail to export at all.
        let default_value = factory
            .default_property_value(capability.canonical_name, schema.name)
            .map_err(|_| ManifestExportError::IncompleteDefaults)?;
        declared.push(schema.name);
        properties.push(DesignerProperty {
            name: schema.name.into(),
            kind: schema.value_kind,
            readable: schema.readable,
            writable: schema.writable,
            tokens: schema.accepted_tokens.iter().map(|token| String::from(*token)).collect(),
            constraints: schema.constraints.iter().map(designer_constraint_from).collect(),
            record_schema: schema.record_schema.map(designer_record_schema_from),
            default_value,
        });
    }
    // RW-35: shared rows a control's own table does not repeat (`a11y_role`). A control's schema is
    // its own declaration, but the shared rows are part of its contract too, and a designer that
    // reads only the doc would otherwise not offer them.
    for schema in crate::widget::capability::properties_trait::BASE_PROPERTY_SCHEMAS {
        if declared.contains(&schema.name) {
            continue;
        }
        let default_value = factory
            .default_property_value(capability.canonical_name, schema.name)
            .map_err(|_| ManifestExportError::IncompleteDefaults)?;
        properties.push(DesignerProperty {
            name: schema.name.into(),
            kind: schema.value_kind,
            readable: schema.readable,
            writable: schema.writable,
            tokens: Vec::new(),
            constraints: Vec::new(),
            record_schema: None,
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

    let mut commands: Vec<DesignerCommand> = capability
        .commands
        .iter()
        .filter_map(|command| {
            super::command_schema::command_schema(command, capability.properties).map(|schema| {
                super::command_schema::designer_command_from(
                    command,
                    &schema,
                    capability.properties,
                )
            })
        })
        .collect();
    commands.sort_by(|left, right| left.name.cmp(&right.name));

    let aliases = crate::widget::capability::manifest_aliases(capability);
    let children = crate::widget::capability::child_factory_names(capability);

    Ok(DesignerManifest {
        protocol: MANIFEST_PROTOCOL_VERSION,
        library: String::from(env!("CARGO_PKG_VERSION")),
        profile: String::from(crate::widget::capability::TargetSpec::current().profile),
        triple: String::from(crate::widget::capability::TargetSpec::current().triple),
        control: capability.canonical_name.into(),
        category: String::from(capability.kind.designer_category()),
        aliases,
        alias_of: crate::widget::capability::resolve(capability.canonical_name)
            .map(String::from)
            .unwrap_or_default(),
        properties,
        events,
        commands,
        children,
        companion_of: companion_hosts(capability.canonical_name)
            .iter()
            .map(|host| String::from(*host))
            .collect(),
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

/// Builds a [`DesignerConstraint`] from a runtime [`Constraint`](crate::widget::capability::Constraint).
///
/// The join between the two layers, kept here so the wire vocabulary (`"range"`/`"precision"`/
/// `"required"`) lives with the rest of the document's vocabulary while the semantics stay in
/// `types`. A variant added to the runtime enum without a case here fails to compile, which is the
/// property that keeps the two from drifting.
pub fn designer_constraint_from(
    constraint: &crate::widget::capability::Constraint,
) -> DesignerConstraint {
    use crate::widget::capability::Constraint as C;
    match constraint {
        C::Range(range) => DesignerConstraint {
            kind: "range",
            minimum: Some(range.minimum),
            maximum: Some(range.maximum),
            step: range.step,
            places: None,
        },
        C::Precision { places } => DesignerConstraint {
            kind: "precision",
            minimum: None,
            maximum: None,
            step: None,
            places: *places,
        },
        C::Required => DesignerConstraint {
            kind: "required",
            minimum: None,
            maximum: None,
            step: None,
            places: None,
        },
    }
}

/// The events a control publishes, as manifest entries.
///
/// Exposed so a consumer that already holds a manifest can reach the event list without going back
/// through the factory.
pub fn events_of_manifest(manifest: &DesignerManifest) -> &[DesignerEvent] {
    &manifest.events
}

/// The canonical control(s) a companion is designed to sit beside (RW-107).
///
/// # What this answers, and what it does not
///
/// A companion has no meaning alone: a transport bar drives a player, a map overview mirrors a map,
/// a data table follows a chart. A designer that knew only the category could insert the companion
/// but not pair it, and the runtime's `attach_*` methods are Rust API that a document cannot state.
/// This table is that missing fact.
///
/// It is a **pairing** fact, not proof of an attach method: the map overlays and the chart
/// crosshair are positioned by the host rather than attached through one call, and the list names
/// them all the same. The empty list means "this control stands alone": a tooltip manager serves
/// every control in the window, so it names no single host rather than inventing one.
///
/// The table is deliberately small and matched by canonical name, so a control that is not a
/// companion needs no entry and no capability literal changes (a new field on every one of the 240
/// rows would be the drift this table avoids).
pub fn companion_hosts(control: &str) -> &'static [&'static str] {
    match control {
        // Media chrome attaches to the player it drives (`attach_video_player`,
        // `VideoPlayer::attach_scrubber` / `attach_subtitle_bar`).
        "media_transport_bar"
        | "media_scrubber"
        | "subtitle_bar"
        | "chapter_list_panel"
        | "playlist_panel" => &["video_player"],
        "web_history_panel" => &["web_engine_view"],
        // Chart companions attach to any chart style that owns a series (the generic
        // `ChartWidget`, `LineChart` and `BarChart` all expose the attach points).
        "chart_data_table" | "zoom_strip" | "chart_crosshair" | "chart_tooltip" => {
            &["chart", "line_chart", "bar_chart"]
        }
        "axis_picker" | "chart_reference_line" => &["line_chart", "bar_chart"],
        // Map overlays sit on a map view; the two with an attach point also call it directly.
        "map_overview" | "map_attribution" | "tile_loader" | "zoom_control" | "scale_bar"
        | "compass" => &["map_view"],
        "list_state_host" => &["list_view"],
        "filter_row" | "grid_status_bar" | "summary_row" | "header_menu" => &["data_grid"],
        "zoom_pan_controller" => &["image_view"],
        "color_history" => &["color_well"],
        "layer_panel" => &["canvas"],
        "line_number_gutter" => &["code_editor"],
        "menu_panel" => &["menu"],
        "calendar_mark_layer" => &["calendar"],
        "find_replace_bar" => &["text_edit", "text_area"],
        "drop_down_popup_host" => &["combo_box", "dropdown"],
        _ => &[],
    }
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
            protocol: MANIFEST_PROTOCOL_VERSION,
            library: String::from(env!("CARGO_PKG_VERSION")),
            profile: String::from("desktop"),
            triple: String::from("aarch64-apple-darwin"),
            control: String::from("slider"),
            category: String::from("input"),
            aliases: vec![String::from("range")],
            alias_of: String::new(),
            properties: vec![DesignerProperty {
                name: String::from("value"),
                kind: PropertyValueKind::Int,
                readable: true,
                writable: true,
                tokens: Vec::new(),
                constraints: Vec::new(),
                record_schema: None,
                default_value: CapabilityValue::Int(42),
            }],
            events: vec![DesignerEvent {
                name: String::from("value_changed"),
                payload: Some(PropertyValueKind::Int),
                shape: Some(EventPayloadShape::Scalar),
            }],
            commands: vec![DesignerCommand {
                name: String::from("set_value"),
                signature: "value",
                parameter: String::from("value"),
                kind: Some(PropertyValueKind::Int),
                tokens: Vec::new(),
                arity: None,
                element: None,
                effect: "mutates-control",
            }],
            children: vec![String::from("*")],
            companion_of: Vec::new(),
        }
    }

    /// RW-41: a container publishes its constraint, an unconstrained container publishes the
    /// wildcard, and a leaf publishes nothing.
    #[test]
    #[cfg(full_widgets)]
    fn a_container_publishes_its_child_constraint() {
        let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
        let dock = designer_manifest(&factory, "dock_widget").expect("dock_widget exports");
        // `DockPanel` is the kind; its factory spelling is `dock_widget`, the only name a document
        // can hand to `create`.
        assert_eq!(dock.children, vec![String::from("dock_widget")]);
        let tabs = designer_manifest(&factory, "tab_widget").expect("tab_widget exports");
        assert_eq!(
            tabs.children,
            vec![String::from(crate::widget::capability::ANY_CHILD_WILDCARD)]
        );
        let button = designer_manifest(&factory, "button").expect("button exports");
        assert!(button.children.is_empty(), "a leaf control has no child constraint");
    }

    /// RW-41: a container's child list survives the round trip, and a leaf's absence of one is
    /// still an empty list rather than a missing key.
    #[test]
    fn child_kinds_round_trip_through_the_document() {
        let mut manifest = sample();
        manifest.children = vec![String::from("dock_panel"), String::from("panel")];
        let text = manifest_to_json(&manifest);
        let reloaded = DesignerManifest::from_json(&text).expect("the document parses");
        assert_eq!(reloaded.children, manifest.children);

        manifest.children.clear();
        let reloaded = DesignerManifest::from_json(&manifest_to_json(&manifest))
            .expect("a leaf document parses");
        assert!(reloaded.children.is_empty());
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

    /// A record-list property's value survives the round trip **with its fields** (B26.09).
    ///
    /// The control that publishes one today starts empty, so the per-control round-trip check only
    /// ever exercises `[]`. That would leave the interesting half — field names, values and their
    /// order — untested, which is exactly the half the writer builds by hand.
    #[test]
    fn a_record_list_value_round_trips_with_its_fields() {
        let mut manifest = sample();
        let list = crate::widget::capability::RecordList::from_items(
            &crate::widget::capability::EMPTY_RECORD_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("a")))
                    .with("label", CapabilityValue::String(String::from("Alpha")))
                    .with("selected", CapabilityValue::Bool(true))
                    .with("weight", CapabilityValue::Float(2.5)),
                // A field whose *name* needs escaping in the document, and one that is spelled
                // differently from its neighbour, so the order is what is compared rather than a set.
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("b\"c")))
                    .with("note", CapabilityValue::String(String::from("no label here"))),
            ],
        );
        let records = CapabilityValue::Records(list.clone());
        manifest.properties.push(DesignerProperty {
            name: String::from("items"),
            kind: crate::widget::capability::PropertyValueKind::Records,
            readable: true,
            writable: true,
            tokens: Vec::new(),
            constraints: Vec::new(),
            record_schema: None,
            default_value: records.clone(),
        });

        let json = manifest_to_json(&manifest);
        let reloaded = DesignerManifest::from_json(&json).expect("the document this wrote parses");
        assert_eq!(manifest_to_json(&reloaded), json, "export -> load -> export changed it");

        let restored = &reloaded.properties[reloaded.properties.len() - 1].default_value;
        let CapabilityValue::Records(round_tripped) = restored else {
            panic!("`items` must come back as a record list, got {restored:?}");
        };
        // Field order is part of the value, so the whole list is compared — a set comparison would
        // accept a document whose fields were reordered.
        assert_eq!(round_tripped.items(), list.items());
        assert_eq!(
            round_tripped.get(0).expect("first").names(),
            vec!["id", "label", "selected", "weight"]
        );
        assert_eq!(round_tripped.get(0).expect("first").text("label"), "Alpha");
        assert_eq!(round_tripped.get(1).expect("second").text("id"), "b\"c");
        // The reloaded list carries no schema, like every other schema-free boundary.
        assert!(round_tripped.schema().fields.is_empty());
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
        let cases: [(PropertyValueKind, CapabilityValue, CapabilityValue); 14] = [
            (PropertyValueKind::Int, CapabilityValue::Null, CapabilityValue::Null),
            (PropertyValueKind::Bool, CapabilityValue::Bool(true), CapabilityValue::Bool(true)),
            (PropertyValueKind::Bool, CapabilityValue::Bool(false), CapabilityValue::Bool(false)),
            (PropertyValueKind::UInt, CapabilityValue::UInt(7), CapabilityValue::UInt(7)),
            (PropertyValueKind::UInt, CapabilityValue::UInt(0), CapabilityValue::UInt(0)),
            (PropertyValueKind::Float, CapabilityValue::Float(1.5), CapabilityValue::Float(1.5)),
            (PropertyValueKind::Int, CapabilityValue::Int(-3), CapabilityValue::Int(-3)),
            // **The cases that used to lose their kind.** `Int(0)` and `Int(100)` are exactly what
            // `Slider`'s capability declares for `minimum`/`maximum`. They are written as ordinary
            // JSON numbers now, and the declared kind restores `Int`.
            (PropertyValueKind::Int, CapabilityValue::Int(0), CapabilityValue::Int(0)),
            (PropertyValueKind::Int, CapabilityValue::Int(100), CapabilityValue::Int(100)),
            (
                PropertyValueKind::String,
                CapabilityValue::String(String::from("a \"quoted\" \\ value\nwith a newline")),
                CapabilityValue::String(String::from("a \"quoted\" \\ value\nwith a newline")),
            ),
            (
                PropertyValueKind::String,
                CapabilityValue::String(String::from("Café ☕ Value")),
                CapabilityValue::String(String::from("Café ☕ Value")),
            ),
            (
                PropertyValueKind::Color,
                CapabilityValue::Color(Color::rgba(1, 2, 3, 4)),
                CapabilityValue::String(String::from("#01020304")),
            ),
            (
                PropertyValueKind::IconSpec,
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                    crate::widget::display_widgets::icon::IconName::Info,
                )),
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                    crate::widget::display_widgets::icon::IconName::Info,
                )),
            ),
            (
                PropertyValueKind::IconSpec,
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                    id: String::from("project/icons/a\"b\\c"),
                    color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
                }),
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
                    id: String::from("project/icons/a\"b\\c"),
                    color_mode: crate::widget::icon_source::IconColorMode::Monochrome,
                }),
            ),
        ];
        for (kind, written, expected) in cases {
            let mut manifest = sample();
            manifest.properties[0].kind = kind;
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

    #[test]
    fn icon_spec_manifest_rejects_unknown_conflicting_and_duplicate_fields() {
        let mut manifest = sample();
        manifest.properties[0].default_value =
            CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::display_widgets::icon::IconName::Info,
            ));
        let valid = manifest_to_json(&manifest);
        for (from, to) in [
            (
                r#"{"kind":"builtin","name":"info"}"#,
                r#"{"kind":"builtin","name":"info","extra":"x"}"#,
            ),
            (
                r#"{"kind":"builtin","name":"info"}"#,
                r#"{"kind":"builtin","name":"info","id":"icons/info"}"#,
            ),
            (
                r#"{"kind":"builtin","name":"info"}"#,
                r#"{"kind":"builtin","name":"info","name":"save"}"#,
            ),
            (r#"{"kind":"builtin","name":"info"}"#, r#"{"kind":"builtin","name":"unknown-token"}"#),
        ] {
            let malformed = valid.replace(from, to);
            assert_ne!(malformed, valid, "fixture must replace the icon descriptor");
            assert!(
                DesignerManifest::from_json(&malformed).is_err(),
                "manifest accepted malformed icon descriptor: {to}"
            );
        }
    }

    /// A non-negative `Int` is written as ordinary JSON, and the declared kind restores it.
    ///
    /// The historical writer prefixed it with `+`, a token RFC 8259 does not allow and every
    /// conforming parser rejects. The kind in the same object is the fact that distinguishes the
    /// two carriers, so no marker is needed.
    #[test]
    fn a_non_negative_int_is_written_without_a_non_json_marker() {
        let mut manifest = sample();
        manifest.properties[0].default_value = CapabilityValue::Int(100);
        let int_json = manifest_to_json(&manifest);
        assert!(
            int_json.contains("\"default\": 100"),
            "a non-negative `Int` must be a standard JSON number: {int_json}"
        );
        assert!(
            !int_json.contains(":+100") && !int_json.contains(": +100"),
            "a leading `+` is not a JSON number token: {int_json}"
        );
        let reloaded = DesignerManifest::from_json(&int_json).expect("standard JSON must parse");
        assert_eq!(
            reloaded.properties[0].default_value,
            CapabilityValue::Int(100),
            "the declared kind must recover `Int` from the bare spelling"
        );

        // A negative integer is unambiguous and keeps its sign.
        manifest.properties[0].default_value = CapabilityValue::Int(-100);
        let negative_json = manifest_to_json(&manifest);
        assert!(negative_json.contains("\"default\": -100"));
        assert!(!negative_json.contains("+-100"));
    }

    /// The exported document is JSON that a conforming parser accepts.
    ///
    /// `capability_manifest_json(&factory, "slider")` is the concrete path the finding named:
    /// `slider.minimum`/`maximum` are non-negative `Int` defaults, and the old writer emitted
    /// `"default": +0` / `+100`, which `json.loads` refuses. This test uses the module's own
    /// parser (the crate deliberately has no serde dependency here) and additionally asserts that
    /// no number token carries a leading `+`.
    #[cfg(full_widgets)]
    #[test]
    fn an_exported_slider_manifest_is_standard_json() {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let json = capability_manifest_json(&factory, "slider").expect("slider exports");
        assert!(
            !json.contains(": +0") && !json.contains(": +100"),
            "no number token may start with `+`: {json}"
        );
        assert!(
            json.split_whitespace().all(|token| !token.starts_with('+')),
            "a `+`-prefixed token is not JSON: {json}"
        );
        let reloaded = DesignerManifest::from_json(&json).expect("standard JSON must parse");
        let minimum = reloaded
            .properties
            .iter()
            .find(|property| property.name == "minimum")
            .expect("slider publishes minimum");
        assert_eq!(minimum.default_value, CapabilityValue::Int(0));
        let maximum = reloaded
            .properties
            .iter()
            .find(|property| property.name == "maximum")
            .expect("slider publishes maximum");
        assert_eq!(maximum.default_value, CapabilityValue::Int(100));
    }

    /// Export → load → export must be a fixed point of the writer.
    ///
    /// The round-trip cases assert that one value survives; this asserts that a whole *document*
    /// does, which is the property a designer's save/load cycle actually needs. A value that
    /// reloads as a different kind re-exports to different bytes, so an equality check on the two
    /// exports catches every such case at once instead of one per kind.
    #[test]
    fn exporting_a_reloaded_document_reproduces_it_byte_for_byte() {
        let mut manifest = sample();
        manifest.properties[0].default_value = CapabilityValue::Int(100);
        let first = manifest_to_json(&manifest);
        let reloaded =
            DesignerManifest::from_json(&first).expect("a document this writer produced must load");
        let second = manifest_to_json(&reloaded);
        assert_eq!(
            first, second,
            "export → load → export must be stable; a value that changes kind on load is the
             reason it is not"
        );
    }

    /// A protocol-4 document has no `category`; it must still open, with the field empty.
    ///
    /// Empty is the honest value: the document did not say, and inventing a section would put the
    /// row somewhere the file never asked for.
    #[test]
    fn a_document_without_a_category_reads_as_an_empty_one() {
        let json = manifest_to_json(&sample())
            .lines()
            .filter(|line| !line.contains("\"category\":"))
            .collect::<Vec<_>>()
            .join("\n");
        let reloaded = DesignerManifest::from_json(&json).expect("the field is optional");
        assert_eq!(reloaded.category, "");
        assert_eq!(reloaded.control, "slider", "the rest of the document must be intact");
    }

    /// Every exported control must carry a palette category, and every token must be a stable
    /// lower-case ASCII word.
    ///
    /// This is one palette fact the four-dimensional readiness table's control column reads; the
    /// full table (controls × properties × events × commands, with typed / wireable / inexpressible
    /// counts and a reason for every inexpressible row) is printed by
    /// `tools/check_event_payload_types.py` (principle #100). A control with no category is a row
    /// the palette cannot place, and a token with capitals or punctuation is one a consumer would
    /// have to normalise before comparing.
    ///
    /// Gated with the factory and the export it reads: a build without the full widget set has no
    /// registration table to enumerate, and `capability_manifest_json` does not exist there.
    #[cfg(full_widgets)]
    #[test]
    fn every_registered_control_declares_a_palette_category() {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let mut uncategorised = Vec::new();
        for capability in factory.capabilities() {
            let category = capability.kind.designer_category();
            if category.is_empty()
                || !category.bytes().all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            {
                uncategorised.push((capability.canonical_name, category));
            }
        }
        assert!(
            uncategorised.is_empty(),
            "these controls have no usable palette category: {uncategorised:?}"
        );

        // The wiring is the part a per-kind method alone cannot prove: the exported document must
        // carry the token, not only the enum.
        let button = capability_manifest_json(&factory, "button").expect("button exports");
        assert!(
            button.contains("\"category\": \"basic\""),
            "the manifest must carry the category: {button}"
        );
        let slider = capability_manifest_json(&factory, "slider").expect("slider exports");
        assert!(
            slider.contains("\"category\": \"input\""),
            "a slider is chosen as an input, regardless of the module it is built in: {slider}"
        );
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
        manifest.properties[0].kind = PropertyValueKind::Float;
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

    /// The document must name its protocol, library, profile and triple (BLUE26 B26.01).
    ///
    /// Without these a designer cannot tell which build answered — the whole reason a manifest
    /// would otherwise read a `desktop` fact as a claim about a `mini` device.
    #[test]
    fn a_manifest_declares_its_protocol_and_target_scope() {
        let json = manifest_to_json(&sample());
        for key in ["\"protocol\":", "\"library\":", "\"profile\":", "\"triple\":"] {
            assert!(json.contains(key), "the document must name `{key}`: {json}");
        }
        assert!(
            json.contains(&format!("\"protocol\": {}", MANIFEST_PROTOCOL_VERSION)),
            "the written protocol must be the current version: {json}"
        );
        let reloaded = DesignerManifest::from_json(&json).expect("parses");
        assert_eq!(reloaded.protocol, MANIFEST_PROTOCOL_VERSION);
        assert_eq!(reloaded.library, env!("CARGO_PKG_VERSION"));
    }

    /// A document with no `protocol` key is read as version 1, not refused.
    ///
    /// Every project saved before B26.01 lacks the key, and reading them as v1 is what keeps an
    /// old project openable — the compatibility half of the protocol contract.
    #[test]
    fn a_protocol_one_document_is_still_readable() {
        // Strip the four new keys to reproduce the v1 shape exactly.
        let json = manifest_to_json(&sample());
        let stripped: String = json
            .lines()
            .filter(|line| {
                !line.contains("\"protocol\":")
                    && !line.contains("\"library\":")
                    && !line.contains("\"profile\":")
                    && !line.contains("\"triple\":")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let reloaded = DesignerManifest::from_json(&stripped)
            .expect("a document without a protocol key is protocol 1");
        assert_eq!(reloaded.protocol, MANIFEST_MIN_PROTOCOL_VERSION);
    }

    /// A document naming a *newer* protocol must be refused, not partially read.
    ///
    /// A partial read of a future document is how a designer silently drops a field it does not
    /// understand and re-saves a lossy file, so the refusal is the point.
    #[test]
    fn a_future_protocol_version_is_refused() {
        let json = manifest_to_json(&sample()).replace(
            &format!("\"protocol\": {MANIFEST_PROTOCOL_VERSION}"),
            &format!("\"protocol\": {}", MANIFEST_PROTOCOL_VERSION + 1),
        );
        let result = DesignerManifest::from_json(&json);
        assert!(result.is_err(), "a newer protocol must be refused");
        let message = result.unwrap_err().detail;
        assert!(
            message.contains("newer"),
            "the refusal must say the document is newer, not just fail: {message}"
        );
    }

    /// A fractional or signed protocol value must be refused rather than truncated.
    #[test]
    fn a_malformed_protocol_value_is_refused() {
        for bad in ["\"protocol\": 2.5", "\"protocol\": -1", "\"protocol\": \"two\""] {
            let json = manifest_to_json(&sample())
                .replace(&format!("\"protocol\": {MANIFEST_PROTOCOL_VERSION}"), bad);
            assert!(
                DesignerManifest::from_json(&json).is_err(),
                "`{bad}` must not be read as a protocol version"
            );
        }
    }

    /// A command must carry its parameter shape and effect through the round trip (B26.03).
    ///
    /// A command used to be a bare string, which says nothing a designer can validate a binding
    /// against. This asserts the object form survives — a name plus the three facts a wire check
    /// needs (shape, parameter, effect).
    #[test]
    fn a_typed_command_round_trips() {
        let mut manifest = sample();
        manifest.commands = vec![
            DesignerCommand {
                name: String::from("set_value"),
                signature: "value",
                parameter: String::from("value"),
                kind: Some(PropertyValueKind::UInt),
                tokens: Vec::new(),
                arity: None,
                element: None,
                effect: "mutates-control",
            },
            DesignerCommand {
                name: String::from("set_alignment"),
                signature: "value",
                parameter: String::from("alignment"),
                kind: Some(PropertyValueKind::Enum),
                tokens: vec![String::from("left"), String::from("centre"), String::from("right")],
                arity: None,
                element: None,
                effect: "mutates-control",
            },
            DesignerCommand {
                name: String::from("toggle"),
                signature: "none",
                parameter: String::new(),
                kind: None,
                tokens: Vec::new(),
                arity: None,
                element: None,
                effect: "mutates-control",
            },
            DesignerCommand {
                name: String::from("set_range"),
                signature: "composite",
                parameter: String::from("lower"),
                kind: None,
                tokens: Vec::new(),
                arity: Some(2),
                element: Some(PropertyValueKind::Float),
                effect: "mutates-control",
            },
        ];
        let json = manifest_to_json(&manifest);
        let reloaded = DesignerManifest::from_json(&json).expect("parses");
        assert_eq!(reloaded.commands, manifest.commands);
        assert_eq!(manifest_to_json(&reloaded), json, "the command objects must be stable");
        // The name must be a JSON key, not a bare element, so a reader sees a shape.
        assert!(json.contains("\"signature\": \"value\""), "{json}");
        assert!(json.contains("\"effect\": \"mutates-control\""), "{json}");
    }

    /// RW-103: an enum command parameter publishes the same tokens as the property it writes.
    ///
    /// A designer builds a picker from `tokens`; before protocol 7 a command carried only `kind`,
    /// so `set_alignment` was offered as free text while `alignment` had a dropdown.
    #[test]
    #[cfg(full_widgets)]
    fn an_enum_command_publishes_its_property_tokens() {
        let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
        let manifest = designer_manifest(&factory, "label").expect("label exports");
        let command = manifest
            .commands
            .iter()
            .find(|command| command.name == "set_alignment")
            .expect("label publishes set_alignment");
        let property = manifest
            .properties
            .iter()
            .find(|property| property.name == "alignment")
            .expect("label publishes alignment");
        assert_eq!(command.kind, Some(PropertyValueKind::Enum));
        assert!(!command.tokens.is_empty(), "an enum parameter needs its legal values");
        assert_eq!(command.tokens, property.tokens, "the picker must match the property's");
    }

    /// RW-103: a command that takes no enum argument carries no tokens, so a reader never sees a
    /// stale list on a command whose parameter is a number.
    #[test]
    #[cfg(full_widgets)]
    fn a_non_enum_command_carries_no_tokens() {
        let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
        let manifest = designer_manifest(&factory, "label").expect("label exports");
        let command = manifest
            .commands
            .iter()
            .find(|command| command.name == "set_text")
            .expect("label publishes set_text");
        assert!(command.tokens.is_empty(), "a text parameter has no token list");
    }

    /// RW-107: a companion names the host(s) it is designed for, and the pair survives a round
    /// trip; a standalone control names none.
    #[test]
    #[cfg(full_widgets)]
    fn a_companion_publishes_its_host() {
        let factory = crate::widget::capability::WidgetFactory::new_with_defaults();
        let strip = designer_manifest(&factory, "zoom_strip").expect("zoom_strip exports");
        assert_eq!(
            strip.companion_of,
            vec![String::from("chart"), String::from("line_chart"), String::from("bar_chart")]
        );
        let reloaded =
            DesignerManifest::from_json(&manifest_to_json(&strip)).expect("a companion parses");
        assert_eq!(reloaded.companion_of, strip.companion_of, "the pairing must round-trip");

        let button = designer_manifest(&factory, "button").expect("button exports");
        assert!(button.companion_of.is_empty(), "a standalone control names no host");
    }

    /// An unrecognised command signature or effect must be refused, not defaulted.
    #[test]
    fn a_malformed_command_shape_is_refused() {
        let json = manifest_to_json(&sample());
        let bad_sig = json.replace("\"signature\": \"value\"", "\"signature\": \"sausage\"");
        assert_ne!(bad_sig, json, "the signature sentinel must appear");
        assert!(DesignerManifest::from_json(&bad_sig).is_err());

        let bad_effect =
            json.replace("\"effect\": \"mutates-control\"", "\"effect\": \"explodes\"");
        assert_ne!(bad_effect, json, "the effect sentinel must appear");
        assert!(DesignerManifest::from_json(&bad_effect).is_err());
    }

    /// A property's constraints must survive the round trip (B26.02).
    #[test]
    fn a_constrained_property_round_trips() {
        use crate::widget::capability::{Constraint, RangeSpec};
        let mut manifest = sample();
        manifest.properties[0].constraints =
            vec![designer_constraint_from(&Constraint::Range(RangeSpec {
                minimum: 0.0,
                maximum: 1.0,
                step: Some(0.05),
            }))];
        let json = manifest_to_json(&manifest);
        assert!(json.contains("\"constraints\": ["), "the constraint list must appear: {json}");
        assert!(json.contains("\"rule\": \"range\""), "{json}");
        let reloaded = DesignerManifest::from_json(&json).expect("parses");
        assert_eq!(reloaded.properties[0].constraints, manifest.properties[0].constraints);
        assert_eq!(manifest_to_json(&reloaded), json, "the constraint must be byte-stable");
    }

    /// An unrecognised constraint kind must be refused, not silently dropped.
    #[test]
    fn a_malformed_constraint_is_refused() {
        use crate::widget::capability::{Constraint, RangeSpec};
        let mut manifest = sample();
        manifest.properties[0].constraints =
            vec![designer_constraint_from(&Constraint::Range(RangeSpec {
                minimum: 0.0,
                maximum: 1.0,
                step: None,
            }))];
        let json = manifest_to_json(&manifest);
        let bad = json.replace("\"rule\": \"range\"", "\"rule\": \"sausage\"");
        assert_ne!(bad, json, "the sentinel must appear");
        assert!(DesignerManifest::from_json(&bad).is_err());
    }

    /// A record-list property publishes the field list, identity field and id rules.
    #[cfg(full_widgets)]
    #[test]
    fn a_records_property_publishes_its_record_schema() {
        let factory = crate::widget::WidgetFactory::new_with_defaults();
        let manifest = designer_manifest(&factory, "chip").expect("chip exports");
        let items = manifest
            .properties
            .iter()
            .find(|property| property.name == "items")
            .expect("chip publishes items");
        let schema = items.record_schema.as_ref().expect("items publishes a record schema");
        assert_eq!(schema.id_field, "id");
        assert!(schema.ids_unique && schema.ids_required);
        let names: Vec<&str> = schema.fields.iter().map(|field| field.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["id", "label", "selected", "icon_source", "avatar", "enabled", "closable"]
        );

        let json = capability_manifest_json(&factory, "chip").expect("chip exports as JSON");
        assert!(json.contains("\"fields\":"), "the field list must be emitted: {json}");
        assert!(json.contains("\"id_field\": \"id\""), "the identity field must be emitted");

        let reloaded = DesignerManifest::from_json(&json).expect("the document parses");
        let restored = reloaded
            .properties
            .iter()
            .find(|property| property.name == "items")
            .expect("items survives");
        assert_eq!(restored.record_schema.as_ref(), Some(schema));
        assert_eq!(
            manifest_to_json(&reloaded),
            json,
            "export -> load -> export must preserve the record schema byte for byte"
        );
    }

    /// Two entries with the same name, and a default that contradicts its declared kind, are
    /// refused rather than loaded and re-exported as if consistent.
    #[test]
    fn duplicate_names_and_a_contradictory_default_are_refused() {
        let duplicate_property = r#"{"control":"x","properties":[
            {"name":"value","kind":"int"},
            {"name":"value","kind":"int"}]}"#;
        assert!(DesignerManifest::from_json(duplicate_property).is_err());

        let duplicate_event = r#"{"control":"x","events":[
            {"name":"clicked","payload":null,"shape":null},
            {"name":"clicked","payload":null,"shape":null}]}"#;
        assert!(DesignerManifest::from_json(duplicate_event).is_err());

        let duplicate_command = r#"{"control":"x","commands":[
            {"name":"clear","signature":"none","effect":"mutates-control"},
            {"name":"clear","signature":"none","effect":"mutates-control"}]}"#;
        assert!(DesignerManifest::from_json(duplicate_command).is_err());

        let contradictory = r#"{"control":"x","properties":[
            {"name":"value","kind":"int","default":"abc"}]}"#;
        assert!(
            DesignerManifest::from_json(contradictory).is_err(),
            "a string default cannot describe an `int` property"
        );

        // `Null` is the documented "not set" value and is legal for every kind.
        let unset = r#"{"control":"x","properties":[
            {"name":"value","kind":"int","default":null}]}"#;
        assert!(DesignerManifest::from_json(unset).is_ok());
    }

    /// A decimal-place count above `u8::MAX` is a parse error, and an integral default above 2^53
    /// keeps its exact value.
    #[test]
    fn a_place_count_does_not_truncate_and_a_large_integer_keeps_its_value() {
        let mut manifest = sample();
        manifest.properties[0].constraints = vec![DesignerConstraint {
            kind: "precision",
            minimum: None,
            maximum: None,
            step: None,
            places: Some(2),
        }];
        let json = manifest_to_json(&manifest);
        let bad = json.replace("\"places\": 2", "\"places\": 300");
        assert_ne!(bad, json, "the sentinel must appear");
        assert!(DesignerManifest::from_json(&bad).is_err(), "300 must not truncate to 44");

        let exact = 9_007_199_254_740_993i64;
        manifest.properties[0].constraints.clear();
        manifest.properties[0].default_value = CapabilityValue::Int(exact);
        let json = manifest_to_json(&manifest);
        let reloaded = DesignerManifest::from_json(&json).expect("the document parses");
        assert_eq!(reloaded.properties[0].default_value, CapabilityValue::Int(exact));
    }
}
