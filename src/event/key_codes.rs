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

/// The first function key, `F1`. `F12` is [`F12`].
///
/// Function keys are part of the framework's key-code convention because a host that
/// normalises a platform key (AppKit `keyCode`, GDK `keyval`) has to produce a value the
/// widget layer can branch on.
///
/// # Why the range is `1120..=1131` and not `112..=123`
///
/// `112..=123` is the Win32 virtual-key range for `VK_F1`..`VK_F12`, but it collides with the
/// framework's own letter codes: lowercase `'p'..'z'` are `112..=122`, so `F1` parsed as
/// `Key::P` and every function-key shortcut silently matched a letter instead. The range below
/// is above the ASCII space the rest of the convention uses, which is what makes the two
/// unambiguous. `shortcut::Key::from_key_code` accepts exactly these values.
pub const F1: u32 = 1120;
/// `F2`.
pub const F2: u32 = 1121;
/// `F3`.
pub const F3: u32 = 1122;
/// `F4`.
pub const F4: u32 = 1123;
/// `F5`.
pub const F5: u32 = 1124;
/// `F6`.
pub const F6: u32 = 1125;
/// `F7`.
pub const F7: u32 = 1126;
/// `F8`.
pub const F8: u32 = 1127;
/// `F9`.
pub const F9: u32 = 1128;
/// `F10`.
pub const F10: u32 = 1129;
/// `F11`.
pub const F11: u32 = 1130;
/// `F12`.
pub const F12: u32 = 1131;

/// Maps a printable character to the framework's key code for it.
///
/// Letters are folded to **upper case** because that is what the control layer matches:
/// `LineEdit` handles `Primary+A` as key `65`, not `97`, so a platform that reports the
/// lower-case character for `⌘A` would otherwise miss every letter shortcut. Digits,
/// punctuation and space keep their ASCII value.
///
/// Returns `None` for a non-ASCII character. A platform boundary should then keep its own
/// native code rather than invent one: the widget layer has no name for it, and neither
/// does this table.
pub fn ascii_key_code(ch: char) -> Option<u32> {
    let folded = ch.to_ascii_uppercase();
    if folded.is_ascii() {
        Some(folded as u32)
    } else {
        None
    }
}

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
            (PAGE_UP, Key::PageUp),
            (PAGE_DOWN, Key::PageDown),
            (DELETE, Key::Delete),
            (F1, Key::F1),
            (F2, Key::F2),
            (F3, Key::F3),
            (F4, Key::F4),
            (F5, Key::F5),
            (F6, Key::F6),
            (F7, Key::F7),
            (F8, Key::F8),
            (F9, Key::F9),
            (F10, Key::F10),
            (F11, Key::F11),
            (F12, Key::F12),
        ] {
            assert_eq!(
                Key::from_key_code(code),
                Some(expected),
                "key code {code} must parse as {expected:?}"
            );
        }
    }

    /// Function-key codes must not overlap the letter codes they used to collide with.
    ///
    /// The previous constants were `112..=123` (the Win32 `VK_F1`.. range), where `112`..`122`
    /// are also lowercase `'p'`..`'z'`. `F1` therefore parsed as `Key::P`, and the test above did
    /// not list the function keys, so nothing noticed.
    #[test]
    fn function_key_codes_are_outside_the_ascii_letter_range() {
        use crate::shortcut::Key;

        assert_eq!(Key::from_key_code(112), Some(Key::P), "112 is still lowercase 'p'");
        for (code, expected) in [(F1, Key::F1), (F12, Key::F12)] {
            assert!(code > 127, "a function key must not share a byte with ASCII: {code}");
            assert_eq!(Key::from_key_code(code), Some(expected));
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

    /// A platform chord reports a character regardless of case, and the widget layer
    /// matches the upper-case code.
    #[test]
    fn ascii_key_codes_fold_letters_to_upper_case() {
        assert_eq!(ascii_key_code('a'), Some(65));
        assert_eq!(ascii_key_code('A'), Some(65));
        assert_eq!(ascii_key_code('z'), Some(90));
        assert_eq!(ascii_key_code('0'), Some(48));
        assert_eq!(ascii_key_code('/'), Some(47));
        assert_eq!(ascii_key_code(' '), Some(32));
        assert_eq!(ascii_key_code('é'), None, "non-ASCII has no framework code");
    }
}
