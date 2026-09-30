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
///
/// # Why the fallback is guarded rather than `unwrap_or(id)`
///
/// The second hop exists for exactly one caller shape: the painter, which Win32 hands an
/// `HWND` it once passed to `create_window`. That caller re-derives the id from the handle
/// (`widget_id_by_native_handle`), so the fallback is needed — but *only* for an id the
/// backend actually minted.
///
/// It used to fall back unconditionally:
///
/// ```ignore
/// let platform_id = crate::widget::runtime::host_window_for(id).unwrap_or(id);
/// ```
///
/// The handle table is **shared** between toplevels and canvas child windows
/// (`bind_native_handle` records both), so an unassociated registry id that happened to
/// equal a live canvas's platform id resolved to *that canvas's* `HWND`. `invalidate_surface`
/// then returned `true` having invalidated a different window, and the window that needed
/// the repaint never received one — a blank window reporting success at every level, which
/// is the failure mode this whole translation exists to remove.
///
/// Both id spaces are allocated from small integers, so a collision is ordinary rather than
/// exotic. The fallback is therefore taken only when the backend can confirm the id is one
/// of its own (`state.kind_of(id)` answers for every widget `create_*` minted), and anything
/// else is reported instead of silently resolving to a stranger.
#[cfg(all(target_os = "windows", widgets_unstripped))]
pub(crate) fn window_hwnd_for_widget_id(
    platform: &types::WindowsPlatform,
    id: crate::core::ObjectId,
) -> Option<winapi::shared::windef::HWND> {
    if let Some(platform_id) = crate::widget::runtime::host_window_for(id) {
        return platform.get_native_handle(platform_id);
    }
    // No association: the id may be the backend's own (the painter's case). Confirm that
    // before using it as a handle-table key, so an unrelated registry id cannot resolve to
    // whichever window happens to share its number.
    if platform.state.kind_of(id).is_none() {
        log::debug!(
            "[windows] widget {id} has no host window and is not a widget of this backend, \
             so it has no HWND to invalidate"
        );
        return None;
    }
    platform.get_native_handle(id)
}

#[cfg(test)]
mod tests;
