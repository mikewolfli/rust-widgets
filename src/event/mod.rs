//! Event system for widget interaction and communication.
//!
//! This module provides:
//! - Event types and handler trait (`types.rs`)
//! - Event queue and sender (`event_queue.rs`)
//! - Focus management (`focus.rs`)
//! - Pointer capture management (`capture.rs`)
//! - Event loop (`loop.rs`)
//! - Generic queue utilities (`queue.rs`)
// Submodules
pub mod capture;
pub mod event_queue;
pub mod focus;
pub mod r#loop;
pub mod queue;
pub mod types;
// Re-export public types
pub use capture::PointerCaptureManager;
pub use event_queue::{EventQueue, EventSender};
pub use focus::FocusManager;
pub use r#loop::EventLoop;
pub use types::{Event, EventHandler, EventPriority};
// Re-export queue utilities
pub use queue::{FixedSizeQueue, QueueError, DEFAULT_QUEUE_CAPACITY};
// Backward-compatible event aliases used by legacy widget implementations.
pub type MouseEvent = (crate::core::Point, u32);
pub type KeyEvent = (u32, u32);
