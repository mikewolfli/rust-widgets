//! Miscellaneous widget types — specialized controls that don't fit
//! neatly into a single category.

#[cfg(not(feature = "mini"))]
pub mod avatar;
#[cfg(not(feature = "mini"))]
pub mod barcode_scanner;
#[cfg(not(feature = "mini"))]
pub mod bezier_curve_editor;
#[cfg(not(feature = "mini"))]
pub mod chip;
#[cfg(not(feature = "mini"))]
pub mod date_range_picker;
#[cfg(not(feature = "mini"))]
pub mod mobile_date_picker;
#[cfg(not(feature = "mini"))]
pub mod qr_code;
#[cfg(not(feature = "mini"))]
pub mod segmented_button;

// Re-exports
#[cfg(not(feature = "mini"))]
pub use avatar::Avatar;
#[cfg(not(feature = "mini"))]
pub use barcode_scanner::{BarcodeFormat, BarcodeResult, BarcodeScanner};
#[cfg(not(feature = "mini"))]
pub use bezier_curve_editor::BezierCurveEditor;
#[cfg(not(feature = "mini"))]
pub use chip::Chip;
#[cfg(not(feature = "mini"))]
pub use date_range_picker::DateRangePicker;
#[cfg(not(feature = "mini"))]
pub use mobile_date_picker::MobileDatePicker;
#[cfg(not(feature = "mini"))]
pub use qr_code::QRCode;
#[cfg(not(feature = "mini"))]
pub use segmented_button::{Segment, SegmentedButton};
