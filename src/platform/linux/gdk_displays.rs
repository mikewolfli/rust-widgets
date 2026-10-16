// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display (monitor) enumeration for the Linux/GTK backend, over GDK.
//!
//! # What this closes
//!
//! [`Platform::dpi_scale_factor`](crate::platform::Platform::dpi_scale_factor) answered a *single*
//! scale for the display the pointer happened to be on, and nothing could enumerate displays at
//! all: a host could not tell which screen a window was on, nor compare that against the screens
//! that remain after one is unplugged, nor notice that the set changed. GDK already knows all three
//! facts (`gdk_display_get_n_monitors`, `…_get_monitor`, `…_get_primary_monitor` and each
//! `GdkMonitor`'s geometry/workarea/scale), so this module is the thin translation from those to
//! this crate's own [`Display`] and probes nothing itself. See
//! `docs/issues/feedback-20261015.md` §十二.
//!
//! # The main-thread rule
//!
//! Every entry point checks `gtk::is_initialized_main_thread()` first, the same rule the rest of
//! this backend follows: GDK objects may only be touched from the thread that called `gtk::init`.
//! A call from anywhere else answers the honest absence (`Vec::new()` / `None` / `0`) rather than
//! touching GDK.
//!
//! # The one thing GDK3 does not give us
//!
//! GDK3 exposes monitors by **index** and has no stable per-monitor identifier, so [`Display::id`]
//! here is that index. Indexes are reassigned when the set of displays changes (unplug monitor 0
//! and what was monitor 1 becomes 0), which is exactly what
//! [`Platform::display_generation`](crate::platform::Platform::display_generation) is for: a host
//! re-reads `displays()` when the generation changes, rather than persisting an id across a change.

use crate::core::Rect;
use crate::platform::types::{Display, DisplayId};
// The `DisplayExt`/`MonitorExt` extension traits (`n_monitors`, `monitor`, `monitor_at_point`,
// `geometry`, `workarea`, `scale_factor`, `is_primary`) live in the prelude, not as inherent
// methods.
use gtk::prelude::*;

/// Runs `body` with GDK's default display, or `None` when this thread has no usable one.
fn with_display<R>(body: impl FnOnce(&gtk::gdk::Display) -> R) -> Option<R> {
    if !gtk::is_initialized_main_thread() {
        return None;
    }
    let display = gtk::gdk::Display::default()?;
    Some(body(&display))
}

/// Converts a GDK rectangle to this crate's [`Rect`].
///
/// `GdkRectangle` holds `i32` extents, so the origin transfers directly; negative extents cannot
/// occur in practice but are clamped so a `u32` width is never produced from a negative one.
fn rect_of(gdk: &gtk::gdk::Rectangle) -> Rect {
    Rect::new(gdk.x(), gdk.y(), gdk.width().max(0) as u32, gdk.height().max(0) as u32)
}

/// The displays GDK reports, in monitor-index order.
///
/// An empty list is returned off the GTK main thread and on a session with no default display
/// (a headless shell), which is the honest "no displays are observable from here".
pub(crate) fn displays() -> Vec<Display> {
    with_display(|display| {
        let count = display.n_monitors();
        let mut out = Vec::new();
        for index in 0..count {
            let Some(monitor) = display.monitor(index) else {
                continue;
            };
            let scale = monitor.scale_factor();
            out.push(Display {
                id: DisplayId::new(index as u64),
                bounds: rect_of(&monitor.geometry()),
                work_area: rect_of(&monitor.workarea()),
                // A non-positive scale would divide by zero downstream; GDK reports >= 1, and the
                // clamp keeps a bad backend value from becoming a divide-by-zero.
                scale: if scale > 0 { scale as f32 } else { 1.0 },
                is_primary: monitor.is_primary(),
            });
        }
        out
    })
    .unwrap_or_default()
}

/// The id of the display containing the point `(x, y)`, or `None` when GDK reports none.
///
/// The point is matched to the same index [`displays`] hands out, via `gdk_display_get_monitor_at_point`
/// — the same lookup GDK itself uses to decide where a window opening at that point lands.
pub(crate) fn display_index_at(x: i32, y: i32) -> Option<u64> {
    with_display(|display| {
        let hit = display.monitor_at_point(x, y)?;
        // GDK has no "which index is this monitor" call, so the index is found by identity against
        // the enumerated list (the same objects `displays()` reads).
        (0..display.n_monitors())
            .find(|&index| display.monitor(index).is_some_and(|m| m == hit))
            .map(|index| index as u64)
    })
    .flatten()
}

/// A revision that changes when the display set (or a display's geometry/scale) changes.
///
/// Derived from the current configuration with FNV-1a, so a host compares consecutive readings
/// instead of subscribing to a signal it cannot receive on a thread it does not own. `0` means
/// "not observable from here" (off the main thread, no display), which the trait contract reserves
/// for that meaning.
pub(crate) fn generation() -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    with_display(|display| {
        let mut hash = FNV_OFFSET;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(FNV_PRIME);
        };
        mix(display.n_monitors() as u64);
        for display in displays() {
            for part in [
                display.id.get(),
                display.bounds.x as i64 as u64,
                display.bounds.y as i64 as u64,
                display.bounds.width as u64,
                display.bounds.height as u64,
                display.work_area.x as i64 as u64,
                display.work_area.y as i64 as u64,
                display.work_area.width as u64,
                display.work_area.height as u64,
                display.scale.to_bits() as u64,
                display.is_primary as u64,
            ] {
                mix(part);
            }
        }
        hash
    })
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A thread that never called `gtk::init` must be answered the honest absence, not crashed into.
    ///
    /// This is the guard every entry point opens with. It holds on any host: a non-main thread is a
    /// non-main thread whether or not a display exists.
    #[test]
    fn a_non_main_thread_is_answered_instead_of_touching_gdk() {
        let answer = std::thread::spawn(|| (displays(), display_index_at(0, 0), generation()))
            .join()
            .expect("the probe thread must not panic");
        assert_eq!(answer, (Vec::new(), None, 0));
    }
}
