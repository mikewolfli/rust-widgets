// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Native UIKit FFI wrappers for the iOS backend.
//!
//! # What this module is responsible for
//!
//! It holds the one thing the iOS host owes the library that Rust cannot obtain
//! by itself: the **`UIWindow`**. Everything else about a widget — its geometry,
//! its text, its enabled/visible state, its painting — is expressed in
//! `BackendState<IosHandleKind>` and drawn by the library.
//!
//! All functions are gated behind `#[cfg(apple_uikit)]` and the
//! `ios-uikit-ffi` feature flag.
//!
//! # BLUE15: the wrapper no longer builds UIKit controls
//!
//! It used to export `create_ui_button` / `create_ui_label` / `create_ui_slider`
//! / … and the Rust-callable helpers that drove them (`store_native_view`,
//! `add_as_subview`, `wire_button_action`, `set_native_text`, …), so that each
//! logical widget became a real `UIView`. Under the self-drawn strategy that is
//! exactly the duplication BLUE15 removes: the library paints every `WidgetKind`,
//! and the host supplies a **window** plus a **drawing surface** (rules
//! #55/#56). A per-kind `create_ui_*` here had no consumer left, and the view
//! registry, the `ButtonTarget` class and its event queue existed only to serve
//! it, so they are gone too (rule #59: delete means delete).
//!
//! What survives is the window the library paints into, plus the geometry helper
//! that builds its frame, and the system pasteboard (§ *The clipboard* below).
//!
//! # The clipboard
//!
//! `UIPasteboard` is the one other host service iOS offers that a Rust program cannot
//! reimplement: the pasteboard is shared with every other app on the device, so an in-process
//! record is not a substitute. It is reached through `objc2-ui-kit`, already a dependency of the
//! `ios` feature, and it needs no window and no main-thread marker (the class is `Send + Sync`
//! and `generalPasteboard()` is a safe method).
//!
//! `setString:` / `setData:forPasteboardType:` report nothing, so every write here is confirmed
//! by reading the pasteboard back. That is the same rule the macOS backend follows by returning
//! `writeObjects:`'s `BOOL`: a write that cannot be observed must not be reported as a success.

#![cfg(apple_uikit)]
#![cfg(feature = "ios-uikit-ffi")]

use crate::compat::{BTreeMap, String, Vec};
use crate::core::Rect;
use crate::platform::clipboard::{ClipboardContent, RichClipboardBackend};
use crate::platform::types::{Display, DisplayId};
use objc2::MainThreadMarker;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSData, NSString};
use objc2_ui_kit::{
    NSObjectUIAccessibility, UIColor, UIPasteboard, UIScreen, UITraitCollection,
    UIUserInterfaceStyle, UIView, UIViewController, UIWindow,
};

/// The `UIWindow` each platform window id owns.
///
/// # Why the backend now keeps the window
///
/// The platform window was deliberately dropped after `makeKeyAndVisible()`, because no
/// control mutator needed it. `display_of_window` does: winit answers "which monitor is this
/// window on?" from `UIWindow.screen()`, and returning the primary screen for every window is
/// wrong on a device with an external display. `Retained<UIWindow>` is not `Send`, so the table
/// is main-thread-local and every accessor answers `None` off the main thread.
thread_local! {
    static UI_WINDOWS: core::cell::RefCell<BTreeMap<u64, objc2::rc::Retained<UIWindow>>> =
        core::cell::RefCell::new(BTreeMap::new());
}

/// The UIKit pasteboard type (UTI) for HTML.
const HTML_TYPE: &str = "public.html";
/// The pasteboard type `setString:` writes.
const PLAIN_TEXT_TYPE: &str = "public.utf8-plain-text";
/// The general text types a receiver may see for plain text.
const TEXT_TYPES: [&str; 2] = [PLAIN_TEXT_TYPE, "public.text"];

// ─── Geometry helpers ───

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(x as f64, y as f64),
        CGSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

// ─── Window creation ───

/// Create the native `UIWindow` the library paints into, and record it under `window_id`.
///
/// The returned handle is the same object the registry keeps. The caller may drop it; UIKit
/// already owns the window after `makeKeyAndVisible()`, and the registry holds the handle needed
/// for `display_of_window` / teardown.
pub(crate) fn create_ui_window(
    window_id: u64,
    mtm: MainThreadMarker,
    title: &str,
    _x: i32,
    _y: i32,
    width: u32,
    height: u32,
) -> objc2::rc::Retained<UIWindow> {
    let frame = make_rect(0, 0, width, height);
    // `init(windowScene:)` requires a UIWindowScene obtained from a real
    // UIApplication scene session, which is unavailable in the preview path.
    #[allow(deprecated)]
    let window = UIWindow::initWithFrame(mtm.alloc(), frame);
    let bg = UIColor::whiteColor();
    window.setBackgroundColor(Some(&bg));
    let vc = UIViewController::initWithNibName_bundle(mtm.alloc(), None, None);
    window.setRootViewController(Some(&vc));
    window.makeKeyAndVisible();
    // The window has no OS title chrome on iOS; record the name as the
    // accessibility label so it stays observable to assistive technology.
    //
    // `setAccessibilityLabel` comes from the `NSObjectUIAccessibility` trait and
    // takes the `MainThreadMarker` as well as the label, so the trait must be in
    // scope for the method to resolve. The marker is passed through rather than
    // re-derived: this function already proved main-thread context by taking one.
    window.setAccessibilityLabel(Some(&NSString::from_str(title)), mtm);
    UI_WINDOWS.with(|windows| {
        windows.borrow_mut().insert(window_id, window.clone());
    });
    window
}

/// Drops the registry entry for a window this backend destroyed.
///
/// UIKit may keep the window alive through its own ownership; this only releases the handle the
/// backend held for `display_of_window`. A stale id must not keep a window reachable forever.
pub(crate) fn forget_ui_window(window_id: u64) {
    UI_WINDOWS.with(|windows| {
        windows.borrow_mut().remove(&window_id);
    });
}

/// The index of the screen a window currently occupies, matching [`displays`].
///
/// This is the same lookup winit's UIKit backend performs (`UIWindow.screen()`), and the index is
/// the id [`displays`] hands out. `None` for an unknown window or off the main thread.
pub(crate) fn screen_id_of_window(window_id: u64) -> Option<u64> {
    let mtm = MainThreadMarker::new()?;
    UI_WINDOWS.with(|windows| {
        let windows = windows.borrow();
        let window = windows.get(&window_id)?;
        let screen = window.screen();
        UIScreen::screens(mtm)
            .iter()
            .position(|candidate| {
                core::ptr::eq(
                    objc2::rc::Retained::as_ptr(candidate),
                    objc2::rc::Retained::as_ptr(&screen),
                )
            })
            .map(|index| index as u64)
    })
}

// ─── Window geometry and appearance ───

/// The window's safe-area insets, in logical pixels.
///
/// `UIView.safeAreaInsets` is the same source winit's UIKit backend hands to
/// `Window::safe_area()`. It reports the notch/status-bar/home-indicator distances for **this**
/// window, which is why a recorded `UIWindow` is required: the primary screen's insets are wrong
/// for a window on an external display, and a missing window is `None` rather than a guess.
///
/// UIKit is main-thread-only, so this answers `None` off the main thread.
pub(crate) fn safe_area_insets(window_id: u64) -> Option<SafeAreaInsets> {
    let _mtm = MainThreadMarker::new()?;
    let insets = UI_WINDOWS.with(|windows| {
        let windows = windows.borrow();
        windows.get(&window_id).map(|window| window.safeAreaInsets())
    })?;
    // UIKit reports `CGFloat`; a negative or sub-pixel value is clamped rather than wrapped, and
    // the four distances are edge lengths, so rounding to the nearest logical pixel is exact
    // enough for layout (`0.4` must not become `0` via truncation, hence `round`).
    Some(SafeAreaInsets {
        top: logical_pixels(insets.top),
        bottom: logical_pixels(insets.bottom),
        left: logical_pixels(insets.left),
        right: logical_pixels(insets.right),
    })
}

fn logical_pixels(value: f64) -> u32 {
    value.max(0.0).round() as u32
}

/// Whether the system appearance is dark, from the current trait collection.
///
/// `UITraitCollection.currentTraitCollection()` is the trait environment UIKit applies to the
/// app; `userInterfaceStyle` is the light/dark bit. Both are main-thread-only (API 13+), so this
/// answers `None` off the main thread. `Unspecified` also answers `None` — "the OS did not say" —
/// rather than defaulting to light, because the two are different facts.
pub(crate) fn system_theme() -> Option<bool> {
    let _mtm = MainThreadMarker::new()?;
    // SAFETY: both calls are documented as main-thread-only; `_mtm` proves this is the main
    // thread, and `traits` is a live retained object for the duration of the second call.
    let traits = unsafe { UITraitCollection::currentTraitCollection() };
    let style = unsafe { traits.userInterfaceStyle() };
    if style == UIUserInterfaceStyle::Dark {
        Some(true)
    } else if style == UIUserInterfaceStyle::Light {
        Some(false)
    } else {
        None
    }
}

// ─── The system pasteboard ───

/// The general (system) pasteboard.
fn pasteboard() -> objc2::rc::Retained<UIPasteboard> {
    UIPasteboard::generalPasteboard()
}

/// Writes `text` to the system pasteboard and reports whether it arrived.
///
/// `setString:` has no result, so the confirmation is a read-back: `UIPasteboard` answers
/// synchronously from the pasteboard server, which is what makes the comparison meaningful
/// rather than a check of the value we just handed over.
pub(crate) fn pasteboard_set_text(text: &str) -> bool {
    let board = pasteboard();
    // SAFETY: `setString:`/`string` are documented as "might not be thread-safe"; the pasteboard
    // is a process-wide singleton and this is the only writer in this crate.
    unsafe {
        board.setString(Some(&NSString::from_str(text)));
        board.string().is_some_and(|stored| stored.to_string() == text)
    }
}

/// The system pasteboard's text, or `None` when it holds none.
pub(crate) fn pasteboard_text() -> Option<String> {
    let board = pasteboard();
    // SAFETY: see `pasteboard_set_text`.
    unsafe { board.string() }.map(|text| text.to_string())
}

/// Writes an HTML fragment (and its plain-text fallback) to the system pasteboard.
///
/// HTML travels as `NSData` under the `public.html` UTI rather than through
/// `setValue:forPasteboardType:`, which takes an `AnyObject` this crate has no safe conversion
/// for. The plain-text fallback goes over the text channel so a receiver that ignores HTML still
/// gets the words.
pub(crate) fn pasteboard_set_html(html: &str, plain: &str) -> bool {
    let board = pasteboard();
    let plain_ok = pasteboard_set_text(plain);
    // `setData:forPasteboardType:` and `dataForPasteboardType:` are the two methods on this class
    // that objc2 does *not* mark unsafe: they copy the bytes rather than hand back a borrowed
    // buffer.
    board.setData_forPasteboardType(
        &NSData::with_bytes(html.as_bytes()),
        &NSString::from_str(HTML_TYPE),
    );
    let html_ok = board
        .dataForPasteboardType(&NSString::from_str(HTML_TYPE))
        .is_some_and(|stored| stored.to_vec() == html.as_bytes());
    // The markup is what this call is asked for, but a pasteboard that refused the HTML channel
    // can still have accepted the plain fallback — which is the same "either channel landed"
    // rule the macOS backend applies to its two `setString:forType:` results.
    html_ok || plain_ok
}

/// The system pasteboard's HTML, or `None` when it holds none.
pub(crate) fn pasteboard_html() -> Option<String> {
    let board = pasteboard();
    let data = board.dataForPasteboardType(&NSString::from_str(HTML_TYPE))?;
    String::from_utf8(data.to_vec()).ok()
}

/// Whether the system pasteboard currently holds data of `content_type` (a MIME type, the
/// spelling [`ClipboardContent::content_type`] answers with).
pub(crate) fn pasteboard_has_format(content_type: &str) -> bool {
    match content_type {
        "text/html" => pasteboard_html().is_some(),
        "text/plain" => pasteboard_text().is_some(),
        // Any other type is carried as `NSData` under the caller's own pasteboard type. Asking
        // for the data is the only way to know whether a writer put it there, and it is
        // side-effect free.
        other => {
            let board = pasteboard();
            board.dataForPasteboardType(&NSString::from_str(other)).is_some()
        }
    }
}

/// Writes `data` under an application-defined pasteboard type.
///
/// `setData:forPasteboardType:` accepts a caller-supplied type name, so the type is carried
/// verbatim (the same rule the macOS backend implements through `NSPasteboard`'s own channel).
/// The write is confirmed by reading it back: the method returns nothing.
pub(crate) fn pasteboard_set_custom(content_type: &str, data: &[u8]) -> bool {
    let board = pasteboard();
    board.setData_forPasteboardType(&NSData::with_bytes(data), &NSString::from_str(content_type));
    board
        .dataForPasteboardType(&NSString::from_str(content_type))
        .is_some_and(|stored| stored.to_vec() == data)
}

/// The bytes the pasteboard holds under `content_type`.
pub(crate) fn pasteboard_custom(content_type: &str) -> Option<Vec<u8>> {
    let board = pasteboard();
    board.dataForPasteboardType(&NSString::from_str(content_type)).map(|data| data.to_vec())
}

/// The first pasteboard type that is not plain text or HTML.
///
/// The two fixed kinds have their own accessors; a caller asking for \"the custom payload\" must
/// not receive their backing types as if they were application-defined.
pub(crate) fn pasteboard_custom_type() -> Option<String> {
    let board = pasteboard();
    // SAFETY: `pasteboardTypes` is documented as \"might not be thread-safe\"; this is the same
    // rule `pasteboard_set_text` records for the pasteboard singleton.
    let types = unsafe { board.pasteboardTypes() };
    for type_name in types.iter() {
        let name = type_name.to_string();
        if name == HTML_TYPE || TEXT_TYPES.contains(&name.as_str()) {
            continue;
        }
        // The caller-facing `content_type` is a MIME name. A UTI (`public.png`, …) is the
        // platform's own type spelling, not a MIME, and reporting it as `Custom` would give the
        // caller a type it cannot use with `get_contents_for_type` on another backend.
        if !name.contains('/') {
            continue;
        }
        return Some(name);
    }
    None
}

/// The main screen's logical-to-pixel scale, or `None` off the main thread.
///
/// `UIScreen::scale` is the Points-to-pixels factor this crate's layout uses. winit's monitor
/// handle reports `nativeScale`, because its monitor geometry is in **physical** pixels; this
/// crate's [`Display`] contract is logical space plus a scale, so `scale` is the consistent
/// choice here (on a downsampling device `nativeScale` would disagree with the logical bounds).
pub(crate) fn main_screen_scale() -> Option<f32> {
    let mtm = MainThreadMarker::new()?;
    let screen = UIScreen::mainScreen(mtm);
    let scale = screen.scale() as f32;
    Some(if scale > 0.0 { scale } else { 1.0 })
}

/// The displays UIKit reports, in screen order.
///
/// `UIScreen` is main-thread-only, so a call from another thread answers the honest absence
/// rather than touching UIKit. The work area is the screen's bounds: iOS has no API here that
/// reports the safe area as a rectangle without a view, and returning a fabricated inset would be
/// worse than returning the full extent.
pub(crate) fn displays() -> Vec<Display> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let screens = UIScreen::screens(mtm);
    let main = UIScreen::mainScreen(mtm);
    let mut out = Vec::new();
    for (index, screen) in screens.iter().enumerate() {
        let bounds = screen.bounds();
        let scale = screen.scale() as f32;
        let rect = Rect::new(
            bounds.origin.x as i32,
            bounds.origin.y as i32,
            bounds.size.width.max(0.0) as u32,
            bounds.size.height.max(0.0) as u32,
        );
        out.push(Display {
            id: DisplayId::new(index as u64),
            bounds: rect,
            work_area: rect,
            scale: if scale > 0.0 { scale } else { 1.0 },
            is_primary: core::ptr::eq(
                objc2::rc::Retained::as_ptr(&screen),
                objc2::rc::Retained::as_ptr(&main),
            ),
        });
    }
    out
}

/// The iOS host clipboard.
///
/// # What it carries, and what it does not
///
/// Text and HTML, which is the same pair the macOS backend implements. Images, RTF and file
/// lists are refused (`false` / `None`) rather than silently dropped: an RGBA buffer would have
/// to become a `UIImage` through CoreGraphics, and that pipeline has no verifier in this crate's
/// gate suite — a body that returned `true` without it would be a claim nothing checks.
pub(crate) struct IosClipboard;

impl IosClipboard {
    /// Creates the backend. It holds no state: the pasteboard is the state.
    pub(crate) fn new() -> Self {
        Self
    }
}

impl RichClipboardBackend for IosClipboard {
    fn set_contents(&self, content: ClipboardContent) -> bool {
        match content {
            ClipboardContent::Text(text) => pasteboard_set_text(&text),
            ClipboardContent::Html { html, plain } => pasteboard_set_html(&html, &plain),
            ClipboardContent::Custom { content_type, data } => {
                pasteboard_set_custom(&content_type, &data)
            }
            other => {
                log::warn!(
                    "[ios clipboard] {} is not carried by this backend; the write is reported \
                     as refused rather than pretended",
                    other.content_type()
                );
                false
            }
        }
    }

    fn get_contents(&self) -> Option<ClipboardContent> {
        // A custom type is checked first: its payload is `NSData`, not text, so the text
        // accessors cannot see it, and returning `None` would hide the type the writer named.
        if let Some(content_type) = pasteboard_custom_type() {
            if let Some(data) = pasteboard_custom(&content_type) {
                return Some(ClipboardContent::Custom { content_type, data });
            }
        }
        if let Some(html) = pasteboard_html() {
            return Some(ClipboardContent::Html {
                html,
                plain: pasteboard_text().unwrap_or_default(),
            });
        }
        pasteboard_text().map(ClipboardContent::Text)
    }

    fn has_format(&self, content_type: &str) -> bool {
        pasteboard_has_format(content_type)
    }

    /// Stores bytes under an application-defined pasteboard type.
    fn set_contents_with_type(&self, content_type: &str, data: &[u8]) -> bool {
        pasteboard_set_custom(content_type, data)
    }

    /// Reads the bytes the pasteboard holds under `content_type`.
    fn get_contents_for_type(&self, content_type: &str) -> Option<Vec<u8>> {
        match content_type {
            "text/plain" => pasteboard_text().map(String::into_bytes),
            "text/html" => pasteboard_html().map(String::into_bytes),
            other => pasteboard_custom(other),
        }
    }
}
