// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Miscellaneous widget types — specialized controls that don't fit
//! neatly into a single category.

#[cfg(not(alloc_frugal))]
pub mod avatar;
#[cfg(not(alloc_frugal))]
pub mod barcode_scanner;
#[cfg(not(alloc_frugal))]
pub mod bezier_curve_editor;
#[cfg(not(alloc_frugal))]
pub mod date_range_picker;
#[cfg(not(alloc_frugal))]
pub mod mobile_date_picker;
#[cfg(not(alloc_frugal))]
pub mod qr_code;
#[cfg(not(alloc_frugal))]
pub mod segmented_button;

// Re-exports
#[cfg(not(alloc_frugal))]
pub use avatar::Avatar;
#[cfg(not(alloc_frugal))]
pub use barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner};
#[cfg(not(alloc_frugal))]
pub use bezier_curve_editor::BezierCurveEditor;
#[cfg(not(alloc_frugal))]
pub use date_range_picker::DateRangePicker;
#[cfg(not(alloc_frugal))]
pub use mobile_date_picker::MobileDatePicker;
#[cfg(not(alloc_frugal))]
pub use qr_code::QRCode;
#[cfg(not(alloc_frugal))]
pub use segmented_button::{Segment, SegmentedButton};
