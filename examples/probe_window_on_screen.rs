// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Asks the X server what is *on screen*, instead of asking the library what it created.
//!
//! # What this answers
//!
//! "A window exists" and "a window is on screen with our pixels in it" are different claims, and
//! the designer feedback listed the second one as unanswerable without a display session
//! ("窗口是否被合成、是否出现在屏幕上"). GDK's `is_mapped` is the toolkit's *bookkeeping*; this
//! probe asks the X server, through tools a person would use:
//!
//! * `wmctrl -l` — the window manager's list: does a window with our title exist at all?
//! * `xwininfo -id <id>` — the server's report for that window, including `Map State: IsViewable`
//!   (mapped **and** every ancestor mapped **and** visible on screen).
//! * `xwd -id <id>` — the window's pixels **as the server stores them**. The probe parses the XWD
//!   dump itself and reports the captured size and how many distinct colours it holds, so "the
//!   image is a real frame of our UI" is a number rather than an impression.
//!
//! The capture is taken while the library owns the window and has painted a control into it, from a
//! watcher thread (so it does not disturb the loop), and the session is then ended with
//! `quit()` from that thread — the same shape as `probe_cross_thread_quit`.
//!
//! # What it still does not answer
//!
//! It does not read a *screen* capture of the framebuffer region (that would need a compositing
//! screenshot tool), so it proves the server holds a viewable window of our size with our pixels —
//! not that a compositor placed it at the coordinates a person sees. And it is a machine judging
//! pixels, not a person judging layout.
//!
//! Run with:
//!     cargo run --no-default-features --features "desktop,gtk-native" --example probe_window_on_screen
//!
//! Exit codes: `0` = every check held, `1` = a check failed, `2` = this host cannot stage it
//! (no display, or one of `wmctrl`/`xwininfo`/`xwd` missing — the line says which).

#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn main() {
    staged()
}

#[cfg(not(all(not(alloc_frugal), unix_desktop, feature = "gtk-native")))]
fn main() {
    println!(
        "probe_window_on_screen: this build has no X11 session to ask (`--features \
         desktop,gtk-native` on a Linux host with DISPLAY set stages it)."
    );
}

/// Whether the operator has allowed this probe to open real windows.
///
/// # Why the probes ask first
///
/// Staging these checks means **opening real windows and dialogs on the session the probe runs
/// in** — that is the whole point of them (a headless stub cannot answer "is it on screen" or "did
/// a keystroke reach the control"), and it is also why they must not be run casually: on a
/// developer's live desktop each run pops a window over their work, and a retry loop pops several.
/// Measured cost of learning this the hard way: one round of iteration opened a dozen dialogs on
/// the maintainer's screen.
///
/// So the default is to refuse and say how to allow it, and the answer is a deliberate act rather
/// than a side effect of running `cargo run --example`.
#[cfg(all(unix_desktop, feature = "gtk-native"))]
fn gui_probe_allowed() -> bool {
    if std::env::var("RW_GUI_PROBE_OK").ok().as_deref() == Some("1") {
        return true;
    }
    println!(
        "this probe opens real windows on the desktop it runs on; set RW_GUI_PROBE_OK=1 to stage \
         it (and prefer a machine whose screen nobody is using)"
    );
    false
}

#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn staged() {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    if !gui_probe_allowed() {
        std::process::exit(2);
    }

    /// The window the probe asks about. Unique so the window-manager lookup cannot match another
    /// application's window.
    const TITLE: &str = "rw on-screen probe (throwaway)";
    /// How long the session stays up before the capture, so the window is mapped by then.
    const CAPTURE_AFTER: Duration = Duration::from_millis(700);
    /// The window's size, which the X server must report back exactly.
    const SIZE: (u32, u32) = (480, 320);

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== window on screen probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("runtime gui mode   : {:?}", rust_widgets::platform::runtime_gui_mode());
    println!("DISPLAY            : {}", std::env::var("DISPLAY").unwrap_or_default());

    // A host limitation, detected before anything is staged (principle #103): a missing tool is
    // reported as such instead of as a failed check about the library.
    for tool in ["wmctrl", "xwininfo", "xwd"] {
        if Command::new(tool)
            .arg("-help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
            && which_missing(tool)
        {
            println!(
                "unsupported host: `{tool}` is not installed, so the X server cannot be asked"
            );
            std::process::exit(2);
        }
    }

    rust_widgets::init();
    let window = rust_widgets::create_window(TITLE, 60, 60, SIZE.0, SIZE.1);
    check(window != 0, "the library created a window");
    // A library-painted control, so the captured pixels hold chrome and text rather than a bare
    // background: a uniform dump would be indistinguishable from an empty window.
    let button = rust_widgets::create_button(window, "on screen", 24, 24, 160, 36);
    check(button != 0, "a library-painted control was created inside it");
    rust_widgets::show_widget(window);

    let (report_tx, report_rx) = std::sync::mpsc::channel::<ScreenReport>();
    let watcher = std::thread::spawn(move || {
        std::thread::sleep(CAPTURE_AFTER);
        let report = ask_the_x_server(TITLE, SIZE);
        let _ = report_tx.send(report);
        // End the session from this thread, exactly as a smoke test would.
        let _ = rust_widgets::quit();
    });

    let started = Instant::now();
    rust_widgets::run();
    let run_took = started.elapsed();
    let report = report_rx.recv_timeout(Duration::from_secs(30)).expect("the watcher must report");
    let _ = watcher.join();
    println!("   run() returned after {run_took:?}");

    println!("\nwhat the X server said:");
    println!("   window manager lists it : {}", report.listed);
    println!("   server map state        : {:?}", report.map_state);
    println!(
        "   server geometry         : {}x{} (library asked for {}x{})",
        report.width, report.height, SIZE.0, SIZE.1
    );
    match &report.capture {
        Some(capture) => println!(
            "   xwd capture             : {}x{} at {} bpp, {} distinct colours",
            capture.width, capture.height, capture.bits_per_pixel, capture.distinct_colours
        ),
        None => println!(
            "   xwd capture             : none ({})",
            report.capture_error.clone().unwrap_or_default()
        ),
    }

    check(report.listed, "the window manager lists a window with the library's title");
    check(
        report.map_state.as_deref() == Some("IsViewable"),
        "the server reports the window as viewable (mapped, ancestors mapped, on screen)",
    );
    check(
        report.width == SIZE.0 && report.height == SIZE.1,
        "the server's geometry for the window is the size the library asked for",
    );
    match &report.capture {
        Some(capture) => {
            check(
                capture.width == SIZE.0 && capture.height == SIZE.1,
                "the captured image is the window's size, so this is a frame of it and not of something else",
            );
            check(
                capture.distinct_colours > 2,
                "the captured pixels hold more than a flat fill, i.e. the library's control was painted into them",
            );
        }
        None => check(false, "the window's pixels could be read back with xwd"),
    }

    println!(
        "\n{}",
        if failures == 0 {
            "probe_window_on_screen: OK (every check held)".to_string()
        } else {
            format!("probe_window_on_screen: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

/// What the X server reported for the probe's window.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
struct ScreenReport {
    /// Whether `wmctrl -l` listed a window with the probe's title.
    listed: bool,
    /// `xwininfo`'s `Map State` field, e.g. `IsViewable`.
    map_state: Option<String>,
    /// The width `xwininfo` reported, in pixels.
    width: u32,
    /// The height `xwininfo` reported, in pixels.
    height: u32,
    /// The parsed `xwd` dump, when the capture succeeded.
    capture: Option<XwdCapture>,
    /// Why the capture did not happen, when it did not.
    capture_error: Option<String>,
}

/// The parts of an `xwd` dump this probe reads.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
struct XwdCapture {
    /// Captured width, from the XWD header.
    width: u32,
    /// Captured height, from the XWD header.
    height: u32,
    /// Bits per pixel, from the XWD header.
    bits_per_pixel: u32,
    /// How many distinct pixel values the image holds, capped while counting.
    distinct_colours: usize,
}

/// Whether `tool` is absent from `PATH`, asked without running it.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn which_missing(tool: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            !std::env::split_paths(&path).any(|dir| {
                let candidate = dir.join(tool);
                candidate.is_file() && is_executable(&candidate)
            })
        })
        .unwrap_or(true)
}

/// Whether `path` is executable, using the mode bits directly (no extra dependency for this).
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|meta| meta.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// Asks `wmctrl`, `xwininfo` and `xwd` about the window titled `title`.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn ask_the_x_server(title: &str, expected: (u32, u32)) -> ScreenReport {
    let mut report = ScreenReport {
        listed: false,
        map_state: None,
        width: 0,
        height: 0,
        capture: None,
        capture_error: None,
    };

    // 1. The window manager's list: the id a person would use.
    let listing = std::process::Command::new("wmctrl").arg("-l").output();
    let id = match listing {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.contains(title))
            .and_then(|line| line.split_whitespace().next().map(str::to_owned)),
        Err(error) => {
            report.capture_error = Some(format!("wmctrl could not be run: {error}"));
            None
        }
    };
    let Some(id) = id else {
        report.capture_error = Some(format!("no managed window is titled {title:?}"));
        return report;
    };
    report.listed = true;

    // 2. The server's own report for that id.
    if let Ok(output) = std::process::Command::new("xwininfo").args(["-id", &id]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        report.map_state = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("Map State:").map(|v| v.trim().to_owned()));
        report.width = field(&text, "Width:");
        report.height = field(&text, "Height:");
    }

    // 3. The pixels, as the server stores them.
    let dump = std::env::temp_dir().join(format!("rw_on_screen_{}.xwd", std::process::id()));
    let capture = std::process::Command::new("xwd")
        .args(["-id", &id, "-silent"])
        .stdout(std::fs::File::create(&dump).expect("a writable temp file for the dump"))
        .output();
    match capture {
        Ok(output) if output.status.success() => match std::fs::read(&dump) {
            Ok(bytes) => match parse_xwd(&bytes) {
                Ok(parsed) => {
                    // A capture whose size is not what the library asked for would mean the id
                    // resolved to another window, which is the mistake a title lookup can make.
                    if parsed.0 != expected.0 || parsed.1 != expected.1 {
                        report.capture_error = Some(format!(
                            "captured {}x{}, not the {}x{} the library created",
                            parsed.0, parsed.1, expected.0, expected.1
                        ));
                    }
                    report.capture = Some(XwdCapture {
                        width: parsed.0,
                        height: parsed.1,
                        bits_per_pixel: parsed.2,
                        distinct_colours: parsed.3,
                    });
                }
                Err(error) => report.capture_error = Some(error),
            },
            Err(error) => {
                report.capture_error = Some(format!("the dump could not be read: {error}"))
            }
        },
        Ok(output) => report.capture_error = Some(format!("xwd exited with {}", output.status)),
        Err(error) => report.capture_error = Some(format!("xwd could not be run: {error}")),
    }
    let _ = std::fs::remove_file(&dump);
    report
}

/// Reads the `label 123` line `xwininfo` prints, as a number.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn field(text: &str, label: &str) -> u32 {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(label))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

/// Parses the pieces of an XWD file this probe reports: size, depth, and colour diversity.
///
/// # The header, and why it is parsed rather than assumed
///
/// `XWDFileHeader` is a big-endian struct of `u32`s: `header_size` first, then the version, the
/// pixmap depth, the **pixmap** width/height, and later the bits per pixel, the bytes per line and
/// the **window** width/height (which is what was asked for — the pixmap may be padded). The window
/// name follows the fixed 100-byte header, padded up to `header_size`, and then the colour map
/// entries, and only then the pixel rows. Reading the offsets out of the dump means a change in the
/// producer cannot silently shift the numbers this probe reports.
#[cfg(all(not(alloc_frugal), unix_desktop, feature = "gtk-native"))]
fn parse_xwd(bytes: &[u8]) -> Result<(u32, u32, u32, usize), String> {
    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
    }
    if bytes.len() < 100 {
        return Err(format!("the dump is {} bytes, shorter than an XWD header", bytes.len()));
    }
    let header_size = u32_at(bytes, 0) as usize;
    let bits_per_pixel = u32_at(bytes, 44);
    let bytes_per_line = u32_at(bytes, 48) as usize;
    let colour_map_entries = u32_at(bytes, 76) as usize;
    let width = u32_at(bytes, 80);
    let height = u32_at(bytes, 84);
    let pixel_start = header_size + colour_map_entries * 12;
    if bits_per_pixel == 0 || bytes_per_line == 0 || width == 0 || height == 0 {
        return Err(format!(
            "the dump declares {width}x{height} at {bits_per_pixel} bpp, which cannot be a frame"
        ));
    }
    let bytes_per_pixel = (bits_per_pixel / 8) as usize;
    let needed = pixel_start + bytes_per_line * height as usize;
    if bytes.len() < needed {
        return Err(format!(
            "the dump holds {} bytes but its header describes {needed}",
            bytes.len()
        ));
    }

    // Count distinct pixel values, capped: the question is "more than a flat fill", so the exact
    // number stops mattering long before the cap, and an uncapped set over a 480x320 window is
    // wasted memory in a probe.
    const CAP: usize = 4096;
    let mut seen = std::collections::HashSet::new();
    'rows: for row in 0..height as usize {
        let start = pixel_start + row * bytes_per_line;
        for column in 0..width as usize {
            let at = start + column * bytes_per_pixel;
            let pixel = &bytes[at..at + bytes_per_pixel];
            seen.insert(pixel.to_vec());
            if seen.len() >= CAP {
                break 'rows;
            }
        }
    }
    Ok((width, height, bits_per_pixel, seen.len()))
}
