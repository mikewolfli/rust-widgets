// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CoreText-backed installed-font enumeration for the Apple backends (B28.39).
//!
//! # Why CoreText and not AppKit/UIKit
//!
//! `NSFontManager.availableFontFamilies` (macOS) and `UIFont.familyNames` (iOS) report names
//! only: neither exposes the file behind a family, so a chosen system font could change the
//! `Font` descriptor but never the bytes the renderer shapes. CoreText is the layer both
//! frameworks sit on, and `kCTFontURLAttribute` is the documented attribute that carries the
//! backing file. Enumerating here also removes the main-thread requirement the two framework
//! lists carried — a worker that asks for fonts gets the real list.
//!
//! A family whose best match is a collection (`.ttc`) yields the collection's file. The
//! renderer opens face 0 of that file; the family name is still the selected one, and a caller
//! that needs a specific face must register it itself.

use crate::compat::{String, ToString, Vec};
use crate::platform::types::SystemFont;
use objc2_core_foundation::{CFString, CFURL};
use objc2_core_text::{
    kCTFontStyleNameAttribute, kCTFontURLAttribute, CTFont,
    CTFontManagerCopyAvailableFontFamilyNames,
};
use std::path::PathBuf;

/// The installed families, each with its style name and backing file.
pub(crate) fn system_fonts() -> Vec<SystemFont> {
    // SAFETY: CoreText returns a retained `CFArray`; `CFRetained` owns that reference and releases
    // it on drop. The array outlives every element read below, and each element is a `CFString`
    // owned by the array.
    let names = unsafe { CTFontManagerCopyAvailableFontFamilyNames() };
    let count = names.count();
    let mut fonts = Vec::with_capacity(count.max(0) as usize);
    for index in 0..count {
        // SAFETY: `index` is inside `0..count`, and CoreText documents the array as CFStrings for
        // that range. The reference does not outlive `names`, which is still alive here.
        let family = unsafe {
            let value = names.value_at_index(index);
            if value.is_null() {
                None
            } else {
                Some(&*(value as *const CFString))
            }
        };
        let Some(family) = family else {
            continue;
        };
        let family = family.to_string();
        if family.trim().is_empty() {
            continue;
        }
        let (style, source) = details(&family);
        fonts.push(SystemFont { family, style, source });
    }
    fonts
}

/// The style name and file behind one family.
fn details(family: &str) -> (String, Option<PathBuf>) {
    let name = CFString::from_str(family);
    // SAFETY: a null matrix is the documented "use the identity matrix". `name` outlives the call,
    // and CoreText returns a retained `CTFont` owned by `font` for the rest of this function.
    let font = unsafe { CTFont::with_name(&name, 0.0, core::ptr::null()) };
    // SAFETY: the descriptor attributes are copied (the "Copy" rule) into `CFRetained`, so the
    // result is independent of `font`, and `downcast` checks the CoreFoundation type id before
    // treating the value as the requested type.
    let style = unsafe { font.attribute(kCTFontStyleNameAttribute) }
        .and_then(|value| value.downcast::<CFString>().ok())
        .map(|style| style.to_string())
        .unwrap_or_default();
    // SAFETY: as above. `kCTFontURLAttribute` is documented as a `CFURL` when present; a font
    // packaged only in memory has no URL and is reported as "no source" rather than a fake path.
    let source = unsafe { font.attribute(kCTFontURLAttribute) }
        .and_then(|value| value.downcast::<CFURL>().ok())
        .and_then(|url| file_path(&url));
    (style, source)
}

/// A `CFURL`'s file-system path.
fn file_path(url: &CFURL) -> Option<PathBuf> {
    // `PATH_MAX` on Darwin is 1024; 4096 leaves room for a longer spelling without ever handing
    // CoreFoundation a buffer smaller than the path it is asked to write.
    let mut buffer = [0u8; 4096];
    // SAFETY: the buffer belongs to this frame and `buffer.len()` is its exact capacity. CFURL
    // writes at most that many bytes and null-terminates on success; the bool reports whether it
    // fit.
    let written =
        unsafe { url.file_system_representation(true, buffer.as_mut_ptr(), buffer.len() as isize) };
    if !written {
        return None;
    }
    let length = buffer.iter().position(|byte| *byte == 0).unwrap_or(buffer.len());
    if length == 0 {
        return None;
    }
    let path = String::from_utf8_lossy(&buffer[..length]).into_owned();
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_text_pairs_families_with_readable_files() {
        let fonts = system_fonts();
        assert!(!fonts.is_empty(), "the host always reports at least one family");
        assert!(
            fonts.iter().any(|font| font.source.is_some()),
            "kCTFontURLAttribute must name a file for at least one family"
        );
        for font in &fonts {
            if let Some(source) = &font.source {
                assert!(source.is_absolute(), "{} is an absolute file URL", source.display());
                assert!(source.exists(), "{} must exist", source.display());
            }
        }
    }
}
