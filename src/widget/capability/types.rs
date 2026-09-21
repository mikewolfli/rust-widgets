// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// Imports for the factory, which only exists where the full control set does.
#[cfg(widgets_unstripped)]
use crate::compat::HashMap;
use crate::compat::{String, Vec};

#[cfg(widgets_unstripped)]
use crate::core::Rect;
#[cfg(widgets_unstripped)]
use crate::widget::Widget;
use crate::widget::WidgetKind;

/// Runtime property value returned by capability-based reflection APIs.
///
/// This is the dynamically-typed counterpart of [`PropertyValueKind`]: the kind
/// says what a property *promises* to hold, and this holds it. Variants are kept
/// distinct even where the underlying type could collapse — `Int` and `UInt` are
/// separate, and both are separate from `Float` — so a written-back value keeps
/// the exact type the property declared it would.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityValue {
    /// A present but empty value: "no selection", "not set". Distinct from
    /// [`CapabilityValue::Bool`]`(false)` and from an empty string.
    Null,
    /// A boolean, for properties whose [`PropertyValueKind`] is
    /// [`PropertyValueKind::Bool`].
    Bool(bool),
    /// A signed integer, for properties declared as
    /// [`PropertyValueKind::Int`].
    Int(i64),
    /// An unsigned integer, for indices, counts and lengths — properties
    /// declared as [`PropertyValueKind::UInt`]. Using the unsigned variant for a
    /// count is what lets a caller rely on it never being negative.
    UInt(u64),
    /// A floating-point number, for properties declared as
    /// [`PropertyValueKind::Float`]. Used even for values that happen to be
    /// whole numbers, so `1.0` and `1` are not interchangeable.
    Float(f64),
    /// A string, for text properties and for enumerated values, which travel as
    /// their token spelling (see [`PropertyValueKind::Enum`]).
    String(String),
    /// An RGBA colour.
    ///
    /// Colours used to travel as strings, which meant every caller had to agree on a
    /// spelling (`#rrggbbaa`? `rgb(..)`? a named colour?) and a typo became a silently
    /// wrong colour instead of a refused write. Carrying the parsed value makes the
    /// declared kind enforceable: a write of anything else is a `TypeMismatch`.
    Color(crate::core::Color),
    /// A rectangle in logical pixels.
    ///
    /// Geometry used to travel as a `"x,y,w,h"` string, which is worse than the
    /// colour case because it is not a type at all — the components had to be split
    /// and re-parsed by every consumer, and a malformed string was indistinguishable
    /// from a valid one until it was too late to report.
    ///
    /// # Scope
    ///
    /// A widget's *own* geometry is still read-only and still set through the
    /// dedicated geometry entry point, because a control's placement is the layout's
    /// business rather than a property. This variant exists for properties that are
    /// genuinely rectangles of their own — a plot area, a clipping region, a source
    /// image crop.
    Rect(crate::core::Rect),
}

impl CapabilityValue {
    /// The text this value carries, or `None` when it is not a string.
    ///
    /// # Why this accessor had to be added (BLUE19 T-23)
    ///
    /// The generated code needs to read a `text`/`title` property out of a `Node`, and there was no
    /// way to ask a `CapabilityValue` for its string: the enum's variants are public, so the only
    /// route was a `match` at every call site. The T-23 round added this after a **compile failure**
    /// in generated code that called `as_str` — the method the API reads as if it existed.
    ///
    /// It is deliberately **narrow**: `Int`/`UInt`/`Bool` return `None` rather than a formatted
    /// string, because a caller that wants text out of a number is making a presentation decision
    /// that belongs at the call site (and `WireCompatibility::Converted` already documents that a
    /// number → text wire is a *conversion* rather than an identity).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// The boolean this value carries, or `None` when it is not one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The signed integer this value carries, or `None`.
    ///
    /// A `UInt` that fits returns `Some`, so a caller reading an index does not have to know which
    /// variant the property was declared as. A `UInt` that does **not** fit returns `None` rather
    /// than wrapping — a silent wrap would be a wrong value presented as a right one.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::UInt(value) => i64::try_from(*value).ok(),
            _ => None,
        }
    }

    /// The unsigned integer this value carries, or `None`.
    ///
    /// The mirror of [`Self::as_i64`]: a non-negative `Int` returns `Some`, a negative one returns
    /// `None` because there is no unsigned value that means it.
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(value) => Some(*value),
            Self::Int(value) => u64::try_from(*value).ok(),
            _ => None,
        }
    }
}

/// Why a capability-based property read or write did not happen.
///
/// The distinction that matters to a caller is between *"you asked for something
/// that does not exist"* and *"it exists but you cannot do that"*: the first
/// means the caller should look elsewhere, the second that it should stop
/// retrying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityAccessError {
    /// The id does not address a known widget. Returned for a stale id as well
    /// as for one that was never valid, so an id that used to work can start
    /// producing this.
    UnknownWidget,
    /// The widget exists but does not publish a property by that name. This is
    /// the honest "no" for an unsupported property — never a silent success.
    UnknownProperty,
    /// The property was found but is not writable. A read of the same name
    /// still succeeds.
    ReadOnlyProperty,
    /// The property exists but the supplied [`CapabilityValue`] is not of its
    /// declared [`PropertyValueKind`]. No coercion is attempted, so a write must
    /// use the variant the property declares.
    TypeMismatch,
    /// The property is meaningful for other widget kinds but not this one. An
    /// operation the widget's interaction model cannot honour is reported here
    /// rather than being accepted and ignored.
    UnsupportedOnWidget,
    /// The property was found and the value was the right type, but the value itself
    /// addresses nothing — an index past the end of the collection, or a position that
    /// does not exist.
    ///
    /// # Why this is distinct from [`CapabilityAccessError::UnsupportedOnWidget`]
    ///
    /// The two say opposite things to the caller. `UnsupportedOnWidget` means "stop
    /// asking, this control will never do that", so a caller should not retry. This one
    /// means "this control does that, but not at *that* index", so the caller's mistake
    /// is the argument and the same call with a valid index succeeds.
    ///
    /// Reporting an out-of-range index as `UnsupportedOnWidget` sent callers to look for
    /// a different control when the real mistake was in their own argument, and it made
    /// the error indistinguishable from a genuine capability gap in logs.
    OutOfRange,
    /// The control has no command by that name.
    ///
    /// The counterpart of [`CapabilityAccessError::UnknownProperty`] for the imperative
    /// half of the contract, and deliberately not folded into it: a caller reading the
    /// error needs to know whether it named state or an action, because the two are
    /// discovered from different lists (`property_names()` versus
    /// `WidgetCapability::commands`).
    ///
    /// # Why this variant had to be added
    ///
    /// `WidgetProperties::command` needs a "no such command" answer, and the
    /// alternatives were both worse: returning `Ok(())` reports success for an action
    /// that did not happen, and returning `UnknownProperty` tells the caller to look in
    /// the property schema for a name that belongs to the command list. This error is
    /// also what makes the command list falsifiable — the test that calls every
    /// published command distinguishes "the control refuses it" from "the control ran
    /// it" by this variant.
    UnknownCommand,
}

/// Primitive property value kinds used by capability metadata.
///
/// These name the *declared* type of a property in [`PropertySchema`], which is
/// what separates [`PropertyValueKind::Int`] from
/// [`PropertyValueKind::UInt`] and [`PropertyValueKind::Enum`] from
/// [`PropertyValueKind::String`] at the metadata level even though both pairs
/// are carried by the same runtime variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyValueKind {
    /// Boolean-valued; carried as [`CapabilityValue::Bool`].
    Bool,
    /// Signed-integer-valued; carried as [`CapabilityValue::Int`].
    Int,
    /// Unsigned-integer-valued, for indices and counts; carried as
    /// [`CapabilityValue::UInt`].
    UInt,
    /// Floating-point-valued; carried as [`CapabilityValue::Float`].
    Float,
    /// A number whose integer/decimal form is chosen at runtime by another property.
    ///
    /// # Why this is not just `Float`
    ///
    /// `spin_box` publishes `value`, `minimum`, `maximum` and `single_step`. With
    /// `decimals == 0` those report `CapabilityValue::Int` — an integer spin box must not
    /// make every caller unwrap a float — and with `decimals > 0` they report `Float`.
    /// Declaring `Int` would be wrong in decimal mode; declaring `Float` would be wrong
    /// in integer mode. Declaring `Number` says the one true thing: this property is a
    /// number, and the task of picking the carrier belongs to the control.
    ///
    /// It is distinct from `Float` because it must *accept* an `Int` as well (a caller
    /// writing `{"value": 3}` must keep working after decimals are enabled), and distinct
    /// from `Int` for the mirror reason. Carried as [`CapabilityValue::Int`] or
    /// [`CapabilityValue::Float`].
    Number,
    /// Free text; carried as [`CapabilityValue::String`].
    String,
    /// One of a fixed set of choices. Carried as [`CapabilityValue::String`]
    /// holding the choice's token spelling, so the value is the enum's name
    /// rather than its ordinal; an unrecognised token is a parse failure, not a
    /// different variant.
    Enum,
    /// An RGBA colour, carried as [`CapabilityValue::Color`].
    Color,
    /// A rectangle in logical pixels, carried as [`CapabilityValue::Rect`].
    Rect,
}

/// Metadata for one readable/writable property.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySchema {
    /// The property's name, as accepted by the property API. A widget may
    /// publish several names for the same property (aliases), in which case each
    /// has its own schema entry pointing at the same underlying value.
    pub name: &'static str,
    /// The declared type of the value. A write of any other kind fails with
    /// [`CapabilityAccessError::TypeMismatch`] rather than being coerced.
    pub value_kind: PropertyValueKind,
    /// Whether the property can be read. `false` means a read answers
    /// [`CapabilityAccessError::UnknownProperty`], exactly as an unresolvable
    /// name would.
    pub readable: bool,
    /// Whether the property can be written. `false` with `readable` true is a
    /// read-only property, and a write answers
    /// [`CapabilityAccessError::ReadOnlyProperty`].
    pub writable: bool,
    /// The accepted spellings for a [`PropertyValueKind::Enum`] property, in the order
    /// they should be offered to a user. Empty for every other kind.
    ///
    /// # Why this is a field and not a lookup elsewhere
    ///
    /// An enum property's legal values were previously undiscoverable from outside: the
    /// tokens existed only as string literals inside each control's `set` arm, so a
    /// caller driving the property API could not present a choice without duplicating
    /// that knowledge — and nothing made the duplicate fail when the control changed.
    /// Carrying the list in the schema makes the control's own declaration the single
    /// source, reachable through `rw_widget_property_tokens`.
    ///
    /// # How existing entries stay valid
    ///
    /// This field is filled by [`PropertySchema::new`] and the `bool`/`number`/
    /// `text`/`enum` constructors. The 1200-odd struct literals in this crate were
    /// written before this field existed, so they are migrated to the constructors
    /// rather than hand-edited — see the note on each constructor.
    pub accepted_tokens: &'static [&'static str],
}

impl PropertySchema {
    /// A schema entry with no accepted-token list.
    ///
    /// The constructor to use for every non-enum property: it keeps `accepted_tokens`
    /// empty, which is the correct answer for a value whose legal inputs are not a fixed
    /// set.
    pub const fn new(
        name: &'static str,
        value_kind: PropertyValueKind,
        readable: bool,
        writable: bool,
    ) -> Self {
        Self { name, value_kind, readable, writable, accepted_tokens: &[] }
    }

    /// A [`PropertyValueKind::Enum`] entry that publishes its legal spellings.
    ///
    /// The tokens must be the spellings `set` actually accepts.
    /// `published_enum_tokens_are_accepted_by_their_control` in
    /// `properties_tests.rs` writes every writable enum token of every registered
    /// capability back through `widget_property_set`, so a list that has drifted from
    /// the control's parser fails rather than misleading a caller.
    pub const fn enumerated(
        name: &'static str,
        readable: bool,
        writable: bool,
        accepted_tokens: &'static [&'static str],
    ) -> Self {
        Self { name, value_kind: PropertyValueKind::Enum, readable, writable, accepted_tokens }
    }

    /// The accepted spellings, or an empty slice for a non-enum property.
    pub const fn accepted_tokens(&self) -> &'static [&'static str] {
        self.accepted_tokens
    }
}

/// How an event's payload is *shaped*, separately from what it holds.
///
/// # Why this is a second field rather than more [`PropertyValueKind`] variants
///
/// A real signal payload is not always one scalar. `PaneLayoutChanged` carries
/// `Vec<f32>`, `TabMoved` carries `(usize, usize)`, `RichEdit::selection_changed`
/// carries `Option<(usize, usize)>`. Folding those into the kind enum would either
/// multiply it (`Tuple2UInt`, `Tuple3UInt`, `ListFloat`, …) or force a false claim:
/// describing a pair of integers as `UInt` loses the second component, and describing
/// it as `String` loses the fact that both are numbers.
///
/// Two fields keep the two questions separate, and each is answerable exactly:
///
/// * `payload_kind` — *what* the value is; the counterpart of
///   [`PropertySchema::value_kind`], which is what makes a wire to a property
///   checkable, and what the JSON round-trip asserts on.
/// * `payload_shape` — *how many* and *in what arrangement*; what a designer needs
///   to render "value: number" versus "values: 2 numbers".
///
/// # Why a `Mixed` row still declares a kind
///
/// A payload such as `(String, u32, String)` has no single kind. It declares
/// [`PropertyValueKind::String`] plus [`EventPayloadShape::Mixed`]: the kind is the
/// type the value is *carried* as across a boundary, and the shape is the honest
/// statement that the real payload is richer. Declaring a scalar shape for it would
/// claim the value is safely usable as that scalar, which is exactly the kind of
/// overstatement that makes a designer offer a wire which cannot exist.
///
/// [`PropertyValueKind`]: PropertyValueKind
/// [`PropertySchema::value_kind`]: PropertySchema::value_kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventPayloadShape {
    /// Exactly one value of the declared kind.
    Scalar,
    /// Zero or one value of the declared kind (`Option<T>`); `CapabilityValue::Null`
    /// is the absent case.
    OptionalScalar,
    /// A variable-length list of the declared kind (`Vec<T>`); empty is a legal value,
    /// so it is distinct from [`Self::OptionalScalar`].
    ListScalar,
    /// Two values of the declared kind: `(T, T)`.
    Tuple2,
    /// Three values of the declared kind: `(T, T, T)`.
    Tuple3,
    /// Four values of the declared kind: `(T, T, T, T)`.
    Tuple4,
    /// An optional pair of the declared kind: `Option<(T, T)>` — a selection that may
    /// cover more than one position, or be absent.
    OptionalTuple2,
    /// More than one kind in one payload (`(String, u32, String)`). The declared kind is
    /// how the whole value travels; there is no arity to promise, because the parts are
    /// not of one type.
    Mixed,
}

/// Metadata for one published event, the counterpart of [`PropertySchema`].
///
/// # Why the event side needed this
///
/// The property side of a capability carries a type ([`PropertyValueKind`]) and a runtime
/// value ([`CapabilityValue`]); the event side used to carry a bare name. A name says
/// nothing a consumer can act on: a designer cannot render the payload type, a wire cannot
/// be type-checked, and JSON cannot express a mapping. Every one of those is a *first*
/// requirement of the designer this library is being prepared for, not a last one.
///
/// # Compatibility
///
/// [`WidgetCapability::events`] changed from `&'static [&'static str]` to
/// `&'static [EventSchema]`. `connect_event` resolves the same names as before — a name
/// published then is published now, and a name refused then is refused now — so this is a
/// widening of the declaration, not a change of behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSchema {
    /// The event's name, spelled exactly as `connect_event` accepts it.
    pub name: &'static str,
    /// The declared type of the payload, or `None` for an event that carries no value.
    ///
    /// `None` is the honest answer for `clicked`, `dismissed` and the rest of the
    /// payload-free majority; it is also the answer for a payload that is the unit type,
    /// because "an event that happened" and "an event that happened and reported `()`"
    /// are the same fact and two representations of it would be indistinguishable.
    pub payload: Option<PropertyValueKind>,
    /// The arrangement of the payload, or `None` when there is no payload.
    pub shape: Option<EventPayloadShape>,
}

impl EventSchema {
    /// A schema for an event that carries no value.
    pub const fn unit(name: &'static str) -> Self {
        Self { name, payload: None, shape: None }
    }

    /// A schema for an event carrying exactly one value of `kind`.
    pub const fn scalar(name: &'static str, kind: PropertyValueKind) -> Self {
        Self { name, payload: Some(kind), shape: Some(EventPayloadShape::Scalar) }
    }

    /// A schema for an event whose payload is a list of `kind`.
    pub const fn list(name: &'static str, kind: PropertyValueKind) -> Self {
        Self { name, payload: Some(kind), shape: Some(EventPayloadShape::ListScalar) }
    }

    /// A schema for an event carrying a pair of `kind`.
    pub const fn pair(name: &'static str, kind: PropertyValueKind) -> Self {
        Self { name, payload: Some(kind), shape: Some(EventPayloadShape::Tuple2) }
    }

    /// Whether this event carries a value a subscriber can read.
    pub const fn has_payload(&self) -> bool {
        self.payload.is_some()
    }
}

/// Capability metadata for a widget kind.
#[derive(Debug, Clone)]
pub struct WidgetCapability {
    /// The widget kind this metadata describes.
    pub kind: WidgetKind,
    /// The kind's canonical name: the spelling the widget factory accepts, e.g.
    /// `"code_editor"`.
    pub canonical_name: &'static str,
    /// Alternative names the factory also accepts for this kind. Lookup is by
    /// name, so aliases exist so callers do not have to know which spelling the
    /// factory happened to register first.
    pub aliases: &'static [&'static str],
    /// Every property this kind publishes, for discovery and for validating a
    /// name before using it.
    pub properties: &'static [PropertySchema],
    /// Every event the kind publishes, with its payload type, for discovery and for
    /// validating a name before subscribing to it.
    ///
    /// The names are the ones [`WidgetFactory::connect_event`] accepts; the payload is
    /// derived from the signal the name resolves to in the control's own source, and kept
    /// honest by `tools/check_event_payload_types.py`.
    ///
    /// [`WidgetFactory::connect_event`]: crate::widget::capability::WidgetFactory::connect_event
    pub events: &'static [EventSchema],
    /// Names of the commands the kind accepts.
    pub commands: &'static [&'static str],
}

/// One property entry in exported capability manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityPropertyManifest {
    /// The property's name, type and accessibility, as published by the widget.
    pub schema: PropertySchema,
    /// The value the property holds on a freshly created widget, before any
    /// write. Captured so a consumer can tell an untouched property from one set
    /// to its default explicitly.
    pub default_value: CapabilityValue,
}

/// One event entry in an exported capability manifest.
///
/// The owned counterpart of [`EventSchema`]. It exists for the same reason
/// [`CapabilityPropertyManifest`] does: a manifest outlives the `&'static` capability table it
/// came from, so every borrowed field is copied out — including the name, which a designer may
/// hold long after the table is gone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventManifest {
    /// The published event name, as `connect_event` accepts it.
    pub name: String,
    /// The declared payload type, or `None` when the event carries no value.
    pub payload: Option<PropertyValueKind>,
    /// The arrangement of the payload.
    pub shape: Option<EventPayloadShape>,
}

/// Exportable snapshot for one widget capability.
///
/// The `&'static` slices of [`WidgetCapability`] become owned `Vec`s here, which
/// is what makes a manifest suitable for serialising, sending across a boundary
/// or storing beyond the lifetime of the capability table it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetCapabilityManifest {
    /// The widget kind the snapshot describes.
    pub kind: WidgetKind,
    /// The kind's canonical factory name.
    pub canonical_name: &'static str,
    /// The kind's alternative factory names.
    pub aliases: Vec<&'static str>,
    /// Every published property with its default value.
    pub properties: Vec<CapabilityPropertyManifest>,
    /// Every published event with its payload type.
    pub events: Vec<EventManifest>,
    /// Names of the commands the kind accepts.
    pub commands: Vec<&'static str>,
}

/// Constructor signature the factory registers for each control.
#[cfg(widgets_unstripped)]
pub(crate) type WidgetCtor = fn(Rect, &str) -> Box<dyn Widget>;

/// Factory + metadata registry for dynamic widget instantiation.
///
/// Gated with the full control set: the factory registers a constructor per
/// concrete control type, so it only exists where those types do. The
/// [property contract](crate::widget::capability::properties_trait::WidgetProperties)
/// is independent of it and available in every profile.
///
/// The four tables are only ever *filled* by `register_core_widgets`, which is
/// itself gated on the full widget set. In a build without it the factory still
/// exists — the creation path needs a value to ask and get an honest "no" from —
/// but every table stays empty, so the dead-code allowance below covers exactly
/// that profile and nothing else.
#[cfg(widgets_unstripped)]
#[cfg_attr(not(full_widgets), allow(dead_code))]
pub struct WidgetFactory {
    pub(crate) capabilities: Vec<WidgetCapability>,
    pub(crate) key_to_index: HashMap<String, usize>,
    pub(crate) kind_to_index: HashMap<WidgetKind, Vec<usize>>,
    pub(crate) constructors: HashMap<String, WidgetCtor>,
}
