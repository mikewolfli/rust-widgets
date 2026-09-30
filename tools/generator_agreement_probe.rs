// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Asserts the designer's generated program can build every control the registry can.
//!
//! # Why this is a runnable probe rather than a source grep
//!
//! The defect this covers was a disagreement between two **runtime** answers. The generator's
//! availability probe asked the registry's `create` (which resolves aliases) while its constructor
//! arms came from a hand-maintained name→type table, so `"btn"` was reported available and then
//! produced no arm. Worse, the table covered only 20 of the registry's 188 constructible controls: a
//! document placing a `table`, `tree_view`, `code_editor`, … generated a `create_for` with no arm
//! for it, `ViewEngine::mount` reported `UnknownWidgetType`, and the report still called the
//! document clean.
//!
//! The fix routes the generated `create_for` through the registry, so the two halves cannot
//! disagree. This probe asserts the invariant that now holds:
//!
//!   [0] The documented aliases (`btn`, `pushbutton`, `text_label`, `main_window`, `toggle`) still
//!       resolve to their controls — the spellings a document actually uses.
//!   [1] Every constructible name and alias is buildable by **the same lookup the generated program
//!       performs**, so a document cannot spell a control in a way that generates nothing.
//!   [2] `availability` never reports `Unknown` for a constructible name.
//!
//! It drives the real registry rather than parsing the source, because only asking the crate can
//! show the two answers disagree.
//!
//! Run by `tools/check_generator_agrees_with_registry.sh`; the reverse injection in that script
//! removes the `btn` alias and requires this probe to fail naming it.

/// The documented aliases, and the control each must resolve to.
const DOCUMENTED_ALIASES: &[(&str, &str)] = &[
    ("btn", "button"),
    ("pushbutton", "button"),
    ("text_label", "label"),
    ("main_window", "window"),
    ("toggle", "toggle_button"),
];

#[test]
fn every_registered_control_is_buildable_by_the_generated_programs_lookup() {
    use rust_widgets::designer::{availability, TargetProfile};
    use rust_widgets::widget::capability::WidgetFactory;

    let factory = WidgetFactory::new_with_defaults();
    let mut problems: Vec<String> = Vec::new();
    let mut canonical_count = 0usize;
    let mut alias_count = 0usize;

    // The one lookup both the generator and the generated program perform.
    let construct =
        |name: &str| factory.create(name, rust_widgets::core::Rect::new(0, 0, 1, 1), "");

    // # Why a fixed list of aliases is also asserted
    //
    // Iterating `aliases_of` alone cannot catch an alias that was *removed*: the list simply gets
    // shorter and every remaining entry passes. These are the spellings a document actually uses
    // (the crate's own docs and cookbook show them), so each must keep resolving.
    for (alias, owner) in DOCUMENTED_ALIASES {
        if construct(alias).is_none() {
            problems.push(format!(
                "[0] the documented alias `{alias}` (for `{owner}`) is no longer creatable, so a \
                 document spelling the control `{alias}` would generate a program that builds \
                 nothing for it"
            ));
        }
        let resolved = factory.canonical_name(alias);
        if resolved != *owner {
            problems.push(format!(
                "[0] the documented alias `{alias}` resolves to `{resolved}` instead of `{owner}`"
            ));
        }
    }

    for canonical in factory.constructible_names() {
        canonical_count += 1;
        if construct(&canonical).is_none() {
            problems.push(format!(
                "[1] `{canonical}` is in `constructible_names()` but `create` answers `None` for \
                 it, so the invariant the generated program relies on does not hold for its own \
                 canonical name"
            ));
        }
        let avail = format!("{:?}", availability(&canonical, TargetProfile::Default));
        if avail.starts_with("Unknown") {
            problems.push(format!(
                "[2] `{canonical}` is constructible but `availability` reports `{avail}`: a \
                 document using the canonical name would be refused"
            ));
        }

        for alias in factory.aliases_of(&canonical) {
            alias_count += 1;
            // [1] The alias must be creatable through the same lookup, so `Node::widget` holding
            // the alias builds the control rather than nothing.
            if construct(&alias).is_none() {
                problems.push(format!(
                    "[1] alias `{alias}` of `{canonical}` is not creatable, so a document spelling \
                     the control `{alias}` would generate a program that builds nothing"
                ));
            }
            let avail = format!("{:?}", availability(&alias, TargetProfile::Default));
            if avail.starts_with("Unknown") {
                problems.push(format!(
                    "[2] alias `{alias}` of `{canonical}` is creatable at run time but \
                     `availability` reports `{avail}`"
                ));
            }
        }
    }

    assert!(canonical_count > 0, "the registry is empty, so this probe would test nothing");
    assert!(
        problems.is_empty(),
        "the generated program cannot build {} of the registry's names:\n  - {}\n\n\
         A name the registry can build must also be buildable by the generated program's own \
         registry lookup, and an alias must resolve to its control. Otherwise `create_for` returns \
         `None` for that node, `ViewEngine::mount` cannot create it, and the program shows a blank \
         window while its report says the document was clean.",
        problems.len(),
        problems.join("\n  - ")
    );
    println!(
        "OK {canonical_count} canonical names, {alias_count} aliases: the generated program's \
         registry lookup can build every one."
    );
}
