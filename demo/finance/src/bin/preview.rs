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

use std::path::PathBuf;

/// The demo's window size. `build_screen` lays the panels out for exactly this box.
const WIDTH: u32 = 1440;
const HEIGHT: u32 = 900;

fn main() {
    let dir = PathBuf::from("target/demo-preview");
    match preview_common::write_previews(&dir, WIDTH, HEIGHT, "finance-demo", |dark| {
        finance_demo::app::render_offscreen(dark, WIDTH, HEIGHT)
    }) {
        Ok(written) => {
            for path in written {
                println!("wrote {path} ({WIDTH}x{HEIGHT})");
            }
        }
        Err(error) => {
            eprintln!("[preview] {error}");
            std::process::exit(1);
        }
    }
}
