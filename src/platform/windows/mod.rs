//! Windows platform backend implementation.

mod dialogs;
pub mod helpers;
mod notify;
mod platform_impl;
pub mod types;

/// Native surface for self-drawn widgets (Windows).
#[cfg(target_os = "windows")]
pub(crate) mod canvas;

/// Win32 menu accelerator (`HACCEL`) tables.
#[cfg(target_os = "windows")]
pub(crate) mod accel;

pub use crate::platform::windows::helpers::*;
pub use crate::platform::windows::types::*;

#[cfg(test)]
mod tests;
