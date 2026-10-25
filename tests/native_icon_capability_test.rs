// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! blue27 §19.13 case I15: a native surface that cannot show an icon must **say so**.
//!
//! # The two failures this separates
//!
//! `Platform::menu_add_item` takes text and a shortcut and nothing else, so an icon handed to a
//! native menu bar has only three possible outcomes: presented, reported unsupported, or silently
//! absent. The last is indistinguishable from "the source resolved and painted nothing", which is
//! exactly the conflation §19.11 forbids — a broken resource and an absent feature would look the
//! same to the user *and* to the host's diagnostics.
//!
//! So the capability is askable (`menu_item_icons_supported`) and the crate's own menu path
//! consults it (`WindowHandle::new_menu_item_with_icon` keeps the text and logs the unsupported
//! presentation). This file pins the answer, because "we do not support this yet" is a product
//! statement that must not drift into an accidental claim of support: if a future backend starts
//! presenting icons, this test fails and forces the decision to be made explicitly.
//!
//! # What is deliberately *not* asserted
//!
//! The window / taskbar / installer icon is a different contract (blue27 §13): an OS application
//! icon is a **file path**, not an arbitrary glyph slot, and it stays that way. The second case here
//! asserts that path contract still reports failure honestly instead of pretending success.
#![cfg(not(alloc_frugal))]

/// No desktop backend presents icons on native menu items yet, and saying `false` is the accurate
/// answer rather than a limitation to hide.
///
/// `false` is what lets a caller skip building the icon at all, and it is what the crate's own
/// `new_menu_item_with_icon` reports when it keeps the text.
#[test]
fn native_menu_icons_are_reported_unsupported_not_assumed() {
    assert!(
        !rust_widgets::menu_item_icons_supported(),
        "if a backend gained native menu-item icons, this assertion must be revisited deliberately: \
         the capability, its DPI/update/destroy behaviour and a real presentation test all have to \
         arrive together (blue27 §19.11)"
    );
}

/// The OS application icon keeps the §13 path contract, and an id it cannot serve fails visibly.
#[test]
fn the_os_application_icon_still_fails_honestly() {
    // No window is registered under id 0, so the call must report failure — not return success for
    // an icon it never installed, which is the "空白/文字伪装全部成功" shape §19.7 rejects.
    assert!(
        !rust_widgets::set_window_icon(0, "/nonexistent/rust_widgets/probe.png"),
        "an unserved window icon request must report failure"
    );
    assert!(
        rust_widgets::window_icon(0).is_none(),
        "a failed install must not leave a remembered path behind"
    );
}
