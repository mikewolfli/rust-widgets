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
}

impl ToastItem {
    /// Creates toast item.
    pub fn new(
        id: impl Into<String>,
        message: impl Into<String>,
        level: ToastLevel,
        ttl_ms: u32,
    ) -> Self {
        Self { id: id.into(), message: message.into(), level, ttl_ms: ttl_ms.max(100) }
    }
}
