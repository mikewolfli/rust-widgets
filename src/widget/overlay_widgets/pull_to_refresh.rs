//! PullToRefresh — type alias for [`RefreshControl`](super::refresh_control::RefreshControl).
//!
//! This module exists for backward compatibility. The canonical implementation
//! is `RefreshControl` in the [`refresh_control`](super::refresh_control) module.

#[cfg(not(feature = "mini"))]
pub use super::refresh_control::RefreshControl as PullToRefresh;
