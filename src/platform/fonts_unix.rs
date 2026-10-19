// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! System font enumeration for the Unix desktop backends (GTK/X11, Wayland, BSD).
//!
//! # Why `fc-list`
//!
//! BLUE28 B28.39 names fontconfig (`FcFontList`) or `fontdb` as the source. This crate has neither
//! dependency, and adding a font-parsing crate for one list would duplicate `ttf-parser`'s job.
//! `fc-list` is the fontconfig *command-line face* of the same database, ships with the library on
//! every distribution and BSD that has fontconfig, and reports both the family and the file:
//!
//! ```text
//! $ fc-list --format '%{family}\t%{style}\t%{file}\n'
//! DejaVu Sans,DejaVu Sans Book\tBook\t/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
//! ```
//!
//! The command runs inside this platform layer (principle #36) and answers an empty list when the
//! binary is absent — the honest "no enumerator here" rather than a fabricated family. The style and
//! path are what make the result usable: a caller reads `source` and registers the bytes.

use crate::compat::{String, Vec};
use crate::platform::types::SystemFont;
use std::process::Command;

/// The installed families, each with its first style and readable font file.
///
/// Families are de-duplicated case-insensitively: fontconfig lists one line per face (regular,
/// bold, italic…), and a chooser wants one row per family. The first line wins, which is the order
/// fontconfig itself considers canonical.
pub(crate) fn system_fonts() -> Vec<SystemFont> {
    let output =
        match Command::new("fc-list").args(["--format", "%{family}\t%{style}\t%{file}\n"]).output()
        {
            Ok(output) if output.status.success() => output,
            Ok(_) | Err(_) => return Vec::new(),
        };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut fonts: Vec<SystemFont> = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let families = fields.next().unwrap_or("").trim();
        let style = fields.next().unwrap_or("").trim();
        let file = fields.next().unwrap_or("").trim();
        // A family list is comma-separated aliases for one face; the first is the primary name.
        let family = families.split(',').next().unwrap_or("").trim();
        if family.is_empty() {
            continue;
        }
        if fonts.iter().any(|known| known.family.eq_ignore_ascii_case(family)) {
            continue;
        }
        fonts.push(SystemFont {
            family: family.to_string(),
            style: style.to_string(),
            source: (!file.is_empty()).then(|| std::path::PathBuf::from(file)),
        });
    }
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enumeration_is_well_formed() {
        // The list depends on the host; the *shape* is what every entry must satisfy.
        for font in system_fonts() {
            assert!(!font.family.is_empty());
            if let Some(source) = font.source {
                assert!(source.is_absolute(), "fc-list reports an absolute font path");
            }
        }
    }
}
