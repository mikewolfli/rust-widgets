// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stages the four GTK input/paint paths a headless test cannot distinguish.
//!
//! # What this answers
//!
//! `feedback-20261020.md` §0.2-§0.5 recorded four defects that only a real GTK/X11 window
//! exposes. The fixes landed in 2.8.13 but were verified by code reading on a macOS host; this
//! probe is the Linux runtime evidence:
//!
//! 1. **§0.2 key normalisation** — a real Return/Escape must reach `LineEdit`'s
//!    `return_pressed`/`editing_finished`. GDK reports keyvals (`0xFF0D`/`0xFF1B`), the control
//!    matches the framework's logical codes (`13`/`27`), so a missing translation shows up as
//!    zero signals for keys that plainly arrived.
//! 2. **§0.3 double click** — a real X11 double click on a `ListBox` row must emit
//!    `item_activated` with that row's index. GTK reports two presses and no click count, so
//!    the platform boundary has to synthesise the gesture.
//! 3. **§0.4 `create_widget_of_kind`** — the documented single creation entry must return a
//!    live id for a `Canvas` and the canvas must receive a real pointer press.
//! 4. **§0.5 timer repaint** — a `ListBox` rebuilt from a `schedule_on_main` callback must
//!    change the *captured pixels*, not only `item_count`. The probe captures the list region
//!    before and after and compares their bytes; a model-only success leaves the hashes equal.
//!
//! # How the input is staged
//!
//! `tools/x11_inject_keys.py` sends real XTEST events. The probe never calls `dispatch_event`
//! and never emits a signal itself: every counter below is incremented from the widget's own
//! signal, which is the only path a person's input takes.
//!
//! Run with:
//!     RW_GUI_PROBE_OK=1 cargo run --no-default-features --features "desktop,gtk-native" \
//!         --example probe_gtk_interactions
//!
//! Exit codes: `0` = every check held, `1` = a check failed, `2` = this host cannot stage it
//! (no display, or one of `python3`/`wmctrl`/`xwininfo`/`xwd`/`xwdtopnm` missing).

#[cfg(gtk_probe)]
fn main() {
    staged()
}

#[cfg(not(gtk_probe))]
fn main() {
    println!(
        "probe_gtk_interactions: this build has no X11 session to ask (`--features \
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    use rust_widgets::app::{App, WidgetHandle};
    use rust_widgets::core::Rect;
    use rust_widgets::widget::capability::coercion::widget_as_mut;
    use rust_widgets::widget::{Canvas, LineEdit, ListBox, Widget as WidgetTrait, WidgetKind};

    if !gui_probe_allowed() {
        std::process::exit(2);
    }

    // Every external tool the staging needs, checked before a window opens so a restricted
    // host is reported as unsupported rather than as a failing check (principle #103).
    for tool in ["python3", "wmctrl", "xwininfo", "xwd", "xwdtopnm"] {
        if which_missing(tool) {
            println!("unsupported host: `{tool}` is not installed, so X11 cannot be staged");
            std::process::exit(2);
        }
    }

    const TITLE_PREFIX: &str = "rw gtk interactions probe";
    // A per-run title: the injector finds the window by title substring, and a reused title
    // could match a window a previous run left on screen.
    let title = format!("{TITLE_PREFIX} {} (throwaway)", std::process::id());

    // Window-relative geometry. The controls keep these rectangles: no window layout runs, so the
    // probe's injected coordinates and the controls' boxes are the same numbers.
    let line_rect = Rect::new(20, 20, 300, 32);
    let list_rect = Rect::new(20, 80, 300, 120);
    let canvas_rect = Rect::new(20, 240, 300, 120);

    let returns = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let activated = Arc::new(Mutex::new(Vec::<usize>::new()));
    let canvas_presses = Arc::new(AtomicUsize::new(0));
    let list_items_after_timer = Arc::new(AtomicUsize::new(0));

    let mut app = App::new();
    app.init();
    // Undecorated: the injector addresses the window's absolute origin, and a client-side
    // title bar would shift every relative coordinate down by its height. Without decorations
    // the client origin is the window origin, so the injected points name the controls the
    // probe thinks they name.
    let window = app.new_window(&title, 60, 60, 420, 400);
    // Undecorated: the injector addresses the window's absolute origin, and a client-side
    // title bar would shift every relative coordinate down by its height. Without decorations
    // the client origin is the window origin, so the injected points name the controls the
    // probe thinks they name.
    window.set_decorated(false);
    let line =
        window.new_line_edit("", line_rect.x, line_rect.y, line_rect.width, line_rect.height);
    let list = window.new_list_box(list_rect.x, list_rect.y, list_rect.width, list_rect.height);
    for item in ["alpha", "bravo", "charlie"] {
        assert!(list.add_item(item), "the list must accept an item");
    }
    list.set_current_index(0);

    // §0.4: the documented single, mechanism-free creation entry.
    let canvas_id = rust_widgets::create_widget_of_kind(
        WidgetKind::Canvas,
        window.raw_id(),
        "",
        canvas_rect.x,
        canvas_rect.y,
        canvas_rect.width,
        canvas_rect.height,
        Some(Box::new(Canvas::new(canvas_rect))),
    );

    // ── Signal probes ────────────────────────────────────────────────────
    // Every counter is incremented from the widget's own signal, so a missing platform
    // translation cannot be papered over by the probe emitting on its behalf.
    {
        let returns = Arc::clone(&returns);
        let finished = Arc::clone(&finished);
        rust_widgets::widget::runtime::with_widget_mut(line.raw_id(), move |widget| {
            let Some(edit) = widget_as_mut::<LineEdit>(widget) else {
                return;
            };
            edit.return_pressed.connect_scoped(edit.connection_scope(), move || {
                returns.fetch_add(1, Ordering::SeqCst);
            });
            edit.editing_finished.connect_scoped(edit.connection_scope(), move || {
                finished.fetch_add(1, Ordering::SeqCst);
            });
        });
    }
    {
        let activated = Arc::clone(&activated);
        rust_widgets::widget::runtime::with_widget_mut(list.raw_id(), move |widget| {
            let Some(control) = widget_as_mut::<ListBox>(widget) else {
                return;
            };
            control.item_activated.connect_scoped(control.connection_scope(), move |index| {
                activated.lock().unwrap().push(*index);
            });
        });
    }
    if canvas_id != 0 {
        let presses = Arc::clone(&canvas_presses);
        rust_widgets::widget::runtime::with_widget_mut(canvas_id, move |widget| {
            let Some(canvas) = widget_as_mut::<Canvas>(widget) else {
                return;
            };
            canvas.mouse_pressed.connect_scoped(canvas.connection_scope(), move || {
                presses.fetch_add(1, Ordering::SeqCst);
            });
        });
    }

    window.show();

    let (tx, rx) = mpsc::channel::<Result<Report, String>>();
    {
        let title = title.clone();
        let list = list.clone();
        let items_after = Arc::clone(&list_items_after_timer);
        std::thread::spawn(move || {
            let mut steps: Vec<String> = Vec::new();
            let result = stage(&title, &mut steps, move || {
                // §0.5: rebuild the list from the main-loop timer, which is the path the
                // feedback recorded as "the model changed and the window did not".
                list.clear_items();
                for row in 0..7 {
                    list.add_item(&format!("row {row} after timer"));
                }
                list.set_current_index(5);
                items_after.store(list.item_count(), Ordering::SeqCst);
            });
            let _ = tx.send(result.map(|(before, after)| Report { steps, before, after }));
            let _ = rust_widgets::quit();
        });
    }

    app.run();
    let report = rx.recv_timeout(Duration::from_secs(60)).expect("the watcher must report");

    let mut failures = 0usize;
    let mut check = |held: bool, what: &str| {
        if held {
            println!("  OK   {what}");
        } else {
            println!("  FAIL {what}");
            failures += 1;
        }
    };

    println!("=== gtk interactions probe ===");
    println!("backend            : {}", rust_widgets::platform::backend_name());
    println!("create_widget_of_kind: {canvas_id}");
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            println!("  FAIL {error}");
            std::process::exit(1);
        }
    };
    for step in &report.steps {
        println!("step               : {step}");
    }
    println!("return_pressed     : {}", returns.load(Ordering::SeqCst));
    println!("editing_finished   : {}", finished.load(Ordering::SeqCst));
    println!("item_activated     : {:?}", activated.lock().unwrap());
    println!("canvas presses     : {}", canvas_presses.load(Ordering::SeqCst));
    let model_rows = list_items_after_timer.load(Ordering::SeqCst);
    println!("list model after   : {model_rows}");
    println!("list bytes before  : {:#018x}", report.before);
    println!("list bytes after   : {:#018x}", report.after);

    check(canvas_id != 0, "create_widget_of_kind returned a live Canvas id (§0.4)");
    check(
        returns.load(Ordering::SeqCst) >= 1,
        "a real Return reached LineEdit::return_pressed (§0.2)",
    );
    check(
        finished.load(Ordering::SeqCst) >= 2,
        "Return and Escape both reached LineEdit::editing_finished (§0.2)",
    );
    let activated = activated.lock().unwrap().clone();
    check(
        activated == vec![1],
        "a real X11 double click activated exactly the second ListBox row (§0.3)",
    );
    check(
        canvas_presses.load(Ordering::SeqCst) >= 1,
        "a real pointer press reached the Canvas created through create_widget_of_kind (§0.4)",
    );
    check(model_rows == 7, "the timer rebuilt the list model with seven rows (§0.5)");
    check(
        report.before != report.after,
        "the list region's captured bytes changed after the timer update (§0.5)",
    );

    println!(
        "\n{}",
        if failures == 0 {
            "probe_gtk_interactions: OK (keys, double click, single creation entry, timer repaint)"
                .to_string()
        } else {
            format!("probe_gtk_interactions: {failures} check(s) failed")
        }
    );
    std::process::exit(if failures == 0 { 0 } else { 1 });
}

/// Captures the list region before and after `update_list` runs on the main loop.
///
/// The capture is taken through `xwd`/`xwdtopnm`, so it reads the window's **presented**
/// pixels rather than the model. `update_list` is handed to `schedule_on_main`, which is the
/// path §0.5 named as the one that did not repaint.
#[cfg(gtk_probe)]
fn stage(
    title: &str,
    steps: &mut Vec<String>,
    update_list: impl FnOnce() + Send + 'static,
) -> Result<(u64, u64), String> {
    use std::time::Duration;

    // Let the window map, paint and take keyboard focus.
    std::thread::sleep(Duration::from_millis(900));

    // §0.2: click the field, then press Return and Escape. The text is empty: this is about
    // the key codes, not about typing.
    run_injector(
        title,
        &["--activate", "--plain", "--click-relative", "170", "36", "--key", "Return", "Escape"],
    )?;
    steps.push("typed Return then Escape in the LineEdit".to_string());
    std::thread::sleep(Duration::from_millis(350));

    // §0.3: the second row of the list. The list is at (20, 80), its content starts one pixel
    // inside the frame, and a row is 20 px tall, so the second row's centre is y = 111.
    run_injector(title, &["--activate", "--plain", "--double-click-relative", "60", "111"])?;
    steps.push("double-clicked the ListBox's second row".to_string());
    std::thread::sleep(Duration::from_millis(350));

    // §0.4: a single real press on the canvas.
    run_injector(title, &["--activate", "--plain", "--click-relative", "60", "300"])?;
    steps.push("clicked the Canvas".to_string());
    std::thread::sleep(Duration::from_millis(350));

    // The list's painted region, sampled as raw bytes.
    let before = capture_region(title, 22, 82, 296, 116)?;
    if !rust_widgets::schedule_on_main(Duration::ZERO, update_list) {
        return Err("the running loop refused the timer work".to_string());
    }
    steps.push("scheduled the ListBox rebuild on the main loop".to_string());
    std::thread::sleep(Duration::from_millis(900));
    let after = capture_region(title, 22, 82, 296, 116)?;
    Ok((before, after))
}

#[cfg(gtk_probe)]
struct Report {
    steps: Vec<String>,
    before: u64,
    after: u64,
}

/// Runs `tools/x11_inject_keys.py` against `title` with `args`.
#[cfg(gtk_probe)]
fn run_injector(title: &str, args: &[&str]) -> Result<(), String> {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tools/x11_inject_keys.py");
    let output = std::process::Command::new("python3")
        .arg(script)
        .arg(title)
        .arg("")
        .args(args)
        .output()
        .map_err(|error| format!("could not run the X11 injector: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "the X11 injector exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

/// Hashes the `(x, y, width, height)` region of the window's presented pixels.
///
/// `xwdtopnm` writes a P6 PPM, which is a three-line header followed by raw RGB. Parsing it
/// here is deliberately smaller than a second XWD header reader: this probe only needs a
/// stable byte digest, not the pixel values.
#[cfg(gtk_probe)]
fn capture_region(title: &str, x: u32, y: u32, width: u32, height: u32) -> Result<u64, String> {
    let id = window_id(title)?;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("xwd -id {id} -silent | xwdtopnm 2>/dev/null"))
        .output()
        .map_err(|error| format!("could not capture the window: {error}"))?;
    if !output.status.success() {
        return Err(format!("the capture pipeline exited with {}", output.status));
    }
    ppm_region_hash(&output.stdout, x, y, width, height)
}

/// FNV-1a over one region of a P6 PPM.
#[cfg(gtk_probe)]
fn ppm_region_hash(bytes: &[u8], x: u32, y: u32, width: u32, height: u32) -> Result<u64, String> {
    let mut cursor = 0usize;
    let token = |cursor: &mut usize| -> Result<String, String> {
        // Skip whitespace and `#` comments, as the Netpbm format allows.
        loop {
            while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
                *cursor += 1;
            }
            if *cursor < bytes.len() && bytes[*cursor] == b'#' {
                while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
                    *cursor += 1;
                }
            } else {
                break;
            }
        }
        let start = *cursor;
        while *cursor < bytes.len() && !bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }
        String::from_utf8(bytes[start..*cursor].to_vec()).map_err(|_| "non-UTF-8 PPM header".into())
    };
    if token(&mut cursor)? != "P6" {
        return Err("the capture is not a P6 PPM".to_string());
    }
    let image_width: u32 =
        token(&mut cursor)?.parse().map_err(|_| "the PPM has no width".to_string())?;
    let image_height: u32 =
        token(&mut cursor)?.parse().map_err(|_| "the PPM has no height".to_string())?;
    let max_value: u32 =
        token(&mut cursor)?.parse().map_err(|_| "the PPM has no maxval".to_string())?;
    // The header ends after one whitespace byte.
    cursor += 1;
    let samples = if max_value < 256 { 1usize } else { 2usize };
    let stride = image_width as usize * 3 * samples;
    let needed = cursor + stride * image_height as usize;
    if bytes.len() < needed {
        return Err(format!(
            "the PPM holds {} bytes but its header describes {needed}",
            bytes.len()
        ));
    }
    if x + width > image_width || y + height > image_height {
        return Err(format!(
            "the requested region is outside the {image_width}x{image_height} capture"
        ));
    }

    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for row in y..y + height {
        let start = cursor + row as usize * stride + x as usize * 3 * samples;
        let end = start + width as usize * 3 * samples;
        for byte in &bytes[start..end] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    Ok(hash)
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
