// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Asset module - generic file watching utilities.
//!
//! Provides a predicate-based file watcher that can monitor any directory
//! for changes to files matching a user-supplied filter.
//!
//! It also carries the **logical resource identity** seam (`resource`): one name for an asset that
//! resolves the same at run time and in a static package. That half is pure data and traits with no
//! OS dependency, so it is compiled wherever the module is — the watcher is the part that needs
//! `desktop-runtime`.
//!
//! # Reachability
//!
//! **State:** Reserved: the asset watcher is exercised only by its own tests; `CssWatcher` covers the one production hot-reload need. Removal condition: when a consumer needs asset invalidation, or when the tests are retired.

mod watcher;

pub use watcher::{AssetEvent, AssetWatcher, ChangeKind};

pub mod resource;
pub use resource::{
    EmbeddedSource, ResolveError, ResourceId, ResourceIdError, ResourceResolver, ResourceSource,
};
