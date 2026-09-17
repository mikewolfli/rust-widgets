// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Overlay widget types — floating, dismissable, and gesture-driven overlays.

#[cfg(widgets_unstripped)]
pub mod banner;
#[cfg(not(alloc_frugal))]
pub mod fab;
#[cfg(not(alloc_frugal))]
#[cfg(not(alloc_frugal))]
pub mod refresh_control;
#[cfg(full_widgets)]
pub mod splash_screen;
#[cfg(not(alloc_frugal))]
pub mod swipe_to_dismiss;

#[cfg(widgets_unstripped)]
pub use banner::Banner;
#[cfg(not(alloc_frugal))]
pub use fab::FAB;
#[cfg(not(alloc_frugal))]
pub use refresh_control::RefreshControl;
#[cfg(full_widgets)]
pub use splash_screen::SplashScreen;
#[cfg(not(alloc_frugal))]
pub use swipe_to_dismiss::SwipeToDismiss;
