//! Renders the code-editor demo's window offscreen, at its real size, in both appearances.
//!
//! # What this answers
//!
//! "Is the demo's composition right?" — the split layout, the tree and editor surfaces, the
//! menu/tool/status rows — without opening a window. It calls the same `render_offscreen` the
//! live demo's builder feeds, so the preview cannot show a layout `run` does not produce.
//!
//! Output is binary PPM (`P6`), written to `target/demo-preview/`. PPM needs no encoder
//! dependency; any image tool can read it (`sips -s format png`, ImageMagick, PIL).
//!
//! Usage:
//! ```text
//! cargo run --manifest-path demo/code_editor/Cargo.toml --bin preview
//! ```

use std::path::PathBuf;

/// The demo's window size, matching `WINDOW_W`/`WINDOW_H` in `app.rs`.
const WIDTH: u32 = 1100;
const HEIGHT: u32 = 720;

fn main() {
    let dir = PathBuf::from("target/demo-preview");
    match preview_common::write_previews(&dir, WIDTH, HEIGHT, "code-editor-demo", |dark| {
        code_editor_demo::app::render_offscreen(dark, WIDTH, HEIGHT)
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
