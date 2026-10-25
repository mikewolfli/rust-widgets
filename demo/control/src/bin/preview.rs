//! Renders the control demo's window offscreen, at its real size, in both appearances.
//!
//! # What this answers
//!
//! "Is the demo's composition right?" — the window's background colour, whether the Chinese
//! labels have glyphs rather than boxes, and whether every row is present — without opening a
//! window. The frame comes from `render_window_frame`: the window tree, then each mounted surface
//! at its own rectangle, which is the composition the platform's painters produce.
//!
//! Output is binary PPM (`P6`), written to `target/demo-preview/`. PPM needs no encoder
//! dependency; any image tool can read it (`sips -s format png`, ImageMagick, PIL).
//!
//! Usage:
//! ```text
//! cargo run --manifest-path demo/control/Cargo.toml --bin preview
//! ```

use std::fs;
use std::path::PathBuf;

/// The demo's window size, matching `build_window`.
const WIDTH: u32 = 1180;
const HEIGHT: u32 = 760;

fn main() {
    let dir = PathBuf::from("target/demo-preview");
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("[preview] could not create {}: {error}", dir.display());
        std::process::exit(1);
    }

    for (dark, suffix) in [(true, "dark"), (false, "light")] {
        let Some(frame) = control_demo::app::render_offscreen(dark, WIDTH, HEIGHT) else {
            eprintln!("[preview] the demo produced no frame for the {suffix} appearance");
            std::process::exit(1);
        };
        let expected = (WIDTH as usize) * (HEIGHT as usize) * 4;
        if frame.len() != expected {
            eprintln!(
                "[preview] expected {expected} bytes of RGBA for {suffix}, got {}",
                frame.len()
            );
            std::process::exit(1);
        }

        let mut ppm = Vec::with_capacity((WIDTH as usize) * (HEIGHT as usize) * 3 + 32);
        ppm.extend_from_slice(format!("P6\n{WIDTH} {HEIGHT}\n255\n").as_bytes());
        for pixel in frame.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }

        let path = dir.join(format!("control-demo-{suffix}.ppm"));
        if let Err(error) = fs::write(&path, &ppm) {
            eprintln!("[preview] could not write {}: {error}", path.display());
            std::process::exit(1);
        }
        println!("wrote {} ({WIDTH}x{HEIGHT})", path.display());
    }
}
