// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Debounce wrapper for signal connections.
//!
//! # What debounce means here
//!
//! A debounced call does not run immediately. It starts (or restarts) a quiet window, and only the
//! **last** call made before the window closes runs, once, after the delay. A text field whose
//! `text_changed` signal updates a model is the canonical case: typing ten characters should
//! produce one model update, not ten.
//!
//! # Why a wrapper type rather than a signal method
//!
//! The timing is a property of one *connection*, not of the signal: two listeners may want
//! different delays, and a non-debounced listener may want every value. [`Debounced`] owns the
//! clock and the slot, and [`Signal::connect_debounced`] is the one-line wiring for the common
//! case. The dispatcher is injectable so the timing rule can be tested without sleeping or
//! starting a frame loop.
//!
//! # Relationship to throttle
//!
//! A throttle would run the *first* call and then ignore the rest for a window; this type is the
//! trailing-edge debounce. The two are not variants of one flag because they answer different
//! questions, and a caller that needs a leading edge can schedule its own first call.

use super::{ConnectionHandle, Signal};
use crate::compat::{lock, Mutex};
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

/// The transport a debounce uses to wait for its window.
pub type DebounceDispatcher = dyn Fn(Duration, Box<dyn FnOnce() + Send + 'static>) + Send + Sync;

/// Trailing-edge debounce around one callback.
///
/// The wrapper is cheap to share (`call` and `cancel` take `&self`), so a connection can capture
/// one value and every emit on every thread goes through the same generation counter.
pub struct Debounced<T: Clone + Send + 'static> {
    delay: Duration,
    /// Incremented on every call and on every explicit cancellation. A scheduled item runs only
    /// when the value it captured is still the newest one; that comparison is what collapses a
    /// burst into one trailing call.
    generation: Arc<AtomicU64>,
    slot: Arc<Mutex<Box<dyn FnMut(T) + Send>>>,
    dispatch: Arc<DebounceDispatcher>,
}

impl<T: Clone + Send + 'static> Debounced<T> {
    /// Creates a debounce that waits on the event loop's main thread.
    pub fn new<F>(delay: Duration, slot: F) -> Self
    where
        F: FnMut(T) + Send + 'static,
    {
        Self::with_dispatcher(delay, slot, |delay, work| {
            // Work queued with zero delay still runs on the loop's next frame; a non-zero delay
            // runs on the first frame at or after the deadline. The frame granularity is the
            // documented resolution of the platform's main-thread queue.
            let _ = crate::platform::main_thread::schedule_in(delay, work);
        })
    }

    /// Creates a debounce with an explicit transport.
    ///
    /// The transport receives the remaining delay and the work to run. A test supplies a queue it
    /// drains by hand; a host with its own scheduler supplies that instead.
    pub fn with_dispatcher<F, D>(delay: Duration, slot: F, dispatch: D) -> Self
    where
        F: FnMut(T) + Send + 'static,
        D: Fn(Duration, Box<dyn FnOnce() + Send + 'static>) + Send + Sync + 'static,
    {
        let slot: Box<dyn FnMut(T) + Send> = Box::new(slot);
        let dispatch: Arc<DebounceDispatcher> = Arc::new(dispatch);
        Self {
            delay,
            generation: Arc::new(AtomicU64::new(0)),
            slot: Arc::new(Mutex::new(slot)),
            dispatch,
        }
    }

    /// The quiet window this wrapper waits for.
    pub fn delay(&self) -> Duration {
        self.delay
    }

    /// Requests `value`. Earlier pending values for this wrapper are discarded.
    pub fn call(&self, value: T) {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let newest = Arc::clone(&self.generation);
        let slot = Arc::clone(&self.slot);
        let work: Box<dyn FnOnce() + Send + 'static> = Box::new(move || {
            if newest.load(Ordering::Acquire) == generation {
                let mut callback = lock(&slot);
                (**callback)(value);
            }
        });
        (self.dispatch.as_ref())(self.delay, work);
    }

    /// Drops any pending value. The next [`Self::call`] starts a fresh window.
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

impl<T: Clone + Send + 'static> Signal<T> {
    /// Connects a slot that receives only the last value of a burst.
    ///
    /// This is [`Debounced::new`] wired into a signal. The returned handle is the ordinary
    /// connection handle; disconnecting it stops future calls, though a value already scheduled
    /// in the quiet window still runs, exactly as an in-flight queued value does.
    pub fn connect_debounced<F>(&self, delay: Duration, slot: F) -> ConnectionHandle
    where
        F: FnMut(T) + Send + Sync + 'static,
    {
        let debounced = Debounced::new(delay, slot);
        self.connect_ref(move |value: &T| debounced.call(value.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::Vec;

    /// Three calls inside one window produce exactly one run, carrying the last value.
    #[test]
    fn a_burst_collapses_to_its_last_value() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>> = Arc::new(Mutex::new(Vec::new()));

        let slot_calls = Arc::clone(&calls);
        let dispatch_queue = Arc::clone(&queue);
        let debounced = Debounced::with_dispatcher(
            Duration::from_millis(50),
            move |value| lock(&slot_calls).push(value),
            move |_delay, work| lock(&dispatch_queue).push(work),
        );

        debounced.call(1);
        debounced.call(2);
        debounced.call(3);
        assert_eq!(lock(&queue).len(), 3, "each call schedules a candidate");
        assert!(lock(&calls).is_empty(), "nothing runs before the window closes");

        let pending: Vec<Box<dyn FnOnce() + Send>> = lock(&queue).drain(..).collect();
        for work in pending {
            work();
        }
        assert_eq!(*lock(&calls), [3], "only the newest candidate may run");
    }

    /// Cancelling invalidates a pending value without scheduling a replacement.
    #[test]
    fn cancelling_drops_the_pending_value() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<Vec<Box<dyn FnOnce() + Send>>>> = Arc::new(Mutex::new(Vec::new()));

        let slot_calls = Arc::clone(&calls);
        let dispatch_queue = Arc::clone(&queue);
        let debounced = Debounced::with_dispatcher(
            Duration::from_millis(10),
            move |value| lock(&slot_calls).push(value),
            move |_delay, work| lock(&dispatch_queue).push(work),
        );

        debounced.call(9);
        debounced.cancel();
        let pending: Vec<Box<dyn FnOnce() + Send>> = lock(&queue).drain(..).collect();
        for work in pending {
            work();
        }
        assert!(lock(&calls).is_empty(), "a cancelled value must not run");
    }

    /// The wrapper reports the delay it was constructed with, so a caller can distinguish two
    /// debounces without keeping the duration beside them.
    #[test]
    fn the_delay_is_reported() {
        let debounced = Debounced::new(Duration::from_millis(75), |_value: u32| {});
        assert_eq!(debounced.delay(), Duration::from_millis(75));
    }
}
