//! Index-based widget registry (ObjectId lookup).
//!
//! Provides a `WidgetRegistry` that maps `ObjectId` → metadata for
//! runtime widget introspection, cross-module lookup, and debugging.

mod registry;

pub use registry::{WidgetEntry, WidgetRegistry};
