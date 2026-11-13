// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Host theme integration: the accent observer and the system UI face.
//!
//! Both answers are platform facts, so the backend selection lives here rather than in the
//! theme layer: a caller outside `src/platform/` names neither a backend nor an OS
//! (principles #35/#36). The theme layer calls the two semantic entries below and stays free of
//! `cfg(target_os)`.

/// Installs the host's system-accent observer, when the backend has one.
///
/// A backend with no notification source does nothing here, and
/// `theme::refresh_system_accent` stays the documented hook for hosts that learn the accent
/// from their own settings screen.
pub(crate) fn install_system_color_observer() {
    #[cfg(all(target_os = "macos", feature = "cocoa-legacy"))]
    {
        let _ = crate::platform::macos::environment::install_system_color_observer();
    }
    #[cfg(all(target_os = "macos", not(feature = "cocoa-legacy"), feature = "macos"))]
    {
        let _ = crate::platform::macos_objc2::environment::install_system_color_observer();
    }
}

/// Registers the host's system UI face and returns the family name it was registered under.
///
/// `None` is the honest answer on every path where the face is not available: no macOS backend
/// is compiled in, the runtime face registry is off, or the system file is missing. Presets
/// check this before naming a family, so a preset never claims a face the renderer cannot draw
/// (W6-12).
pub(crate) fn register_system_ui_face() -> Option<&'static str> {
    #[cfg(all(target_os = "macos", feature = "cocoa-legacy", feature = "runtime-fonts"))]
    {
        return crate::platform::macos::environment::register_system_ui_face();
    }
    #[cfg(all(
        target_os = "macos",
        not(feature = "cocoa-legacy"),
        feature = "macos",
        feature = "runtime-fonts"
    ))]
    {
        return crate::platform::macos_objc2::native::register_system_ui_face();
    }
    #[allow(unreachable_code)]
    None
}
