//! Native UIKit FFI wrappers for the iOS backend (BLUE11 R2.4 100%).
//!
//! This module provides real UIKit UIView/UIButton/UILabel etc. creation
//! via the `objc2-ui-kit` crate, replacing the state-only backend.
//!
//! All functions are gated behind `#[cfg(target_os = "ios")]` and the
//! `ios-uikit-ffi` feature flag.

#![cfg(target_os = "ios")]
#![cfg(feature = "ios-uikit-ffi")]

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::{CGPoint, CGRect, CGSize, NSString};
use objc2_ui_kit::{UIButton, UIButtonType, UIColor, UILabel, UIView, UIViewController, UIWindow};

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Wrapper around `*mut c_void` that implements Send and Sync.
#[derive(Clone, Copy)]
struct NativePtr(*mut std::ffi::c_void);
unsafe impl Send for NativePtr {}
unsafe impl Sync for NativePtr {}

/// Thread-local storage for native widget handles.
static NATIVE_VIEWS: LazyLock<Mutex<HashMap<u64, NativePtr>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn store_native_view(widget_id: u64, view: *mut std::ffi::c_void) {
    NATIVE_VIEWS.lock().unwrap().insert(widget_id, NativePtr(view));
}

pub(crate) fn get_native_view(widget_id: u64) -> Option<*mut std::ffi::c_void> {
    NATIVE_VIEWS.lock().unwrap().get(&widget_id).map(|p| p.0)
}

pub(crate) fn remove_native_view(widget_id: u64) {
    NATIVE_VIEWS.lock().unwrap().remove(&widget_id);
}

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(x as f64, y as f64),
        CGSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

/// Create a native UIWindow.
pub(crate) fn create_ui_window(
    mtm: MainThreadMarker,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIWindow> {
    let frame = make_rect(0, 0, width, height);
    let window = unsafe { UIWindow::initWithFrame(mtm.alloc(), frame) };
    window.setBackgroundColor(UIColor::white());
    let vc = unsafe { UIViewController::initWithNibName_bundle(mtm.alloc(), None, None) };
    window.setRootViewController(Some(&vc));
    window.makeKeyAndVisible();
    // Set window title via accessibility label
    window.setAccessibilityLabel(&NSString::from_str(title));
    window
}

/// Create a native UIButton.
pub(crate) fn create_ui_button(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIButton> {
    let frame = make_rect(x, y, width, height);
    let button = unsafe { UIButton::initWithFrame(mtm.alloc(), frame) };
    button.setTitle(&NSString::from_str(text));
    button.setButtonType(UIButtonType::System);
    button
}

/// Create a native UILabel.
pub(crate) fn create_ui_label(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UILabel> {
    let frame = make_rect(x, y, width, height);
    let label = unsafe { UILabel::initWithFrame(mtm.alloc(), frame) };
    label.setText(&NSString::from_str(text));
    label
}
