// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The painted key-label table of the on-screen keyboard (`W7-28`).
//!
//! `Keyboard::key_display_label` answers a `Cow`, so it borrows the stored label whenever that
//! label already has the right case and allocates only when it has to be re-cased. Asking it for
//! every key on every frame therefore allocated one `String` per letter key for the whole time
//! the uppercase state was active — which is the state a keyboard is in whenever a user types a
//! capital. This module owns the table and the key it was built for, so the rebuild happens once
//! per case change and the paint borrows the result.
//!
//! The cache lives beside the control rather than inside it because the table is not state the
//! control's API exposes: it is a derived artefact of `(case state, keys generation)` and must
//! never be written by a caller.

use crate::compat::{String, Vec};
use crate::widget::input_widgets::keyboard::KeyDefinition;

/// The painted key labels for one case state.
pub(crate) struct KeyLabelCache {
    /// `(shift ^ caps_lock, lowercase)` the labels were built for.
    state: (bool, bool),
    /// The `keys` generation the labels were built from.
    revision: u32,
    /// Row-major labels, parallel to `Keyboard::keys`.
    labels: Vec<Vec<String>>,
}

impl KeyLabelCache {
    /// An empty table that the first [`Self::synchronise`] always fills.
    ///
    /// The revision starts at `u32::MAX`, a generation the control can never reach, so the first
    /// paint rebuilds whatever the case state happens to be: a cache whose initial `state` matched
    /// the control's would paint a table it never built.
    pub(crate) fn new() -> Self {
        Self { state: (false, false), revision: u32::MAX, labels: Vec::new() }
    }

    /// The labels built for the current state and key generation.
    pub(crate) fn labels(&self) -> &[Vec<String>] {
        &self.labels
    }

    /// Rebuilds the table when the case state or the key grid changed, then keeps the result.
    ///
    /// `label` is the control's own `key_display_label`, so the case rule stays in one place: this
    /// cache decides *when* the table is stale, never *what* a label says.
    pub(crate) fn synchronise<F>(
        &mut self,
        state: (bool, bool),
        revision: u32,
        keys: &[Vec<KeyDefinition>],
        label: F,
    ) where
        F: Fn(&KeyDefinition) -> String,
    {
        if self.state == state && self.revision == revision {
            return;
        }
        self.labels = keys.iter().map(|row| row.iter().map(|key| label(key)).collect()).collect();
        self.state = state;
        self.revision = revision;
    }
}
