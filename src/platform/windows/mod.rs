//! Windows platform backend implementation.

pub mod helpers;
pub mod types;
mod notify;
mod platform_impl;

pub use crate::platform::windows::helpers::*;
pub use crate::platform::windows::types::*;

#[cfg(test)]
mod tests;
