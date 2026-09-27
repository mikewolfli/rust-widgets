// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Named key codes for the values a control actually branches on.
//!
//! # The defect this closes
//!
//! `Event::KeyPress { key, .. }` carries a raw `u32`, and control handlers compared it against
//! **literals** — `*key == 13`, `*key == 27`, `*key == 40`. Across the crate that was 19 files
//! spelling the same numbers, and the numbers say nothing about what they mean: a reader has to know
//! that 27 is Escape and 40 is Down, and the compiler cannot catch a typo like `37`/`47`. It also
//! made the tab/enter and escape conventions unsearchable — `grep '== 27'` finds some Escape
//! handlers and misses the ones spelled `0x1b`.
//!
//! # Why constants here and not the `Key` enum
//!
//! [`crate::shortcut::Key`] is the *semantic* vocabulary (it has a `Key::Enter` variant and a
//! `Key::from_key_code` parser), and a handler that wants to *name* a key should use it. But a
//! handler comparing `Event::KeyPress.key` would then have to call `Key::from_key_code(key)` on every
//! event — a `match` over ~120 arms per keystroke, in the hot path of the event loop, to answer a
//! question the caller already knows the shape of ("is this Enter?").
//!
//! So these are the cheap, exact form for the common case: `u32` literals with names, one
//! definition of each value, no parsing. A control whose condition is *semantic* ("is this the
//! shortcut's key?") still goes through `Key`; a control asking "is this Enter?" gets a constant.
//!
//! # Why these values
//!
//! They are the framework's own key-code convention, and each one is the value
//! [`crate::shortcut::Key::from_key_code`] maps to the matching variant — so a constant and its
//! `Key` spelling cannot disagree. `tests/event/key_codes_agree_with_the_shortcut_parser_test.rs`
//! asserts exactly that pairing, which is what keeps this table from drifting into a second
//! convention (principle #54).

/// The Enter key.
///
/// Also the **line feed** code, which some hosts deliver for the numeric keypad's Enter: the
/// framework treats `10` and `13` as one key (`Key::Enter` maps both), and the handlers that
/// checked for both before this module existed checked them because of exactly that.
pub const ENTER: u32 = 13;

/// The line-feed form of Enter, delivered by some hosts for the keypad's Enter key.
///
/// Separate from [`ENTER`] because a handler must accept *both* to be correct, and writing
/// `key == ENTER || key == LINE_FEED` says that where `13 || 10` did not.
pub const LINE_FEED: u32 = 10;

/// The Escape key.
pub const ESCAPE: u32 = 27;

/// The Backspace key.
pub const BACKSPACE: u32 = 8;

/// The Delete (forward-delete) key.
pub const DELETE: u32 = 46;

/// The space bar.
pub const SPACE: u32 = 32;

/// The Tab key.
pub const TAB: u32 = 9;

/// The Left arrow key.
pub const LEFT: u32 = 37;

/// The Up arrow key.
pub const UP: u32 = 38;

/// The Right arrow key.
pub const RIGHT: u32 = 39;

/// The Down arrow key.
pub const DOWN: u32 = 40;

/// The Home key.
pub const HOME: u32 = 36;

/// The End key.
pub const END: u32 = 35;

/// The Page Up key.
pub const PAGE_UP: u32 = 33;

/// The Page Down key.
pub const PAGE_DOWN: u32 = 34;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every constant names the code the shortcut parser maps to the matching `Key`.
    ///
    /// # Why this test exists
    ///
    /// The two tables are independent — this module is `u32` literals, the parser is a `match` — so
    /// nothing but this assertion stops one of them being corrected without the other. That is the
    /// "two tables that must agree" shape BLUE25's C-7 is about, checked here rather than trusted.
    #[test]
    fn every_constant_agrees_with_the_shortcut_parser() {
        use crate::shortcut::Key;

        for (code, expected) in [
            (ENTER, Key::Enter),
            (ESCAPE, Key::Escape),
            (BACKSPACE, Key::Backspace),
            (SPACE, Key::Space),
            (TAB, Key::Tab),
            (LEFT, Key::Left),
            (UP, Key::Up),
            (RIGHT, Key::Right),
            (DOWN, Key::Down),
            (HOME, Key::Home),
            (END, Key::End),
        ] {
            assert_eq!(
                Key::from_key_code(code),
                Some(expected),
                "key code {code} must parse as {expected:?}"
            );
        }
    }

    /// The three codes whose meaning is *not* one-to-one are the documented exceptions.
    ///
    /// `ENTER`/`LINE_FEED` are one key, `DELETE` has two spellings, and neither is a defect — they
    /// are the host conventions the framework folds together. Pinning them here means a later
    /// "simplification" that drops one is a failure rather than a silent behaviour change.
    #[test]
    fn the_aliases_are_the_documented_ones() {
        use crate::shortcut::Key;

        assert_eq!(Key::from_key_code(LINE_FEED), Some(Key::Enter));
        assert_eq!(Key::from_key_code(ENTER), Key::from_key_code(LINE_FEED));
        // `127` is the other Delete spelling; the constant uses the one `Key::from_key_code` lists
        // first, and both must still agree with the enum.
        assert_eq!(Key::from_key_code(DELETE), Some(Key::Delete));
        assert_eq!(Key::from_key_code(127), Some(Key::Delete));
    }
}
