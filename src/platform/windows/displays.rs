// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Display (monitor) enumeration for the Windows backend, over Win32's monitor API.
//!
//! # What this closes
//!
//! [`Platform::displays`](crate::platform::Platform::displays) had no Windows implementation, so
//! the backend answered the trait's honest-but-empty default: a host could not enumerate monitors,
//! ask which one a window was on, or learn that the set changed (see
//! `docs/issues/feedback-20261015.md` §十二, and §5.1/§9 of `docs/log/log-20261016-1.md`, which
//! named Windows as the one desktop backend still missing this). Win32 already knows all three
//! facts — `EnumDisplayMonitors` walks the monitors, `GetMonitorInfoW` answers each one's bounds,
//! work area and primary flag, and `MonitorFromWindow` names the monitor a window is on — so this
//! module is the translation from those to this crate's own [`Display`]. It probes nothing itself.
//!
//! # The scale, and what it is measured against
//!
//! `scale` is the monitor's `LOGPIXELSX / 96`, read from a display DC created with the monitor's
//! own device name (`\\.\DISPLAY1`). This is the same measurement [`Platform::dpi_scale_factor`]
//! already makes for the primary monitor via `GetDC(NULL)`, generalised to every monitor by asking
//! Win32 for that monitor's DC instead of the screen DC. It reports the value the process is
//! configured for: a process that is not per-monitor DPI aware sees the system DPI on every
//! monitor, exactly as `dpi_scale_factor` does, and a per-monitor-aware process sees each monitor's
//! own scale. That is the honest answer for whichever DPI-awareness mode the host chose.
//!
//! # The id
//!
//! [`DisplayId`] is an FNV-1a hash of the monitor's device name (`\\.\DISPLAY1`). The device name
//! is stable while a monitor stays attached and is what [`display_of_window_handle`] can
//! re-derive from a window's monitor, so the two readings agree without persisting a handle.
//! `HMONITOR` handles are **not** used: Win32 may hand out a different handle for the same physical
//! monitor, and a host comparing ids across a hot-plug would then see two different displays.
//!
//! # Threading
//!
//! `EnumDisplayMonitors` / `GetMonitorInfoW` / `MonitorFromWindow` are callable from any thread —
//! unlike GDK or AppKit, Win32 does not bind them to a UI thread — so, like the other Win32
//! queries in this backend, no main-thread guard is needed.

use crate::compat::{String, Vec};
use crate::core::Rect;
use crate::platform::types::{Display, DisplayId};
use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, HWND, LPRECT, RECT};
use winapi::um::wingdi::{CreateDCW, DeleteDC, GetDeviceCaps, LOGPIXELSX};
use winapi::um::winuser::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITORINFOEXW,
    MONITORINFOF_PRIMARY, MONITOR_DEFAULTTONEAREST,
};

/// FNV-1a 64-bit parameters, the hash used for a stable [`DisplayId`].
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Converts a Win32 rectangle to this crate's [`Rect`].
///
/// `RECT` holds `left`/`top`/`right`/`bottom` edges, so the origin transfers directly and each
/// extent is the difference. A negative extent cannot occur for a real monitor but is clamped so a
/// `u32` width is never produced from one.
fn rect_of(rect: &RECT) -> Rect {
    Rect::new(
        rect.left,
        rect.top,
        (rect.right - rect.left).max(0) as u32,
        (rect.bottom - rect.top).max(0) as u32,
    )
}

/// The device name a `MONITORINFOEXW` carries, decoded from its fixed-width UTF-16 buffer.
fn device_name(raw: &[u16]) -> String {
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..len])
}

/// A stable [`DisplayId`] for a monitor's device name.
///
/// See the module docs for why the device name — not an `HMONITOR` — is what is hashed.
fn display_id(device: &str) -> DisplayId {
    let mut hash = FNV_OFFSET;
    for byte in device.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    DisplayId::new(hash)
}

/// The scale factor for a monitor's device, from its display DC's `LOGPIXELSX`.
///
/// A display DC that cannot be created, or a `LOGPIXELSX` of zero, answers `1.0` rather than
/// dividing by zero — the same clamp `Platform::dpi_scale_factor` applies.
///
/// # Safety
///
/// Calls Win32; no Rust invariants beyond those of `CreateDCW`/`GetDeviceCaps` are relied on, and
/// the DC created here is released before returning.
unsafe fn scale_factor_of(device: &str) -> f32 {
    use std::os::windows::ffi::OsStrExt;
    let name: Vec<u16> =
        std::ffi::OsStr::new(device).encode_wide().chain(std::iter::once(0)).collect();
    let hdc = CreateDCW(std::ptr::null(), name.as_ptr(), std::ptr::null(), std::ptr::null());
    if hdc.is_null() {
        return 1.0;
    }
    let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
    DeleteDC(hdc);
    if dpi > 0 {
        dpi as f32 / 96.0
    } else {
        1.0
    }
}

/// Reads one monitor's [`Display`], or `None` when Win32 cannot answer for that handle.
///
/// # Safety
///
/// `monitor` must be a handle Win32 produced (from `EnumDisplayMonitors` or `MonitorFromWindow`).
unsafe fn display_of_monitor(monitor: HMONITOR) -> Option<Display> {
    let mut info: MONITORINFOEXW = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // `MONITORINFOEXW` begins with a `MONITORINFO`; `GetMonitorInfoW` fills that prefix and, when
    // sized for the extended form, also the device name that follows it.
    let as_base = &mut info as *mut MONITORINFOEXW as *mut MONITORINFO;
    if GetMonitorInfoW(monitor, as_base) == 0 {
        return None;
    }
    let device = device_name(&info.szDevice);
    Some(Display {
        id: display_id(&device),
        bounds: rect_of(&info.rcMonitor),
        work_area: rect_of(&info.rcWork),
        scale: scale_factor_of(&device),
        is_primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

/// The `EnumDisplayMonitors` callback that appends each monitor to the collector it is handed.
///
/// # Safety
///
/// `data` must be the `*mut Vec<Display>` passed by [`displays`], which lives for the whole
/// synchronous enumeration.
unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: LPRECT,
    data: LPARAM,
) -> BOOL {
    if let Some(display) = display_of_monitor(monitor) {
        (*(data as *mut Vec<Display>)).push(display);
    }
    // Returning zero would abort the enumeration; every monitor must be visited.
    TRUE
}

/// The displays Win32 reports, in `EnumDisplayMonitors` order.
///
/// An empty list is returned when the enumeration itself fails (a session with no window station,
/// or a host that cannot reach one), which is the honest "no displays are observable from here".
pub(crate) fn displays() -> Vec<Display> {
    let mut out: Vec<Display> = Vec::new();
    let data = &mut out as *mut Vec<Display> as LPARAM;
    // SAFETY: `data` points at `out`, which outlives the call; the callback only pushes into it.
    let ok = unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(collect_monitor), data)
    };
    if ok == 0 {
        log::debug!(
            "[windows] EnumDisplayMonitors failed; no displays are observable from this session"
        );
    }
    out
}

/// The id of the display a window is on, or `None` when the handle names no window.
///
/// `MonitorFromWindow` with `MONITOR_DEFAULTTONEAREST` is Win32's own answer to "which monitor is
/// this window on", matching a window that intersects no monitor to the nearest one — the same
/// nearest-monitor semantics GDK's `monitor_at_point` gives the Linux backend. The id is read back
/// from that monitor's device name, so it is one of the ids [`displays`] publishes.
pub(crate) fn display_of_window_handle(hwnd: HWND) -> Option<DisplayId> {
    if hwnd.is_null() {
        return None;
    }
    // SAFETY: `hwnd` is non-null; `MonitorFromWindow` only reads the window's placement.
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        display_of_monitor(monitor).map(|display| display.id)
    }
}

/// A revision that changes when the display set (or a display's geometry/scale) changes.
///
/// Derived from the current configuration with FNV-1a, so a host compares consecutive readings
/// instead of subscribing to a notification it cannot receive on a thread it does not own. `0` is
/// reserved by the trait contract for "this backend does not report display changes", which is the
/// honest answer when no monitor is observable at all (a session with no window station, or a
/// failed enumeration) — a non-zero hash of the empty set would claim an observation that was not
/// made.
pub(crate) fn generation() -> u64 {
    let displays = displays();
    if displays.is_empty() {
        return 0;
    }
    let mut hash = FNV_OFFSET;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(FNV_PRIME);
    };
    mix(displays.len() as u64);
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
            mix(part);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pure function over a Win32 buffer: the device name is whatever the monitor reported, and
    /// the id is a stable hash of it. This needs no session, so it runs anywhere this module
    /// compiles.
    #[test]
    fn a_device_name_is_decoded_up_to_its_nul() {
        let mut raw = [0u16; 32];
        for (slot, unit) in raw.iter_mut().zip("\\\\.\\DISPLAY1".encode_utf16()) {
            *slot = unit;
        }
        assert_eq!(device_name(&raw), "\\\\.\\DISPLAY1");
    }

    /// The id is a function of the name, so the same monitor names the same id and two monitors do
    /// not collide in the ordinary case.
    #[test]
    fn the_id_is_stable_and_distinct_per_device_name() {
        assert_eq!(display_id("\\\\.\\DISPLAY1"), display_id("\\\\.\\DISPLAY1"));
        assert_ne!(display_id("\\\\.\\DISPLAY1"), display_id("\\\\.\\DISPLAY2"));
    }

    /// The clamped rectangle never produces a `u32` from a negative extent.
    #[test]
    fn a_degenerate_rect_clamps_to_zero_extent() {
        let rect = RECT { left: 10, top: 20, right: 5, bottom: 4 };
        assert_eq!(rect_of(&rect), Rect::new(10, 20, 0, 0));
    }
}
