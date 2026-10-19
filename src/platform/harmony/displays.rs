// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display enumeration for the OpenHarmony backend, over `libnative_display_manager`.
//!
//! # What this closes
//!
//! [`Platform::displays`](crate::platform::Platform::displays),
//! [`Platform::display_of_window`](crate::platform::Platform::display_of_window) and
//! [`Platform::display_generation`](crate::platform::Platform::display_generation) were left at
//! the trait's honest defaults on this backend: no list, no window association, no revision. The
//! OpenHarmony NDK publishes all three facts (`OH_NativeDisplayManager_CreateAllDisplays`,
//! `GetDisplayPosition` and `GetDefaultDisplayId`), and the SDK ships the header and the library,
//! so this module is the thin translation from those calls to this crate's [`Display`].
//!
//! # Why one module rather than inside `platform_impl`
//!
//! The FFI is target-only: on a host build the declarations do not exist and linking them would
//! fail. Keeping the declarations and the stand-in in one file is the same shape the pasteboard
//! module uses, and it keeps `platform_impl` free of native types (principle #39).
//!
//! # Availability is a build fact, not a runtime probe
//!
//! `CreateAllDisplays` is API 14. This module is compiled only for `target_env = "ohos"`, and the
//! SDK's own sysroot is what supplies the symbol; a build for an older capability fails to link
//! rather than silently answering an empty list. On a host build the stand-in answers the honest
//! absence, which is what the non-ohos `cargo check` job compiles.

use crate::compat::Vec;
use crate::platform::types::Display;

#[cfg(target_env = "ohos")]
mod sys {
    use super::Display;
    use crate::compat::Vec;
    use crate::core::Rect;
    use crate::platform::types::DisplayId;
    use core::ffi::{c_char, c_void};

    /// `DISPLAY_MANAGER_OK`.
    const DISPLAY_MANAGER_OK: i32 = 0;
    /// `OH_DISPLAY_NAME_LENGTH` from `oh_display_info.h`.
    const DISPLAY_NAME_LENGTH: usize = 32;

    /// `NativeDisplayManager_DisplayInfo` from `oh_display_info.h`.
    ///
    /// `bool` is spelled `u8`: a Rust `bool` has one valid bit pattern pair, and reading an
    /// arbitrary C byte into it would be undefined for a value outside `0..=1`. The field is only
    /// compared to zero.
    #[repr(C)]
    struct NativeDisplayManager_DisplayInfo {
        id: u32,
        name: [c_char; DISPLAY_NAME_LENGTH + 1],
        is_alive: u8,
        width: i32,
        height: i32,
        physical_width: i32,
        physical_height: i32,
        refresh_rate: u32,
        available_width: u32,
        available_height: u32,
        density_dpi: f32,
        density_pixels: f32,
        scaled_density: f32,
        x_dpi: f32,
        y_dpi: f32,
        rotation: i32,
        state: i32,
        orientation: i32,
        hdr_format: *mut c_void,
        color_space: *mut c_void,
    }

    /// `NativeDisplayManager_DisplaysInfo` from `oh_display_info.h`.
    #[repr(C)]
    struct NativeDisplayManager_DisplaysInfo {
        displays_length: u32,
        displays_info: *mut NativeDisplayManager_DisplayInfo,
    }

    extern "C" {
        fn OH_NativeDisplayManager_CreateAllDisplays(
            all_displays: *mut *mut NativeDisplayManager_DisplaysInfo,
        ) -> i32;
        fn OH_NativeDisplayManager_DestroyAllDisplays(
            all_displays: *mut NativeDisplayManager_DisplaysInfo,
        );
        fn OH_NativeDisplayManager_GetDisplayPosition(
            display_id: u64,
            x: *mut i32,
            y: *mut i32,
        ) -> i32;
        fn OH_NativeDisplayManager_GetDefaultDisplayId(display_id: *mut u64) -> i32;
    }

    /// The displays the display manager reports, in its own order.
    pub(super) fn displays() -> Vec<Display> {
        let mut all: *mut NativeDisplayManager_DisplaysInfo = core::ptr::null_mut();
        let status = unsafe { OH_NativeDisplayManager_CreateAllDisplays(&mut all) };
        if status != DISPLAY_MANAGER_OK || all.is_null() {
            log::warn!(
                "[harmony displays] OH_NativeDisplayManager_CreateAllDisplays answered {status}"
            );
            return Vec::new();
        }

        let mut default_id = 0u64;
        let has_default = unsafe { OH_NativeDisplayManager_GetDefaultDisplayId(&mut default_id) }
            == DISPLAY_MANAGER_OK;

        let mut out = Vec::new();
        let count = unsafe { (*all).displays_length as usize };
        let first = unsafe { (*all).displays_info };
        if !first.is_null() {
            for index in 0..count {
                let info = unsafe { &*first.add(index) };
                if info.is_alive == 0 {
                    continue;
                }
                let width = info.width.max(0) as u32;
                let height = info.height.max(0) as u32;
                // The position is a separate call: `DisplayInfo` has no origin field. A failure
                // is not fatal — a primary display starts at (0, 0), which is the only sensible
                // fallback and is what the platform reports for it.
                let mut x = 0i32;
                let mut y = 0i32;
                let positioned = unsafe {
                    OH_NativeDisplayManager_GetDisplayPosition(info.id as u64, &mut x, &mut y)
                } == DISPLAY_MANAGER_OK;
                let origin_x = if positioned { x } else { 0 };
                let origin_y = if positioned { y } else { 0 };
                // The available area is what a window may occupy (system bars excluded). A zero
                // reading means the platform did not fill it in; falling back to the full bounds
                // keeps a window from reconciling against an empty work area.
                let work_width =
                    if info.available_width > 0 { info.available_width } else { width };
                let work_height =
                    if info.available_height > 0 { info.available_height } else { height };
                out.push(Display {
                    id: DisplayId::new(info.id as u64),
                    bounds: Rect::new(origin_x, origin_y, width, height),
                    work_area: Rect::new(origin_x, origin_y, work_width, work_height),
                    // `densityPixels` is the logical scale (1.0 at the baseline density), the
                    // value the layout layer expects. A non-positive reading is not a scale.
                    scale: if info.density_pixels > 0.0 { info.density_pixels } else { 1.0 },
                    is_primary: if has_default { info.id as u64 == default_id } else { index == 0 },
                });
            }
        }
        unsafe { OH_NativeDisplayManager_DestroyAllDisplays(all) };
        out
    }

    /// A revision of the display configuration.
    ///
    /// Derived from the same fields [`displays`] returns with FNV-1a, so a host that compares two
    /// readings re-reads the list exactly when a display was added/removed or its geometry, work
    /// area, scale or primary flag changed.
    pub(super) fn generation() -> u64 {
        const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
        let displays = displays();
        if displays.is_empty() {
            return 0;
        }
        let mut hash = FNV_OFFSET;
        for display in displays {
            for part in [
                display.id.get(),
                display.bounds.x as i64 as u64,
                display.bounds.y as i64 as u64,
                display.bounds.width as u64,
                display.bounds.height as u64,
                display.work_area.x as i64 as u64,
                display.work_area.y as i64 as u64,
                display.work_area.width as u64,
                display.work_area.height as u64,
                display.scale.to_bits() as u64,
                display.is_primary as u64,
            ] {
                hash ^= part;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        }
        hash
    }
}

#[cfg(not(target_env = "ohos"))]
mod sys {
    //! Host stand-in: the same surface, with nothing behind it.

    use super::Display;
    use crate::compat::Vec;

    /// A host has no OpenHarmony display manager.
    pub(super) fn displays() -> Vec<Display> {
        Vec::new()
    }

    /// No configuration is observable here.
    pub(super) fn generation() -> u64 {
        0
    }
}

/// The displays this host can place windows on; see [`Platform::displays`].
///
/// [`Platform::displays`]: crate::platform::Platform::displays
pub(crate) fn displays() -> Vec<Display> {
    sys::displays()
}

/// A value that changes when the display set changes; see
/// [`Platform::display_generation`].
///
/// [`Platform::display_generation`]: crate::platform::Platform::display_generation
pub(crate) fn generation() -> u64 {
    sys::generation()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A host build must answer the honest absence, not a made-up screen.
    #[test]
    fn a_host_without_the_display_manager_reports_no_displays() {
        if cfg!(target_env = "ohos") {
            return;
        }
        assert!(displays().is_empty());
        assert_eq!(generation(), 0);
    }
}
