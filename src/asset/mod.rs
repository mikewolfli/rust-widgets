//! Asset module - generic file watching utilities.
//!
//! Provides a predicate-based file watcher that can monitor any directory
//! for changes to files matching a user-supplied filter.

mod watcher;

pub use watcher::{AssetEvent, AssetWatcher};
