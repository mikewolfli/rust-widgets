//! Backward-compatible event aliases used by legacy widget implementations.

use crate::core::Point;

/// Backward-compatible mouse event alias.
pub type MouseEvent = (Point, u32);

/// Backward-compatible key event alias.
pub type KeyEvent = (u32, u32);
