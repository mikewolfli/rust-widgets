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
//! All functions are gated behind `#[cfg(target_os = "ios")]` and the
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

#![cfg(target_os = "ios")]
#![cfg(feature = "ios-uikit-ffi")]

use crate::compat::String;
use crate::platform::clipboard::{ClipboardContent, RichClipboardBackend};
use objc2::MainThreadMarker;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSData, NSString};
use objc2_ui_kit::{NSObjectUIAccessibility, UIColor, UIPasteboard, UIViewController, UIWindow};

/// The UIKit pasteboard type (UTI) for HTML.
const HTML_TYPE: &str = "public.html";

// ─── Geometry helpers ───

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(x as f64, y as f64),
        CGSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

// ─── Window creation ───

/// Create the native `UIWindow` the library paints into.
pub(crate) fn create_ui_window(
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
    window
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
        // Every other type this crate can name is either text or something this FFI does not
        // carry: `public.utf8-plain-text` is what `setString:` writes.
        "text/plain" => pasteboard_text().is_some(),
        _ => false,
    }
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
}
