// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared offscreen preview writer for the demo binaries (DEMO-008).
//!
//! Both `control` and `finance` render their window offscreen and write binary PPM (`P6`) in
//! both appearances. The loop was duplicated line for line in their two `bin/preview.rs` files,
//! including the size check and the error text. It lives here so a change to the format or the
//! output directory is made once, and a demo only supplies its size, file prefix and renderer.
//!
//! The same crate also owns the demos' **appearance** helper (DM3-1). All three demos used to
//! carry their own copy of "register both macOS palettes → select one → repaint what is already
//! mounted", and one of those copies had already dropped the repaint half. Registering and
//! selecting the palettes is one operation, so it is written once here.

use std::fs;
use std::path::Path;

use rust_widgets::theme::{global_theme_manager, AppearanceMode, Theme};

/// Name of the dark palette [`apply_appearance_mode`] registers.
pub const MACOS_DARK: &str = "macos-dark";
/// Name of the light palette [`apply_appearance_mode`] registers.
pub const MACOS_LIGHT: &str = "macos-light";

/// Registers both macOS palettes and selects the one `dark` names.
///
/// # Why both halves are one function
///
/// `set_theme` changes what **newly created** controls use, and `reapply_active_theme` restyles
/// what is already on screen. A demo that called only the first half looked like a no-op when it
/// switched themes after building its window — the bug is easy to reintroduce in a private copy,
/// so the two calls live together here and every demo reaches them through this one function.
///
/// The palettes are the library's own presets (`Theme::macos_dark` / `Theme::macos_light`), not a
/// demo-local table: a private copy would drift the moment a token changed.
///
/// Registration is idempotent (`register_theme` updates a name in place), so calling this again
/// after a theme reload refreshes the palette instead of accumulating copies.
pub fn apply_appearance_mode(dark: bool) {
    let mut manager = global_theme_manager();
    manager.register_theme(Theme::macos_dark());
    manager.register_theme(Theme::macos_light());
    let name = if dark { MACOS_DARK } else { MACOS_LIGHT };
    manager.set_theme(name);
    drop(manager);
    rust_widgets::reapply_active_theme();
}

/// Whether the active palette is dark.
///
/// Asked of the theme manager rather than mirrored in a `bool`: the theme is global state and
/// another part of the process may switch it, so a private copy would point a menu entry or a
/// toggle the wrong way and the next click would select the palette already in use.
pub fn dark_active() -> bool {
    global_theme_manager()
        .current_theme()
        .map(|theme| theme.appearance == AppearanceMode::Dark)
        .unwrap_or(false)
}

/// Writes the dark and light previews of one demo to `dir`.
///
/// `render` is called with `true` for the dark appearance and `false` for the light one, and
/// returns the RGBA frame when the demo produced one. The frame must be exactly
/// `width * height * 4` bytes; a mismatch is an error rather than a truncated image.
///
/// Returns the paths written, in appearance order, so a caller can report them.
pub fn write_previews<F>(
    dir: &Path,
    width: u32,
    height: u32,
    prefix: &str,
    mut render: F,
) -> Result<Vec<String>, String>
where
    F: FnMut(bool) -> Option<Vec<u8>>,
{
    fs::create_dir_all(dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    let expected = (width as usize) * (height as usize) * 4;
    let mut written = Vec::with_capacity(2);
    for (dark, suffix) in [(true, "dark"), (false, "light")] {
        let frame = render(dark)
            .ok_or_else(|| format!("the demo produced no frame for the {suffix} appearance"))?;
        if frame.len() != expected {
            return Err(format!(
                "expected {expected} bytes of RGBA for {suffix}, got {}",
                frame.len()
            ));
        }

        let mut ppm = Vec::with_capacity((width as usize) * (height as usize) * 3 + 32);
        ppm.extend_from_slice(format!("P6\n{width} {height}\n255\n").as_bytes());
        for pixel in frame.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }

        let path = dir.join(format!("{prefix}-{suffix}.ppm"));
        fs::write(&path, &ppm)
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
        written.push(path.display().to_string());
    }
    Ok(written)
}
