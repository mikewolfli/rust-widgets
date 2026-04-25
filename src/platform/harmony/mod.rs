//! Harmony desktop backend shell (sub-module split).
pub mod platform_impl;
pub mod types;

pub use types::*;
#[cfg(test)]
pub mod tests;
