//! Windows platform backend implementation.

pub mod helpers;
mod notify;
mod platform_impl;
pub mod types;

pub use crate::platform::windows::helpers::*;
pub use crate::platform::windows::types::*;

#[cfg(test)]
mod tests;
