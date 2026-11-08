// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Canvas widget for custom drawing operations.
//!
//! The `Canvas` widget provides a drawing surface where users can push rendering commands
//! (lines, circles, rectangles, paths, text) and have them replayed during the `draw()` pass.
//! It also emits mouse interaction signals so callers can respond to clicks, drags and hover
//! events on the drawing area.
//!
//! This module is the entry point only: `widget.rs` holds the implementation, `ext.rs` holds
//! the canvas-only types plus the additive C-11'/RW-29..RW-33 seams, and `command.rs` holds the
//! shared command mapping (transform, snapping, bounds, culling). Re-exporting their public
//! surface keeps every historical path (`canvas::Canvas`, `canvas::CanvasTransform`, …) unchanged.

mod widget;

pub use widget::*;
