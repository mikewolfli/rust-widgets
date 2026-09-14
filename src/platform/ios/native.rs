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
//! that builds its frame.

#![cfg(target_os = "ios")]
#![cfg(feature = "ios-uikit-ffi")]

use objc2::MainThreadMarker;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_ui_kit::{NSObjectUIAccessibility, UIColor, UIViewController, UIWindow};

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
