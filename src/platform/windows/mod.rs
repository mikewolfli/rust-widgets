// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform backend implementation.

mod notify;
mod platform_impl;
pub mod types;

/// Native surface for self-drawn widgets (Windows).
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`,
/// which this module is built on.
#[cfg(all(target_os = "windows", widgets_unstripped))]
pub(crate) mod canvas;

pub use crate::platform::windows::types::*;

/// The toplevel `HWND` that draws the widget-registry id `id`, if this backend owns one.
///
/// # Why this translation exists, and why it is two hops
///
/// The widget registry and this backend name the same window differently: the registry keys
/// the tree by a **registry** id (the id `WidgetHandle::raw_id` reports), while the backend's
/// handle table is keyed by the **platform** id `Platform::create_window` returned. A caller
/// holding a registry id — `invalidate_surface`, and through it every repaint request — cannot
/// index the handle table with it, and doing so answered `None` for a window the backend had
/// plainly created.
///
/// The association between the two ids is recorded on the widget side, so the first hop asks
/// [`crate::widget::runtime::host_window_for`]. A widget that is **already** a host window id
/// (the painter's case: Win32 hands it the `HWND` it once passed to `create_window`) resolves
/// on the second hop without the first.
///
/// Returning `None` is the honest answer for a widget this backend draws no window for — an
/// ordinary control with no surface of its own, or an id that is not mounted at all.
#[cfg(all(target_os = "windows", widgets_unstripped))]
pub(crate) fn window_hwnd_for_widget_id(
    platform: &types::WindowsPlatform,
    id: crate::core::ObjectId,
) -> Option<winapi::shared::windef::HWND> {
    // The widget registry's id for a window resolves to the platform id this backend created;
    // an id that is already a platform id (an `HWND` Win32 handed back) is tried as itself.
    let platform_id = crate::widget::runtime::host_window_for(id).unwrap_or(id);
    platform.get_native_handle(platform_id)
}

#[cfg(test)]
mod tests;
