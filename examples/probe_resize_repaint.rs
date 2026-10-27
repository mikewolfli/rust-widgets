// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Proves that a **user-driven** window resize repaints the window without any pointer or
//! keyboard event.
//!
//! # The defect this answers
//!
//! On the GTK backend, a window's ordinary children are painted by the window's own
//! `DrawingArea`. `invalidate_surface` only consulted the mounted-surface table, so the id a
//! `WindowHandle` carries (a widget-registry id) never resolved to the paint area. A resize
//! re-ran the layout — the host's own log said so — but the window kept the frame it drew at
//! the previous size until some unrelated expose arrived.
//!
//! The probe stages that exact sequence with real X11 tools:
//!
//! 1. A window with a full-width progress bar (a `BorderLayout` north track) is shown and
//!    painted at 480x320.
//! 2. A watcher thread resizes it to 760x320 with `wmctrl`. **No pointer or keyboard event is
//!    sent**; the only thing that happens is the window manager's `ConfigureNotify`.
//! 3. After the resize, `xwd` captures the window and the probe samples two pixels inside the
//!    bar's track. If the layout followed the resize and the window repainted, both are the
//!    bar's fill colour and differ from the background below the track.
//!
//! Run with:
//!     RW_GUI_PROBE_OK=1 cargo run --no-default-features --features "desktop,gtk-native" \
//!         --example probe_resize_repaint
//!
//! Exit codes: `0` = the new layout is on screen, `1` = a check failed, `2` = this host cannot
//! stage it (no display, or one of `wmctrl`/`xwininfo`/`xwd` is missing).

#[cfg(gtk_probe)]
fn main() {
    staged()
}

#[cfg(not(gtk_probe))]
fn main() {
    println!(
        "probe_resize_repaint: this build has no X11 session to ask (`--features \
         desktop,gtk-native` on a Linux host with DISPLAY set stages it)."
    );
}

#[cfg(gtk_probe)]
#[path = "common/gui_probe.rs"]
mod gui_probe;
#[cfg(gtk_probe)]
use gui_probe::gui_probe_allowed;

#[cfg(gtk_probe)]
fn staged() {
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    use rust_widgets::app::{App, WidgetHandle};
    use rust_widgets::layout::{BorderLayout, BorderRegion};

    if !gui_probe_allowed() {
        std::process::exit(2);
    }

    const TITLE: &str = "rw resize repaint probe (throwaway)";
    const INITIAL: (u32, u32) = (480, 320);
    const RESIZED: (u32, u32) = (760, 320);

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

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    let mut app = App::new();
    app.init();
    let window = app.new_window(TITLE, 60, 60, INITIAL.0, INITIAL.1);
    // A full-width track: the bar spans whatever width the layout gives it, and at 100% its
    // fill covers the whole track. A resize therefore moves paint into the newly exposed
    // right-hand strip, which is what the pixel sample below reads.
    let bar = window.new_progress_bar(0, 0, 0, 0);
    bar.set_value(100);
    let mut layout = BorderLayout::new(0, 0);
    layout.add(BorderRegion::North, bar.raw_id(), 40);
    window.set_layout(layout);
    window.show();

    let (tx, rx) = mpsc::channel::<Result<Report, String>>();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(800));
        let result = resize_capture_and_sample(TITLE, INITIAL, RESIZED);
        let _ = tx.send(result);
        let _ = rust_widgets::quit();
    });

    app.run();
    let report = rx.recv_timeout(Duration::from_secs(30)).expect("the watcher must report");
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            println!("  FAIL {error}");
            std::process::exit(1);
        }
    };

    println!("=== resize repaint probe ===");
    println!("backend          : {}", rust_widgets::platform::backend_name());
    println!("geometry asked   : {}x{} -> {}x{}", INITIAL.0, INITIAL.1, RESIZED.0, RESIZED.1);
    println!("server geometry  : {}x{}", report.width, report.height);
    println!(
        "bar pixel        : {:?} (left) / {:?} (right)",
        report.left_pixel, report.right_pixel
    );
    println!("background pixel : {:?}", report.background_pixel);

    check(report.width >= RESIZED.0.saturating_sub(2), "the window manager applied the resize");
    check(
        report.capture_width == report.width && report.capture_height == report.height,
        "the capture is the resized window, not the old frame",
    );
    check(
        report.left_pixel == report.right_pixel,
        "the bar reaches the new right edge, so the layout followed the resize and was repainted",
    );
    check(
        report.left_pixel != report.background_pixel,
        "the sampled track really is the bar's fill and not the window background",
    );

    println!(
        "\n{}",
        if failures == 0 {
            "probe_resize_repaint: OK (resize repainted without input)".to_string()
        } else {
            format!("probe_resize_repaint: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

#[cfg(gtk_probe)]
struct Report {
    width: u32,
    height: u32,
    capture_width: u32,
    capture_height: u32,
    left_pixel: Vec<u8>,
    right_pixel: Vec<u8>,
    background_pixel: Vec<u8>,
}

#[cfg(gtk_probe)]
fn resize_capture_and_sample(
    title: &str,
    _initial: (u32, u32),
    resized: (u32, u32),
) -> Result<Report, String> {
    let id = window_id(title)?;
    let geometry = format!("0,60,60,{},{}", resized.0, resized.1);
    let status = std::process::Command::new("wmctrl")
        .args(["-i", "-r", id.as_str(), "-e", geometry.as_str()])
        .status()
        .map_err(|error| format!("wmctrl could not resize the window: {error}"))?;
    if !status.success() {
        return Err(format!("wmctrl exited with {status}"));
    }
    // Let the toolkit process the ConfigureNotify and finish the paint the library queued.
    std::thread::sleep(std::time::Duration::from_millis(700));

    let info = std::process::Command::new("xwininfo")
        .args(["-id", &id])
        .output()
        .map_err(|error| format!("xwininfo could not be run: {error}"))?;
    let text = String::from_utf8_lossy(&info.stdout);
    let width = field(&text, "Width:");
    let height = field(&text, "Height:");
    if width == 0 || height == 0 {
        return Err("xwininfo reported no usable geometry".to_string());
    }

    let dump = std::env::temp_dir().join(format!("rw_resize_{}.xwd", std::process::id()));
    let capture = std::process::Command::new("xwd")
        .args(["-id", &id, "-silent"])
        .stdout(std::fs::File::create(&dump).map_err(|error| error.to_string())?)
        .output()
        .map_err(|error| format!("xwd could not be run: {error}"))?;
    if !capture.status.success() {
        return Err(format!("xwd exited with {}", capture.status));
    }
    let bytes = std::fs::read(&dump).map_err(|error| error.to_string())?;
    let _ = std::fs::remove_file(&dump);
    let parsed = Xwd::parse(bytes)?;

    // The track is 40px tall at the top. Sample inside it on the far left and the far right;
    // sample well below it for the window background. A stale frame leaves the right sample at
    // the background colour (the strip the old frame never covered).
    let y = 20.min(parsed.height.saturating_sub(1));
    let left = parsed.pixel(10.min(parsed.width.saturating_sub(1)), y)?;
    let right = parsed.pixel(parsed.width.saturating_sub(20), y)?;
    let background_y = parsed.height.saturating_sub(20);
    let background = parsed.pixel(10.min(parsed.width.saturating_sub(1)), background_y)?;

    Ok(Report {
        width,
        height,
        capture_width: parsed.width,
        capture_height: parsed.height,
        left_pixel: left,
        right_pixel: right,
        background_pixel: background,
    })
}

/// The X window id `wmctrl` lists for `title`.
#[cfg(gtk_probe)]
fn window_id(title: &str) -> Result<String, String> {
    let output = std::process::Command::new("wmctrl")
        .arg("-l")
        .output()
        .map_err(|error| format!("wmctrl could not be run: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.contains(title))
        .and_then(|line| line.split_whitespace().next().map(str::to_owned))
        .ok_or_else(|| format!("no managed window is titled {title:?}"))
}

/// Reads the `label 123` line `xwininfo` prints, as a number.
#[cfg(gtk_probe)]
fn field(text: &str, label: &str) -> u32 {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(label))
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

/// Whether `tool` is absent from `PATH`, asked without running it.
#[cfg(gtk_probe)]
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

/// Whether `path` is executable, using the mode bits directly.
#[cfg(gtk_probe)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|meta| meta.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

/// The parts of an XWD dump this probe reads.
#[cfg(gtk_probe)]
struct Xwd {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    bytes_per_pixel: usize,
    bytes_per_line: usize,
    pixel_start: usize,
}

#[cfg(gtk_probe)]
impl Xwd {
    /// Parses the header `xwd` writes (`XWDFileHeader` is a big-endian struct of `u32`s).
    fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        fn u32_at(bytes: &[u8], offset: usize) -> u32 {
            u32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ])
        }
        if bytes.len() < 100 {
            return Err(format!("the dump is {} bytes, shorter than an XWD header", bytes.len()));
        }
        let header_size = u32_at(&bytes, 0) as usize;
        let bits_per_pixel = u32_at(&bytes, 44);
        let bytes_per_line = u32_at(&bytes, 48) as usize;
        let colour_map_entries = u32_at(&bytes, 76) as usize;
        let width = u32_at(&bytes, 80);
        let height = u32_at(&bytes, 84);
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
        Ok(Self { bytes, width, height, bytes_per_pixel, bytes_per_line, pixel_start })
    }

    fn pixel(&self, x: u32, y: u32) -> Result<Vec<u8>, String> {
        if x >= self.width || y >= self.height {
            return Err(format!(
                "pixel ({x},{y}) is outside the {}x{} capture",
                self.width, self.height
            ));
        }
        let at =
            self.pixel_start + y as usize * self.bytes_per_line + x as usize * self.bytes_per_pixel;
        // `parse` already proved the buffer long enough for every in-range pixel; this bound
        // keeps the indexing here panic-free if that ever changes.
        if at + self.bytes_per_pixel > self.bytes.len() {
            return Err("the pixel lies past the end of the dump".to_string());
        }
        Ok(self.bytes[at..at + self.bytes_per_pixel].to_vec())
    }
}
