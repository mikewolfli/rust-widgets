// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Global shortcut system for menu items and actions.
//!
//! This module provides keyboard shortcut registration, conflict detection,
//! and dispatching for menu items across the application.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/platform/types.rs:43` (`compile_target_shortcut_style`), `src/platform/types.rs:497` (`format_shortcut`). 15 files reference it.
mod manager;
#[cfg(test)]
mod tests;
mod types;
pub use manager::ShortcutManager;
pub use types::{
    format_shortcut_for_platform, Key, Modifiers, PlatformShortcutStyle, Shortcut, ShortcutEntry,
};
