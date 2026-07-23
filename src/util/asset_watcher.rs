//! Generic file-system asset watcher.
//!
//! Re-exports the consolidated `AssetWatcher` from `crate::asset::watcher`.
//! This module exists for backward compatibility and provides the richer
//! `watch()` API with recursive mode support.

#[cfg(feature = "desktop")]
pub use crate::asset::AssetEvent;
#[cfg(feature = "desktop")]
pub use crate::asset::AssetWatcher;
