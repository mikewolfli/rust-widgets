// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display (monitor) enumeration for the macOS (cocoa) backend, over AppKit's `NSScreen`.
//!
//! # What this closes
//!
//! [`Platform::displays`](crate::platform::Platform::displays) had no macOS implementation, so a
//! host could not enumerate screens, ask which screen a window was on, or learn that the set
//! changed (see `docs/issues/feedback-20261015.md` §十二). AppKit already knows all three
//! (`+[NSScreen screens]`, `-[NSWindow screen]`, and the device description each screen carries), so
//! this module is the translation from those to this crate's own [`Display`].
//!
//! # The coordinate space
//!
//! AppKit's global screen space has its origin at the **bottom-left of the main display**, with `y`
//! increasing upward; a display mounted above the main one therefore reports a positive `y`. This
//! backend positions every `NSWindow` in that same space (`MacOSPlatform::make_rect` hands the
//! host's coordinates straight to `initWithContentRect:…`), so the rectangles below are reported
//! **unconverted**: a host comparing `displays()` with the geometry it gave `create_window` reads
//! two values in one space. The Linux/GTK backend reports GDK's top-left space instead; a host that
//! needs a single convention across backends converts, which is a separate concern from reading the
//! screens correctly here.
//!
//! # The id
//!
//! [`DisplayId`] is `NSScreen.deviceDescription[@"NSScreenNumber"]` — AppKit's `CGDirectDisplayID`,
//! which is stable while a display stays attached. [`window_display`] derives the id the same way
//! for the screen a window is on, so a caller can match the two.
//!
//! # The main-thread rule
//!
//! `NSScreen` is an AppKit object: it may only be messaged from the thread that owns AppKit. Every
//! entry point checks `[NSThread isMainThread]` first and answers the honest absence
//! (`Vec::new()` / `None` / `0`) otherwise, rather than messaging AppKit and raising.

use crate::core::Rect;
use crate::platform::types::{Display, DisplayId};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSRect, NSString};
use objc::{class, msg_send, sel, sel_impl};

/// The `deviceDescription` key that carries a screen's `CGDirectDisplayID`.
const SCREEN_NUMBER_KEY: &str = "NSScreenNumber";

/// Converts an AppKit rectangle into this crate's [`Rect`].
///
/// `NSRect` holds `CGFloat` (`f64`) values, so each is rounded rather than truncated (principle
/// #50: `round()`, not `as i32`). A negative extent cannot occur for a real screen but is clamped so
/// a `u32` width is never produced from one.
fn rect_of(rect: NSRect) -> Rect {
    Rect::new(
        rect.origin.x.round() as i32,
        rect.origin.y.round() as i32,
        rect.size.width.max(0.0).round() as u32,
        rect.size.height.max(0.0).round() as u32,
    )
}

/// A screen's `CGDirectDisplayID`, or `None` when AppKit reports no device description.
///
/// # Safety
///
/// `screen` must be a live `NSScreen` on the AppKit main thread. `types::is_main_thread()` at each
/// caller establishes that.
unsafe fn screen_number(screen: id) -> Option<u32> {
    let description: id = msg_send![screen, deviceDescription];
    if description == nil {
        return None;
    }
    // The key is an autoreleased `NSString`; the caller's autorelease pool owns it.
    let key = NSString::alloc(nil).init_str(SCREEN_NUMBER_KEY).autorelease();
    let number: id = msg_send![description, objectForKey: key];
    if number == nil {
        return None;
    }
    // `deviceDescription` is documented to hold an `NSNumber` under this key.
    Some(msg_send![number, unsignedIntValue])
}

/// The [`DisplayId`] for the screen at `index`.
///
/// One definition so [`displays`] and [`window_display`] cannot disagree about what a screen is
/// called: the device number when AppKit reports it, the enumeration index otherwise (a documented
/// fallback for a key that should always be present).
///
/// # Safety
///
/// `screen` must be a live `NSScreen` on the AppKit main thread.
unsafe fn display_id_at(index: usize, screen: id) -> DisplayId {
    DisplayId::new(screen_number(screen).unwrap_or(index as u32) as u64)
}

/// The displays AppKit reports, in `+[NSScreen screens]` order.
///
/// An empty list is returned off the AppKit main thread: messaging `NSScreen` from another thread
/// raises, and an honest `false`/empty answer is what the caller can act on.
pub(crate) fn displays() -> Vec<Display> {
    if !super::types::is_main_thread() {
        return Vec::new();
    }
    // SAFETY: the main thread is established, and every object messaged below (`NSScreen`, the
    // `NSArray` of screens, each `NSScreen`, its `deviceDescription`) is owned by AppKit for the
    // duration of the call. No object is retained.
    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        if screens == nil {
            return Vec::new();
        }
        let count: usize = msg_send![screens, count];
        let main_screen: id = msg_send![class!(NSScreen), mainScreen];
        // One pool for the autoreleased key strings `screen_number` makes below.
        let pool = NSAutoreleasePool::new(nil);
        let mut out = Vec::new();
        for index in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: index];
            if screen == nil {
                continue;
            }
            let frame: NSRect = msg_send![screen, frame];
            let visible: NSRect = msg_send![screen, visibleFrame];
            let scale: f64 = msg_send![screen, backingScaleFactor];
            out.push(Display {
                id: display_id_at(index, screen),
                bounds: rect_of(frame),
                work_area: rect_of(visible),
                // A non-positive scale would divide by zero downstream; AppKit reports >= 1.0, and
                // the clamp keeps a bad value from becoming a divide-by-zero.
                scale: if scale > 0.0 { scale as f32 } else { 1.0 },
                is_primary: screen == main_screen,
            });
        }
        pool.drain();
        out
    }
}

/// The `DisplayId` of the screen a window is currently on, or `None` when AppKit reports none.
///
/// `-[NSWindow screen]` is AppKit's own answer to "which screen is this window on", which is
/// exactly the question, so the window's frame is not re-matched to the screen list here.
///
/// # Safety
///
/// `window` must be a live `NSWindow` on the AppKit main thread.
pub(crate) unsafe fn window_display(window: id) -> Option<DisplayId> {
    if window == nil {
        return None;
    }
    let screen: id = msg_send![window, screen];
    if screen == nil {
        return None;
    }
    // The screen object is identified against the same enumeration `displays` walks, so the id is
    // the one `displays` published for it.
    let screens: id = msg_send![class!(NSScreen), screens];
    if screens == nil {
        return None;
    }
    let count: usize = msg_send![screens, count];
    let pool = NSAutoreleasePool::new(nil);
    let mut found = None;
    for index in 0..count {
        let candidate: id = msg_send![screens, objectAtIndex: index];
        if candidate == screen {
            found = Some(display_id_at(index, screen));
            break;
        }
    }
    pool.drain();
    found
}

/// A revision that changes when the display set (or a display's geometry/scale) changes.
///
/// Derived from the current configuration with FNV-1a, so a host compares consecutive readings
/// instead of subscribing to a notification it cannot receive on a thread it does not own. `0` means
/// "not observable from here" (off the main thread), which the trait contract reserves for that.
pub(crate) fn generation() -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    if !super::types::is_main_thread() {
        return 0;
    }
    let mut hash = FNV_OFFSET;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(FNV_PRIME);
    };
    let displays = displays();
    mix(displays.len() as u64);
    for display in displays {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An off-main call must be answered, not messaged into AppKit (which would raise).
    ///
    /// This holds on any host that compiles this module: a non-main thread is a non-main thread
    /// whether or not the process is a Cocoa one.
    #[test]
    fn an_off_main_thread_is_answered_instead_of_messaging_appkit() {
        let answer = std::thread::spawn(|| (displays(), generation()))
            .join()
            .expect("the probe thread must not panic");
        assert_eq!(answer, (Vec::new(), 0));
    }
}
