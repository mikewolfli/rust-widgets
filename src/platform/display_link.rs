// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CoreVideo display-link callback that wakes the frame loop at presentation time.
//!
//! # Why this is shared rather than per backend
//!
//! Two macOS backends need the same signal: the cocoa sibling (`macos/display_link.rs`) and the
//! objc2 backend. The CoreVideo side is pure C ABI with no Objective-C in it, so duplicating the
//! `CVTimeStamp` layout and the mach timebase conversion in both modules was two copies of one
//! fact — and the linker noticed, because both copies declare the same symbols. This module owns
//! the link; a backend adds only what the C ABI cannot express (the cocoa backend's AppKit
//! observers).
//!
//! The callback does no rendering and owns no frame state: it publishes the presented host time
//! and calls [`crate::platform::frame_wait::wake`], the same wake primitive every other backend
//! uses. The loop keeps driving [`crate::drive_frame`] and reads the deadline through
//! [`next_deadline`].
//!
//! # Lifecycle
//!
//! The link is process-wide and idempotent. [`start`] creates it once, installs the callback, and
//! starts it; [`stop`] stops and releases it. A display that is added, removed, or reconfigured
//! while the process runs is handled by [`restart_if_stalled`]: a link whose callback has gone
//! quiet is restarted, which is the documented way to make CoreVideo re-read the active display
//! set. A backend with an AppKit notification for the same event calls [`restart`] directly and
//! gets the same result sooner.
//!
//! # Failure is an answer
//!
//! A host with no window server (CI, a headless session, a sandbox that denies CoreVideo) gets
//! `false` from [`start`] and `None` from [`next_deadline`]; nothing panics and the fixed frame
//! grid keeps running.

#![cfg(all(
    target_os = "macos",
    any(all(feature = "cocoa-legacy", widgets_unstripped), feature = "macos")
))]

use core::ffi::c_void;
use core::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `kCVReturnSuccess`.
const CV_RETURN_SUCCESS: i32 = 0;

/// `kCVTimeStampHostTimeValid`: the `host_time` field of a timestamp is meaningful.
const CV_TIME_STAMP_HOST_TIME_VALID: u64 = 1 << 1;

/// `CVDisplayLinkOutputCallback`'s option flags.
type CvOptionFlags = u64;

/// An opaque `CVDisplayLinkRef`.
type CvDisplayLinkRef = *mut c_void;

/// `CVSMPTETime`, reproduced because the callback receives it by value inside `CVTimeStamp`.
///
/// Only the surrounding timestamp's `host_time` and `flags` are read, but every field must stay so
/// the struct's size and offsets match CoreVideo's.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct CvSmpTeTime {
    subframes: i16,
    subframe_divisor: i16,
    counter: u32,
    time_type: u32,
    flags: u32,
    hours: i16,
    minutes: i16,
    seconds: i16,
    frames: i16,
}

/// `CVTimeStamp` in its C ABI form.
///
/// FFI layout struct: the Rust side reads only some fields, but dropping any field
/// would change the struct's size and offsets against CoreVideo's ABI, hence the allow.
#[repr(C)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct CvTimeStamp {
    version: u32,
    video_time_scale: i32,
    video_time: i64,
    host_time: u64,
    rate_scalar: f64,
    video_refresh_period: i64,
    smpte_time: CvSmpTeTime,
    flags: u64,
    reserved: u64,
}

/// The callback CoreVideo invokes on its own thread.
type CvDisplayLinkOutputCallback = extern "C" fn(
    CvDisplayLinkRef,
    *const CvTimeStamp,
    *const CvTimeStamp,
    CvOptionFlags,
    *mut CvOptionFlags,
    *mut c_void,
) -> i32;

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(display_link: *mut CvDisplayLinkRef) -> i32;
    fn CVDisplayLinkSetOutputCallback(
        display_link: CvDisplayLinkRef,
        callback: CvDisplayLinkOutputCallback,
        user_info: *mut c_void,
    ) -> i32;
    fn CVDisplayLinkStart(display_link: CvDisplayLinkRef) -> i32;
    fn CVDisplayLinkStop(display_link: CvDisplayLinkRef) -> i32;
    fn CVDisplayLinkRelease(display_link: CvDisplayLinkRef);
}

/// `mach_timebase_info_data_t`.
#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

#[link(name = "System")]
unsafe extern "C" {
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
}

/// The host time of the newest presented frame, or `0` when none has arrived.
static NEXT_PRESENTED_HOST_TIME: AtomicU64 = AtomicU64::new(0);

/// The host time of the newest callback, whether or not it carried a valid presentation stamp.
///
/// Kept separately from [`NEXT_PRESENTED_HOST_TIME`] because it is the liveness signal: a link
/// that fires but cannot describe its presentation is still alive, and restarting it would be
/// wrong.
static LAST_CALLBACK_HOST_TIME: AtomicU64 = AtomicU64::new(0);

/// Whether the display link is started, readable without taking a lock.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Whether a backend registered its own lifecycle observers on top of this link.
static OBSERVERS_INSTALLED: AtomicBool = AtomicBool::new(false);

/// The link pointer plus its running flag.
struct LinkState {
    /// `CVDisplayLinkRef` as a `usize`, because the raw pointer is not `Send` and the mutex must
    /// be shared across threads. It is only converted back on the thread that takes the lock.
    link: usize,
    running: bool,
}

static STATE: Mutex<LinkState> = Mutex::new(LinkState { link: 0, running: false });

/// The mach timebase, resolved once.
static TIMEBASE: OnceLock<(u32, u32)> = OnceLock::new();

fn lock_state() -> std::sync::MutexGuard<'static, LinkState> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Creates the process-wide display link and starts it.
///
/// Returns `true` only when CoreVideo accepted every step. A second call while running is a no-op
/// that answers `true`; a call after [`stop`] creates a fresh link. A failure leaves the fixed
/// frame grid in charge and does not poison later attempts.
pub(crate) fn start() -> bool {
    let mut state = lock_state();
    if state.running {
        return true;
    }

    let link = if state.link == 0 {
        let mut created: CvDisplayLinkRef = ptr::null_mut();
        // SAFETY: `created` is a valid out-pointer for the initializer, which writes one owned
        // `CVDisplayLinkRef` into it. No other memory is read or written by this call.
        let create_result = unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut created) };
        if create_result != CV_RETURN_SUCCESS || created.is_null() {
            return false;
        }
        // SAFETY: `created` is a live reference returned by the initializer above. The callback
        // has the ABI CoreVideo documents, and the null context means the callback needs no
        // per-instance state: it publishes into the module's atomics.
        let callback_result = unsafe {
            CVDisplayLinkSetOutputCallback(created, display_link_callback, ptr::null_mut())
        };
        if callback_result != CV_RETURN_SUCCESS {
            // SAFETY: `created` is still owned here and no callback has been installed.
            unsafe { CVDisplayLinkRelease(created) };
            return false;
        }
        state.link = created as usize;
        created
    } else {
        state.link as CvDisplayLinkRef
    };

    // SAFETY: `link` is a live reference created above or carried over from a completed stop.
    let start_result = unsafe { CVDisplayLinkStart(link) };
    if start_result != CV_RETURN_SUCCESS {
        // Keep the reference so a later start can retry without creating a second link.
        return false;
    }

    state.running = true;
    RUNNING.store(true, Ordering::Release);
    true
}

/// Stops and releases the process-wide display link.
///
/// Safe to call when nothing is running, and safe to call twice: the second call is a no-op.
pub(crate) fn stop() {
    let mut state = lock_state();
    if state.link != 0 {
        let link = state.link as CvDisplayLinkRef;
        if state.running {
            // SAFETY: `link` is the live reference this module started. `CVDisplayLinkStop`
            // returns after the callback thread is quiesced, so releasing it below cannot race a
            // callback.
            unsafe { CVDisplayLinkStop(link) };
        }
        // SAFETY: the reference was created by this module and is released exactly once.
        unsafe { CVDisplayLinkRelease(link) };
        state.link = 0;
    }
    state.running = false;
    RUNNING.store(false, Ordering::Release);
    NEXT_PRESENTED_HOST_TIME.store(0, Ordering::Release);
    LAST_CALLBACK_HOST_TIME.store(0, Ordering::Release);
}

/// Whether the display link is currently started.
pub(crate) fn is_running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

/// Whether a backend has already registered its own lifecycle observers.
///
/// Only the cocoa backend has any, but the flag lives here because the registration decision is
/// made in [`start`], which both backends call through their own thin wrapper.
pub(crate) fn observers_installed() -> bool {
    OBSERVERS_INSTALLED.load(Ordering::Relaxed)
}

/// Records that a backend registered its lifecycle observers.
pub(crate) fn mark_observers_installed() {
    OBSERVERS_INSTALLED.store(true, Ordering::Relaxed);
}

/// The next presented-time instant, when a callback has supplied one.
///
/// A link that has gone quiet is restarted here; see this module's docs for why that takes the
/// place of a notification observer.
pub(crate) fn next_deadline() -> Option<Instant> {
    if !is_running() {
        return None;
    }
    restart_if_stalled();
    deadline_from_host_time(NEXT_PRESENTED_HOST_TIME.load(Ordering::Acquire))
}

/// Restarts the link when its callback has not fired for well over a refresh interval.
///
/// The threshold is deliberately generous: a paused or occluded window may legitimately not
/// present for a while, and a restart is only useful when CoreVideo has genuinely stopped — a
/// display was added, removed, or reconfigured. A restart re-reads the active display set, which
/// is the documented way to follow that change without an AppKit notification.
fn restart_if_stalled() {
    let last = LAST_CALLBACK_HOST_TIME.load(Ordering::Acquire);
    if last == 0 {
        // The link was started and has not fired yet; give it time rather than restarting it on
        // its first frame.
        return;
    }
    // SAFETY: `mach_absolute_time` takes no arguments and only reads process-wide clock state.
    let now = unsafe { mach_absolute_time() };
    if now <= last {
        return;
    }
    const STALL_NANOS: u64 = 1_000_000_000;
    if host_ticks_to_nanos(now - last) < STALL_NANOS {
        return;
    }
    restart();
}

/// Restarts the display link so CoreVideo re-reads the display configuration.
///
/// The cocoa backend calls this from its screen-parameters observer; a backend without that
/// observer relies on [`restart_if_stalled`].
pub(crate) fn restart() {
    stop();
    let _ = start();
}

/// Converts a mach host time to an [`Instant`], or `None` when no time was recorded.
fn deadline_from_host_time(host_time: u64) -> Option<Instant> {
    if host_time == 0 {
        return None;
    }
    // SAFETY: `mach_absolute_time` and `mach_timebase_info` take no arguments and only read
    // process-wide clock state; neither can fail in a way that affects memory safety.
    let now_host = unsafe { mach_absolute_time() };
    let now = Instant::now();
    if host_time <= now_host {
        return Some(now);
    }
    Some(
        now.checked_add(Duration::from_nanos(host_ticks_to_nanos(host_time - now_host)))
            .unwrap_or(now),
    )
}

/// Converts mach ticks to nanoseconds using the resolved timebase.
fn host_ticks_to_nanos(ticks: u64) -> u64 {
    let (numer, denom) = timebase();
    let nanos = u128::from(ticks) * u128::from(numer) / u128::from(denom);
    u64::try_from(nanos).unwrap_or(u64::MAX)
}

/// Reads the mach timebase, falling back to 1:1 when the kernel refuses the query.
fn timebase() -> (u32, u32) {
    *TIMEBASE.get_or_init(|| {
        let mut info = MachTimebaseInfo { numer: 0, denom: 0 };
        // SAFETY: `info` is a valid out-pointer for `mach_timebase_info_data_t`.
        let result = unsafe { mach_timebase_info(&mut info) };
        if result != 0 || info.numer == 0 || info.denom == 0 {
            (1, 1)
        } else {
            (info.numer, info.denom)
        }
    })
}

/// The CoreVideo callback.
///
/// It publishes the presented host time and wakes a loop parked in the frame wait. No frame is
/// rendered and no state machine is touched here; that work stays on the loop's own thread.
extern "C" fn display_link_callback(
    _link: CvDisplayLinkRef,
    _in_now: *const CvTimeStamp,
    in_output_time: *const CvTimeStamp,
    _flags_in: CvOptionFlags,
    _flags_out: *mut CvOptionFlags,
    _context: *mut c_void,
) -> i32 {
    // SAFETY: `mach_absolute_time` takes no arguments and only reads clock state.
    LAST_CALLBACK_HOST_TIME.store(unsafe { mach_absolute_time() }, Ordering::Release);
    if !in_output_time.is_null() {
        // SAFETY: CoreVideo owns `in_output_time` and guarantees it is a readable `CVTimeStamp`
        // for the duration of this callback.
        let stamp = unsafe { &*in_output_time };
        if stamp.flags & CV_TIME_STAMP_HOST_TIME_VALID != 0 {
            NEXT_PRESENTED_HOST_TIME.store(stamp.host_time, Ordering::Release);
        }
    }
    crate::platform::frame_wait::wake();
    CV_RETURN_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backing-scale conversion that the kernel refused falls back to 1:1 rather than dividing
    /// by zero.
    #[test]
    fn the_timebase_is_never_zero() {
        let (numer, denom) = timebase();
        assert!(numer > 0 && denom > 0);
    }

    /// No presentation has been recorded on a fresh process, so the honest answer is `None`.
    #[test]
    fn an_unrecorded_presentation_has_no_deadline() {
        assert_eq!(deadline_from_host_time(0), None);
    }

    /// A presented time in the past maps to "now", not to a negative duration.
    #[test]
    fn a_past_presentation_maps_to_now() {
        let deadline = deadline_from_host_time(1).expect("a host time of 1 is still a time");
        assert!(deadline <= Instant::now() + Duration::from_millis(1));
    }
}
