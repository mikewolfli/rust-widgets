// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The bundled icon data must line up with `IconName`, in every build (BLUE25 ICON-3).
//!
//! # What this protects
//!
//! `IconName::data` indexes [`ICON_DATA`] with the variant's discriminant, which is the shape
//! that makes a missing icon a **compile error** rather than a runtime placeholder — the whole
//! point of ICON-3. The cost of that shape is an ordering assumption: the enum's declaration
//! order must equal the table's order. `data()` carries a `debug_assert!` for it, and a
//! `debug_assert!` is absent from a release build, so this test is what checks the assumption
//! where it actually ships.
//!
//! It also checks the property the old hand-drawn set violated: **two distinct names must not
//! draw one picture**. `Close == Cross` was real, and an upstream rename could reintroduce it.

#![cfg(all(feature = "icons", not(feature = "mini")))]

use rust_widgets::widget::{IconData, IconName};

/// Every token the data table carries, in its own order.
fn table_tokens() -> Vec<&'static str> {
    IconName::all_tokens().to_vec()
}

#[test]
fn every_variant_resolves_to_the_table_entry_that_names_it() {
    // The single assertion this file exists for: `ICON_DATA[i].name == variants[i].as_str()`.
    // A reordered enum, a hand-edited table, or a token renamed on one side fails here by name.
    for variant in IconName::ALL {
        let data: IconData = variant.data();
        assert_eq!(
            data.name,
            variant.as_str(),
            "{variant:?} resolved to the data entry named {:?}, not {:?}; the enum order and \
             `ICON_DATA`'s order have drifted",
            data.name,
            variant.as_str()
        );
    }
}

#[test]
fn the_token_list_and_the_variant_list_are_the_same_length() {
    // Guards against a table that lost an entry (which would index-shift every later icon) or
    // gained one (which `data()` could never reach).
    assert_eq!(
        IconName::ALL.len(),
        table_tokens().len(),
        "the IconName variants and the icon data table disagree on how many icons exist"
    );
}

#[test]
fn no_two_icons_share_one_outline() {
    // The `Close == Cross` rule, checked on the shipped data rather than only at generation
    // time: a build that vendored a second copy of one `d` under another token draws two names
    // as one picture, and that is a defect no matter how it arrived.
    let mut seen: Vec<(&'static str, &'static [&'static str])> = Vec::new();
    for variant in IconName::ALL {
        let data = variant.data();
        if let Some((other, _)) = seen.iter().find(|(_, paths)| *paths == data.paths) {
            panic!(
                "{variant:?} and {other} resolve to byte-identical outlines, so two distinct \
                 icon names draw one picture"
            );
        }
        seen.push((data.name, data.paths));
    }
}

#[test]
fn every_icon_has_ink() {
    // An entry with no path (or an empty `d`) draws nothing, which is indistinguishable from a
    // missing icon on screen. The data is upstream's, but "upstream shipped an empty file" and
    // "we generated an empty entry" must not be able to pass silently.
    for variant in IconName::ALL {
        let data = variant.data();
        assert!(!data.paths.is_empty(), "{variant:?} has no path at all");
        for path in data.paths {
            assert!(!path.trim().is_empty(), "{variant:?} has an empty path");
        }
        assert!(data.grid > 0, "{variant:?} has a zero grid, so it cannot be scaled");
    }
}

#[test]
fn the_data_accessor_agrees_with_the_variant_lookup() {
    // `IconName::from_name` and `data()` are two spellings of one fact; a token that parses but
    // has no data (or the reverse) would strand one of them.
    for variant in IconName::ALL {
        let token = variant.as_str();
        let parsed = IconName::from_name(token).unwrap_or_else(|| {
            panic!("{token:?} is a declared token but `from_name` does not parse it")
        });
        assert_eq!(parsed, variant, "{token:?} round-trips to a different variant");
        assert_eq!(parsed.data(), variant.data(), "{token:?} resolves to different data");
    }
}
