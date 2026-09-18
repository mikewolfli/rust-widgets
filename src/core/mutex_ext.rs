// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Extension trait for standardized mutex lock handling.
//!
//! Provides a `.lock_guard()` method that recovers from poisoned
//! mutexes by calling `into_inner()` on the poison error, instead
//! of panicking with `expect("... poisoned")`.

use crate::compat::{Mutex, MutexGuard};

/// Extension trait that adds `.lock_guard()` to `Mutex<T>`.
pub trait MutexExt<T> {
    /// Acquire the mutex lock, recovering from poison by discarding
    /// the poison flag and continuing with the inner value.
    fn lock_guard(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T> {
        crate::compat::lock(self)
    }
}
