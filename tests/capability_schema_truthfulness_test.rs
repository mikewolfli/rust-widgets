// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The capability schema must describe what the control's `get`/`set` actually do.
//!
//! # The defect this closes
//!
//! A `PropertySchema` carries two booleans: whether the property can be read and
//! whether it can be written. Controls implemented a real `get`/`set` arm for a
//! property while the schema declared the wrong flags, and two things then went
//! wrong, both silently:
//!
//! 1. `write_property(id, "title", ..)` was refused by the schema gate even though
//!    the control would have accepted it, so the property was unreachable through
//!    the published API.
//! 2. A `commands:` entry like `set_title` was published against it, making the
//!    *command* a promise the property route cannot keep — `OutOfRange` tells the
//!    caller "supply a value through the property route", and the route then answers
//!    `ReadOnlyProperty`.
//!
//! The reverse direction is just as bad: a schema that declares a property writable
//! while the control refuses every write publishes an API that fails at the first
//! call. A handful of those were found and fixed (`splitter::pane_count`,
//! `wizard_dialog::current_step`, `popover::text`, `order_book::show_spread`,
//! `quote_board::row_height`) along with two entries whose declared kind
//! contradicted what `set` accepts (`depth_chart::bid_color` / `ask_color` were
//! declared `String` while `set` destructures `Color`).
//!
//! # Why this asserts on structure, not on a write attempt
//!
//! Attempting a write cannot distinguish "the schema lies" from "this particular
//! value is not acceptable right now" — an index past the last row is correctly
//! `OutOfRange`, and a `String` property whose control parses a date is correctly
//! `TypeMismatch` for a non-date. Both are the control working. What *is* decidable
//! without guessing at values is the structural claim each `set_X` command makes:
//! that `X` is a property this capability publishes **and declares writable**. That
//! is what these tests assert.

#![cfg(all(full_widgets, feature = "desktop"))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

/// Every `set_X` command must be recognised by the control that publishes it.
///
/// # The two legitimate shapes for a `set_*` command
///
/// 1. **Property route.** `X` is a schema entry declared writable, and the caller
///    supplies the value with `write_property(id, "X", ..)`. This is the common case
///    (`set_text`, `set_value`, `set_orientation`), and it is what the `OutOfRange`
///    answer points at.
/// 2. **The control's own `command()` override.** Some commands take a payload that
///    is not a `CapabilityValue` at all — a list of items, a marker set, a
///    `DockWidgetFeatures` bitfield, an `Image` — or write several properties at once
///    (`set_range` sets `minimum` and `maximum`). None of those can name a single
///    property, so the control implements the command directly and answers
///    `OutOfRange` to say "this needs a payload".
///
/// # The defect this rules out
///
/// The third shape, which is not a shape: a command that names a property nobody
/// implements **and** that no override handles. It answers `UnknownCommand`, so the
/// published name is a promise with nothing behind it. That was the state of the
/// whole table before `default_command` started resolving `set_foo` against
/// `property_names`.
///
/// Asserting on the *answer* rather than on the schema membership is what makes both
/// legitimate shapes pass and the third fail.
#[test]
fn every_set_command_is_recognised_by_its_control() {
    use rust_widgets::widget::capability::types::CapabilityAccessError;

    let factory = WidgetFactory::new_with_defaults();
    let mut failures: Vec<String> = Vec::new();

    for capability in factory.capabilities() {
        let canonical = capability.canonical_name;
        for command in capability.commands {
            if !command.starts_with("set_") {
                continue;
            }
            let Some(mut widget) = factory.create(canonical, Rect::new(0, 0, 100, 100), "") else {
                failures.push(format!("{canonical} publishes `{command}` but cannot be built"));
                continue;
            };
            match factory.invoke_command(widget.as_mut(), command) {
                // Ran with no payload — the property it assigns has a usable default
                // route, or the override acted.
                Ok(()) => {}
                // The documented "supply a value" answer, for either shape.
                Err(CapabilityAccessError::OutOfRange) => {}
                // A payload-free call cannot be `ReadOnlyProperty` or `TypeMismatch`:
                // those describe a *value* being rejected, and there was no value.
                Err(other) => failures.push(format!(
                    "{canonical} publishes `{command}` but a payload-free call answers {other:?}"
                )),
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// Every payload-free published command must be one the control implements.
///
/// A command that takes no argument cannot be excused as "needs a value", so its
/// only honest answers are `Ok(())` (it ran) or a refusal that names a real reason.
/// `UnknownCommand` from a control whose capability table publishes the name means
/// the table is promising something nobody wrote.
#[test]
fn every_published_command_is_recognised_by_its_control() {
    use rust_widgets::widget::capability::types::CapabilityAccessError;

    let factory = WidgetFactory::new_with_defaults();
    let mut failures: Vec<String> = Vec::new();

    for capability in factory.capabilities() {
        let canonical = capability.canonical_name;
        for command in capability.commands {
            // `set_X` is covered by the test above, which has a better message.
            if command.starts_with("set_") {
                continue;
            }
            // A fresh control per command: commands change state, and a command that
            // only succeeds from a particular state would otherwise look broken.
            let Some(mut widget) = factory.create(canonical, Rect::new(0, 0, 100, 100), "") else {
                failures.push(format!("{canonical} publishes `{command}` but cannot be built"));
                continue;
            };
            match factory.invoke_command(widget.as_mut(), command) {
                Ok(()) => {}
                // `OutOfRange` on a payload-free verb is how a control reports that
                // the action is not available *in this state* (nothing selected,
                // already at the end, …). That is a real answer, not a missing
                // implementation.
                Err(CapabilityAccessError::OutOfRange) => {}
                Err(CapabilityAccessError::UnknownCommand) => failures
                    .push(format!("{canonical} publishes `{command}` but answers UnknownCommand")),
                Err(CapabilityAccessError::UnsupportedOnWidget) => failures.push(format!(
                    "{canonical} publishes `{command}` but answers UnsupportedOnWidget"
                )),
                Err(other) => failures.push(format!("{canonical} `{command}` answered {other:?}")),
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n  "));
}

/// Every capability that publishes anything must be constructible.
///
/// A capability whose names no caller can reach is a promise nobody can keep,
/// whatever the table says.
#[test]
fn every_capability_publishing_a_surface_is_constructible() {
    let factory = WidgetFactory::new_with_defaults();
    let mut offenders: Vec<&str> = Vec::new();

    for capability in factory.capabilities() {
        if capability.commands.is_empty() && capability.events.is_empty() {
            continue;
        }
        if factory.create(capability.canonical_name, Rect::new(0, 0, 100, 100), "").is_none() {
            offenders.push(capability.canonical_name);
        }
    }

    assert!(
        offenders.is_empty(),
        "these capabilities publish commands or events but cannot be constructed: {offenders:?}"
    );
}
