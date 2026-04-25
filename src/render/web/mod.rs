//! Web rendering: web engine view integration.
//!
//! # Note
//! Items in `engine` and `view` are currently unused outside their module
//! boundary. Once integration is wired up, remove this module-level allow.
#![allow(dead_code)]
pub(crate) mod engine;
pub(crate) mod view;
