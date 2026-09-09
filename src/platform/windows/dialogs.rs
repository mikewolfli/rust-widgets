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
            _ => {}
        }
    }
}
