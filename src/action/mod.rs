// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Action/shortcut/command framework.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/capability/properties/menu.rs:14` (the `action` capability's property contract).

mod app;
mod definition;
mod manager;
mod types;

#[cfg(test)]
mod tests;

pub use app::ActionRouter;
pub use definition::{ActionDefinition, ActionLiveState};
pub use manager::{ActionManager, ShortcutDisagreement};
pub(crate) use types::{canonicalize_shortcut_type, normalize_shortcut};
pub use types::{Action, ActionBinding, ActionHostKind};
