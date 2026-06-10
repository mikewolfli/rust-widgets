//! Language bindings module — provides FFI, interop bridges, and
//! binding implementations for integrating with other languages.
mod binding_impl;
pub use binding_impl::*;

#[cfg(feature = "jni")]
pub mod java_jni;
