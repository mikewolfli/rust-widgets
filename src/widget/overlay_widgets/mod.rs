// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Overlay widget types — floating, dismissable, and gesture-driven overlays.

#[cfg(widgets_unstripped)]
pub mod banner;
pub mod fab;
pub mod refresh_control;
#[cfg(full_widgets)]
pub mod splash_screen;
pub mod swipe_to_dismiss;

#[cfg(widgets_unstripped)]
pub use banner::Banner;
pub use fab::FAB;
pub use refresh_control::RefreshControl;
#[cfg(full_widgets)]
pub use splash_screen::SplashScreen;
pub use swipe_to_dismiss::SwipeToDismiss;
