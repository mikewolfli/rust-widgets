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

#[cfg(not(feature = "mini"))]
pub mod core;
#[cfg(not(feature = "mini"))]
pub mod date_picker;
#[cfg(not(feature = "mini"))]
pub mod nav_bar;
#[cfg(not(feature = "mini"))]
pub mod segmented_control;

// Re-exports from core
#[cfg(not(feature = "mini"))]
pub use core::{
    CupertinoAlertDialog, CupertinoSlider, CupertinoSwitch, MaterialNavigationRail,
    MaterialSnackbar, RailItem,
};
#[cfg(not(feature = "mini"))]
pub use date_picker::CupertinoDatePicker;
#[cfg(not(feature = "mini"))]
pub use nav_bar::CupertinoNavigationBar;
#[cfg(not(feature = "mini"))]
pub use segmented_control::CupertinoSegmentedControl;
