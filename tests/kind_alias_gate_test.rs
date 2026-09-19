// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Every `KIND_*` alias in `src/lib.rs` must resolve to the control its `create_*`
//! function is named after, **on every profile that ships that control**.
//!
//! # The defect this closes
//!
//! `src/lib.rs` reaches each reduced-profile `WidgetKind` through an alias pair:
//! a `const KIND_X: WidgetKind = WidgetKind::X` when the variant exists, and a
//! `const KIND_X: WidgetKind = WidgetKind::Panel` fallback when it does not. The
//! fallback exists so the public `create_*` surface stays callable in every profile
//! (principle #53).
//!
//! For `KIND_MENU_BAR`, `KIND_MENU`, `KIND_TOOL_BAR` and `KIND_STATUS_BAR` the
//! gate was `#[cfg(desktop_surface)]` — a narrower predicate than the
//! `#[cfg(widgets_unstripped)]` that gates the variants themselves in
//! `src/widget/kind.rs`. `tablet` and `mobile` satisfy `widgets_unstripped` but
//! **not** `desktop_surface`, so on exactly those two profiles
//! `create_menu_bar(..)`, `create_menu(..)`, `create_tool_bar(..)` and
//! `create_status_bar(..)` silently mounted a **`Panel`** and returned a valid id.
//! Nothing failed: an id is precisely what the defect produced, and the one test
//! that walks these routes (`tests/control_backend_named_creation_test.rs`) is
//! gated `feature = "desktop"` — the single profile where the alias was correct.
//!
//! # The invariant asserted here
//!
//! An alias gate must be **implied by** the gate on the variant it names. This file
//! states that as a per-profile assertion over the observable factory name each
//! `create_*` route reaches, so a future editor who narrows a gate again sees a
//! failure on the profile they broke rather than on `desktop`.

#![cfg(full_widgets)]

use rust_widgets::core::ObjectId;
use rust_widgets::widget::capability::WidgetFactory;
use rust_widgets::widget::runtime::with_widget_mut;

/// Bounds large enough that no control is laid out to nothing.
const W: u32 = 400;
const H: u32 = 300;

/// The capability name of the control mounted at `id`, or `None` when the id
/// addresses nothing.
///
/// Read back from the **live control** rather than from the request, so a control
/// that was silently substituted reports its own name instead of the one asked
/// for. This is the same resolution the property and event paths use.
fn mounted_control_name(id: ObjectId) -> Option<&'static str> {
    with_widget_mut(id, |widget| {
        let factory = WidgetFactory::new_with_defaults();
        factory.capability_for_kind_instance(widget).map(|capability| capability.canonical_name)
    })
    .flatten()
}

/// One alias route: the public entry point, the id it returned, and the control it
/// must have built.
type AliasCase = (&'static str, ObjectId, &'static str);

/// The alias routes whose gate was wrong, plus `list_view` as the control case.
///
/// `list_view` already used the correct gate and is included so that a regression
/// in the *pattern* (not just in one alias) is visible: if a future edit changes
/// how the pairs are written, this row fails on the same run.
fn alias_cases(parent: ObjectId) -> Vec<AliasCase> {
    vec![
        ("create_menu_bar", rust_widgets::create_menu_bar(parent, 0, 0, W, 24), "menu_bar"),
        ("create_menu", rust_widgets::create_menu(parent, "File", 0, 0, 80, 24), "menu"),
        ("create_tool_bar", rust_widgets::create_tool_bar(parent, 0, 0, W, 32), "tool_bar"),
        (
            "create_status_bar",
            rust_widgets::create_status_bar(parent, "ready", 0, 0, W, 24),
            "status_bar",
        ),
        ("create_list_view", rust_widgets::create_list_view(parent, 0, 0, W, H), "list_view"),
    ]
}

/// Every alias route must build the control it is named after.
///
/// Runs on `desktop`, `tablet` and `mobile` (`full_widgets`), which is the point:
/// the defect this closes was invisible on `desktop`.
#[test]
fn every_alias_route_builds_the_control_it_is_named_after() {
    let parent = rust_widgets::create_window("alias-gate", 0, 0, 1024, 768);
    let mut failures = Vec::new();

    for (method, id, expected) in alias_cases(parent) {
        match mounted_control_name(id) {
            Some(actual) if actual == expected => {}
            Some(actual) => {
                failures.push(format!("{method} mounted `{actual}`, expected `{expected}`"))
            }
            None => failures.push(format!(
                "{method} returned id {id}, which addresses no control (expected `{expected}`)"
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "alias gate is narrower than the variant gate in src/widget/kind.rs:\n  {}",
        failures.join("\n  ")
    );
}

/// The negative control: the `Panel`/`GroupBox` stand-in must be distinguishable
/// from the menu-family controls at **both** observable levels.
///
/// Without this row, a future `KIND_*` fallback that resolved to some other
/// valid-looking control could satisfy the assertion above if the expected names
/// were ever edited to match. Pinning the stand-in makes the substitution itself
/// the failure.
///
/// # Why the reported names are `group_box` and `GroupBox`
///
/// `create_panel(..)` builds a `GroupBox`: `src/widget/mod.rs` declares
/// `pub type Panel = GroupBox`. `WidgetKind::GroupBox` carries two capabilities
/// (`group_box` and `panel`, per `panel_capability`'s doc comment), and
/// `capability_for_widget` resolves a shared kind by concrete type then by
/// registration order — so the first registered of the two answers. The mounted
/// control is therefore a `GroupBox` under the name `group_box`; that pairing is
/// the documented state, and pinning it here means a future *change* to it is a
/// visible decision rather than a silent re-substitution.
///
/// Both are asserted because the assertion that matters for this file is the
/// **kind**: a fallback route reaching the stand-in produces `GroupBox` whatever
/// capability name it happens to report.
#[test]
fn a_substituted_panel_is_not_mistaken_for_the_named_control() {
    let parent = rust_widgets::create_window("alias-gate-negative", 0, 0, 1024, 768);
    let panel = rust_widgets::create_panel(parent, 0, 0, W, H);

    let panel_kind = with_widget_mut(panel, |widget| widget.base().kind())
        .expect("the stand-in id must address a mounted control");
    assert_eq!(
        panel_kind,
        rust_widgets::widget::WidgetKind::GroupBox,
        "`Panel` is a `pub type` for `GroupBox`; the mounted kind pins that pairing"
    );
    assert_eq!(
        mounted_control_name(panel),
        Some("group_box"),
        "`WidgetKind::GroupBox` resolves through registration order to `group_box`; \
         see `panel_capability` and `capability_for_widget` in `src/widget/capability.rs`"
    );

    // The real assertion: no alias route may reach the stand-in. Checked on the kind
    // (the discriminator that cannot be renamed away) *and* on the capability name.
    for (method, id, expected) in alias_cases(parent) {
        let Some(actual) = mounted_control_name(id) else { continue };
        let kind = with_widget_mut(id, |widget| widget.base().kind())
            .expect("a valid alias route must address a mounted control");
        assert_ne!(
            kind,
            rust_widgets::widget::WidgetKind::GroupBox,
            "{method} reached the `Panel`/`GroupBox` stand-in (expected `{expected}`)"
        );
        assert_ne!(
            actual, "group_box",
            "{method} reached the `Panel`/`GroupBox` stand-in (expected `{expected}`)"
        );
    }
}
