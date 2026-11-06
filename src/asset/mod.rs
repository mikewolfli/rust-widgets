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
//! **State:** Reserved: the asset watcher is exercised only by its own tests. `CssWatcher` is the
//! host-driven stylesheet reload entry (see its own Reachability note) and has no in-crate caller
//! either; neither module backs a production hot-reload path today. The `resource` half is the
//! logical-identity seam a host may adopt; it has no in-repo caller yet.
//!
//! **Removal condition:** delete `asset` when a consumer needs asset invalidation (then the watcher is production) or when the identity seam is adopted by `image::cache`/the icon resolver (then it stops being reserved). If neither happens by the next major release, remove the module and its tests together.

mod watcher;

pub use watcher::{AssetEvent, AssetWatcher, ChangeKind};

pub mod resource;
pub use resource::{
    EmbeddedSource, ResolveError, ResourceId, ResourceIdError, ResourceResolver, ResourceSource,
};
