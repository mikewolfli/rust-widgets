//! Embedded system optimizations and support.
pub mod config;
pub mod dpi;
pub mod flags;
pub mod input;
pub mod lightweight;
pub use config::*;
pub use dpi::*;
pub use flags::*;
pub use input::*;
pub use lightweight::*;
