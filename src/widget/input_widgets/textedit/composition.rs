// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `TextEdit`'s input-method composition state.
//!
//! Split out of `model.rs` by seam (principle #9): the model owns the value, caret, selection
//! and history, while this file owns the one piece of editing state that is deliberately
//! **not** part of the value — the uncommitted preedit. The type is still defined once, in
//! `model`; this is another `impl` block for it.

use super::model::*;

impl TextEdit {
    /// Starts or updates an input-method composition without changing the value.
    ///
    /// The field's value must not change while the user is still building a word: emitting
    /// `text_changed` here would run a form's validation against text the user has not committed.
    /// An empty preedit clears the composition without committing it, which is what an input
    /// method sends when it withdraws.
    pub fn set_composition(&mut self, preedit: &str) {
        let next = if preedit.is_empty() { None } else { Some(preedit.to_string()) };
        if self.composition == next {
            return;
        }
        self.composition = next;
        self.base.request_redraw();
    }

    /// The uncommitted composition, or `None` when no IME session is active.
    pub fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }

    /// Whether an input-method composition is in progress.
    pub fn has_composition(&self) -> bool {
        self.composition.is_some()
    }

    /// Accepts the composition: the preedit becomes text through the ordinary insert path.
    ///
    /// Routing through [`Self::insert_str`] means `max_length`, the selection replacement, undo
    /// and `text_changed` behave exactly as they do for any other insertion. A commit with no
    /// active composition is a no-op rather than an error, because an input method may commit and
    /// clear in either order.
    pub fn commit_composition(&mut self) -> bool {
        let Some(preedit) = self.composition.take() else {
            return false;
        };
        self.insert_str(&preedit);
        self.base.request_redraw();
        true
    }

    /// Drops the composition without changing the value.
    ///
    /// Returns `true` when there was one. The text is untouched by construction, not by restoring
    /// a snapshot: the composition was never written into it.
    pub fn cancel_composition(&mut self) -> bool {
        let had = self.composition.take().is_some();
        if had {
            self.base.request_redraw();
        }
        had
    }
}
