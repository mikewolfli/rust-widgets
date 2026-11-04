// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-safe widget handles backed by `ObjectId`, split by the event family each
//! handle serves.
//!
//! `types` owns the `WidgetHandle` trait, `SurfaceHandle` and the process-wide callback
//! registry; `properties` owns the one property-access route every family shares; `keys`,
//! `pointer`, `scroll`, `menu` and `window` own the handle/state families. The `impl_handle!`
//! macro lives with the pointer family, which expands it for most standard controls, and is
//! re-exported so every child can invoke it.

mod keys;
mod menu;
mod pointer;
mod properties;
mod scroll;
mod tabwidget;
mod types;
mod window;

#[cfg(test)]
mod tests;

pub use keys::*;
pub use menu::*;
pub use pointer::*;
pub use scroll::*;
pub use types::*;
pub use window::*;
// The shared property route is crate-visible rather than public: a host uses the handles, not
// these functions. Re-exported so every `use super::*` in a handle family reaches the one
// implementation instead of declaring its own.
pub(crate) use properties::*;
