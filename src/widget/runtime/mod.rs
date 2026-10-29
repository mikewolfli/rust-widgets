// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Registry for library-painted widgets mounted into windows.
//!
//! # Why this exists
//!
//! `widget::WidgetFactory` can build a `Box<dyn Widget>` for any kind —
//! `CodeEditor`, `ColorPicker`, `GanttWidget`, … — but until this module existed
//! there was no path from such a box to pixels inside a real window. The platform
//! layer only knew how to create OS controls (an OS button, a text field, a
//! toolkit widget); a library-painted widget had to be handed to
//! `render_to_svg()` or it went nowhere. Mounting one into a window produced an
//! empty surface.
//!
//! This registry closes that loop. The host keeps ownership of the widget here,
//! keyed by [`ObjectId`]; a backend supplies the surface it paints widgets into
//! and, when a repaint is wanted, calls [`with_widget_mut`] and paints a
//! frame. **Which surface that is is a backend detail** — see
//! [`crate::platform::Platform::mount_surface`].
//!
//! # Threading
//!
//! Widgets are `!Send` (they own `Rc`/`RefCell` state), and the desktop backends
//! require UI work on the platform main thread anyway. The registry is therefore
//! **thread-local**, and [`register`] returns `None` when called from a thread
//! that has no registry rather than smuggling a non-`Send` value across threads.
//!
//! [`with_widget_mut`]: crate::widget::runtime::with_widget_mut

mod access;
mod cache;
mod dispatch;
mod focus;
mod frame;
mod pointer;
mod registry;
mod render;
mod repaint;
mod state;
mod window_map;

pub use access::*;
pub(crate) use cache::*;
pub use dispatch::*;
pub use focus::*;
pub use frame::*;
pub use pointer::*;
pub use registry::*;
pub use render::*;
pub use repaint::*;
pub use state::*;
pub use window_map::*;

/// Test module for the runtime registry and self-drawn frame rendering.
///
/// Runs only in `full_widgets` profiles: the tests exercise the self-drawn bridge
/// through concrete widgets in `special_widgets`, which is itself gated on
/// `full_widgets` (defined in `build.rs`). Gating on plain `test` alone would
/// make a stripped-down profile try to resolve types that are compiled out.
#[cfg(all(test, full_widgets))]
mod tests;
