//! Main-thread AppKit verification probe for the **objc2** backend (macOS only).
//!
//! Sibling of `apple_appkit_probe` (which covers the cocoa-legacy backend). The
//! two macOS backends use different Objective-C bindings, so each gets its own
//! probe rather than a fragile feature-switched compromise.
//!
//! ```text
//! cargo run --example apple_appkit_probe_objc2 --features "macos,serde,serde_json"
//! ```
//!
//! Runs on the AppKit main thread and asserts against **live** AppKit objects:
//! a real `NSWindow` registered in `NSApplication.windows`, a real `NSMenu`
//! installed as `mainMenu`, an `NSPasteboard` round-trip, real dialog objects,
//! and — critically — that geometry/visibility/text changes reach the native
//! object rather than only the backend state. That last class is what caught
//! BLUE14 F-5 (`setFrame:` sent to an `NSWindow`, which implements
//! `setFrame:display:` instead).
//!
//! Exit code 0 means every assertion held.

#[cfg(all(target_os = "macos", feature = "macos", not(feature = "mini")))]
fn main() {
    std::process::exit(run());
}

#[cfg(not(all(target_os = "macos", feature = "macos", not(feature = "mini"))))]
fn main() {
    println!("apple_appkit_probe_objc2: skipped (needs macOS, the `macos` feature, and no `mini` profile)");
}

#[cfg(all(target_os = "macos", feature = "macos", not(feature = "mini")))]
fn run() -> i32 {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSRect;
    use rust_widgets::platform::{get_platform, Platform};

    let mut failures: Vec<String> = Vec::new();
    let mut count: usize = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        count += 1;
        let status = if ok { "PASS" } else { "FAIL" };
        println!("[{status}] {name}: {detail}");
        if !ok {
            failures.push(name.to_string());
        }
    };

    let platform = get_platform();
    let backend = platform.backend_name();
    println!("backend = {backend}");

    // 1. Confirm the probe really runs on the AppKit main thread; otherwise the
    //    native path cannot be exercised and a pass would prove nothing.
    let on_main_thread: bool = unsafe {
        let cls = AnyClass::get(c"NSThread").expect("NSThread");
        msg_send![cls, isMainThread]
    };
    check("main_thread", on_main_thread, format!("+[NSThread isMainThread] = {on_main_thread}"));

    // 2. Native bootstrap.
    platform.init();

    // 3. The AppKit singleton must exist.
    let app: *mut AnyObject = unsafe {
        let cls = AnyClass::get(c"NSApplication").expect("NSApplication");
        msg_send![cls, sharedApplication]
    };
    check("ns_application", !app.is_null(), format!("NSApplication = {app:p}"));

    // 4. A window created on the main thread must be a *real* NSWindow: AppKit's
    //    own window list must contain it. The off-main path (state-only, no
    //    native object) would leave the list empty, so this doubles as the
    //    "not a fake fix" negative control.
    let window = Platform::create_window(platform, "objc2-probe", 40, 40, 480, 320);
    check("create_window_id", window != 0, format!("window id = {window}"));

    let windows: *mut AnyObject =
        if app.is_null() { std::ptr::null_mut() } else { unsafe { msg_send![app, windows] } };
    let window_count: usize =
        if windows.is_null() { 0 } else { unsafe { msg_send![windows, count] } };
    check(
        "native_window_registered",
        window_count >= 1,
        format!("NSApplication.windows.count = {window_count}"),
    );

    // 5. Real child controls.
    let button = Platform::create_button(platform, window, "Click", 20, 20, 120, 32);
    let label = Platform::create_label(platform, window, "Label", 20, 70, 200, 24);
    let edit = Platform::create_line_edit(platform, window, "edit", 20, 110, 200, 24);
    check(
        "native_children",
        button != 0 && label != 0 && edit != 0,
        format!("button={button} label={label} line_edit={edit}"),
    );

    // 6. Text round-trip (state) — and the native object must reflect it too.
    Platform::set_widget_text(platform, button, "Clicked");
    let button_text = Platform::get_widget_text(platform, button);
    check(
        "widget_text_roundtrip",
        button_text == "Clicked",
        format!("get_widget_text = {button_text:?}"),
    );

    // 7. Real NSMenu installed as the app main menu.
    let menu_bar = Platform::create_menu_bar(platform, window, 0, 0, 0, 0);
    let file_menu = Platform::create_menu(platform, menu_bar, "File", 0, 0, 0, 0);
    let quit_item = Platform::menu_add_item(platform, file_menu, "Quit", Some("q"));
    let attached = Platform::attach_menu_bar_to_window(platform, window, menu_bar);
    let main_menu: *mut AnyObject =
        if app.is_null() { std::ptr::null_mut() } else { unsafe { msg_send![app, mainMenu] } };
    check(
        "native_menu_bar",
        menu_bar != 0 && file_menu != 0 && quit_item != 0 && attached && !main_menu.is_null(),
        format!(
            "menu_bar={menu_bar} file_menu={file_menu} item={quit_item} attached={attached} mainMenu={main_menu:p}"
        ),
    );

    // 8. NSPasteboard round-trip (window-server singleton, main-thread-only).
    let clip_ok = Platform::set_clipboard_text(platform, "objc2-probe-clip");
    let clip_back = Platform::get_clipboard_text(platform);
    check(
        "native_clipboard",
        clip_ok && clip_back == "objc2-probe-clip",
        format!("set={clip_ok} get={clip_back:?}"),
    );

    // 9. Real dialog objects (no modal loop entered).
    let alert = Platform::create_message_box(platform, window, "Confirm", "Save?", 0, 0, 300, 120);
    let file_dlg = Platform::create_file_dialog(platform, window, 0, 0, 400, 300);
    let color_dlg = Platform::create_color_dialog(platform, window, 0, 0, 300, 200);
    let font_dlg = Platform::create_font_dialog(platform, window, 0, 0, 300, 200);
    check(
        "native_dialog_kinds",
        alert != 0 && file_dlg != 0 && color_dlg != 0 && font_dlg != 0,
        format!("alert={alert} file={file_dlg} color={color_dlg} font={font_dlg}"),
    );

    // 10. Geometry must reach the *native* NSWindow. An NSWindow does not
    //     implement `setFrame:` (it uses `setFrame:display:`), so this check is
    //     what exposed BLUE14 F-5.
    Platform::set_widget_geometry(platform, window, 60, 70, 400, 300);
    let window_frame_ok: bool = if windows.is_null() {
        false
    } else {
        let first: *mut AnyObject = unsafe { msg_send![windows, firstObject] };
        if first.is_null() {
            false
        } else {
            let frame: NSRect = unsafe { msg_send![first, frame] };
            (frame.size.width - 400.0).abs() < 0.5 && (frame.size.height - 300.0).abs() < 0.5
        }
    };
    check(
        "native_window_frame_applied",
        window_frame_ok,
        format!("NSWindow frame updated to 400x300 = {window_frame_ok}"),
    );

    // 11. Hide/show must order the native window out of / back into AppKit's
    //     window list (NSWindow uses orderOut:, not setHidden:).
    Platform::hide_widget(platform, window);
    let visible_after_hide: bool = if windows.is_null() {
        false
    } else {
        let first: *mut AnyObject = unsafe { msg_send![windows, firstObject] };
        !first.is_null() && unsafe { msg_send![first, isVisible] }
    };
    Platform::show_widget(platform, window);
    let visible_after_show: bool = if windows.is_null() {
        false
    } else {
        let first: *mut AnyObject = unsafe { msg_send![windows, firstObject] };
        !first.is_null() && unsafe { msg_send![first, isVisible] }
    };
    check(
        "native_window_visibility",
        !visible_after_hide && visible_after_show,
        format!("after_hide={visible_after_hide} after_show={visible_after_show}"),
    );

    // 12. Control-level geometry/visibility state round-trip.
    Platform::set_widget_geometry(platform, button, 30, 30, 140, 36);
    Platform::hide_widget(platform, button);
    let hidden = !Platform::is_widget_visible(platform, button);
    Platform::show_widget(platform, button);
    let shown = Platform::is_widget_visible(platform, button);
    check(
        "visibility_roundtrip",
        hidden && shown,
        format!("hidden_reported={hidden} shown_reported={shown}"),
    );

    if failures.is_empty() {
        println!("\nRESULT: PASS ({count} named checks, backend={backend})");
        0
    } else {
        println!("\nRESULT: FAIL — {}", failures.join(", "));
        1
    }
}
