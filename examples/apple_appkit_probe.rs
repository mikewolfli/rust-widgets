//! Main-thread AppKit verification probe (macOS only).
//!
//! Fixes `blue14.md` §二 A 类 #5 ("macOS AppKit 交互"): the previous rounds
//! declared this blocked by "no macOS runtime environment". This host *is*
//! macOS, so the interactive path can finally be executed for real.
//!
//! Run with one of:
//! ```text
//! cargo run --example apple_appkit_probe --features desktop   # cocoa-legacy backend
//! cargo run --example apple_appkit_probe --features macos     # objc2 backend
//! ```
//!
//! `main()` runs on the AppKit main thread, which is the only thread allowed to
//! create or mutate AppKit objects. The probe therefore exercises the *native*
//! path (not the state-only off-main fallback) and asserts against live
//! Objective-C objects found through the real `NSApplication` singleton.
//!
//! Exit code 0 means every assertion held; any failure prints the failing check
//! and exits non-zero.

#[cfg(target_os = "macos")]
fn main() {
    std::process::exit(run());
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("apple_appkit_probe: skipped (host is not macOS)");
}

#[cfg(target_os = "macos")]
fn run() -> i32 {
    use cocoa::appkit::NSApplication;
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    use rust_widgets::platform::{get_platform, Platform};

    let mut failures: Vec<String> = Vec::new();
    let mut check = |name: &str, ok: bool, detail: String| {
        let status = if ok { "PASS" } else { "FAIL" };
        println!("[{status}] {name}: {detail}");
        if !ok {
            failures.push(name.to_string());
        }
    };
    const TOTAL_CHECKS: usize = 7;

    let platform = get_platform();
    let backend = platform.backend_name();
    println!("backend = {backend}");

    // 1. Confirm we really are on the AppKit main thread. If this were false the
    //    run would only prove the state fallback, not the native path.
    // SAFETY: `+[NSThread isMainThread]` is a thread-safe class method.
    let on_main_thread: bool = unsafe { msg_send![class!(NSThread), isMainThread] };
    check("main_thread", on_main_thread, format!("+[NSThread isMainThread] = {on_main_thread}"));

    // 2. Native bootstrap (NSApplication sharedApplication + finishLaunching).
    platform.init();

    // SAFETY: `NSApp()`/`sharedApplication` is the documented AppKit singleton,
    // safe to message on the main thread after init().
    let app: id = unsafe { NSApplication::sharedApplication(nil) };
    check("ns_application", app != nil, format!("NSApplication::sharedApplication = {app:p}"));

    // 3. A window created on the main thread must be a *real* NSWindow, i.e.
    //    AppKit's own window list must contain it. This is the key distinction
    //    from the off-main state-only fallback (which registers ptr == 0).
    let window = Platform::create_window(platform, "appkit-probe", 40, 40, 480, 320);
    check("create_window_id", window != 0, format!("window id = {window}"));

    let windows: id = if app == nil {
        nil
    } else {
        // SAFETY: `-[NSApplication windows]` returns an NSArray of NSWindow.
        unsafe { msg_send![app, windows] }
    };
    let window_count: usize = if windows == nil {
        0
    } else {
        // SAFETY: NSArray responds to `-count` with an NSUInteger.
        unsafe { msg_send![windows, count] }
    };
    check(
        "native_window_registered",
        window_count >= 1,
        format!("NSApplication.windows.count = {window_count}"),
    );

    // 4. Real child controls on the main thread.
    let button = Platform::create_button(platform, window, "Click", 20, 20, 120, 32);
    let label = Platform::create_label(platform, window, "Label", 20, 70, 200, 24);
    let edit = Platform::create_line_edit(platform, window, "edit", 20, 110, 200, 24);
    check(
        "native_children",
        button != 0 && label != 0 && edit != 0,
        format!("button={button} label={label} line_edit={edit}"),
    );

    // 5. Round-trip text through the native NSButton/NSTextField state.
    Platform::set_widget_text(platform, button, "Clicked");
    let button_text = Platform::get_widget_text(platform, button);
    check(
        "widget_text_roundtrip",
        button_text == "Clicked",
        format!("get_widget_text = {button_text:?}"),
    );

    // 6. Native menu bar: NSMenu + NSMenuItem installed as the app main menu.
    //    `create_menu_bar` is parented to the window (the cocoa-legacy backend
    //    also accepts 0, the objc2 backend requires a window parent).
    let menu_bar = Platform::create_menu_bar(platform, window, 0, 0, 0, 0);
    let file_menu = Platform::create_menu(platform, menu_bar, "File", 0, 0, 0, 0);
    let quit_item = Platform::menu_add_item(platform, file_menu, "Quit", Some("q"));
    let attached = Platform::attach_menu_bar_to_window(platform, window, menu_bar);
    let main_menu: id = if app == nil {
        nil
    } else {
        // SAFETY: `-[NSApplication mainMenu]` returns the installed NSMenu.
        unsafe { msg_send![app, mainMenu] }
    };
    check(
        "native_menu_bar",
        menu_bar != 0 && file_menu != 0 && quit_item != 0 && attached && main_menu != nil,
        format!(
            "menu_bar={menu_bar} file_menu={file_menu} item={quit_item} attached={attached} mainMenu={main_menu:p}"
        ),
    );

    // 7. NSPasteboard round-trip (window-server singleton, main-thread-only).
    let clip_ok = Platform::set_clipboard_text(platform, "appkit-probe-clip");
    let clip_back = Platform::get_clipboard_text(platform);
    check(
        "native_clipboard",
        clip_ok && clip_back == "appkit-probe-clip",
        format!("set={clip_ok} get={clip_back:?}"),
    );

    // 8. Native dialogs: NSAlert / NSOpenPanel / NSColorPanel / NSFontPanel.
    //    These only construct objects; no modal loop is entered.
    let alert = Platform::create_message_box(platform, window, "Confirm", "Save?", 0, 0, 300, 120);
    let file_dlg = Platform::create_file_dialog(platform, window, 0, 0, 400, 300);
    let color_dlg = Platform::create_color_dialog(platform, window, 0, 0, 300, 200);
    let font_dlg = Platform::create_font_dialog(platform, window, 0, 0, 300, 200);
    let dialogs_ok = alert != 0 && file_dlg != 0 && color_dlg != 0 && font_dlg != 0;
    // Dialogs on the main thread go through `create_native_dialog`, which builds
    // real NSAlert/NSOpenPanel objects; report them via a soft check so the
    // process still exits non-zero when they regress.
    let _ = (alert, file_dlg, color_dlg, font_dlg);
    check(
        "native_dialog_kinds",
        dialogs_ok,
        format!("alert={alert} file={file_dlg} color={color_dlg} font={font_dlg}"),
    );

    // 9. Geometry / visibility round-trips must not disturb the live window.
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

    // Keep the process alive on the main thread briefly so AppKit can process
    // any pending window-server messages before teardown.
    std::thread::sleep(std::time::Duration::from_millis(50));

    if failures.is_empty() {
        println!("\nRESULT: PASS ({TOTAL_CHECKS} named checks, backend={backend})");
        0
    } else {
        println!("\nRESULT: FAIL — {}", failures.join(", "));
        1
    }
}
