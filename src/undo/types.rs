// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

/// Unique identifier for an undo command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

/// Merge policy for consecutive commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    /// Cannot merge with any other command.
    Never,
    /// Can merge with the previous command of the same type.
    WithPrevious,
}

/// Describes the scope/effect of a command.
#[derive(Debug, Clone)]
pub struct CommandDescription {
    /// Human-readable text (e.g., "Delete text").
    pub text: String,
    /// Timestamp in milliseconds since UNIX epoch when the command was created.
    /// On no_std targets (mini), this is set to 0.
    pub timestamp_ms: u64,
    /// Static string identifying the command type.
    pub command_type: &'static str,
}
