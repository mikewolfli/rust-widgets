// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Shared offscreen preview writer for the demo binaries (DEMO-008).
//!
//! Both `control` and `finance` render their window offscreen and write binary PPM (`P6`) in
//! both appearances. The loop was duplicated line for line in their two `bin/preview.rs` files,
//! including the size check and the error text. It lives here so a change to the format or the
//! output directory is made once, and a demo only supplies its size, file prefix and renderer.

use std::fs;
use std::path::Path;

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
