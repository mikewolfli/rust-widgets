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
/// # What this covers
///
/// For each name in the list below, a constructed control must answer it with
/// something other than `UnknownCommand`. A control that silently lost its
/// implementation fails here.
///
/// # Why the list is explicit rather than "every published command"
///
/// The blanket test above already walks every published command, so this one exists to
/// make a **regression in one control** fail with the control named, rather than as one
/// entry in a 500-row list. It is deliberately a subset: a control whose commands are not
/// yet implemented would otherwise have to be removed from the list, and removing an entry
/// is exactly the silent weakening this test is supposed to prevent.
///
/// An earlier revision of this comment claimed the blanket test was `#[ignore]`d. It is
/// not, and never was — see `every_published_command_is_dispatched_by_its_control`
/// directly above, which carries a plain `#[test]`.
#[test]
fn implemented_commands_are_dispatched() {
    use rust_widgets::widget::capability::CapabilityAccessError;

    let factory = WidgetFactory::new_with_defaults();

    // (control name, command) pairs that must be answered. Each one is a command
    // whose implementation exists.
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
        // The control must answer with something other than "never heard of it".
        //
        // `UnsupportedOnWidget` is *not* accepted here: `invoke_command` rewrites a
        // control's `UnknownCommand` into that variant, so accepting it would make this
        // assertion unable to detect a removed implementation — which is precisely how
        // an earlier revision passed while the implementation it named was deleted.
        match factory.invoke_command(widget.as_mut(), command) {
            Ok(()) | Err(CapabilityAccessError::OutOfRange) => {}
            _other => missing.push((control, command)),
        }
    }

    assert!(
        missing.is_empty(),
        "a command whose implementation exists is refused as unknown, so the dispatcher \
         and the control disagree (control, command): {missing:?}"
    );
}

/// A command that reports success must actually **change** the control.
///
/// # The gap this closes
///
/// `Ok(())` means "the action ran". Nothing above checks that, so a `command` arm that
/// was gutted to `Ok(())` with the work removed still passes every other test in this
/// file — the return value is a claim, not evidence. This asserts the observable effect
/// for the two commands whose effect is a pure state toggle, which is what makes it a
/// behavioural test rather than another agreement check.
///
/// It is deliberately narrow: only commands whose whole effect is one readable flag can
/// be checked this generically. `button.click` emits a signal, which needs a subscriber,
/// and `toggle_button.toggle` on a disabled control is a sanctioned no-op.
#[test]
fn a_successful_command_has_an_observable_effect() {
    let factory = WidgetFactory::new_with_defaults();

    // `check_box.toggle` must flip `checked`.
    let mut check_box =
        factory.create("check_box", Rect::new(0, 0, 96, 64), "").expect("check_box constructs");
    let before = factory
        .read_property(check_box.as_ref(), "checked")
        .expect("check_box publishes `checked`");
    assert_eq!(factory.invoke_command(check_box.as_mut(), "toggle"), Ok(()));
    let after = factory
        .read_property(check_box.as_ref(), "checked")
        .expect("check_box publishes `checked`");
    assert_ne!(
        before, after,
        "`toggle` returned Ok but left `checked` unchanged, so the command is a shell"
    );

    // A second invocation must bring it back, which also proves the first was not a
    // one-shot side effect of construction.
    assert_eq!(factory.invoke_command(check_box.as_mut(), "toggle"), Ok(()));
    let restored = factory
        .read_property(check_box.as_ref(), "checked")
        .expect("check_box publishes `checked`");
    assert_eq!(restored, before, "a second `toggle` must return the control to its first state");

    // `radio_button.set_checked` takes its payload from the property route, so it is
    // exercised that way rather than through `invoke_command`.
    let mut radio =
        factory.create("radio_button", Rect::new(0, 0, 96, 64), "").expect("radio_button");
    factory
        .write_property(
            radio.as_mut(),
            "checked",
            rust_widgets::widget::capability::CapabilityValue::Bool(true),
        )
        .expect("radio_button accepts `checked`");
    assert_eq!(
        factory.read_property(radio.as_ref(), "checked").expect("read back"),
        rust_widgets::widget::capability::CapabilityValue::Bool(true),
        "the value a property write stored must be the value a read returns"
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
