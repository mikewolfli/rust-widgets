// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::super::{PathPickerError, PathPurpose};
// `String`/`Vec` come from the compat bridge rather than the prelude: the `mini` profile is
// `no_std`, so the std prelude that normally supplies them is suppressed and this module failed to
// compile for `--target x86_64-pc-windows-msvc --features mini` with `cannot find type Vec` /
// `cannot find type String` (the same split `platform_impl.rs` records).
use crate::compat::{String, Vec};
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

/// `MAX_PATH` no longer bounds what a dialog may return, and a buffer sized for it would truncate
/// a long path instead of reporting it. This is the length `\\?\`-prefixed paths are documented
/// to reach.
const PATH_BUFFER_LEN: usize = 4096;

/// The filter list the dialog shows, as comdlg32 wants it: pairs of `label\0pattern\0`, terminated
/// by an empty pattern.
const FILTER_ALL_FILES: &str = "All files\0*.*\0\0";

/// Presents the Win32 picker and answers with what the user chose.
///
/// # Cancel versus failure
///
/// `GetOpenFileNameW` returning zero means "nothing was chosen", which covers both the user
/// pressing Cancel and the dialog failing to appear. `CommDlgExtendedError` is what tells them
/// apart: it is documented to return zero for the Cancel case, so a non-zero code is reported as
/// [`PathPickerError::Unavailable`] rather than silently as a cancellation — the merge the
/// clipboard read was fixed for.
///
/// # What is not claimed
///
/// The returned path is not checked for existence or writability. `OFN_FILEMUSTEXIST` makes the
/// dialog itself refuse a name that does not exist for the open case; a save dialog legitimately
/// names a new file, and whether this process may create it is what the caller's own `create`
/// reports.
pub(crate) fn choose_path(
    purpose: PathPurpose,
    title: &str,
) -> Result<Option<PathBuf>, PathPickerError> {
    match purpose {
        PathPurpose::SelectDirectory => browse_for_folder(title),
        PathPurpose::OpenFile | PathPurpose::SaveFile => file_dialog(purpose, title),
    }
}

/// The open/save dialog, sharing everything except the two flags that differ.
fn file_dialog(purpose: PathPurpose, title: &str) -> Result<Option<PathBuf>, PathPickerError> {
    use winapi::um::commdlg::{
        CommDlgExtendedError, GetOpenFileNameW, GetSaveFileNameW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
        OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };

    let saving = matches!(purpose, PathPurpose::SaveFile);
    let mut buffer = [0u16; PATH_BUFFER_LEN];
    let title_wide: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain(Some(0)).collect();
    let filter_wide: Vec<u16> = FILTER_ALL_FILES.encode_utf16().collect();

    // SAFETY: `OPENFILENAMEW` is a C struct of plain integers, pointers and `u32` flags, for which
    // an all-zero value is the documented "unset" state; every pointer stored below points into a
    // local that outlives the call, and `lStructSize` is set to the struct the API expects.
    let mut ofn: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.lpstrFilter = filter_wide.as_ptr();
    ofn.lpstrTitle = title_wide.as_ptr();
    ofn.lpstrFile = buffer.as_mut_ptr();
    ofn.nMaxFile = PATH_BUFFER_LEN as u32;
    ofn.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        // The one difference between the two purposes: an open dialog must name something that
        // exists, a save dialog must be allowed to name something that does not, and must ask
        // before replacing something that does.
        | if saving { OFN_OVERWRITEPROMPT } else { OFN_FILEMUSTEXIST };

    // SAFETY: `ofn` is fully initialised above and lives until after the call.
    let accepted = unsafe {
        if saving {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        }
    };
    if accepted == 0 {
        // SAFETY: no arguments; the function reads the calling thread's last dialog error.
        let extended = unsafe { CommDlgExtendedError() };
        return if extended == 0 { Ok(None) } else { Err(PathPickerError::Unavailable) };
    }
    Ok(Some(path_from_wide_buffer(&buffer)))
}

/// The shell's folder browser, which answers with an item id list rather than a path.
///
/// # Why this one is declared here rather than imported
///
/// `winapi` 0.3 declares `SHGetPathFromIDListW` (used below) but not the browser itself or its
/// argument struct, and it exposes no `OFN_PICKFOLDERS` equivalent for the file dialogs — so this
/// is the only folder picker reachable from the crate's existing dependency set. The declaration
/// is the same move `src/platform/android_jni.rs` makes for the one log function the JNI binding
/// does not expose: the smallest FFI surface that answers the question, in the platform layer
/// where OS knowledge belongs (principles #36/#40), and compiled for the target by the probe in
/// `docs/log/log-20261014-1.md` rather than reviewed by eye.
fn browse_for_folder(title: &str) -> Result<Option<PathBuf>, PathPickerError> {
    // `CoTaskMemFree` lives in `combaseapi` (the module that declares the COM task allocator),
    // not in `objbase`, which the first probe compile is what established.
    use winapi::um::combaseapi::CoTaskMemFree;
    use winapi::um::shlobj::SHGetPathFromIDListW;

    /// `BROWSEINFOW` (`shlobj.h`): all-pointer-or-`u32` fields, so `repr(C)` reproduces the layout
    /// the SDK declares without needing its header.
    #[repr(C)]
    struct BrowseInfoW {
        /// `HWND` of the owner window; `null` asks for an unowned dialog, which is what a library
        /// that does not know the host's window can offer.
        hwnd_owner: *mut core::ffi::c_void,
        /// `PCIDLIST_ABSOLUTE` of the root to browse from; `null` means the desktop.
        pidl_root: *mut core::ffi::c_void,
        /// `LPWSTR` scratch buffer the browser fills with the display name; a null pointer is
        /// allowed and means "not interested".
        display_name: *mut u16,
        /// `LPCWSTR` label shown above the tree.
        label: *const u16,
        /// `UINT` `BIF_*` flags.
        flags: u32,
        /// `BFFCALLBACK`; unused.
        callback: *mut core::ffi::c_void,
        /// `LPARAM` for the callback; unused.
        callback_param: isize,
        /// `int` index of the system image list icon the browser uses; the browser may set it.
        image: i32,
    }

    extern "system" {
        /// `shell32!SHBrowseForFolderW`: shows the folder browser, returning an item id list the
        /// caller must free with `CoTaskMemFree`, or null when dismissed.
        fn SHBrowseForFolderW(info: *mut BrowseInfoW) -> *mut core::ffi::c_void;
    }

    /// `BIF_RETURNONLYFSDIRS`: keeps the browser from returning a virtual folder (the Recycle Bin,
    /// a control panel) that has no filesystem path to hand back. Spelled out because `winapi`
    /// does not declare the `BIF_*` set.
    const BIF_RETURNONLYFSDIRS: u32 = 0x0000_0001;

    let label_wide: Vec<u16> = std::ffi::OsStr::new(title).encode_wide().chain(Some(0)).collect();
    // SAFETY: `BrowseInfoW` is a plain C struct of pointers and integers, for which an all-zero
    // value is the documented "unset" state; `label` points at a local that outlives the call.
    let mut browse: BrowseInfoW = unsafe { std::mem::zeroed() };
    browse.label = label_wide.as_ptr();
    browse.flags = BIF_RETURNONLYFSDIRS;

    // SAFETY: the struct is initialised above; the returned list is owned by this process until it
    // is freed below.
    let item_id_list = unsafe { SHBrowseForFolderW(&mut browse) };
    if item_id_list.is_null() {
        // The browser answers null when the user dismissed it *and* has no extended-error call of
        // its own, so dismissal is the honest reading — and it is the only outcome other than a
        // path that this call can produce.
        return Ok(None);
    }

    let mut path_buffer = [0u16; PATH_BUFFER_LEN];
    // SAFETY: `item_id_list` is the live list returned above, and `path_buffer` is a local of the
    // length the API is told about through its own `MAX_PATH`-era contract.
    let resolved =
        unsafe { SHGetPathFromIDListW(item_id_list as *const _, path_buffer.as_mut_ptr()) != 0 };
    // SAFETY: `item_id_list` was allocated for this caller by the shell and is not used again.
    // The cast is to `winapi`'s own `c_void` alias, which is the type `CoTaskMemFree` takes.
    unsafe {
        CoTaskMemFree(item_id_list as *mut winapi::ctypes::c_void);
    }
    if !resolved {
        return Ok(None);
    }
    Ok(Some(path_from_wide_buffer(&path_buffer)))
}

/// Reads a NUL-terminated wide buffer into a `PathBuf`, stopping at the terminator.
///
/// A dialog that fills the whole buffer leaves no terminator, which is why the scan falls back to
/// the full length instead of panicking: the API guarantees a terminator when it accepts a name
/// that fits, and this reads what it wrote rather than assuming it.
fn path_from_wide_buffer(buffer: &[u16]) -> PathBuf {
    let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
    PathBuf::from(String::from_utf16_lossy(&buffer[..length]))
}
