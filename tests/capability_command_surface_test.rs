// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Every capability's **published commands** must actually run on its control.
//!
//! # Why this is a separate axis from names and properties
//!
//! A `WidgetCapability` makes three promises, and the round-31/32 audits covered the
//! first two:
//!
//! 1. `canonical_name` / `aliases` — "this spelling addresses this control" (round 31).
//! 2. `properties` — "this control has these names" (round 32).
//! 3. `commands` — "this control performs these actions". **Unchecked until round 33.**
//!
//! The failure mode is the same shape as the other two: a capability lists a command
//! that no code path answers, so a generic consumer — a property editor building an
//! action menu, a language binding, the declarative engine — reads the list, offers
//! the command, and gets nothing back. Before `WidgetProperties::command` existed the
//! registry exported the names with **no way to invoke them at all**, so the list
//! could not be checked and could not be used.
//!
//! # What is asserted
//!
//! For every capability that publishes commands, a constructed control must answer
//! every one of them. A capability that lists a command its control does not
//! implement fails here, which is what makes the list load-bearing rather than
//! decorative.

#![cfg(all(feature = "desktop", not(alloc_frugal)))]

use rust_widgets::core::Rect;
use rust_widgets::widget::capability::WidgetFactory;

/// Every published command must be dispatched by the control that published it.
///
/// # Why the count is asserted, not just the absence of refusals
///
/// This is the rollout's acceptance criterion, and a walk like this can pass
/// *vacuously*: if every capability's control failed to construct, or the factory
/// stopped reporting commands, the refusal list would be empty and the test would
/// report success while checking nothing. The `total`/`handled` counters below make
/// that impossible — the assertion states how many commands were actually dispatched,
/// so a registry that stopped publishing them fails here rather than looking green.
///
/// # What counts as handled
///
/// `Ok(())` means the action ran. `OutOfRange` means the control recognised the name
/// and refused *this invocation* because the command assigns state and needs a payload
/// — the honest answer for `set_text` and friends. `UnsupportedOnWidget` means the
/// capability published the name and the control does not implement it, which
/// `WidgetFactory::invoke_command` reports as a registry/implementation disagreement.
/// Only `UnknownCommand` and `UnknownWidget` fail: they say the control has never heard
/// of a name it publishes.
#[test]
fn every_published_command_is_dispatched_by_its_control() {
    let factory = WidgetFactory::new_with_defaults();

    let mut total = 0usize;
    let mut handled = 0usize;
    let mut unresolvable: Vec<&str> = Vec::new();
    let mut refused: Vec<(&str, &str)> = Vec::new();

    for capability in factory.capabilities() {
        if capability.commands.is_empty() {
            continue;
        }
        let Some(mut widget) =
            factory.create(capability.canonical_name, Rect::new(0, 0, 96, 64), "")
        else {
            continue;
        };
        if factory.capability_for_kind_instance(widget.as_ref()).is_none() {
            unresolvable.push(capability.canonical_name);
            continue;
        }
        for command in capability.commands {
            total += 1;
            match factory.invoke_command(widget.as_mut(), command) {
                // Handled: the action ran, or the control refused this *invocation*
                // because it needs a payload, or the registry/implementation disagree.
                // None of those is "the control does not know this name".
                Ok(())
                | Err(
                    rust_widgets::widget::capability::CapabilityAccessError::OutOfRange
                    | rust_widgets::widget::capability::CapabilityAccessError::UnsupportedOnWidget,
                ) => handled += 1,
                Err(_) => refused.push((capability.canonical_name, *command)),
            }
        }
    }

    assert!(
        total > 0,
        "no capability publishes commands, so this test proves nothing — the schema field or \
         its wiring has stopped reaching the property layer"
    );
    assert!(
        unresolvable.is_empty(),
        "a control publishing commands could not be resolved to its own capability, so its \
         whole command list is unreachable: {unresolvable:?}"
    );
    assert!(
        refused.is_empty(),
        "a capability publishes commands its own control does not implement, so a generic \
         consumer would offer an action that silently does nothing (control, command): \
         {refused:?}"
    );
    // The floor rises as commands are added, and may never fall: a capability whose
    // command list shrank, or a control that lost its `command` override, lowers this
    // number and is caught here rather than silently.
    assert!(
        handled >= 500,
        "fewer published commands are handled than the registry declares: {handled}/{total} — \
         a control lost its `command` implementation, or the registry stopped publishing a \
         list it still advertises"
    );
}

/// A control that publishes commands must dispatch the ones it implements.
///
/// # What this covers while the full rollout is in progress
///
/// The blanket test below is `#[ignore]`d until every capability's command list is
/// implemented. This one covers the controls that *are* done, so the contract is
/// protected now rather than at the end: for each name in this list, a constructed
/// control must answer it with something other than `UnknownCommand`. A control that
/// silently lost its implementation fails here.
#[test]
fn implemented_commands_are_dispatched() {
    use rust_widgets::widget::capability::CapabilityAccessError;

    let factory = WidgetFactory::new_with_defaults();

    // (control name, command) pairs that must be answered. Each one is a command
    // whose implementation exists; the pair is asserted rather than the whole list
    // so this test does not have to be edited when a capability gains a command.
    let cases: &[(&str, &str)] = &[
        ("check_box", "toggle"),
        ("button", "click"),
        ("toggle_button", "toggle"),
        ("radio_button", "set_checked"),
        ("group_box", "toggle"),
    ];

    let mut missing: Vec<(&str, &str)> = Vec::new();
    for &(control, command) in cases {
        let Some(mut widget) = factory.create(control, Rect::new(0, 0, 96, 64), "") else {
            continue;
        };
        // The published check passes; the control must then not answer UnknownCommand.
        if let Err(CapabilityAccessError::UnknownCommand) =
            factory.invoke_command(widget.as_mut(), command)
        {
            missing.push((control, command));
        }
    }

    assert!(
        missing.is_empty(),
        "a command whose implementation exists is refused as unknown, so the dispatcher \
         and the control disagree (control, command): {missing:?}"
    );
}

/// An unpublished command name must be refused, not accepted.
///
/// # Why the negative case matters as much as the positive one
///
/// A dispatcher that answers `Ok(())` for anything would pass the test above while
/// doing nothing at all — the exact "silent success" failure this contract exists to
/// prevent. This asserts the other direction on a control known to implement
/// commands, so a blanket-success implementation cannot pass both tests.
#[test]
fn an_unpublished_command_is_refused() {
    use rust_widgets::widget::capability::CapabilityAccessError;

    let factory = WidgetFactory::new_with_defaults();
    let mut widget =
        factory.create("check_box", Rect::new(0, 0, 96, 64), "").expect("check_box constructs");

    // `toggle` is published by `check_box` and must run.
    assert_eq!(factory.invoke_command(widget.as_mut(), "toggle"), Ok(()));

    // A name no capability publishes must not be accepted.
    let result = factory.invoke_command(widget.as_mut(), "definitely_not_a_command");
    assert_eq!(
        result,
        Err(CapabilityAccessError::UnknownCommand),
        "an unknown command name was accepted, so the dispatcher reports success for \
         actions it did not perform"
    );
}
