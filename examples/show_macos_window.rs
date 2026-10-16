// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Opens a real window on the running desktop so the backend can be seen, then closes itself.
//!
//! Exists to answer "show me one" for the macOS round (`docs/log/log-20261016-3.md`): the window,
//! its library-painted controls and the display facts the round added are all readable on the
//! screen. The bounded session is the §十.2 fix in action — a `schedule_on_main` callback ends the
//! loop at its deadline instead of leaving a window nobody can close (the cocoa backend has no
//! `windowWillClose:` that stops the loop).
//!
//! Run:
//!     cargo run --no-default-features --features desktop --example show_macos_window

#[cfg(not(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
)))]
fn main() {
    println!("show_macos_window: this profile has no platform window backend.");
}

#[cfg(all(
    not(alloc_frugal),
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
fn main() {
    use std::time::Duration;

    /// How long the window stays up before it ends its own session.
    const LIFETIME: Duration = Duration::from_secs(600);

    println!("backend: {}", rust_widgets::platform::backend_name());
    rust_widgets::init();

    let displays = rust_widgets::platform::displays();
    let display_line = match displays.first() {
        Some(d) => format!(
            "displays(): {} x {} @ {}x   (work area {} x {})   count={}",
            d.bounds.width,
            d.bounds.height,
            d.scale,
            d.work_area.width,
            d.work_area.height,
            displays.len()
        ),
        None => "displays(): (this backend reports none)".to_string(),
    };

    let window = rust_widgets::create_window("rust-widgets on macOS", 140, 140, 760, 420);
    if window == 0 {
        println!("the backend created no window here");
        std::process::exit(2);
    }

    rust_widgets::create_label(window, "rust-widgets — live window", 28, 24, 640, 34);
    rust_widgets::create_label(window, &display_line, 28, 66, 680, 26);
    rust_widgets::create_label(
        window,
        "§十二 display enumeration is live (bounds / work area / scale read from NSScreen)",
        28,
        98,
        700,
        24,
    );
    rust_widgets::create_label(
        window,
        "§十.2: this session is bounded and ends itself in 180 s",
        28,
        126,
        700,
        24,
    );
    rust_widgets::create_button(window, "Click me", 28, 168, 180, 40);
    rust_widgets::create_button(window, "Second button", 224, 168, 200, 40);
    rust_widgets::create_label(
        window,
        "§十一: a custom MIME type is on the clipboard right now — paste into another app to see 'text/plain'",
        28,
        228,
        700,
        24,
    );
    rust_widgets::create_label(
        window,
        "closing this window does not stop the loop on this backend; it self-closes at 180 s",
        28,
        258,
        700,
        24,
    );

    rust_widgets::show_widget(window);
    println!("window shown: {window}");

    // In-process evidence that the window's tree has ink to paint: the canvas blits exactly this
    // frame out of `render_frame_tree`, so one colour here would mean a blank client area.
    match rust_widgets::widget::runtime::render_frame_tree(
        window,
        rust_widgets::core::Size::new(760, 420),
        rust_widgets::core::Color::WHITE,
    ) {
        Some(frame) => {
            let mut colors = std::collections::HashSet::new();
            let mut non_white = 0usize;
            for px in frame.chunks(4) {
                if px.len() < 3 {
                    continue;
                }
                if !(px[0] == 255 && px[1] == 255 && px[2] == 255) {
                    non_white += 1;
                }
                colors.insert((px[0], px[1], px[2]));
            }
            println!(
                "window tree frame: {} bytes, non-white pixels={} distinct_rgb={}",
                frame.len(),
                non_white,
                colors.len()
            );
        }
        None => println!("window tree frame: none (nothing to paint)"),
    }

    // Leave something pasteable behind so the clipboard round can be seen from outside too.
    let _ = rust_widgets::set_clipboard_text("rust-widgets clipboard check (plain text)");
    if let Some(clip) = rust_widgets::platform_clipboard() {
        let _ = clip.set_contents_with_type(
            "application/vnd.rustage.nodes+json",
            br#"{"vendor":"rustage","kind":"nodes"}"#,
        );
    }

    let scheduled = rust_widgets::schedule_on_main(LIFETIME, || {
        let _ = rust_widgets::quit();
    });
    println!("bounded session scheduled: {scheduled} (lifetime {LIFETIME:?})");

    rust_widgets::run();
    println!("event loop exited cleanly");
    std::process::exit(0);
}
