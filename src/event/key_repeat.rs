// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Keyboard auto-repeat tracking (W-204).
//!
//! # Why the flag is additive rather than a new `KeyPress` field
//!
//! The published event contract spells `KeyPress` as a braced variant used in **553** literal
//! constructions and exhaustive patterns across the crate (`src/widget/**`, `src/platform/**`,
//! `src/shortcut/**`, tests). Adding a field to the variant would make every one of those a
//! compile error, and none of them are in this module's write scope. The repeat fact is therefore
//! an additive, dispatch-scoped capability:
//!
//! * the event stream is the source — a platform that repeats a held key sends another
//!   `KeyPress` for the same code with no intervening `KeyRelease`, and [`KeyRepeatState`] turns
//!   that into `true` for the second and later presses;
//! * the loop scopes the answer around the delivery of that event with
//!   [`with_dispatch_repeat`], so every node on the capture/target/bubble route sees the same
//!   fact;
//! * a consumer opts in with [`crate::event::Event::is_key_repeat`]. Code that ignores the
//!   accessor behaves exactly as before, because the event itself is unchanged.
//!
//! The flag is deliberately **not** a property of a detached `Event` value: repeating is a fact
//! about the stream, not about one copy of an event, and a value that could lie after it left the
//! dispatcher would be worse than no value at all. Asking outside a dispatch returns `false`.

use crate::compat::BTreeSet;

/// Tracks which key codes are physically held.
///
/// One instance belongs to one loop: the held set has to survive across events, and a
/// process-global set would let two independent loops (or a test) contaminate each other.
#[derive(Debug, Default)]
pub struct KeyRepeatState {
    held: BTreeSet<u32>,
}

impl KeyRepeatState {
    /// Creates a tracker with no key held.
    pub fn new() -> Self {
        Self { held: BTreeSet::new() }
    }

    /// Records a press and reports whether it is a repeat.
    ///
    /// `true` means the code was already held, which is exactly what a platform auto-repeat
    /// sends. The first press inserts the code and reports `false`.
    pub fn note_press(&mut self, key: u32) -> bool {
        !self.held.insert(key)
    }

    /// Records a release. A release for a code that was not held is ignored, so a lost press
    /// cannot make a later press look like a repeat forever.
    pub fn note_release(&mut self, key: u32) {
        self.held.remove(&key);
    }

    /// Drops every held key.
    ///
    /// Used when the loop stops or a host installs a fresh input scene: a key held across that
    /// boundary must not turn its next press into a phantom repeat.
    pub fn reset(&mut self) {
        self.held.clear();
    }
}

thread_local! {
    /// Whether the event currently being delivered is a keyboard auto-repeat.
    static DISPATCH_REPEAT: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// Restores the previous repeat flag when dropped, including during an unwind.
struct RepeatFlagGuard {
    previous: bool,
}

impl Drop for RepeatFlagGuard {
    fn drop(&mut self) {
        let _ = DISPATCH_REPEAT.try_with(|cell| cell.set(self.previous));
    }
}

/// Runs `deliver` with the repeat flag set to `repeat`, restoring the previous value afterwards.
///
/// Nested dispatches (a handler that posts and synchronously pumps an event) restore the outer
/// value rather than clearing it, so the inner event cannot make the outer one look like a
/// different kind of press.
pub fn with_dispatch_repeat<T>(repeat: bool, deliver: impl FnOnce() -> T) -> T {
    let previous = DISPATCH_REPEAT.try_with(|cell| cell.replace(repeat)).unwrap_or(false);
    let _guard = RepeatFlagGuard { previous };
    deliver()
}

/// Whether the event currently being delivered is a keyboard auto-repeat.
///
/// Read by [`crate::event::Event::is_key_repeat`]. Returns `false` outside a dispatch.
pub fn is_dispatch_repeat() -> bool {
    DISPATCH_REPEAT.try_with(|cell| cell.get()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_press_is_not_a_repeat_and_the_second_is() {
        let mut state = KeyRepeatState::new();
        assert!(!state.note_press(65));
        assert!(state.note_press(65), "a second press with no release is the platform repeat");
        state.note_release(65);
        assert!(!state.note_press(65), "after the release the key starts a new run");
    }

    #[test]
    fn an_unmatched_release_is_ignored() {
        let mut state = KeyRepeatState::new();
        state.note_release(65);
        assert!(!state.note_press(65));
    }

    #[test]
    fn independent_keys_do_not_affect_each_other() {
        let mut state = KeyRepeatState::new();
        assert!(!state.note_press(65));
        assert!(!state.note_press(66));
        assert!(state.note_press(65));
        assert!(state.note_press(66));
    }

    #[test]
    fn the_dispatch_flag_is_scoped_and_restores_the_outer_value() {
        assert!(!is_dispatch_repeat());
        let observed = with_dispatch_repeat(true, || {
            let inner = is_dispatch_repeat();
            let nested = with_dispatch_repeat(false, is_dispatch_repeat);
            (inner, nested, is_dispatch_repeat())
        });
        assert_eq!(observed, (true, false, true));
        assert!(!is_dispatch_repeat(), "the flag must not leak out of the dispatch");
    }

    #[test]
    fn a_panicking_handler_does_not_leak_the_flag() {
        let outcome = std::panic::catch_unwind(|| {
            with_dispatch_repeat(true, || panic!("handler failure"));
        });
        assert!(outcome.is_err());
        assert!(!is_dispatch_repeat(), "the guard restores the flag during an unwind");
    }
}
