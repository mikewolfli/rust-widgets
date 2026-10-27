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
//! - `core` — CupertinoSwitch, CupertinoAlertDialog, CupertinoSlider
//! - `date_picker` — CupertinoDatePicker
//! - `material` — MaterialNavigationRail, MaterialSnackbar, RailItem
//! - `nav_bar` — CupertinoNavigationBar
//! - `segmented_control` — CupertinoSegmentedControl

pub mod core;
pub mod date_picker;
pub mod material;
pub mod nav_bar;
pub mod navigation_stack;
pub mod segmented_control;

// Re-exports from core
pub use core::{CupertinoAlertDialog, CupertinoSlider, CupertinoSwitch};
pub use date_picker::CupertinoDatePicker;
pub use material::{MaterialNavigationRail, MaterialSnackbar, NavigationRailLabelMode, RailItem};
pub use nav_bar::{CupertinoNavigationBar, NavBarItem, NavBarTitleMode};
pub use navigation_stack::CupertinoNavigationStack;
pub use segmented_control::{CupertinoSegment, CupertinoSegmentedControl, SegmentTint};
