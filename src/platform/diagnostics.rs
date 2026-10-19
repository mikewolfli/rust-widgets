// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! One platform-independent diagnostic probe every backend can run.
//!
//! # Why this exists
//!
//! Before BLUE28 only Android had an on-device probe (`nativePlatformProbe`). Every other backend's
//! platform facts could only be judged from source or from cross-compilation, which is the
//! "looks right" evidence principle #1 rejects. This module defines the stable bit order the whole
//! crate shares, drives the real [`Platform`] methods, and reports each fact as **pass**, **skip**
//! (the backend honestly has no such concept) or **fail** (the backend claims one and it did not
//! work).
//!
//! # Stable bit order
//!
//! The order is part of the contract: hosts and CI compare masks across platforms.
//!
//! | bit | fact |
//! |---:|---|
//! | 0 | `clipboard_backend` carries an application-defined MIME type |
//! | 1 | `displays()` returns at least one usable display |
//! | 2 | `display_of_window()` places a real window |
//! | 3 | `display_generation()` is non-zero and stable across two reads |
//! | 4 | `dpi_scale_factor()` is a usable, measured scale |
//! | 5 | `system_theme()` answers `Some(..)` |
//! | 6 | `text_scale()` answers a usable multiplier |
//! | 7 | `safe_area_insets()` answers `Some(..)` |
//! | 8 | `ime_bridge()` is present |
//! | 9 | `accessibility_bridge()` is present |
//! | 10 | the host picker is declared (`choose_path` / `file_dialog`) |
//! | 11 | external opening is declared (`open_external`) |
//!
//! A **skip** is not a failure: a desktop backend that has no OS text-size preference reports
//! `text_scale() == 1.0`, and a state-only backend that cannot name a window reports `None` for
//! safe-area insets. The distinction is recorded per entry so acceptance is "every fact is either
//! measured or named as inapplicable", not "every bit is set".

use crate::compat::format;
use crate::compat::String;

/// The number of diagnostic bits. Part of the published contract.
pub const DIAGNOSTIC_BIT_COUNT: usize = 12;

/// Bit names in their stable order. Index `i` names bit `i`.
pub const DIAGNOSTIC_NAMES: [&str; DIAGNOSTIC_BIT_COUNT] = [
    "clipboard_mime",
    "displays",
    "display_of_window",
    "display_generation",
    "dpi",
    "system_theme",
    "text_scale",
    "safe_area",
    "ime",
    "accessibility",
    "file_dialog",
    "open_external",
];

/// The outcome of one diagnostic fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticStatus {
    /// The backend answered from a real source and the answer was usable.
    Pass,
    /// The backend honestly does not have this concept, or cannot answer from this thread.
    Skip,
    /// The backend claims the capability but the call did not produce a usable answer.
    Fail,
}

impl DiagnosticStatus {
    /// The stable spelling used by the JSON form.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Skip => "skip",
            Self::Fail => "fail",
        }
    }

    /// The stable numeric code: `0` pass, `1` skip, `2` fail.
    pub const fn code(self) -> u8 {
        match self {
            Self::Pass => 0,
            Self::Skip => 1,
            Self::Fail => 2,
        }
    }
}

/// One diagnostic fact and the reason it has its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticEntry {
    /// Bit index, `0..DIAGNOSTIC_BIT_COUNT`.
    pub bit: u8,
    /// Stable name; equal to [`DIAGNOSTIC_NAMES`] at `bit`.
    pub name: &'static str,
    /// Pass, skip or fail.
    pub status: DiagnosticStatus,
    /// A short, static explanation. Empty for a plain pass.
    pub detail: &'static str,
}

impl DiagnosticEntry {
    /// The entry's bit as a mask.
    pub const fn mask(self) -> u32 {
        if matches!(self.status, DiagnosticStatus::Pass) {
            1 << self.bit
        } else {
            0
        }
    }
}

/// The complete result of one probe run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformDiagnostics {
    /// The backend that answered, from [`Platform::backend_name`](crate::platform::Platform::backend_name).
    pub backend: &'static str,
    /// Exactly [`DIAGNOSTIC_BIT_COUNT`] entries, in bit order.
    pub entries: [DiagnosticEntry; DIAGNOSTIC_BIT_COUNT],
}

impl PlatformDiagnostics {
    /// The mask of passed bits, matching the historical `nativePlatformProbe` encoding.
    pub fn mask(&self) -> u32 {
        self.entries.iter().fold(0, |mask, entry| mask | entry.mask())
    }

    /// How many facts passed.
    pub fn passed(&self) -> usize {
        self.entries.iter().filter(|e| e.status == DiagnosticStatus::Pass).count()
    }

    /// How many facts were honestly skipped.
    pub fn skipped(&self) -> usize {
        self.entries.iter().filter(|e| e.status == DiagnosticStatus::Skip).count()
    }

    /// How many facts failed.
    pub fn failed(&self) -> usize {
        self.entries.iter().filter(|e| e.status == DiagnosticStatus::Fail).count()
    }

    /// Whether the run is acceptable: no failures. Skips are named, not failures.
    pub fn is_pass(&self) -> bool {
        self.failed() == 0
    }

    /// The JSON form hosts and CI read when they cannot call the C ABI.
    ///
    /// The shape is deliberately flat and stable:
    /// `{"backend":"…","mask":N,"passed":N,"skipped":N,"failed":N,"entries":[{"bit":0,"name":"…","status":"pass","detail":"…"},…]}`
    pub fn to_json(&self) -> String {
        let mut out = format!(
            "{{\"backend\":\"{}\",\"mask\":{},\"passed\":{},\"skipped\":{},\"failed\":{},\"entries\":[",
            escape_json(self.backend),
            self.mask(),
            self.passed(),
            self.skipped(),
            self.failed()
        );
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"bit\":{},\"name\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
                entry.bit,
                escape_json(entry.name),
                entry.status.as_str(),
                escape_json(entry.detail)
            ));
        }
        out.push_str("]}");
        out
    }

    /// A one-line summary suitable for a log.
    pub fn summary(&self) -> String {
        format!(
            "{}: passed {}/{} skipped {} failed {} (mask 0x{:03x})",
            self.backend,
            self.passed(),
            DIAGNOSTIC_BIT_COUNT,
            self.skipped(),
            self.failed(),
            self.mask()
        )
    }
}

/// Escapes the characters JSON requires to be escaped in a string value.
///
/// The inputs are backend names and static details written in this crate, so this covers the
/// general case rather than trusting the inputs: a backend name is still host-controlled in a
/// downstream build.
fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Builds one entry, taking the name from the stable table so the two cannot drift.
fn entry(bit: usize, status: DiagnosticStatus, detail: &'static str) -> DiagnosticEntry {
    DiagnosticEntry { bit: bit as u8, name: DIAGNOSTIC_NAMES[bit], status, detail }
}

/// Runs every diagnostic fact against the active backend.
///
/// This drives the real platform methods; it is not a capability-flag summary. The only side
/// effects are (a) a custom-MIME clipboard entry, restored when the clipboard had prior content,
/// and (b) a short-lived 64×64 probe window when display ownership can be checked. Both are the
/// same operations the Android probe has always performed.
pub fn run() -> PlatformDiagnostics {
    let platform = crate::platform::get_platform();
    let capabilities = platform.capabilities();
    let mut entries = [entry(0, DiagnosticStatus::Skip, ""); DIAGNOSTIC_BIT_COUNT];

    // 0 — clipboard carrying an application-defined MIME type.
    entries[0] = match platform.clipboard_backend() {
        None if capabilities.clipboard => entry(
            0,
            DiagnosticStatus::Fail,
            "capabilities().clipboard is true but clipboard_backend() returned None",
        ),
        None => entry(0, DiagnosticStatus::Skip, "no rich clipboard backend"),
        Some(clipboard) => {
            const PROBE_TYPE: &str = "application/vnd.rust-widgets.probe+json";
            const PROBE_BYTES: &[u8] = b"{\"probe\":true,\"bytes\":\"\x00\xff\x80\"}";
            let previous = clipboard.get_contents();
            let written = clipboard.set_contents_with_type(PROBE_TYPE, PROBE_BYTES);
            let readable = written
                && clipboard.has_format(PROBE_TYPE)
                && matches!(clipboard.get_contents_for_type(PROBE_TYPE), Some(bytes) if bytes == PROBE_BYTES);
            if let Some(previous) = previous {
                if !clipboard.set_contents(previous) {
                    log::warn!("[diagnostics] could not restore the previous clipboard contents");
                }
            }
            if readable {
                entry(0, DiagnosticStatus::Pass, "")
            } else if written {
                entry(0, DiagnosticStatus::Fail, "custom MIME write succeeded but read did not")
            } else {
                entry(0, DiagnosticStatus::Fail, "custom MIME write was refused")
            }
        }
    };

    // 1 — displays.
    let displays = platform.displays();
    entries[1] = if displays.is_empty() {
        entry(1, DiagnosticStatus::Skip, "backend reports no displays")
    } else if displays.iter().all(|d| d.scale > 0.0 && d.bounds.width > 0 && d.bounds.height > 0) {
        entry(1, DiagnosticStatus::Pass, "")
    } else {
        entry(1, DiagnosticStatus::Fail, "a display has a non-positive scale or bounds")
    };

    // One short-lived window serves both the ownership and safe-area checks. A backend that
    // cannot create one skips both facts instead of pretending an unknown id is a window.
    let probe_window = crate::create_window("Platform Diagnostics", 0, 0, 64, 64);

    // 2 — window ownership. Only meaningful once displays are enumerated.
    entries[2] = if displays.is_empty() {
        entry(2, DiagnosticStatus::Skip, "no displays to place a window on")
    } else if probe_window == 0 {
        entry(2, DiagnosticStatus::Skip, "create_window returned 0")
    } else {
        match platform.display_of_window(probe_window) {
            Some(id) if displays.iter().any(|d| d.id == id) => entry(2, DiagnosticStatus::Pass, ""),
            Some(_) => entry(2, DiagnosticStatus::Fail, "answered a display id not in displays()"),
            None => entry(2, DiagnosticStatus::Fail, "display_of_window returned None"),
        }
    };

    // 3 — display generation.
    entries[3] = if displays.is_empty() {
        entry(3, DiagnosticStatus::Skip, "backend reports no displays")
    } else {
        let first = platform.display_generation();
        let second = platform.display_generation();
        if first != 0 && first == second {
            entry(3, DiagnosticStatus::Pass, "")
        } else if first == 0 {
            entry(3, DiagnosticStatus::Fail, "display_generation returned 0")
        } else {
            entry(
                3,
                DiagnosticStatus::Fail,
                "display_generation changed between two immediate reads",
            )
        }
    };

    // 4 — DPI.
    let dpi = platform.dpi_scale_factor();
    entries[4] = if !capabilities.dpi_scaling {
        entry(4, DiagnosticStatus::Skip, "backend does not declare dpi_scaling")
    } else if dpi.is_finite() && dpi > 0.0 {
        entry(4, DiagnosticStatus::Pass, "")
    } else {
        entry(4, DiagnosticStatus::Fail, "dpi_scale_factor is not a positive finite number")
    };

    // 5 — system theme.
    entries[5] = match platform.system_theme() {
        Some(_) => entry(5, DiagnosticStatus::Pass, ""),
        None => entry(5, DiagnosticStatus::Skip, "no system appearance source on this backend"),
    };

    // 6 — text scale. `1.0` is a real answer on a platform with no preference.
    let text_scale = platform.text_scale();
    entries[6] = if text_scale.is_finite() && text_scale > 0.0 {
        entry(6, DiagnosticStatus::Pass, "")
    } else {
        entry(6, DiagnosticStatus::Fail, "text_scale is not a positive finite number")
    };

    // 7 — safe area, read from the same real window as the display check.
    entries[7] = if probe_window == 0 {
        entry(7, DiagnosticStatus::Skip, "no probe window to read insets from")
    } else {
        match platform.safe_area_insets(probe_window) {
            Some(_) => entry(7, DiagnosticStatus::Pass, ""),
            None => entry(7, DiagnosticStatus::Skip, "backend cannot read safe-area insets"),
        }
    };

    if probe_window != 0 {
        let _ = crate::control_backend::get_control_backend().destroy_widget(probe_window);
    }

    // 8 — IME.
    entries[8] = match platform.ime_bridge() {
        Some(_) => entry(8, DiagnosticStatus::Pass, ""),
        None => entry(8, DiagnosticStatus::Skip, "no IME bridge"),
    };

    // 9 — accessibility.
    entries[9] = match platform.accessibility_bridge() {
        Some(_) => entry(9, DiagnosticStatus::Pass, ""),
        None => entry(9, DiagnosticStatus::Skip, "no accessibility bridge"),
    };

    // 10 — host path picker.
    entries[10] = if capabilities.file_dialog {
        entry(10, DiagnosticStatus::Pass, "")
    } else {
        entry(10, DiagnosticStatus::Skip, "no host path picker")
    };

    // 11 — external opening.
    entries[11] = if capabilities.open_external {
        entry(11, DiagnosticStatus::Pass, "")
    } else {
        entry(11, DiagnosticStatus::Skip, "no external handler")
    };

    PlatformDiagnostics { backend: platform.backend_name(), entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_names_are_stable_and_unique() {
        assert_eq!(DIAGNOSTIC_NAMES.len(), DIAGNOSTIC_BIT_COUNT);
        let mut seen = [false; DIAGNOSTIC_BIT_COUNT];
        for name in DIAGNOSTIC_NAMES {
            assert!(!name.is_empty());
            let index = DIAGNOSTIC_NAMES.iter().position(|n| *n == name).unwrap();
            assert!(!seen[index], "duplicate diagnostic name: {name}");
            seen[index] = true;
        }
    }

    #[test]
    fn mask_counts_only_passes() {
        let entries = core::array::from_fn(|bit| entry(bit, DiagnosticStatus::Pass, ""));
        let result = PlatformDiagnostics { backend: "test", entries };
        assert_eq!(result.mask(), (1 << DIAGNOSTIC_BIT_COUNT) - 1);
        assert_eq!(result.passed(), DIAGNOSTIC_BIT_COUNT);
        assert!(result.is_pass());
    }

    #[test]
    fn skipped_facts_do_not_fail_a_run() {
        let mut entries = core::array::from_fn(|bit| entry(bit, DiagnosticStatus::Pass, ""));
        entries[3] = entry(3, DiagnosticStatus::Skip, "not applicable");
        let result = PlatformDiagnostics { backend: "test", entries };
        assert!(result.is_pass(), "a named skip must not count as failure");
        assert_eq!(result.skipped(), 1);
        assert_eq!(result.mask() & (1 << 3), 0);
    }

    #[test]
    fn failed_facts_fail_a_run() {
        let mut entries = core::array::from_fn(|bit| entry(bit, DiagnosticStatus::Pass, ""));
        entries[7] = entry(7, DiagnosticStatus::Fail, "returned a bad value");
        let result = PlatformDiagnostics { backend: "test", entries };
        assert!(!result.is_pass());
        assert_eq!(result.failed(), 1);
    }

    #[test]
    fn json_escapes_quotes_and_reports_every_entry() {
        let entries = core::array::from_fn(|bit| {
            entry(bit, DiagnosticStatus::Skip, "a \"quoted\" \\ reason")
        });
        let result = PlatformDiagnostics { backend: "backend \"x\"", entries };
        let json = result.to_json();
        assert!(json.starts_with("{\"backend\":\"backend \\\"x\\\"\""));
        assert!(json.contains("\"mask\":0"));
        assert_eq!(json.matches("\"name\":").count(), DIAGNOSTIC_BIT_COUNT);
        assert!(json.contains("a \\\"quoted\\\" \\\\ reason"));
        assert!(json.ends_with("]}"));
    }

    /// The live probe is exercised by `examples/platform_diagnostics`, not here.
    ///
    /// A unit test runs on a worker thread; touching `NSPasteboard` or creating a window from
    /// there is outside the platform's documented thread rule and can block. The harness is the
    /// place that runs the real probe with a real main-thread session.
    #[test]
    fn every_entry_reports_its_own_bit_and_name() {
        let entries = core::array::from_fn(|bit| entry(bit, DiagnosticStatus::Pass, ""));
        let result = PlatformDiagnostics { backend: "test", entries };
        for (index, item) in result.entries.iter().enumerate() {
            assert_eq!(item.bit as usize, index);
            assert_eq!(item.name, DIAGNOSTIC_NAMES[index]);
        }
    }
}
