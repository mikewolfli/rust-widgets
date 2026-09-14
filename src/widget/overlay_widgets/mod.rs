// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Overlay widget types — floating, dismissable, and gesture-driven overlays.

#[cfg(not(alloc_frugal))]
pub mod fab;
#[cfg(not(alloc_frugal))]
#[cfg(not(alloc_frugal))]
pub mod refresh_control;
#[cfg(not(alloc_frugal))]
pub mod swipe_to_dismiss;

#[cfg(not(alloc_frugal))]
pub use fab::FAB;
#[cfg(not(alloc_frugal))]
pub use refresh_control::RefreshControl;
#[cfg(not(alloc_frugal))]
pub use swipe_to_dismiss::SwipeToDismiss;
