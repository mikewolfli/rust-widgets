// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Gesture recognizer system (BLUE8 P4-2).
//!
//! This module provides gesture recognition primitives that transform
//! raw touch events (`TouchBegin`/`TouchEnd`/`TouchMove`) into
//! semantic gesture events (`Tap`, `DoubleTap`, `LongPress`, `Swipe`,
//! `Pinch`, `Rotate`).
//!
//! # Reachability
//!
//! **State:** Production callers: `src/event/loop.rs:196` (`gesture_engine.process(...)`).

pub mod engine;
pub mod pinch;
pub mod press;
pub mod rotate;
pub mod swipe;
pub mod tap;

pub use engine::{GestureEngine, GestureRecognizer};
pub use pinch::{PinchGesture, PinchTouch};
pub use press::{LongPressDragGesture, LongPressGesture, PanGesture};
pub use rotate::RotateGesture;
pub use swipe::{FlingGesture, SwipeGesture, TwoFingerSwipeGesture};
pub use tap::{DoubleTapGesture, TapGesture, TwoFingerTapGesture};

// Re-export constants and helpers used by sub-modules.
pub(crate) use engine::{
    distance, DOUBLE_TAP_TIMEOUT_MS, LONG_PRESS_MAX_MOVE, LONG_PRESS_MIN_MS,
    MAX_STATIONARY_DISTANCE, PAN_MIN_DISTANCE, SWIPE_MIN_DISTANCE, SWIPE_MIN_VELOCITY,
    TAP_TIMEOUT_MS,
};
