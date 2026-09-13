// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Menu system configuration with hardware-adaptive features.
//!
//! This module provides automatic feature detection based on hardware capabilities,
//! while allowing users to override settings.
mod config;
mod dialog;
mod manager;
mod persistence;
#[cfg(test)]
mod tests;
mod types;
pub use config::MenuConfig;
pub use dialog::MenuConfigDialog;
pub use manager::MenuConfigManager;
pub use persistence::ConfigPersistence;
pub use types::{HardwareCapabilities, PerformanceLevel, UserOverrides};
