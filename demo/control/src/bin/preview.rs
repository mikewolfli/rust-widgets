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

use std::path::PathBuf;

/// The demo's window size, matching `build_window`.
const WIDTH: u32 = 1180;
const HEIGHT: u32 = 760;

fn main() {
    let dir = PathBuf::from("target/demo-preview");
    match preview_common::write_previews(&dir, WIDTH, HEIGHT, "control-demo", |dark| {
        control_demo::app::render_offscreen(dark, WIDTH, HEIGHT)
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
