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
//!
//! # Reachability
//!
//! **State:** Production callers: `src/lib.rs:1`; 191 files reference it (the event loop, gesture engine and every control's `EventHandler`).
/// Pointer capture: routes pointer events to the widget that claimed the
/// pointer, and releases it on explicit release or on widget destruction.
pub mod capture;
/// Drag-and-drop vocabulary: a payload, a placement, and a target that decides
/// whether to accept one.
///
/// Not a widget: it produces no `WidgetKind`. It is the shared state machine a
/// control uses when it is a drag source or a drop destination.
pub mod dnd;
pub mod event_queue;
/// Focus ownership and traversal order.
pub mod focus;
pub mod r#loop;
/// A generic, runtime-typed queue used by the event and task plumbing.
pub mod queue;
pub mod timer;
#[cfg(feature = "touch")]
pub mod translator;
pub mod types;
// Re-export public types
pub use capture::PointerCaptureManager;
pub use dnd::{DragPayload, DragSession, DropEffect, DropTarget};
pub use event_queue::{EventQueue, EventSender};
pub use focus::FocusManager;
pub use focus::FocusTraversalStrategy;
pub use r#loop::AnimationFrameRequest;
pub use r#loop::EventLoop;
pub use timer::IdleTask;
pub use timer::TimerManager;
/// Named mouse-button codes, re-exported so a widget never writes a bare `2` to
/// mean "secondary button".
pub use types::mouse_button;
pub use types::{Event, EventHandler, EventPriority, GestureClass, TouchId};
// Re-export queue utilities
pub use queue::{FixedSizeQueue, QueueError, DEFAULT_QUEUE_CAPACITY};
pub mod legacy_types;

pub use legacy_types::{KeyEvent, MouseEvent};
