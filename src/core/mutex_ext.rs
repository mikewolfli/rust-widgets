//! Extension trait for standardized mutex lock handling.
//!
//! Provides a `.lock_guard()` method that recovers from poisoned
//! mutexes by calling `into_inner()` on the poison error, instead
//! of panicking with `expect("... poisoned")`.

use std::sync::{Mutex, MutexGuard};

/// Extension trait that adds `.lock_guard()` to `Mutex<T>`.
pub trait MutexExt<T> {
    /// Acquire the mutex lock, recovering from poison by discarding
    /// the poison flag and continuing with the inner value.
    fn lock_guard(&self) -> MutexGuard<'_, T>;
}

impl<T> MutexExt<T> for Mutex<T> {
    fn lock_guard(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
