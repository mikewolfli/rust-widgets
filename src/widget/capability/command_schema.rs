// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The typed command table a designer needs to validate an event-to-command binding
//! (BLUE26 B26.03).
//!
//! # What problem this solves
//!
//! `WidgetCapability::commands` is a list of names.
//! A name is enough to *offer* a command but not to *validate a binding to it*: a designer
//! drawing a connection from an event to a command must know whether the command takes an
//! argument, what type that argument is, and whether invoking it has an effect the user should be
//! warned about. None of that is in the name.
//!
//! # Why the schema is derived, not hand-written
//!
//! The crate publishes **465** command instances (254 distinct names) across **247** controls. A
//! hand-written table of 465 rows would be a second copy of a fact the dispatcher already encodes
//! — and copies drift (principle #54/#101). Instead, the schema is *derived* from the two things
//! that already decide how a command behaves:
//!
//! 1. the `set_foo` convention, which [`WidgetProperties::command`] itself
//!    (see `super::properties_trait::WidgetProperties::default_command`) resolves by looking
//!    `foo` up in the control's property list; and
//! 2. an explicit classification of the non-`set_` verbs in `VERB_CLASSES`, which is the one
//!    thing the convention cannot say (a verb like `toggle` has no property named `toggle`).
//!
//! So the schema cannot disagree with the dispatcher about `set_*` commands: it consults the same
//! property list, and a `set_foo` name that no property of the control backs is described by
//! nothing — the same answer the dispatcher gives. For the verbs, the table below is the single
//! declaration, and `every_published_command_has_a_typed_schema` walks every published command and
//! fails if one is unclassified — which is what keeps the table from silently falling behind a new
//! command.
//!
//! # Compatibility
//!
//! Nothing here changes how a command *dispatches*. A payload-free command that was invokable
//! before is still invokable; a `set_*` command still reports that it needs a value. The schema
//! is a **description** of the existing behaviour, added so a designer can act on it.
//!
//! `WidgetProperties::command`

use crate::compat::{String, Vec};

/// A short alias for the payload-free signature, so the verb table below reads as one line per
/// verb rather than wrapping `CommandSignature::PayloadFree` 114 times.
use CommandSignature::PayloadFree;

/// What invoking a command does, for a designer that must decide whether to warn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandEffect {
    /// Changes the control's own state and nothing outside it: `set_value`, `toggle`, `clear`.
    /// Safe to invoke from a wire without a confirmation.
    MutatesControl,
    /// Changes state the user can see but not undo from the control alone — an appended row, a
    /// pushed page. A designer may still wire it, but the label should say what it does.
    MutatesCollection,
    /// Affects something outside the control entirely: a navigation, a file load, a network
    /// fetch. The user's context may change, so a designer should present it as an action rather
    /// than a state toggle.
    SideEffecting,
}

impl CommandEffect {
    /// The stable token this effect travels as in a manifest.
    pub const fn token(self) -> &'static str {
        match self {
            Self::MutatesControl => "mutates-control",
            Self::MutatesCollection => "mutates-collection",
            Self::SideEffecting => "side-effecting",
        }
    }

    /// The inverse of [`Self::token`]; `None` for an unrecognised spelling.
    pub const fn from_token(token: &str) -> Option<Self> {
        match token.as_bytes() {
            b"mutates-control" => Some(Self::MutatesControl),
            b"mutates-collection" => Some(Self::MutatesCollection),
            b"side-effecting" => Some(Self::SideEffecting),
            _ => None,
        }
    }
}

/// One command's parameter shape: how many arguments it takes and of what type.
///
/// Modelled as an enum rather than an `Option<kind>` because "takes no argument" and "takes one
/// argument of kind `K`" are genuinely different shapes, and a binding validator has to treat
/// them differently (a payload-free command is the only form an event wire may drive — see
/// `wire_rules::WireTarget::PayloadFreeCommand`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSignature {
    /// The command takes no argument.
    PayloadFree,
    /// The command takes exactly one value of the named kind, supplied through the property
    /// route (a `set_foo` command's value is written to property `foo`).
    OneValue {
        /// The property the value is written to, or the parameter's own name for a non-property
        /// command.
        parameter: &'static str,
        /// The declared type of the value.
        kind: super::types::PropertyValueKind,
    },
    /// The command takes a structured payload that is **not** one property: a pair, a list, or a
    /// record.
    ///
    /// # Why this is a separate shape rather than an `OneValue`
    ///
    /// A payload of several values cannot be described as "one value of kind `K`" without telling
    /// a designer it can be driven by a single-value wire, which it cannot. The shape says "this
    /// needs a payload, and here is what that payload is", so a binding validator refuses a scalar
    /// wire to it.
    ///
    /// It carries no `kind` because the payload is not of one kind; [`describe_command`] names the
    /// shape meaningfully. No command in this build publishes one — every published `set_*`
    /// command writes exactly one property (see [`command_schema`]) — but the shape stays part of
    /// the schema language, so a manifest can describe a richer payload if a control grows one.
    Composite {
        /// The name of the payload, for a designer's inspector (`"lower"`, `"items"`).
        parameter: &'static str,
        /// How many values the payload carries, or `None` for a variable-length list.
        arity: Option<usize>,
        /// The declared kind of each element, when the payload is homogeneous.
        element: Option<super::types::PropertyValueKind>,
    },
}

/// One command's parameter shape and effect.
///
/// # Why the name is not a field
///
/// The name is the *key* a caller looks a schema up by, and it is already in the caller's hand.
/// Storing it here forced a choice between a `&'static str` (impossible for a `set_*` command,
/// whose name is composed at run time) and an allocation (which the designer paths do not need).
/// Leaving it out makes the schema a pure description of the *shape*; [`describe_command`] takes
/// the name alongside it when a rendered line is wanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSchema {
    /// The parameter shape.
    pub signature: CommandSignature,
    /// What invoking it does.
    pub effect: CommandEffect,
}

impl CommandSchema {
    /// Whether the command takes no argument, and so may be the target of an event wire.
    pub const fn is_payload_free(&self) -> bool {
        matches!(self.signature, CommandSignature::PayloadFree)
    }

    /// The parameter name and kind, or `None` for a payload-free command.
    ///
    /// A composite command's payload is not one value, so it answers `None` here; ask
    /// [`Self::signature`] for the full shape.
    pub const fn parameter(&self) -> Option<(&'static str, super::types::PropertyValueKind)> {
        match self.signature {
            CommandSignature::PayloadFree | CommandSignature::Composite { .. } => None,
            CommandSignature::OneValue { parameter, kind } => Some((parameter, kind)),
        }
    }
}

/// The classification of a non-`set_` verb.
///
/// A closed struct rather than a bare `(name, signature, effect)` tuple so a reader at the table
/// sees what each field means without counting columns.
struct VerbClass {
    name: &'static str,
    signature: CommandSignature,
    effect: CommandEffect,
}

/// The `set_` prefix every property-route command carries.
const SET_PREFIX: &str = "set_";

/// The explicit classification of the non-`set_` verbs.
///
/// # Scope
///
/// Only verbs that do **not** begin with `set_` need an entry: a `set_*` command's signature is
/// derived from the control's property list (see the module doc), so those 176 names are absent
/// here on purpose — listing them would be the duplicate declaration this module exists to avoid.
///
/// # The effect column
///
/// `MutatesControl` is the default stance for a verb that acts on the control itself. A verb is
/// `SideEffecting` when it reaches past the control (navigation, loading), and
/// `MutatesCollection` when it changes a list the user perceives as content rather than state.
/// The distinction is what lets a designer label a wire honestly; it does not change dispatch.
///
/// # Completeness
///
/// `published_commands_all_have_a_schema` fails if a published command is neither `set_`-prefixed
/// with a real property nor present here, so a newly published verb is caught rather than silently
/// described as whatever the fallback happens to be.
static VERB_CLASSES: &[VerbClass] = &[
    // ── Selection / focus: move the pointer, change what is highlighted ──
    verb("activate_action", PayloadFree, CommandEffect::MutatesControl),
    verb("activate_highlighted", PayloadFree, CommandEffect::MutatesControl),
    verb("activate_selected", PayloadFree, CommandEffect::MutatesControl),
    verb("back", PayloadFree, CommandEffect::SideEffecting),
    verb("cancel_drag", PayloadFree, CommandEffect::MutatesControl),
    // `tile_loader` (COMP-13): drops every cached tile. It only empties the loader's own
    // cache — refetching is the host's answer to the next `tile_loaded` request — so it is
    // a control-local mutation, not a side effect.
    verb("clear_cache", PayloadFree, CommandEffect::MutatesControl),
    verb("clear_focused_node", PayloadFree, CommandEffect::MutatesControl),
    verb("clear_focused_row", PayloadFree, CommandEffect::MutatesControl),
    verb("clear_selection", PayloadFree, CommandEffect::MutatesControl),
    verb("collapse", PayloadFree, CommandEffect::MutatesControl),
    verb("expand", PayloadFree, CommandEffect::MutatesControl),
    verb("next", PayloadFree, CommandEffect::MutatesControl),
    verb("previous", PayloadFree, CommandEffect::MutatesControl),
    verb("previous_page", PayloadFree, CommandEffect::MutatesControl),
    verb("next_page", PayloadFree, CommandEffect::SideEffecting),
    verb("select_all", PayloadFree, CommandEffect::MutatesControl),
    verb("clear", PayloadFree, CommandEffect::MutatesControl),
    // SP-121: the chip input commits its pending text into the chip band. It changes only the
    // control's own collection, the same class as `clear`.
    verb("commit", PayloadFree, CommandEffect::MutatesControl),
    verb("toggle", PayloadFree, CommandEffect::MutatesControl),
    verb("toggle_preview_mode", PayloadFree, CommandEffect::MutatesControl),
    verb("toggle_shift", PayloadFree, CommandEffect::MutatesControl),
    verb("toggle_conjunction", PayloadFree, CommandEffect::MutatesControl),
    verb("step_down", PayloadFree, CommandEffect::MutatesControl),
    verb("step_up", PayloadFree, CommandEffect::MutatesControl),
    verb("dismiss_selected", PayloadFree, CommandEffect::MutatesControl),
    verb("zoom", PayloadFree, CommandEffect::MutatesControl),
    // ── Activation / action verbs ──
    verb("accept", PayloadFree, CommandEffect::MutatesControl),
    verb("click", PayloadFree, CommandEffect::MutatesControl),
    verb("complete", PayloadFree, CommandEffect::MutatesControl),
    verb("dismiss", PayloadFree, CommandEffect::MutatesControl),
    verb("finish", PayloadFree, CommandEffect::MutatesControl),
    verb("finish_editing", PayloadFree, CommandEffect::MutatesControl),
    verb("start_editing", PayloadFree, CommandEffect::MutatesControl),
    verb("reject", PayloadFree, CommandEffect::MutatesControl),
    verb("submit", PayloadFree, CommandEffect::MutatesControl),
    verb("trigger", PayloadFree, CommandEffect::MutatesControl),
    verb("trigger_primary", PayloadFree, CommandEffect::MutatesControl),
    verb("undo", PayloadFree, CommandEffect::MutatesControl),
    verb("redo", PayloadFree, CommandEffect::MutatesControl),
    verb("show_today", PayloadFree, CommandEffect::MutatesControl),
    // ── Visibility / open-close ──
    verb("hide", PayloadFree, CommandEffect::MutatesControl),
    verb("show", PayloadFree, CommandEffect::MutatesControl),
    verb("open", PayloadFree, CommandEffect::MutatesControl),
    verb("close", PayloadFree, CommandEffect::MutatesControl),
    verb("open_menu", PayloadFree, CommandEffect::MutatesControl),
    verb("close_menu", PayloadFree, CommandEffect::MutatesControl),
    verb("open_popup", PayloadFree, CommandEffect::MutatesControl),
    verb("hide_popup", PayloadFree, CommandEffect::MutatesControl),
    verb("show_popup", PayloadFree, CommandEffect::MutatesControl),
    // ── Media transport ──
    verb("play", PayloadFree, CommandEffect::MutatesControl),
    verb("pause", PayloadFree, CommandEffect::MutatesControl),
    verb("stop", PayloadFree, CommandEffect::MutatesControl),
    // ── Text editing ──
    verb("backspace", PayloadFree, CommandEffect::MutatesControl),
    verb("close_find", PayloadFree, CommandEffect::MutatesControl),
    verb("copy", PayloadFree, CommandEffect::MutatesControl),
    verb("cut", PayloadFree, CommandEffect::MutatesControl),
    verb("delete_char", PayloadFree, CommandEffect::MutatesControl),
    verb("find", PayloadFree, CommandEffect::MutatesControl),
    verb("find_next", PayloadFree, CommandEffect::MutatesControl),
    verb("find_previous", PayloadFree, CommandEffect::MutatesControl),
    verb("insert_char", PayloadFree, CommandEffect::MutatesControl),
    verb("paste", PayloadFree, CommandEffect::MutatesControl),
    verb("replace", PayloadFree, CommandEffect::MutatesControl),
    // ── Stack / list structure: these change content the user perceives ──
    verb("pop", PayloadFree, CommandEffect::MutatesCollection),
    verb("insert", PayloadFree, CommandEffect::MutatesCollection),
    verb("clear_segments", PayloadFree, CommandEffect::MutatesCollection),
    verb("add_separator", PayloadFree, CommandEffect::MutatesCollection),
    verb("add_overlay", PayloadFree, CommandEffect::MutatesCollection),
    verb("clear_model", PayloadFree, CommandEffect::MutatesCollection),
    verb("clear_data_source", PayloadFree, CommandEffect::MutatesCollection),
    verb("clear_source", PayloadFree, CommandEffect::MutatesCollection),
    verb("clear_query", PayloadFree, CommandEffect::MutatesCollection),
    verb("scroll_rows", PayloadFree, CommandEffect::MutatesControl),
    verb("mark_all_read", PayloadFree, CommandEffect::MutatesCollection),
    verb("fetch_visible_rows", PayloadFree, CommandEffect::MutatesControl),
    verb("fetch_visible_window", PayloadFree, CommandEffect::MutatesControl),
    // ── Editor buffer structure: edit the lines the user perceives as content ──
    verb("delete_line", PayloadFree, CommandEffect::MutatesCollection),
    verb("duplicate_line", PayloadFree, CommandEffect::MutatesCollection),
    verb("indent", PayloadFree, CommandEffect::MutatesCollection),
    verb("outdent", PayloadFree, CommandEffect::MutatesCollection),
    verb("sort_lines", PayloadFree, CommandEffect::MutatesCollection),
    verb("toggle_block_comment", PayloadFree, CommandEffect::MutatesCollection),
    verb("toggle_comment", PayloadFree, CommandEffect::MutatesCollection),
    // Folding is view state over the buffer rather than content.
    verb("fold_all", PayloadFree, CommandEffect::MutatesControl),
    verb("unfold_all", PayloadFree, CommandEffect::MutatesControl),
    // ── Navigation / loading: reach past the control ──
    verb("go_back", PayloadFree, CommandEffect::SideEffecting),
    verb("go_forward", PayloadFree, CommandEffect::SideEffecting),
    verb("reload", PayloadFree, CommandEffect::SideEffecting),
];

/// Shorthand constructor for a [`VerbClass`] entry, so the table above reads as one line per verb.
const fn verb(name: &'static str, signature: CommandSignature, effect: CommandEffect) -> VerbClass {
    VerbClass { name, signature, effect }
}

/// The schema for one command of one control, or `None` when the command is unrecognised.
///
/// # Why the property list is an argument
///
/// A `set_foo` command's parameter type is the type of property `foo`, which differs per control
/// (`set_value` is a value on a slider, a count on a list). Passing the control's own property
/// list — which is what the dispatcher consults — means the schema reports the control's real
/// type rather than a guess, and means the two cannot disagree.
///
/// # Errors as `None`
///
/// `None` means "this name is not a command of a control with this property list", which the
/// caller renders as `UnknownCommand`. It is not an error value because the caller (a validator)
/// has the same answer either way.
pub fn command_schema(
    name: &str,
    properties: &[super::types::PropertySchema],
) -> Option<CommandSchema> {
    // The `set_` route: the value lands in a property, so the schema is that property's. A
    // `set_foo` naming no property `foo` of this control is described by nothing — the same
    // answer the dispatcher gives (`UnknownCommand`) — so a published `set_*` command is always
    // one property wide, and its kind is the control's own declaration.
    if let Some(property_name) = name.strip_prefix(SET_PREFIX) {
        return properties.iter().find(|schema| schema.name == property_name).map(|schema| {
            CommandSchema {
                signature: CommandSignature::OneValue {
                    parameter: schema.name,
                    kind: schema.value_kind,
                },
                effect: CommandEffect::MutatesControl,
            }
        });
    }

    // The verb table: the one thing the convention cannot derive.
    VERB_CLASSES
        .iter()
        .find(|class| class.name == name)
        .map(|class| CommandSchema { signature: class.signature, effect: class.effect })
}

/// Whether `name` is a command this build knows how to describe.
///
/// The predicate a designer calls before offering a command in a menu, so an unclassified name is
/// refused rather than offered with an invented signature.
pub fn is_described_command(name: &str, properties: &[super::types::PropertySchema]) -> bool {
    command_schema(name, properties).is_some()
}

/// Every verb this build classifies, for a report or a completeness check.
pub fn described_verbs() -> Vec<&'static str> {
    VERB_CLASSES.iter().map(|class| class.name).collect()
}

/// The effect words a manifest may carry, for a consumer that validates a document.
pub fn effect_tokens() -> Vec<&'static str> {
    vec![
        CommandEffect::MutatesControl.token(),
        CommandEffect::MutatesCollection.token(),
        CommandEffect::SideEffecting.token(),
    ]
}

/// Renders a command schema the way it would appear in a designer's inspector.
///
/// `name` is passed rather than read from the schema because a `set_*` command's name is composed
/// at run time and is not stored — see [`CommandSchema`].
pub fn describe_command(name: &str, schema: &CommandSchema) -> String {
    match schema.signature {
        CommandSignature::PayloadFree => {
            crate::compat::format!("{name}() — {}", schema.effect.token())
        }
        CommandSignature::OneValue { parameter, kind } => crate::compat::format!(
            "{name}({parameter}: {}) — {}",
            super::designer_manifest::value_kind_token(kind),
            schema.effect.token()
        ),
        CommandSignature::Composite { parameter, arity, element } => {
            let elements = match element {
                Some(kind) => crate::compat::format!(
                    " of {}",
                    super::designer_manifest::value_kind_token(kind)
                ),
                None => String::new(),
            };
            match arity {
                Some(count) => crate::compat::format!(
                    "{name}({parameter}: {count} values{elements}) — {}",
                    schema.effect.token()
                ),
                None => crate::compat::format!(
                    "{name}({parameter}: list{elements}) — {}",
                    schema.effect.token()
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::capability::types::PropertySchema;

    /// A `set_foo` command must describe itself from the property it writes.
    #[test]
    fn a_set_command_takes_its_property_kind() {
        static PROPS: &[PropertySchema] = &[PropertySchema::new(
            "value",
            super::super::types::PropertyValueKind::UInt,
            true,
            true,
        )];
        let schema = command_schema("set_value", PROPS).expect("set_value is described");
        assert_eq!(
            schema.parameter(),
            Some(("value", super::super::types::PropertyValueKind::UInt))
        );
        assert!(!schema.is_payload_free());
    }

    /// A `set_foo` command naming no property of the control must not be described — the same
    /// answer the dispatcher gives (`UnknownCommand`), so the two cannot disagree.
    #[test]
    fn a_set_command_for_a_missing_property_is_not_described() {
        static PROPS: &[PropertySchema] = &[PropertySchema::new(
            "text",
            super::super::types::PropertyValueKind::String,
            true,
            true,
        )];
        // `set_turbo` names no property of the control, so there is nothing to describe it from.
        assert!(command_schema("set_turbo", PROPS).is_none());
    }

    /// A payload-free verb must describe as payload-free, and one that is not in the table must
    /// not be described at all — an unclassified name is refused rather than invented.
    #[test]
    fn verbs_are_described_from_the_table_only() {
        static NO_PROPS: &[PropertySchema] = &[];
        let toggle = command_schema("toggle", NO_PROPS).expect("toggle is a known verb");
        assert!(toggle.is_payload_free());
        assert_eq!(toggle.effect, CommandEffect::MutatesControl);
        assert!(command_schema("frobnicate_the_widget", NO_PROPS).is_none());
    }

    /// Every effect must round-trip through its token.
    #[test]
    fn effects_round_trip() {
        for effect in [
            CommandEffect::MutatesControl,
            CommandEffect::MutatesCollection,
            CommandEffect::SideEffecting,
        ] {
            assert_eq!(CommandEffect::from_token(effect.token()), Some(effect));
        }
        assert_eq!(CommandEffect::from_token("no_such_effect"), None);
    }

    /// The verb table must not contain a duplicate, because a duplicate would make the *first*
    /// entry win silently and hide a later, different classification.
    #[test]
    fn the_verb_table_has_no_duplicates() {
        let mut names: Vec<&str> = VERB_CLASSES.iter().map(|class| class.name).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "the verb table declares a verb twice: {names:?}");
    }

    /// A verb must not be declared with a `set_` prefix, which would shadow the derived route.
    #[test]
    fn no_verb_uses_the_set_prefix() {
        for class in VERB_CLASSES {
            assert!(
                !class.name.starts_with(SET_PREFIX),
                "`{}` belongs in the derived route, not the verb table",
                class.name
            );
        }
    }

    /// `describe_command` must render both shapes as a non-empty line.
    #[test]
    fn describe_renders_both_shapes() {
        static PROPS: &[PropertySchema] = &[PropertySchema::new(
            "text",
            super::super::types::PropertyValueKind::String,
            true,
            true,
        )];
        let one =
            describe_command("set_text", &command_schema("set_text", PROPS).expect("set_text"));
        assert!(one.contains("set_text(text: string)"), "{one}");
        let none = describe_command("toggle", &command_schema("toggle", &[]).expect("toggle"));
        assert!(none.contains("toggle()"), "{none}");
    }
}
