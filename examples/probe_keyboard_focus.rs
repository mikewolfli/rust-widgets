// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages a real pointer press and real X keystrokes against a `LineEdit`.
//!
//! # What this answers
//!
//! The designer feedback (`docs/issues/feedback-20261015.md` §十七) recorded two facts that a
//! headless test cannot distinguish:
//!
//! 1. clicking a `LineEdit` did **not** move the library's focus, because the pointer router never
//!    called `focus_on_pointer_press`; and
//! 2. the GTK `DrawingArea` that paints a window's tree was never given GTK focus, so the toolkit
//!    delivered keys to the toplevel instead of to the area that forwards them.
//!
//! This probe clicks the field through XTEST, types `abc`, and then reads the field's value **and**
//! its `on_value_changed` notifications. The notification half is the other gap the feedback
//! recorded: the text could change while the host was forced to poll, because the widget layer
//! produced no `ValueChanged` trigger.
//!
//! Run with:
//!     RW_GUI_PROBE_OK=1 cargo run --no-default-features --features "desktop,gtk-native" \
//!         --example probe_keyboard_focus
//!
//! Exit codes: `0` = every check held, `1` = a check failed, `2` = this host cannot stage it (no
//! display, or one of `python3`/`wmctrl`/`xwininfo`/`xprop` missing — the line says which).

#[cfg(all(not(alloc_frugal), target_os = "linux", feature = "gtk-native"))]
fn main() {
    staged()
}

#[cfg(not(all(not(alloc_frugal), target_os = "linux", feature = "gtk-native")))]
fn main() {
    println!(
        "probe_keyboard_focus: this build has no X11 session to ask (`--features \
         desktop,gtk-native` on a Linux host with DISPLAY set stages it)."
    );
}

/// Whether the operator has allowed this probe to open real windows and inject input.
#[cfg(all(not(alloc_frugal), target_os = "linux", feature = "gtk-native"))]
fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe opens a real window and injects pointer/key events on the desktop it runs on; \
         set RW_GUI_PROBE_OK=1 to stage it (and prefer a machine whose screen nobody is using)"
    );
    false
}

#[cfg(all(not(alloc_frugal), target_os = "linux", feature = "gtk-native"))]
fn staged() {
    use rust_widgets::app::WidgetHandle;
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    if !gui_probe_allowed() {
        std::process::exit(2);
    }

    // A per-run title: the injector finds the window by title substring, and a title reused across
    // runs could match a window the previous run left on screen (or receive its late key events).
    let title = format!("rw keyboard focus probe {} (throwaway)", std::process::id());
    // The typed text and the value it must produce. They are the same for ordinary typing; an IME
    // run passes pinyin plus a selecting space and expects the committed characters, which is what
    // distinguishes "the context composed" from "the field received ASCII".
    let typed = std::env::var("RW_PROBE_TEXT").unwrap_or_else(|_| "abc".to_string());
    let expected = std::env::var("RW_PROBE_EXPECT").unwrap_or_else(|_| typed.clone());
    /// The field's box inside the window. The click lands at its centre.
    const FIELD: (i32, i32, u32, u32) = (32, 40, 300, 36);
    /// How long the window stays up before the injection, so it is mapped and focused.
    const SETTLE: Duration = Duration::from_millis(600);

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== keyboard focus probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("runtime gui mode   : {:?}", rust_widgets::platform::runtime_gui_mode());
    println!("DISPLAY            : {}", std::env::var("DISPLAY").unwrap_or_default());

    // A host limitation, detected before anything is staged: a missing tool is reported as such
    // instead of as a failed check about the library.
    for tool in ["python3", "wmctrl", "xwininfo", "xprop"] {
        if Command::new(tool)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_err()
            && !which(tool)
        {
            println!("unsupported host: `{tool}` is not installed, so input cannot be staged");
            std::process::exit(2);
        }
    }

    rust_widgets::init();
    let window = rust_widgets::create_window(&title, 140, 140, 520, 260);
    check(window != 0, "the library created a window");

    let handle = rust_widgets::app::WindowHandle::from_raw(window);
    let field = handle.new_line_edit("", FIELD.0, FIELD.1, FIELD.2, FIELD.3);
    // A button exists so the probe can prove that focus moved to the field and not to "whatever
    // was clicked": its click count must stay zero.
    let button = handle.new_button("unclicked", 32, 140, 160, 36);

    let values: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&values);
    field.on_value_changed(move |text| {
        if let Ok(mut values) = seen.lock() {
            values.push(text);
        }
    });
    let clicks = Arc::new(AtomicUsize::new(0));
    let clicked = Arc::clone(&clicks);
    button.on_click(move || {
        clicked.fetch_add(1, Ordering::SeqCst);
    });

    rust_widgets::show_widget(window);

    let (report_tx, report_rx) = std::sync::mpsc::channel::<Result<String, String>>();
    let watcher_title = title.clone();
    let watcher = std::thread::spawn(move || {
        std::thread::sleep(SETTLE);
        let helper =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/x11_inject_keys.py");
        let click_x = FIELD.0 + FIELD.2 as i32 / 2;
        let click_y = FIELD.1 + FIELD.3 as i32 / 2;
        let ime_activate = std::env::var_os("RW_PROBE_IME").is_some();
        let output = Command::new("python3")
            .arg(&helper)
            .arg(&watcher_title)
            .arg(&typed)
            .args([
                "--activate",
                "--plain",
                "--click-relative",
                &click_x.to_string(),
                &click_y.to_string(),
            ])
            .args(if ime_activate { vec!["--ime-activate"] } else { Vec::new() })
            .output();
        let result = match output {
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            }
            Ok(output) => Err(format!(
                "injector exit {:?}: {}{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )),
            Err(error) => Err(format!("could not run the injector: {error}")),
        };
        // Give the loop a frame to drain the value-changed trigger before ending the session.
        std::thread::sleep(Duration::from_millis(400));
        let _ = report_tx.send(result);
        let _ = rust_widgets::quit();
    });

    rust_widgets::run();
    let injection =
        report_rx.recv_timeout(Duration::from_secs(30)).expect("the watcher must report");
    let _ = watcher.join();

    match &injection {
        Ok(line) => println!("   injector            : {}", line.trim()),
        Err(error) => println!("   injector            : {error}"),
    }

    let text = rust_widgets::get_widget_text(field.raw_id());
    let values = values.lock().map(|values| values.clone()).unwrap_or_default();
    let point =
        rust_widgets::core::Point::new(FIELD.0 + FIELD.2 as i32 / 2, FIELD.1 + FIELD.3 as i32 / 2);
    println!("   field id            : {}", field.raw_id());
    println!(
        "   field geometry      : {:?}",
        rust_widgets::widget::runtime::geometry_of(field.raw_id())
    );
    println!(
        "   focusable           : {}",
        rust_widgets::widget::runtime::widget_wants_keyboard_focus(field.raw_id())
    );
    println!(
        "   accepts IME         : {}",
        rust_widgets::widget::runtime::accepts_ime_input(field.raw_id())
    );
    println!("   focused widget      : {:?}", rust_widgets::widget::runtime::focused_widget());
    println!(
        "   widget under click  : {:?}",
        rust_widgets::widget::runtime::widget_at(window, point)
    );
    println!("   field text          : {text:?}");
    println!("   value notifications : {values:?}");
    println!("   button clicks       : {}", clicks.load(Ordering::SeqCst));

    check(injection.is_ok(), "real X key events were injected into the window");
    check(text == expected, "the clicked field received the typed characters exactly once");
    check(
        values.last().map(String::as_str) == Some(expected.as_str()),
        "`on_value_changed` reported the final value without the host polling",
    );
    check(!values.is_empty(), "an edit produces at least one value notification");
    check(
        clicks.load(Ordering::SeqCst) == 0,
        "the button was not clicked, so focus moved to the field the pointer hit",
    );

    println!(
        "\n{}",
        if failures == 0 {
            "probe_keyboard_focus: OK (every check held)".to_string()
        } else {
            format!("probe_keyboard_focus: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

/// Whether `tool` resolves on `PATH` (used to turn a failed `--help` into a host limitation).
#[cfg(all(not(alloc_frugal), target_os = "linux", feature = "gtk-native"))]
fn which(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|directory| directory.join(tool).is_file()))
        .unwrap_or(false)
}
