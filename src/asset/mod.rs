// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Asset module - generic file watching utilities.
//!
//! Provides a predicate-based file watcher that can monitor any directory
//! for changes to files matching a user-supplied filter.

mod watcher;

pub use watcher::{AssetEvent, AssetWatcher};
