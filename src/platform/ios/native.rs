//! Native UIKit FFI wrappers for the iOS backend (BLUE11 R2.4 100%).
//!
//! This module provides real UIKit UIView/UIButton/UILabel etc. creation
//! via the `objc2-ui-kit` crate, replacing the state-only backend.
//!
//! All functions are gated behind `#[cfg(target_os = "ios")]` and the
//! `ios-uikit-ffi` feature flag.

#![cfg(target_os = "ios")]
#![cfg(feature = "ios-uikit-ffi")]
// Preview/FFI helpers are wired from platform_impl.rs via conditional compilation.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObject};
use objc2::MainThreadMarker;
use objc2::{msg_send, DefinedClass};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::NSString;
use objc2_ui_kit::{
    NSObjectUIAccessibility, UIActivityIndicatorView, UIActivityIndicatorViewStyle, UIButton,
    UIColor, UIControlState, UILabel, UIPickerView, UIProgressView, UIScrollView, UISlider,
    UIStackView, UISwitch, UITableView, UITableViewStyle, UITextField, UIView, UIViewController,
    UIWindow,
};

// ─── Native view pointer storage ───

/// Wrapper around `*mut c_void` that implements Send and Sync.
#[derive(Clone, Copy)]
struct NativePtr(*mut std::ffi::c_void);
unsafe impl Send for NativePtr {}
unsafe impl Sync for NativePtr {}

/// Thread-safe storage for native widget handles.
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

// ─── Button target retention storage ───

/// Stores `ButtonTarget` instances so they are not deallocated.
/// iOS `UIControl.addTarget:action:forControlEvents:` does NOT retain
/// the target, so we must keep a reference alive.
static BUTTON_TARGETS: LazyLock<Mutex<HashMap<u64, Retained<ButtonTarget>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Remove and release the button target for the given widget.
pub(crate) fn remove_button_target(widget_id: u64) {
    BUTTON_TARGETS.lock().unwrap().remove(&widget_id);
}

// ─── Parent-child relationship tracking ───

/// Maps child widget IDs to their parent window IDs.
static PARENT_MAP: LazyLock<Mutex<HashMap<u64, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn set_parent(widget_id: u64, parent_id: u64) {
    PARENT_MAP.lock().unwrap().insert(widget_id, parent_id);
}

#[allow(dead_code)]
pub(crate) fn get_parent(widget_id: u64) -> Option<u64> {
    PARENT_MAP.lock().unwrap().get(&widget_id).copied()
}

// ─── Native state setters (forward Rust state to UIKit views) ───

/// Apply a frame rect to a stored native UIView.
pub(crate) fn set_native_frame(widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    let frame = make_rect(x, y, width, height);
    unsafe {
        let view: *mut AnyObject = ptr as *mut AnyObject;
        let _: () = msg_send![view, setFrame: frame];
    }
}

/// Show/hide a stored native UIView.
pub(crate) fn set_native_hidden(widget_id: u64, hidden: bool) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    unsafe {
        let view: *mut AnyObject = ptr as *mut AnyObject;
        let _: () = msg_send![view, setHidden: hidden];
    }
}

/// Enable/disable a stored native UIControl when the receiver supports it.
pub(crate) fn set_native_enabled(widget_id: u64, enabled: bool) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    unsafe {
        let view: *mut AnyObject = ptr as *mut AnyObject;
        let selector = sel!(setEnabled:);
        let responds: bool = msg_send![view, respondsToSelector: selector];
        if responds {
            let _: () = msg_send![view, setEnabled: enabled];
        }
    }
}

/// Update a stored native view's text through the UIKit text/title setters.
pub(crate) fn set_native_text(widget_id: u64, text: &str) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    let value = NSString::from_str(text);
    unsafe {
        let view: *mut AnyObject = ptr as *mut AnyObject;
        for selector in [sel!(setText:), sel!(setTitle:forState:), sel!(setAccessibilityLabel:)] {
            let responds: bool = msg_send![view, respondsToSelector: selector];
            if responds {
                let _: () = msg_send![view, performSelector: selector, withObject: &*value];
                return;
            }
        }
    }
}

// ─── View hierarchy management ───

/// Add a widget's native view as a subview of its parent window.
///
/// Looks up the parent window from `NATIVE_VIEWS` using the parent ID,
/// then calls `addSubview:` with the child widget's native view on the
/// window's root view controller's view.
pub(crate) fn add_as_subview(widget_id: u64, parent_id: u64) {
    let views = NATIVE_VIEWS.lock().unwrap();
    let Some(parent_ptr) = views.get(&parent_id).map(|p| p.0) else {
        return;
    };
    let Some(child_ptr) = views.get(&widget_id).map(|c| c.0) else {
        return;
    };
    drop(views);

    unsafe {
        // Get the root view controller's view from the parent window.
        // UIWindow::rootViewController returns a UIViewController whose `view`
        // property is the content view where subviews should be added.
        let parent: *mut AnyObject = parent_ptr as *mut AnyObject;
        let root_vc: *mut AnyObject = msg_send![parent, rootViewController];
        if root_vc.is_null() {
            return;
        }
        let content_view: *mut AnyObject = msg_send![root_vc, view];
        if content_view.is_null() {
            return;
        }
        // Add the child view as a subview of the content view.
        let child: *mut AnyObject = child_ptr as *mut AnyObject;
        let _: () = msg_send![content_view, addSubview: child];
    }
}

// ─── Button action/target event queue ───

use std::collections::VecDeque;

/// Global queue of button tap events (widget IDs that were tapped).
static BUTTON_EVENT_QUEUE: LazyLock<Mutex<VecDeque<u64>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

/// Drain all pending button tap events from the queue.
pub(crate) fn drain_button_events() -> Vec<u64> {
    let mut queue = BUTTON_EVENT_QUEUE.lock().unwrap();
    queue.drain(..).collect()
}

// ─── Objective-C button target class ───

use objc2::define_class;
use objc2::sel;

// A lightweight Objective-C helper class that holds a widget ID and forwards
// `buttonTapped:` messages to the Rust event queue.
define_class!(
    #[unsafe(super(NSObject))]
    #[ivars = u64]
    struct ButtonTarget;

    impl ButtonTarget {
        #[unsafe(method(buttonTapped:))]
        fn handle_tap(&self, _sender: &NSObject) {
            BUTTON_EVENT_QUEUE.lock().unwrap().push_back(*self.ivars());
        }
    }
);

impl ButtonTarget {
    /// Create a new `ButtonTarget` with the given widget ID.
    fn new(mtm: MainThreadMarker, widget_id: u64) -> Retained<Self> {
        let obj: Allocated<Self> = mtm.alloc();
        let obj = obj.set_ivars(widget_id);
        unsafe { msg_send![super(obj), init] }
    }
}

/// Wire a UIButton's touch-up-inside event to fire our Rust callback.
///
/// Creates a `ButtonTarget` instance holding the widget ID, sets it as
/// the button's target, and connects `buttonTapped:` as the action
/// selector for `UIControlEventTouchUpInside` (value 64).
pub(crate) fn wire_button_action(widget_id: u64) {
    let Some(ptr) = get_native_view(widget_id) else {
        return;
    };
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    let target = ButtonTarget::new(mtm, widget_id);
    let action_sel = sel!(buttonTapped:);

    // SAFETY: `ptr` is a valid UIButton (stored via Retained::into_raw).
    // `target` is a valid NSObject subclass with the `buttonTapped:` method.
    // `UIControlEventTouchUpInside = 1 << 6 = 64`.
    unsafe {
        let button: *mut AnyObject = ptr as *mut AnyObject;
        let _: () =
            msg_send![button, addTarget: &*target, action: action_sel, forControlEvents: 64u64];
    }

    // Store the target so it stays alive for the button's lifetime.
    // iOS `addTarget:action:forControlEvents:` does NOT retain the
    // target, so we must keep a reference alive in our own storage.
    // The target is released when `remove_button_target` is called
    // (which should happen when the widget is destroyed).
    BUTTON_TARGETS.lock().unwrap().insert(widget_id, target);
}

// ─── Geometry helpers ───

fn make_rect(x: i32, y: i32, width: u32, height: u32) -> CGRect {
    CGRect::new(
        CGPoint::new(x as f64, y as f64),
        CGSize::new(width.max(1) as f64, height.max(1) as f64),
    )
}

// ─── Widget creation functions ───

/// Create a native UIWindow.
pub(crate) fn create_ui_window(
    mtm: MainThreadMarker,
    title: &str,
    _x: i32,
    _y: i32,
    width: u32,
    height: u32,
) -> Retained<UIWindow> {
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
    // Set window title via accessibility label
    window.setAccessibilityLabel(Some(&NSString::from_str(title)), mtm);
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
    // SAFETY: UIButton::initWithFrame on main thread (mtm guard).
    // objc2 init methods return Retained<T> which ensures the object is valid.
    // No additional error checking is needed since objc2 handles memory management.
    let button = UIButton::initWithFrame(mtm.alloc(), frame);
    button.setTitle_forState(Some(&NSString::from_str(text)), UIControlState::Normal);
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
    // SAFETY: UILabel::initWithFrame on main thread (mtm).
    // Retained<UILabel> is guaranteed valid by objc2.
    let label = UILabel::initWithFrame(mtm.alloc(), frame);
    label.setText(Some(&NSString::from_str(text)));
    label
}

/// Create a native UISwitch (CheckBox equivalent on iOS).
pub(crate) fn create_ui_checkbox(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UISwitch> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UISwitch::initWithFrame on main thread (mtm).
    // objc2 Retained<UISwitch> is always valid after init.
    let switch_ctl = UISwitch::initWithFrame(mtm.alloc(), frame);
    switch_ctl.setAccessibilityLabel(Some(&NSString::from_str(text)), mtm);
    switch_ctl
}

/// Create a native UITextField (LineEdit equivalent on iOS).
pub(crate) fn create_ui_line_edit(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UITextField> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UITextField::initWithFrame on main thread (mtm).
    // objc2 init returns valid Retained<UITextField>.
    let text_field = UITextField::initWithFrame(mtm.alloc(), frame);
    text_field.setText(Some(&NSString::from_str(text)));
    text_field
}

/// Create a native UIButton configured as a radio button.
pub(crate) fn create_ui_radio_button(
    mtm: MainThreadMarker,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIButton> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UIButton::initWithFrame on main thread (mtm).
    // objc2 guarantees valid Retained<UIButton>.
    let button = UIButton::initWithFrame(mtm.alloc(), frame);
    button.setTitle_forState(Some(&NSString::from_str(text)), UIControlState::Normal);
    button
}

/// Create a native UISlider.
pub(crate) fn create_ui_slider(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UISlider> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UISlider::initWithFrame on main thread (mtm).
    // Retained<UISlider> is always valid.
    let slider = UISlider::initWithFrame(mtm.alloc(), frame);
    slider.setMinimumValue(0.0);
    slider.setMaximumValue(100.0);
    slider
}

/// Create a native UIProgressView.
pub(crate) fn create_ui_progress_bar(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIProgressView> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UIProgressView::initWithFrame on main thread (mtm).
    // objc2 Retained<UIProgressView> is guaranteed valid.
    let progress = UIProgressView::initWithFrame(mtm.alloc(), frame);
    progress
}

/// Create a native UIPickerView (ComboBox equivalent on iOS).
pub(crate) fn create_ui_combo_box(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIPickerView> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UIPickerView::initWithFrame on main thread (mtm).
    // objc2 Retained<UIPickerView> is always valid after init.
    let picker = UIPickerView::initWithFrame(mtm.alloc(), frame);
    picker
}

/// Create a native UITableView (ListBox equivalent on iOS).
pub(crate) fn create_ui_list_box(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UITableView> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UITableView::initWithFrame_style on main thread (mtm).
    // objc2 init reliably returns a valid Retained<UITableView>.
    let table = UITableView::initWithFrame_style(mtm.alloc(), frame, UITableViewStyle::Plain);
    table
}

/// Create a native UIView (generic panel container).
pub(crate) fn create_ui_panel(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIView> {
    let frame = make_rect(x, y, width, height);
    // SAFETY: UIView::initWithFrame on main thread (mtm).
    // Retained<UIView> is guaranteed valid by objc2.
    let panel = UIView::initWithFrame(mtm.alloc(), frame);
    let bg = UIColor::clearColor();
    panel.setBackgroundColor(Some(&bg));
    panel
}

/// Create a native UIScrollView.
pub(crate) fn create_ui_scroll(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIScrollView> {
    let frame = make_rect(x, y, width, height);
    let scroll = UIScrollView::initWithFrame(mtm.alloc(), frame);
    scroll.setScrollEnabled(true);
    scroll.setContentSize(frame.size);
    scroll
}

/// Create a native UIAlertController (message box equivalent on iOS).
///
/// Returns a prepared alert with a single "OK" action.
pub(crate) fn create_ui_alert(
    _mtm: MainThreadMarker,
    title: &str,
    text: &str,
) -> Retained<AnyObject> {
    // SAFETY: UIAlertController and UIAlertAction are created via msg_send!.
    // `alertControllerWithTitle:message:preferredStyle:` returns a retained
    // UIAlertController. `UIAlertControllerStyleAlert` = 1.
    // `actionWithTitle:style:handler:` returns a retained UIAlertAction.
    // `UIAlertActionStyleDefault` = 0. The nil handler is safe.
    unsafe {
        let cls = AnyClass::get(c"UIAlertController").unwrap();
        let title_str = NSString::from_str(title);
        let text_str = NSString::from_str(text);
        let alert: Retained<AnyObject> = msg_send![cls, alertControllerWithTitle: &*title_str, message: &*text_str, preferredStyle: 1u64];

        let action_cls = AnyClass::get(c"UIAlertAction").unwrap();
        let ok_str = NSString::from_str("OK");
        let action: Retained<AnyObject> = msg_send![action_cls, actionWithTitle: &*ok_str, style: 0u64, handler: 0u64 as *mut AnyObject];
        let _: () = msg_send![&*alert, addAction: &*action];

        alert
    }
}

/// Create a native UIStackView (stack widget equivalent on iOS).
pub(crate) fn create_ui_stack(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIStackView> {
    let frame = make_rect(x, y, width, height);
    let stack = UIStackView::initWithFrame(mtm.alloc(), frame);
    stack.setSpacing(8.0);
    stack
}

/// Create a native UIActivityIndicatorView (spinner equivalent on iOS).
pub(crate) fn create_ui_spinner(
    mtm: MainThreadMarker,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Retained<UIActivityIndicatorView> {
    let frame = make_rect(x, y, width, height);
    let spinner = UIActivityIndicatorView::initWithActivityIndicatorStyle(
        mtm.alloc(),
        UIActivityIndicatorViewStyle::Medium,
    );
    spinner.setFrame(frame);
    spinner.startAnimating();
    spinner
}
