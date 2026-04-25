//! Win32 helper functions for native control creation.

use crate::platform::state::BackendState;
use crate::platform::DropEvent;
use crate::platform::Platform;

/// Attempt to downcast a `&dyn Platform` to `&WindowsPlatform`.
fn platform_as_windows(platform: &dyn Platform) -> Option<&super::WindowsPlatform> {
    platform.as_any().downcast_ref::<super::WindowsPlatform>()
}

/// Native Win32 Label (STATIC control) creation
pub fn try_create_label(
    platform: &dyn super::Platform,
    parent: u64,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null_mut;
        use winapi::um::commctrl::InitCommonControls;
        use winapi::um::winuser::{CreateWindowExW, SS_LEFT, SS_NOPREFIX, WS_CHILD, WS_VISIBLE};

        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class: Vec<u16> = OsStr::new("Static").encode_wide().chain(Some(0)).collect();
        let text_wide: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                text_wide.as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_LEFT | SS_NOPREFIX,
                x,
                y,
                width as i32,
                height as i32,
                parent_hwnd,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::Label,
            "Label",
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, text, x, y, width, height);
        None
    }
}

/// Public function for cross-platform slider creation dispatch
pub fn try_create_slider(
    platform: &dyn super::Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    let windows = platform_as_windows(platform)?;
    <super::WindowsPlatform as super::WindowsPlatformExtSlider>::try_create_slider(
        windows, parent, x, y, width, height,
    )
}

pub fn try_create_progress_bar(
    platform: &dyn super::Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use std::ptr::null_mut;
        use winapi::um::commctrl::{InitCommonControls, PROGRESS_CLASS};
        use winapi::um::winuser::{CreateWindowExW, WS_BORDER, WS_CHILD, WS_VISIBLE};

        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class: Vec<u16> = OsStr::new(PROGRESS_CLASS)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE | WS_BORDER,
                x,
                y,
                width as i32,
                height as i32,
                parent_hwnd,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}

/// Native Win32 ComboBox creation
pub fn try_create_combo_box(
    platform: &dyn super::Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::InitCommonControls;
        use winapi::um::winuser::{
            CreateWindowExW, CBS_DROPDOWNLIST, CBS_HASSTRINGS, WS_BORDER, WS_CHILD, WS_TABSTOP,
            WS_VISIBLE, WS_VSCROLL,
        };

        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("ComboBox");
        let dropdown_height = (height as i32).max(180);
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_BORDER
                    | WS_VSCROLL
                    | CBS_DROPDOWNLIST
                    | CBS_HASSTRINGS,
                x,
                y,
                width as i32,
                dropdown_height,
                parent_hwnd,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if hwnd.is_null() {
            return None;
        }
        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::ComboBox,
            "ComboBox",
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        platform_instance.bind_control_command(widget_id, hwnd);
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}
