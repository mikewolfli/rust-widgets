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
pub use dialog::{MenuConfigDialog, MenuConfigPanel};
pub use manager::{
    apply_overrides, current_effects, init, install_policy, manager, set_enabled,
    use_verification_policy, MenuConfigManager,
};
pub use persistence::{ConfigLoadError, ConfigPersistence, CURRENT_CONFIG_VERSION};
pub use types::{HardwareCapabilities, PerformanceLevel, UserOverrides};

use crate::compat::{String, Vec};

/// Serializes a project's command list as a JSON array.
///
/// # Why this lives beside the menu configuration
///
/// The configuration module already owns "what the menu system persists". Commands and their
/// chords were the one part of that state with no document spelling, so a designer could express
/// a menu's *appearance* but not its *commands*. The shape is the actions section of
/// [`crate::json::JsonProject`], written here so both documents share one writer.
pub fn actions_to_json(actions: &[crate::action::ActionDefinition]) -> String {
    let mut out = String::from("[");
    for (index, action) in actions.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        out.push_str(&action.to_json());
    }
    out.push(']');
    out
}

/// Reads the array written by [`actions_to_json`].
///
/// Object boundaries are counted rather than split on `,`, because a command's text may contain a
/// comma. A missing or unterminated object is an error: a partially parsed command list would make
/// a menu silently lose commands.
pub fn actions_from_json(text: &str) -> Result<Vec<crate::action::ActionDefinition>, String> {
    let trimmed = text.trim();
    let body = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| String::from("actions JSON must be an array"))?;
    let mut actions = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut start = None;
    for (index, ch) in body.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            '}' => {
                if depth == 0 {
                    return Err(String::from("actions JSON has an unmatched `}`"));
                }
                depth -= 1;
                if depth == 0 {
                    let begin = start.take().ok_or_else(|| {
                        String::from("actions JSON has an object with no opening brace")
                    })?;
                    let object = &body[begin..=index];
                    actions.push(
                        crate::action::ActionDefinition::from_json(object)
                            .map_err(|reason| format!("invalid action entry: {reason}"))?,
                    );
                }
            }
            _ => {}
        }
    }
    if depth != 0 || in_string {
        return Err(String::from("actions JSON ends inside an object or string"));
    }
    Ok(actions)
}
