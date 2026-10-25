// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The data a toast carries: its severity, and one message.

/// Toast severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    /// Neutral informational message.
    Info,
    /// Positive outcome confirmation.
    Success,
    /// Non-fatal problem the user may want to act on.
    Warning,
    /// Failure the user must notice; hosts typically keep these on screen
    /// longer than the other levels.
    Error,
}

/// The one mapping from a toast severity to the theme's semantic token.
///
/// The single and stacked toast controls both used to spell this match out, so adding a
/// fifth severity meant editing both copies and a missed one painted the wrong colour
/// silently (H-311). They now both call this implementation.
impl From<ToastLevel> for crate::style::SemanticColor {
    fn from(level: ToastLevel) -> Self {
        match level {
            ToastLevel::Info => Self::Info,
            ToastLevel::Success => Self::Success,
            ToastLevel::Warning => Self::Warning,
            ToastLevel::Error => Self::Error,
        }
    }
}

/// One toast message item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastItem {
    /// Stable toast id.
    pub id: String,
    /// Message text.
    pub message: String,
    /// Severity level.
    pub level: ToastLevel,
    /// Suggested ttl for host runtime.
    pub ttl_ms: u32,
    /// Optional action button label.
    pub action_label: Option<String>,
}

impl ToastItem {
    /// Creates toast item.
    pub fn new(
        id: impl Into<String>,
        message: impl Into<String>,
        level: ToastLevel,
        ttl_ms: u32,
    ) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
            level,
            ttl_ms: ttl_ms.max(100),
            action_label: None,
        }
    }

    /// Adds an action button to the toast.
    pub fn with_action(mut self, label: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self
    }
}
