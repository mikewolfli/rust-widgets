// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu system configuration with hardware-adaptive features.
//!
//! This module provides automatic feature detection based on hardware capabilities,
//! while allowing users to override settings.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/app/app_core.rs:220` (and `src/lib.rs:1169`) start the
//! process-wide manager,
//! the menu family reads its policy through the menu toolbar's `effects::current`
//! boundary, and the master switch defaults to **on**. A host that wants the
//! pre-adaptive rendering calls [`set_enabled`]`(false)`; verification paths install
//! [`MenuConfig::verification`] through [`use_verification_policy`] so committed
//! snapshots stay host-independent.
mod config;
mod dialog;
mod manager;
mod persistence;
#[cfg(test)]
mod tests;
mod types;
/// The effect snapshot the menu controls consume.
pub use crate::style::MenuEffects;
pub use config::MenuConfig;
pub use dialog::MenuConfigDialog;
pub use manager::{
    apply_overrides, current_effects, init, install_policy, manager, set_enabled,
    use_verification_policy, MenuConfigManager,
};
pub use persistence::{ConfigLoadError, ConfigPersistence};
pub use types::{HardwareCapabilities, PerformanceLevel, UserOverrides};
