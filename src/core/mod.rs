//! Core primitives and library-wide contracts.
//!
//! # Coordinate System
//!
//! This framework uses a **screen coordinate system** with the origin at the **top-left corner**:
//!
//! - **X axis**: Increases from left to right (0 -> width)
//! - **Y axis**: Increases from top to bottom (0 -> height)
//!
//! ## Coordinate Conventions
//!
//! ```text
//! (0, 0) -------------> X
//!   |
//!   |    Screen Space (pixels)
//!   |    Origin: Top-Left
//!   |
//!   v Y
//! ```
//!
//! ## Coordinate Transformations
//!
//! When working with other coordinate systems, use the helper functions:
//!
//! - `to_screen_y()`: Convert from Cartesian (bottom-left origin) to screen (top-left origin)
//! - `to_cartesian_y()`: Convert from screen (top-left origin) to Cartesian (bottom-left origin)
//! - `to_pdf_y()`: Convert from screen (top-left origin) to PDF (bottom-left origin)
//!
//! ## Module-Specific Notes
//!
//! - **Charts**: Data coordinates use Cartesian system (y increases upward), automatically converted to screen coordinates
//! - **PDF**: PDF uses bottom-left origin, converted from screen coordinates when rendering
//! - **SVG**: Uses same top-left origin as screen coordinates, no conversion needed
//! - **Widgets**: All widget positioning uses screen coordinates
mod alignment;
mod color;
pub mod coords;
mod font;
mod geometry;
mod mutex_ext;
pub mod rect_merge;
mod types;
pub use alignment::{Alignment, HorizontalAlignment, VerticalAlignment};
pub use color::Color;
pub use font::Font;
pub use geometry::{deg_to_rad, Orientation, Point, Rect, Size};
pub use mutex_ext::MutexExt;
pub use types::{
    CoreConfig, CoreError, CoreObject, CoreResult, DeviceClass, ObjectId, PlatformCapabilities,
    PlatformFamily, RuntimeProfile, Version,
};
