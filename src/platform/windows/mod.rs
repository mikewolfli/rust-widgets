// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Windows platform backend implementation.

/// Display (monitor) enumeration, over Win32's monitor API.
mod displays;
/// The Win32 common file dialogs, behind
/// [`Platform::choose_path`](crate::platform::Platform::choose_path).
///
/// # Which dialog, and why "choose a directory" is a different one
///
/// `GetOpenFileNameW`/`GetSaveFileNameW` (comdlg32) are the file dialogs: the places sidebar, the
/// filter list, and a **path** answer. Folder selection is not one of their modes on every Windows
/// this crate supports — `OFN_PICKFOLDERS` is a Vista-era flag that older comdlg32 ignores — so the
/// directory case uses `SHBrowseForFolderW`, the shell's own folder browser, which has existed
/// since Windows 95 and answers with an item id list rather than a path. That is the same split
/// the macOS backend makes with `NSSavePanel`/`NSOpenPanel`, for the same reason: the platform's
/// own vocabulary differs per purpose.
///
/// # Why this is its own file
///
/// The body is `winapi` calls, and a host without an MSVC toolchain cannot compile this crate for
/// `x86_64-pc-windows-msvc` (a C dependency in the graph needs a Windows SDK). Keeping the body
/// free of crate-internal names — it depends on `winapi` and on the two types
/// `super::super::{PathPurpose, PathPickerError}` and nothing else — is what lets the same file be
/// compiled on its own for the target (see `docs/log/log-20261014-1.md`, probe P-2), so the code is
/// verified by a compiler rather than by review.
pub(crate) mod file_picker;
mod notify;
mod platform_impl;
pub mod types;

/// Native surface for self-drawn widgets (Windows).
///
/// Compiled out for `mini`/`embedded`: those profiles have no `widget::runtime`,
/// which this module is built on.
#[cfg(all(target_os = "windows", widgets_unstripped))]
pub(crate) mod canvas;
#[cfg(all(target_os = "windows", widgets_unstripped))]
mod hwnd;

pub use crate::platform::windows::types::*;

#[cfg(all(target_os = "windows", widgets_unstripped))]
pub(crate) use hwnd::window_hwnd_for_widget_id;

#[cfg(test)]
mod tests;
