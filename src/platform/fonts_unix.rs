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
//! binary cannot be launched — the honest "no enumerator here" rather than a fabricated family
//! (principle #37; the judge is spawn success, principle #38, so a non-zero exit with real output is
//! still parsed). The style and path are what make the result usable: a caller reads `source` and
//! registers the bytes.
//!
//! The enumeration is cached for the process and invalidated by
//! [`refresh_system_fonts`](crate::platform::fonts::refresh_system_fonts), so
//! [`Platform::system_fonts`](crate::platform::Platform::system_fonts) keeps its "a new `Vec` every
//! call" contract without spawning `fc-list` for each one (P-05).

use crate::compat::{Mutex, OnceLock};
use crate::compat::{String, Vec};
use crate::platform::types::SystemFont;
use std::process::Command;

/// The cached enumeration: the [`enumeration_epoch`](crate::platform::fonts::enumeration_epoch())
/// it was produced at, and the process-lifetime list.
///
/// # Why the trait method stopped spawning per call (P-05)
///
/// `Platform::system_fonts` is an ordinary trait method: a diagnostic probe, a second font dialog
/// or a host that asks per window all reach it directly, and each call used to spawn `fc-list` and
/// parse its whole output. `platform::fonts` keeps a process-wide snapshot in front of the trait
/// method, which hid the cost for today's callers rather than removing it.
///
/// The slot is keyed by the epoch rather than by time: the installed font set changes only when the
/// host installs a font, which no backend can observe, so
/// [`refresh_system_fonts`](crate::platform::fonts::refresh_system_fonts) — documented as the only
/// explicit invalidation path — is what advances the epoch and makes the next call enumerate again.
static CACHE: OnceLock<Mutex<Option<(u64, &'static [SystemFont])>>> = OnceLock::new();

/// The installed families, each with its first style and readable font file.
///
/// Families are de-duplicated case-insensitively: fontconfig lists one line per face (regular,
/// bold, italic…), and a chooser wants one row per family. The first line wins, which is the order
/// fontconfig itself considers canonical.
///
/// The answer is a fresh `Vec` on every call, as the trait contract says, but the underlying
/// enumeration is reused until the epoch changes.
pub(crate) fn system_fonts() -> Vec<SystemFont> {
    let epoch = crate::platform::fonts::enumeration_epoch();
    let slot = CACHE.get_or_init(|| Mutex::new(None));
    {
        let guard = crate::compat::lock(slot);
        if let Some((cached_epoch, fonts)) = *guard {
            if cached_epoch == epoch {
                return fonts.to_vec();
            }
        }
    }
    // Enumerate outside the lock: `fc-list` can take hundreds of milliseconds, and holding the slot
    // across it would block every reader. Two racing first calls both enumerate and only one list
    // is installed; the loser is a bounded, one-time leak of the same kind `platform::fonts`
    // documents.
    let enumerated: &'static [SystemFont] =
        crate::compat::Box::leak(enumerate().into_boxed_slice());
    *crate::compat::lock(slot) = Some((epoch, enumerated));
    enumerated.to_vec()
}

/// Runs `fc-list` once and parses one `SystemFont` per family.
fn enumerate() -> Vec<SystemFont> {
    let output =
        match Command::new("fc-list").args(["--format", "%{family}\t%{style}\t%{file}\n"]).output()
        {
            Ok(output) => output,
            Err(e) => {
                // The binary is absent (or not executable), which is the one case that truthfully
                // means "no enumerator on this host". An empty list is that answer; a fabricated
                // family would be a made-up fact (principle #37).
                log::warn!("[fonts] failed to run fc-list: {e}; reporting no installed fonts");
                return Vec::new();
            }
        };
    if !output.status.success() {
        // A successful spawn already proved fontconfig is installed (principle #38: the judge is
        // whether the client could be launched, not its exit code), so the output it did produce is
        // still a real enumeration. Only warn — discarding it would make a fontconfig that emits a
        // warning on stderr look like a host with no fonts.
        log::warn!("[fonts] fc-list exited with {}; using the output it produced", output.status);
    }
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

    /// `refresh_system_fonts` is the documented invalidation path, so it must advance the revision
    /// the cache compares (P-05). A refresh that left the revision alone would make the explicit
    /// "I just installed a font" request a no-op.
    #[test]
    fn a_refresh_advances_the_enumeration_revision() {
        let before = crate::platform::fonts::enumeration_epoch();
        let _ = crate::platform::fonts::refresh_system_fonts();
        assert!(
            crate::platform::fonts::enumeration_epoch() > before,
            "refresh_system_fonts must invalidate the backend's enumeration cache"
        );
    }
}
