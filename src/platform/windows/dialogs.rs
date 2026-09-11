//! Win32 native dialog presentation (MessageBox / File / Color / Font).
//!
//! `create_message_box` / `create_file_dialog` / `create_color_dialog` /
//! `create_font_dialog` stay non-blocking: they register a state handle plus
//! the dialog metadata needed to present later. The native modal dialog is
//! shown from `show_widget` (see [`present_native_dialog`]) so creation never
//! blocks the caller.

use super::types::{WindowsHandleKind, WindowsPlatform};
use crate::core::ObjectId;

#[cfg(target_os = "windows")]
pub(crate) fn present_native_dialog(
    platform: &WindowsPlatform,
    widget_id: ObjectId,
    kind: WindowsHandleKind,
) {
    use std::mem;
    use winapi::shared::windef::HWND;

    let (parent_hwnd, title) = {
        let guard = match platform.dialog_data.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        match guard.get(&widget_id) {
            Some(data) => (data.parent_hwnd, data.title.clone()),
            None => return,
        }
    };
    let parent_hwnd = parent_hwnd as HWND;
    let text = platform.state.text(widget_id);

    unsafe {
        match kind {
            WindowsHandleKind::MessageBox => {
                use winapi::um::winuser::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
                let title_wide = WindowsPlatform::to_wide(&title);
                let text_wide = WindowsPlatform::to_wide(&text);
                let _ = MessageBoxW(
                    parent_hwnd,
                    text_wide.as_ptr(),
                    title_wide.as_ptr(),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
            WindowsHandleKind::FileDialog => {
                use winapi::um::commdlg::{
                    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
                };
                let mut ofn: OPENFILENAMEW = mem::zeroed();
                let mut file_buf = vec![0u16; 260];
                let title_wide = WindowsPlatform::to_wide(&title);
                ofn.lStructSize = mem::size_of::<OPENFILENAMEW>() as u32;
                ofn.hwndOwner = parent_hwnd;
                ofn.lpstrFile = file_buf.as_mut_ptr();
                ofn.nMaxFile = file_buf.len() as u32;
                ofn.lpstrTitle = title_wide.as_ptr();
                ofn.Flags = OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST;
                if GetOpenFileNameW(&mut ofn) != 0 {
                    let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
                    let path = String::from_utf16_lossy(&file_buf[..len]);
                    let _ = platform.state.set_text(widget_id, &path);
                }
            }
            WindowsHandleKind::ColorDialog => {
                use winapi::um::commdlg::{ChooseColorW, CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW};
                let mut cc: CHOOSECOLORW = mem::zeroed();
                cc.lStructSize = mem::size_of::<CHOOSECOLORW>() as u32;
                cc.hwndOwner = parent_hwnd;
                cc.Flags = CC_RGBINIT | CC_FULLOPEN;
                if ChooseColorW(&mut cc) != 0 {
                    let value = format!("#{:06X}", cc.rgbResult & 0x00FF_FFFF);
                    let _ = platform.state.set_text(widget_id, &value);
                }
            }
            WindowsHandleKind::FontDialog => {
                use winapi::um::commdlg::{
                    ChooseFontW, CF_INITTOLOGFONTSTRUCT, CF_SCREENFONTS, CHOOSEFONTW,
                };
                let mut cf: CHOOSEFONTW = mem::zeroed();
                cf.lStructSize = mem::size_of::<CHOOSEFONTW>() as u32;
                cf.hwndOwner = parent_hwnd;
                cf.Flags = CF_SCREENFONTS | CF_INITTOLOGFONTSTRUCT;
                if ChooseFontW(&mut cf) != 0 && !cf.lpLogFont.is_null() {
                    let lf = &*cf.lpLogFont;
                    let name_len =
                        lf.lfFaceName.iter().position(|&c| c == 0).unwrap_or(lf.lfFaceName.len());
                    let name = String::from_utf16_lossy(&lf.lfFaceName[..name_len]);
                    let _ = platform.state.set_text(widget_id, &name);
                }
            }
            WindowsHandleKind::DirectoryDialog => {
                present_folder_picker(platform, widget_id, parent_hwnd, &title);
            }
            _ => {}
        }
    }
}

/// Present the modern folder picker (`IFileDialog` + `FOS_PICKFOLDERS`).
///
/// `SHBrowseForFolder` is the legacy API; the supported replacement is the
/// Common Item Dialog, created as `CLSID_FileOpenDialog` and switched to folder
/// mode with `FOS_PICKFOLDERS`. `FOS_FORCEFILESYSTEM` guarantees the result is
/// a real path that `IShellItem::GetDisplayName(SIGDN_FILESYSPATH)` can return,
/// which is what the widget stores.
///
/// On success the selected directory path is written to the state text; if the
/// user cancels, the text is left unchanged.
#[cfg(target_os = "windows")]
fn present_folder_picker(
    platform: &WindowsPlatform,
    widget_id: ObjectId,
    parent_hwnd: winapi::shared::windef::HWND,
    title: &str,
) {
    use std::ptr;
    use winapi::ctypes::c_void;
    use winapi::shared::guiddef::GUID;
    use winapi::shared::minwindef::DWORD;
    use winapi::shared::winerror::SUCCEEDED;
    use winapi::shared::wtypesbase::CLSCTX_INPROC_SERVER;
    use winapi::um::combaseapi::{CoCreateInstance, CoTaskMemFree};
    use winapi::um::shobjidl::{IFileDialog, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS};
    use winapi::um::shobjidl_core::{CLSID_FileOpenDialog, IShellItem, SIGDN_FILESYSPATH};
    use winapi::um::unknwnbase::IUnknown;

    // `IID_IFileDialog` is not exported by winapi, so declare it here. The value
    // is the documented interface ID for IFileDialog.
    const IID_IFILE_DIALOG_LOCAL: GUID = GUID {
        Data1: 0x42f85136,
        Data2: 0xdb7e,
        Data3: 0x439c,
        Data4: [0x85, 0xf1, 0xe4, 0x07, 0x1f, 0x2a, 0x1f, 0x00],
    };

    // SAFETY: all COM calls are checked; every acquired interface is released
    // before returning, including on the error paths.
    unsafe {
        let mut dialog: *mut IFileDialog = ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_FileOpenDialog,
            ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_IFILE_DIALOG_LOCAL,
            &mut dialog as *mut *mut IFileDialog as *mut *mut c_void,
        );
        if !SUCCEEDED(hr) || dialog.is_null() {
            log::warn!(
                "[rust_widgets][windows] folder picker: CoCreateInstance failed (hr={hr:#x})"
            );
            return;
        }

        // Read the current options, then add folder-picking flags. Starting from
        // GetOptions() preserves the defaults (e.g. FOS_PATHMUSTEXIST).
        let mut options: DWORD = 0;
        let options_ok = SUCCEEDED((*dialog).GetOptions(&mut options));
        let flags = if options_ok { options } else { 0 } | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM;
        let _ = (*dialog).SetOptions(flags);

        if !title.is_empty() {
            let title_wide = WindowsPlatform::to_wide(title);
            let _ = (*dialog).SetTitle(title_wide.as_ptr());
        }

        let shown = SUCCEEDED((*dialog).Show(parent_hwnd));
        if shown {
            let mut item: *mut IShellItem = ptr::null_mut();
            if SUCCEEDED((*dialog).GetResult(&mut item)) && !item.is_null() {
                let mut name: *mut u16 = ptr::null_mut();
                if SUCCEEDED((*item).GetDisplayName(SIGDN_FILESYSPATH, &mut name))
                    && !name.is_null()
                {
                    let mut len = 0usize;
                    while *name.add(len) != 0 {
                        len += 1;
                    }
                    let path = String::from_utf16_lossy(std::slice::from_raw_parts(name, len));
                    let _ = platform.state.set_text(widget_id, &path);
                    CoTaskMemFree(name as *mut c_void);
                }
                (*(item as *mut IUnknown)).Release();
            }
        }
        (*(dialog as *mut IUnknown)).Release();
    }
}
