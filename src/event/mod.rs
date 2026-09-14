// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
pub mod timer;
#[cfg(feature = "touch")]
pub mod translator;
pub mod types;
// Re-export public types
pub use capture::PointerCaptureManager;
pub use event_queue::{EventQueue, EventSender};
pub use focus::FocusManager;
pub use r#loop::AnimationFrameRequest;
pub use r#loop::EventLoop;
pub use timer::TimerManager;
/// Named mouse-button codes, re-exported so a widget never writes a bare `2` to
/// mean "secondary button".
pub use types::mouse_button;
pub use types::{Event, EventHandler, EventPriority, GestureClass, TouchId};
// Re-export queue utilities
pub use queue::{FixedSizeQueue, QueueError, DEFAULT_QUEUE_CAPACITY};
pub mod legacy_types;

pub use legacy_types::{KeyEvent, MouseEvent};
