// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Asset module - generic file watching utilities.
//!
//! Provides a predicate-based file watcher that can monitor any directory
//! for changes to files matching a user-supplied filter.
//!
//! # Reachability
//!
//! **State:** Reserved: the asset watcher is exercised only by its own tests; `CssWatcher` covers the one production hot-reload need. Removal condition: when a consumer needs asset invalidation, or when the tests are retired.

mod watcher;

pub use watcher::{AssetEvent, AssetWatcher};
