// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! PullToRefresh — type alias for [`RefreshControl`](super::refresh_control::RefreshControl).
//!
//! This module exists for backward compatibility. The canonical implementation
//! is `RefreshControl` in the [`refresh_control`](super::refresh_control) module.

#[cfg(not(feature = "mini"))]
pub use super::refresh_control::RefreshControl as PullToRefresh;
