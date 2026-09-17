// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Freeform shape — a path-based non-rectangular clickable control.
//!
//! # What makes this control different
//!
//! Every other control in this crate occupies a rectangle and takes every pointer
//! event inside it. This one hit-tests the pointer against its actual outline, so a
//! press in the transparent corner *outside* a star falls through to whatever is
//! behind it. That is the whole reason the control exists, and it is why
//! [`ShapePath`] is data rather than a private detail: the hit test and the drawing
//! have to agree about the same outline.
//!
//! # Module layout
//!
//! * `types` — [`ShapePath`], [`PathSegment`] and [`BubbleTailDirection`]: the
//!   outline vocabulary. A host can build and store one without constructing a
//!   control, so it is not an implementation detail of one.
//! * `shape` — [`FreeformShapeWidget`], the control that draws a path and hit-tests
//!   against it.
//! * `tests` — both, kept together because the hit test is only meaningful against
//!   the path it agrees with.

mod shape;
mod types;

#[cfg(test)]
mod tests;

pub use shape::FreeformShapeWidget;
pub use types::{BubbleTailDirection, PathSegment, ShapePath};
