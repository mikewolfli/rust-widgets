// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Global shortcut system for menu items and actions.
//!
//! This module provides keyboard shortcut registration, conflict detection,
//! and dispatching for menu items across the application.
//!
//! # Who pumps the events
//!
//! [`ShortcutManager`] is a **host-owned** object, not a framework service:
//! nothing in `src/platform` feeds it, and no global instance exists. The host
//! application creates it, registers shortcuts on it, and calls
//! [`ShortcutManager::handle_event`] from its own event loop for every key
//! press it wants shortcuts to see — the same division 参考工具包 draws with
//! `shortcut` living inside the application, not inside a widget. The
//! framework deliberately does not intercept keys behind the host's back:
//! whether (and in which window, and in which focus order) a key press is
//! offered to the shortcut map is an application policy, and the platform
//! layer has no single place that could decide it for every host.
//!
//! The signal side is wired, the input side is not by design:
//! [`ShortcutManager::shortcut_triggered`] connects to
//! `ActionRouter::route_action_id` (`src/action/app.rs`), which routes the
//! action id to the [`crate::action::ActionManager`]. A minimal host loop:
//!
//! ```ignore
//! let mut shortcuts = ShortcutManager::new();
//! shortcuts.register("save", Shortcut::primary(Key::S), "Save");
//! // in the host's key-event path:
//! if shortcuts.handle_event(&event) {
//!     // the shortcut_triggered signal has already routed the action
//! }
//! ```
//!
//! # Reachability
//!
//! **State:** Production callers: `src/platform/types/values.rs:204`
//! (`compile_target_shortcut_style`) and the shortcut formatting it feeds. 15 files reference it.
mod manager;
#[cfg(test)]
mod tests;
mod types;
pub use manager::ShortcutManager;
pub use types::{
    format_shortcut_for_platform, Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry,
};
