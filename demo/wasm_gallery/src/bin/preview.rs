//! Renders one gallery page offscreen, without a browser.
//!
//! # What this answers
//!
//! "Does the gallery actually compose, and is one page shown at a time?" — how many
//! kinds the factory enumerates, how many cards mounted, and what the page looks
//! like. The pixels come from `render_window_frame`, the same entry the browser's
//! paint loop blits, so a composition defect is visible here instead of only in a
//! browser.
//!
//! Output is binary PPM (`P6`) under `target/demo-preview/`: no encoder dependency,
//! and any image tool reads it (`sips -s format png`, ImageMagick, PIL).
//!
//! ```text
//! cargo run --manifest-path demo/wasm_gallery/Cargo.toml --bin preview -- --page 1
//! ```

use std::fs;
use std::path::PathBuf;

use wasm_gallery_demo::app::{render_offscreen, HEIGHT, WIDTH};

fn main() {
    let mut page = 0usize;
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--page" {
            match args.next().and_then(|value| value.parse::<usize>().ok()) {
                Some(value) => page = value.saturating_sub(1),
                None => {
                    eprintln!("[preview] --page needs a 1-based number");
                    std::process::exit(2);
                }
            }
        }
    }

    let Some(frame) = render_offscreen(page) else {
        eprintln!("[preview] render_window_frame returned no frame");
        std::process::exit(1);
    };
    if frame.len() != (WIDTH as usize) * (HEIGHT as usize) * 4 {
        eprintln!("[preview] unexpected frame length {}", frame.len());
        std::process::exit(1);
    }

    let mut ppm = Vec::with_capacity(frame.len() / 4 * 3 + 32);
    ppm.extend_from_slice(format!("P6\n{WIDTH} {HEIGHT}\n255\n").as_bytes());
    for pixel in frame.as_chunks::<4>().0 {
        ppm.extend_from_slice(&pixel[..3]);
    }

    let path = PathBuf::from(format!("target/demo-preview/wasm-gallery-page{}.ppm", page + 1));
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("[preview] cannot create {}: {error}", parent.display());
            std::process::exit(1);
        }
    }
    if let Err(error) = fs::write(&path, &ppm) {
        eprintln!("[preview] cannot write {}: {error}", path.display());
        std::process::exit(1);
    }
    println!("wrote {} ({WIDTH}x{HEIGHT})", path.display());
}
