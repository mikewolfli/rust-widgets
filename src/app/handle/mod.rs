// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Type-safe widget handles backed by `ObjectId`, split by the event family each
//! handle serves.
//!
//! `types` owns the `WidgetHandle` trait, `SurfaceHandle`, the process-wide callback
//! registry and the property-access helpers; `keys`, `pointer`, `scroll`, `menu` and
//! `window` own the handle/state families. The `impl_handle!` macro lives with the
//! pointer family, which expands it for most standard controls, and is re-exported
//! so every child can invoke it.

mod keys;
mod menu;
mod pointer;
mod scroll;
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
