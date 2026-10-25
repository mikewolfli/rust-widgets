//! Renders the finance demo's screen offscreen, at its real size, in both appearances.
//!
//! "Are the images complete in the light appearance?" is a question about painted pixels, so the
//! answer is a picture: `render_window_frame` produces the same frame the platform's painter
//! presents (window tree plus every mounted surface), without opening a window.
//!
//! Output is binary PPM (`P6`) in `target/demo-preview/`, which needs no encoder dependency; any
//! image tool can read it (`sips -s format png`, ImageMagick, PIL).
//!
//! Usage:
//! ```text
//! cargo run --manifest-path demo/finance/Cargo.toml --bin preview
//! ```

use std::fs;
use std::path::PathBuf;

/// The demo's window size. `build_screen` lays the panels out for exactly this box.
const WIDTH: u32 = 1440;
const HEIGHT: u32 = 900;

fn main() {
    let dir = PathBuf::from("target/demo-preview");
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("[preview] could not create {}: {error}", dir.display());
        std::process::exit(1);
    }

    for (dark, suffix) in [(true, "dark"), (false, "light")] {
        let Some(frame) = finance_demo::app::render_offscreen(dark, WIDTH, HEIGHT) else {
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

        let path = dir.join(format!("finance-demo-{suffix}.ppm"));
        if let Err(error) = fs::write(&path, &ppm) {
            eprintln!("[preview] could not write {}: {error}", path.display());
            std::process::exit(1);
        }
        println!("wrote {} ({WIDTH}x{HEIGHT})", path.display());
    }
}
