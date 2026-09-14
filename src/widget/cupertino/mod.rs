// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Cupertino (iOS-style) widget collection.
//!
//! This module provides iOS-style wrappers around existing widgets. These
//! style aliases apply Cupertino design language (colors, typography, and
//! interaction patterns) while delegating all widget mechanics to the
//! underlying control.
//!
//! ## Sub-modules
//!
//! - `core` — CupertinoSwitch, CupertinoAlertDialog, CupertinoSlider,
//!   MaterialNavigationRail, MaterialSnackbar, RailItem
//! - `date_picker` — CupertinoDatePicker
//! - `nav_bar` — CupertinoNavigationBar
//! - `segmented_control` — CupertinoSegmentedControl

#[cfg(not(alloc_frugal))]
pub mod core;
#[cfg(not(alloc_frugal))]
pub mod date_picker;
#[cfg(not(alloc_frugal))]
pub mod nav_bar;
#[cfg(not(alloc_frugal))]
pub mod segmented_control;

// Re-exports from core
#[cfg(not(alloc_frugal))]
pub use core::{
    CupertinoAlertDialog, CupertinoSlider, CupertinoSwitch, MaterialNavigationRail,
    MaterialSnackbar, RailItem,
};
#[cfg(not(alloc_frugal))]
pub use date_picker::CupertinoDatePicker;
#[cfg(not(alloc_frugal))]
pub use nav_bar::CupertinoNavigationBar;
#[cfg(not(alloc_frugal))]
pub use segmented_control::CupertinoSegmentedControl;
