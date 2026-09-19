// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Miscellaneous widget types — specialized controls that don't fit
//! neatly into a single category.

pub mod avatar;
pub mod barcode_scanner;
pub mod bezier_curve_editor;
pub mod date_range_picker;
pub mod date_utils;
pub mod drop_zone;
pub mod mobile_date_picker;
pub mod qr_code;
pub mod segmented_button;

// Re-exports
pub use avatar::Avatar;
pub use barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner};
pub use bezier_curve_editor::BezierCurveEditor;
pub use date_range_picker::DateRangePicker;
pub use drop_zone::DropZone;
pub use mobile_date_picker::MobileDatePicker;
pub use qr_code::QRCode;
pub use segmented_button::{Segment, SegmentedButton};
