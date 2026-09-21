// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The type-compatibility table a designer needs to validate a wire (BLUE19 step 3).
//!
//! # What problem this solves
//!
//! A designer lets a user draw a line from an event to a target. Some of those lines are
//! meaningful and some are not, and the distinction is a **type** question: `value_changed`
//! carries an `Int`, so it can drive a numeric property directly, and a `String`-carrying
//! `text_changed` cannot — "42" is not a number until something parses it, and parsing can fail.
//!
//! # Why the table is data rather than a `match`
//!
//! There are two consumers of this rule set:
//!
//! 1. **The runtime** (`src/json/`, `src/view/`), which validates a wire when a project loads;
//! 2. **The generator** (BLUE19 T-23), which must decide at *generation* time whether to emit a
//!    direct assignment or a conversion, because a generated program cannot ask a question the
//!    generator already knows the answer to.
//!
//! A `match` inside the runtime would be write-once-read-once for consumer 1 and invisible to
//! consumer 2, so the two would drift into disagreeing about which wires are legal — and a project
//! the designer accepted would fail to generate. The rules below are therefore an array of values,
//! walked by both.
//!
//! # What D5/D6 decided (recorded here, since the plan left them open)
//!
//! * **D5 — where a binding is written: JSON, not CSS.** CSS is the *appearance* channel
//!   (`src/style/css.rs`) and structure is JSON (`src/json/`); the library already draws that line
//!   (see `blue19.md` §0.1.1). Putting event bindings in CSS would have made the two channels
//!   overlap, and a stylesheet re-applied on a theme change would have re-created subscriptions.
//!   CSS gains nothing here: it has no notion of a target property to assign to.
//! * **D6 — a wire's target may be a **property**, and a command only where the command is
//!   payload-free.** The compatibility question below is "can this event's value reach this
//!   target?", which a property answers with a type. A command that needs a payload would have to
//!   be given one, and a command's *parameter type* is not part of the capability table, so
//!   validating such a wire would be guessing. `CommandTarget::PayloadFree` is therefore the only
//!   command form the table admits, and the reason is stated rather than implied.

/// What a wire's value arrives at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireTarget {
    /// A writable property, described by the kind of value it accepts.
    Property(crate::widget::capability::types::PropertyValueKind),
    /// A command that takes no payload.
    ///
    /// The only command form a wire may name — see the module doc for D6. A command taking a
    /// payload has no declared parameter type to check against, so admitting it would make this
    /// table answer a question it cannot answer.
    PayloadFreeCommand,
}

/// Whether a wire from an event to a target is legal, and if not, why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireCompatibility {
    /// The value reaches the target unchanged.
    Direct,
    /// The value reaches the target after a defined, lossless-enough conversion (a number
    /// formatted as text).
    Converted,
    /// The value cannot reach the target. The payload carries the reason.
    Rejected(&'static str),
}

impl WireCompatibility {
    /// Whether a designer should offer this wire at all.
    pub fn is_offered(self) -> bool {
        !matches!(self, Self::Rejected(_))
    }
}

/// How one event payload kind relates to one target kind.
///
/// `Option` on `payload: None` (a payload-free event) so the rule set can be walked uniformly;
/// a payload-free event is expressed as a `None` source, which the table handles explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireRule {
    /// `None` = a payload-free event (`clicked`).
    pub source: Option<crate::widget::capability::types::PropertyValueKind>,
    /// The kind the target accepts.
    pub target: crate::widget::capability::types::PropertyValueKind,
    /// The verdict.
    pub verdict: WireCompatibility,
    /// Why, in the words a designer shows a user.
    pub reason: &'static str,
}

use crate::widget::capability::types::PropertyValueKind as K;

/// The rule set, as data. Walked by [`compatibility`]; also readable by the generator.
///
/// # The shape of the table
///
/// `Direct` means the value is already the target's type. `Converted` means a *widening* conversion
/// that cannot fail. Anything absent from this array is [`WireCompatibility::Rejected`] — the table
/// is a whitelist, so a kind added to `PropertyValueKind` later is rejected until someone states
/// how it converts. That direction matters: the opposite default (accept unknown pairs) would let a
/// new kind silently produce wires that cannot run.
pub const WIRE_RULES: &[WireRule] = &[
    // ── same kind: the value is already the right type ──
    rule(Some(K::Bool), K::Bool, Direct, "the event's boolean is already the property's type"),
    rule(Some(K::Int), K::Int, Direct, "the event's integer is already the property's type"),
    rule(Some(K::UInt), K::UInt, Direct, "the event's integer is already the property's type"),
    rule(Some(K::Float), K::Float, Direct, "the event's number is already the property's type"),
    rule(Some(K::String), K::String, Direct, "the event's text is already the property's type"),
    rule(Some(K::Color), K::Color, Direct, "the event's colour is already the property's type"),
    rule(Some(K::Rect), K::Rect, Direct, "the event's rectangle is already the property's type"),
    rule(Some(K::Enum), K::Enum, Direct, "both sides use the same enumerated token"),
    // ── numeric widening between integer kinds ──
    rule(Some(K::Int), K::UInt, Direct, "an integer fits an unsigned property in this range"),
    rule(Some(K::UInt), K::Int, Direct, "an unsigned integer fits a signed property"),
    rule(Some(K::Int), K::Float, Direct, "an integer is exactly representable as a number"),
    rule(
        Some(K::UInt),
        K::Float,
        Direct,
        "an unsigned integer is exactly representable as a number",
    ),
    // ── anything can be *displayed* as text; this is the one widening conversion allowed ──
    rule(Some(K::Bool), K::String, Converted, "a boolean is formatted as text for display"),
    rule(Some(K::Int), K::String, Converted, "an integer is formatted as text for display"),
    rule(
        Some(K::UInt),
        K::String,
        Converted,
        "an unsigned integer is formatted as text for display",
    ),
    rule(Some(K::Float), K::String, Converted, "a number is formatted as text for display"),
    rule(Some(K::Color), K::String, Converted, "a colour is written as its `#rrggbbaa` token"),
    rule(Some(K::Rect), K::String, Converted, "a rectangle is written as its `x,y,w,h` token"),
    rule(Some(K::Enum), K::String, Converted, "an enumerated token is already text"),
];

/// A wire from a payload-free event to a payload-free command.
///
/// Separate from the array above because there is no *value* to compare: the wire exists or it does
/// not. Kept as a named rule rather than an `if` in [`compatibility`] so both consumers see it.
pub fn payload_free_command_verdict() -> WireCompatibility {
    WireCompatibility::Direct
}

const fn rule(
    source: Option<K>,
    target: K,
    verdict: WireCompatibility,
    reason: &'static str,
) -> WireRule {
    WireRule { source, target, verdict, reason }
}

/// The verdict for a wire carrying `source` into `target`.
///
/// # Why rejection carries a reason
///
/// A designer that greys out a wire without saying why teaches the user nothing, and a generator
/// that refuses without a reason produces a build error nobody can act on. The reason is part of
/// the contract, which is why [`WireCompatibility::Rejected`] holds a `&'static str` rather than
/// being a unit variant.
pub fn compatibility(
    source: Option<crate::widget::capability::types::PropertyValueKind>,
    target: WireTarget,
) -> WireCompatibility {
    match target {
        WireTarget::PayloadFreeCommand => match source {
            None => payload_free_command_verdict(),
            Some(kind) => WireCompatibility::Rejected(payload_free_command_reason(kind)),
        },
        WireTarget::Property(target_kind) => {
            for candidate in WIRE_RULES {
                if candidate.source == source && candidate.target == target_kind {
                    return candidate.verdict;
                }
            }
            WireCompatibility::Rejected(rejection_reason(source, target_kind))
        }
    }
}

/// Why a payload-carrying event cannot start a payload-free command.
///
/// Returned as a `&'static str` because the message does not vary by kind: what varies is the
/// kind's *name*, which the designer already displays next to the wire.
fn payload_free_command_reason(
    _kind: crate::widget::capability::types::PropertyValueKind,
) -> &'static str {
    "this command takes no payload, so a value-carrying event has nowhere to put its value; \
     use a property target or a command that accepts one"
}

/// Why a pair the table does not list is rejected.
///
/// # The specific case the reason names first
///
/// `String → Int` is the one users hit, and it is rejected **on purpose**: parsing text into a
/// number can fail, and a failed parse is a runtime error that a designer's simple wire has no way
/// to report. A project that needs it must say so with an explicit conversion node, which is a
/// decision the author makes rather than one the tool makes silently.
fn rejection_reason(
    source: Option<crate::widget::capability::types::PropertyValueKind>,
    target: crate::widget::capability::types::PropertyValueKind,
) -> &'static str {
    use crate::widget::capability::types::PropertyValueKind as Kind;
    match (source, target) {
        (Some(Kind::String), Kind::Int | Kind::UInt | Kind::Float) => {
            "text cannot be assigned to a number: parsing can fail, and a failed parse has no \
             defined result here; add an explicit conversion"
        }
        (Some(_), _) => {
            "this event's value type does not match the property's type, and no conversion is \
             defined for the pair"
        }
        (None, _) => {
            "this event carries no value, so it cannot assign a property; target a \
             payload-free command instead"
        }
    }
}

// Convenience re-exports, so the table and the tests do not repeat the enum's path on every line.
// `Rejected` is named only in tests, so it is imported there rather than left unused here
// (an unused import in the production namespace trips `-D warnings` on the cross-target gate).
use WireCompatibility::{Converted, Direct};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::capability::types::PropertyValueKind as Kind;
    use WireCompatibility::Rejected;

    fn prop(kind: Kind) -> WireTarget {
        WireTarget::Property(kind)
    }

    #[test]
    fn a_matching_kind_is_direct() {
        for kind in [
            Kind::Bool,
            Kind::Int,
            Kind::UInt,
            Kind::Float,
            Kind::String,
            Kind::Color,
            Kind::Rect,
            Kind::Enum,
        ] {
            assert_eq!(
                compatibility(Some(kind), prop(kind)),
                Direct,
                "{kind:?} → {kind:?} must be offered"
            );
        }
    }

    #[test]
    fn a_number_can_be_displayed_as_text() {
        for kind in [Kind::Int, Kind::UInt, Kind::Float] {
            assert_eq!(
                compatibility(Some(kind), prop(Kind::String)),
                Converted,
                "{kind:?} → text is a formatting step, not a rejection"
            );
        }
    }

    /// The rejection branch BLUE19 §3.3 names by hand: text into a number must be refused.
    #[test]
    fn text_into_a_number_is_rejected_with_that_reason() {
        for target in [Kind::Int, Kind::UInt, Kind::Float] {
            let verdict = compatibility(Some(Kind::String), prop(target));
            match verdict {
                Rejected(reason) => assert!(
                    reason.contains("parsing can fail"),
                    "the rejection must name the actual hazard, got: {reason}"
                ),
                other => panic!("text → {target:?} must be rejected, got {other:?}"),
            }
            assert!(!verdict.is_offered());
        }
    }

    #[test]
    fn a_payload_free_event_cannot_assign_a_property() {
        // A payload-free event (`clicked`) says *that* something happened, not *what* the value
        // should become. ``on_click → set this checkbox`` is a rule the user wants, and it is not
        // this table's job: a boolean target implies a `true` the event never carried, and
        // inventing that value here would be the tool deciding what the user meant. A wire from a
        // payload-free event goes to a payload-free **command**, which is an action rather than an
        // assignment -- which is exactly why D6 admits that target.
        for target in [
            Kind::Bool,
            Kind::Int,
            Kind::UInt,
            Kind::Float,
            Kind::String,
            Kind::Color,
            Kind::Rect,
            Kind::Enum,
        ] {
            let verdict = compatibility(None, prop(target));
            assert!(
                matches!(verdict, Rejected(_)),
                "a payload-free event carries no value, so it cannot assign {target:?}"
            );
        }
        // The honest alternative is offered instead, and it is a different kind of target.
        assert_eq!(compatibility(None, WireTarget::PayloadFreeCommand), Direct);
    }

    #[test]
    fn a_value_carrying_event_cannot_start_a_payload_free_command() {
        let verdict = compatibility(Some(Kind::Int), WireTarget::PayloadFreeCommand);
        match verdict {
            Rejected(reason) => {
                assert!(reason.contains("no payload"), "the reason must say why, got: {reason}")
            }
            other => {
                panic!("a payload-carrying event cannot feed a payload-free command: {other:?}")
            }
        }
        assert_eq!(compatibility(None, WireTarget::PayloadFreeCommand), Direct);
    }

    /// The table is a whitelist: a pair it does not list is rejected rather than assumed fine.
    #[test]
    fn an_unlisted_pair_is_rejected_not_assumed_compatible() {
        // `Rect → Bool` is nonsense and no rule mentions it, so it must not be `Direct`.
        assert!(matches!(compatibility(Some(Kind::Rect), prop(Kind::Bool)), Rejected(_)));
        assert!(matches!(compatibility(Some(Kind::Bool), prop(Kind::Rect)), Rejected(_)));
    }

    /// Every rule must carry a reason, so the designer never shows an unexplained verdict.
    #[test]
    fn every_rule_states_a_reason() {
        for rule in WIRE_RULES {
            assert!(!rule.reason.trim().is_empty(), "rule {rule:?} has no reason");
        }
    }

    /// No two rules may describe the same pair, or the first one would silently win.
    #[test]
    fn the_rule_table_has_no_duplicate_pairs() {
        for (index, first) in WIRE_RULES.iter().enumerate() {
            for second in &WIRE_RULES[index + 1..] {
                assert!(
                    !(first.source == second.source && first.target == second.target),
                    "two rules describe {:?} → {:?}; the later one is unreachable",
                    first.source,
                    second.target
                );
            }
        }
    }
}
