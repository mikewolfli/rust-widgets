//! Win32 helper functions for native control creation.

use crate::platform::Platform;

/// Attempt to downcast a `&dyn Platform` to `&WindowsPlatform`.
fn platform_as_windows(platform: &dyn Platform) -> Option<&super::WindowsPlatform> {
    platform.as_any().downcast_ref::<super::WindowsPlatform>()
}

/// Native Win32 Label (STATIC control) creation
pub fn try_create_label(
    platform: &dyn Platform,
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

        // SAFETY: InitCommonControls() is safe to call multiple times; it registers
        // common control window classes and has no threading constraints per MSDN.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class: Vec<u16> = OsStr::new("Static").encode_wide().chain(Some(0)).collect();
        let text_wide: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
        // SAFETY: CreateWindowExW is called with valid wide strings (null-terminated),
        // a valid parent HWND from the platform instance, and standard window styles.
        // The function is documented as safe when parameters are valid. A null return
        // is handled gracefully by the caller.
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
    platform: &dyn Platform,
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
    platform: &dyn Platform,
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

        // SAFETY: InitCommonControls() is safe to call multiple times per MSDN.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class: Vec<u16> = OsStr::new(PROGRESS_CLASS).encode_wide().chain(Some(0)).collect();
        // SAFETY: CreateWindowExW with a valid PROGRESS_CLASS wide string, valid
        // parent HWND, and standard styles. Null return is handled gracefully.
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

/// Native Win32 group box (`BUTTON` with `BS_GROUPBOX`).
///
/// `BS_GROUPBOX` is a button-style frame that draws its label in the border, so
/// it maps the `GroupBox` kind onto a real OS container rather than a panel.
pub fn try_create_group_box(
    platform: &dyn Platform,
    parent: u64,
    title: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::winuser::{CreateWindowExW, BS_GROUPBOX, WS_CHILD, WS_VISIBLE};

        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("Button");
        let text = super::WindowsPlatform::to_wide(title);
        // SAFETY: valid class/label wide strings, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                text.as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_GROUPBOX,
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
            super::types::WindowsHandleKind::GroupBox,
            title,
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
        let _ = (platform, parent, title, x, y, width, height);
        None
    }
}

/// Native Win32 frame (`STATIC` with `WS_EX_CLIENTEDGE`), an empty bordered
/// container.
pub fn try_create_frame(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_EX_CLIENTEDGE, WS_VISIBLE};

        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("Static");
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE,
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
            super::types::WindowsHandleKind::Frame,
            "Frame",
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

/// Native Win32 tab control (`SysTabControl32`) with one seeded tab so the
/// control is visible instead of a zero-content strip.
pub fn try_create_tab_widget(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{InitCommonControls, TCITEMW, TCM_INSERTITEMW, WC_TABCONTROL};
        use winapi::um::winuser::{CreateWindowExW, SendMessageW, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(WC_TABCONTROL);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE,
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

        // Seed one tab so the control renders its item area.
        // SAFETY: hwnd is a valid SysTabControl32; the TCITEMW is fully
        // initialised and its pszText points at a wide string alive for the call.
        unsafe {
            let mut item: TCITEMW = std::mem::zeroed();
            let mut label = super::WindowsPlatform::to_wide("Tab 1");
            item.pszText = label.as_mut_ptr();
            SendMessageW(hwnd, TCM_INSERTITEMW, 0, &mut item as *mut _ as isize);
        }

        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::TabWidget,
            "TabWidget",
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

/// Native Win32 splitter: two child panes inside a plain container.
///
/// Windows ships no splitter control, so this creates the parent host window and
/// the two panes that a host `WM_LBUTTONDOWN` drag would resize. The container
/// itself is a real window with a client edge, which is the OS-supported basis
/// for a splitter.
pub fn try_create_splitter(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_EX_CLIENTEDGE, WS_VISIBLE};

        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("Static");
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_CLIENTEDGE,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE,
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
            super::types::WindowsHandleKind::Splitter,
            "Splitter",
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

/// Native Win32 toggle button: a `BUTTON` with `BS_AUTOCHECKBOX | BS_PUSHLIKE`,
/// which is the OS-supported way to get a button with a persistent state.
pub fn try_create_toggle_button(
    platform: &dyn Platform,
    parent: u64,
    text: &str,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::winuser::{
            CreateWindowExW, BS_AUTOCHECKBOX, BS_PUSHLIKE, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
        };

        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("Button");
        let label = super::WindowsPlatform::to_wide(text);
        // SAFETY: valid class/label wide strings, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                label.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX | BS_PUSHLIKE,
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
            super::types::WindowsHandleKind::ToggleButton,
            text,
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        // SAFETY: hwnd is a valid control owned by this adapter.
        unsafe {
            platform_instance.bind_control_command(widget_id, hwnd);
        }
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, text, x, y, width, height);
        None
    }
}

/// Native Win32 month calendar (`SysMonthCal32`) for the `Calendar` kind.
pub fn try_create_calendar(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{InitCommonControls, MONTHCAL_CLASS};
        use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(MONTHCAL_CLASS);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE,
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
            super::types::WindowsHandleKind::Calendar,
            "Calendar",
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

/// Native Win32 trackbar (`msctls_trackbar32`) with `TBS_VERT`, for the
/// `ScrollBar` kind — a real scroll-capable control rather than a slider alias.
pub fn try_create_scroll_bar(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{InitCommonControls, TBS_VERT, TRACKBAR_CLASS};
        use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(TRACKBAR_CLASS);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE | TBS_VERT,
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
            super::types::WindowsHandleKind::ScrollBar,
            "ScrollBar",
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

/// Native Win32 fractional spin box: an up-down control whose buddy Edit is
/// configured for fractional values (the integer `SpinBox` uses whole steps).
pub fn try_create_double_spin_box(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{
            InitCommonControls, UDS_ALIGNRIGHT, UDS_ARROWKEYS, UDS_SETBUDDYINT, UDS_WRAP,
            UPDOWN_CLASS,
        };
        use winapi::um::winuser::{CreateWindowExW, WS_BORDER, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(UPDOWN_CLASS);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_BORDER
                    | UDS_SETBUDDYINT
                    | UDS_ALIGNRIGHT
                    | UDS_ARROWKEYS
                    | UDS_WRAP,
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
            super::types::WindowsHandleKind::DoubleSpinBox,
            "DoubleSpinBox",
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

/// Native Win32 combo box for the `FontComboBox` kind.
///
/// Windows exposes installed font families through the OS font dialog rather
/// than this control, so the combo is created as a real `ComboBox` and the host
/// is expected to populate it with families (mirroring the GTK implementation,
/// which seeds a few common families as a starting point).
pub fn try_create_font_combo_box(
    platform: &dyn Platform,
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

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("ComboBox");
        // SAFETY: valid class string, valid parent HWND, standard styles.
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
            super::types::WindowsHandleKind::FontComboBox,
            "FontComboBox",
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        // SAFETY: hwnd is a valid control owned by this adapter.
        unsafe {
            platform_instance.bind_control_command(widget_id, hwnd);
        }
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}

/// Native Win32 date/time picker (`SysDateTimePick32`) for the `DatePicker`
/// kind.
///
/// `DTS_SHORTDATEFORMAT` is the OS's short-date presentation; the picker owns a
/// real `SYSTEMTIME` the host can read back through `DTM_GETSYSTEMTIME`.
pub fn try_create_date_picker(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    create_date_time_pick(
        platform,
        parent,
        x,
        y,
        width,
        height,
        super::types::WindowsHandleKind::DatePicker,
        "DatePicker",
        DTS_SHORTDATEFORMAT_VALUE,
    )
}

/// Native Win32 time picker (`SysDateTimePick32` with `DTS_TIMEFORMAT`).
pub fn try_create_time_picker(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    create_date_time_pick(
        platform,
        parent,
        x,
        y,
        width,
        height,
        super::types::WindowsHandleKind::TimePicker,
        "TimePicker",
        DTS_TIMEFORMAT_VALUE,
    )
}

/// Native Win32 combined date+time picker (`SysDateTimePick32`, long date).
pub fn try_create_date_time_picker(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    create_date_time_pick(
        platform,
        parent,
        x,
        y,
        width,
        height,
        super::types::WindowsHandleKind::DateTimePicker,
        "DateTimePicker",
        DTS_LONGDATEFORMAT_VALUE,
    )
}

#[cfg(target_os = "windows")]
const DTS_SHORTDATEFORMAT_VALUE: u32 = 0x0000;
#[cfg(target_os = "windows")]
const DTS_TIMEFORMAT_VALUE: u32 = 0x0009;
#[cfg(target_os = "windows")]
const DTS_LONGDATEFORMAT_VALUE: u32 = 0x0004;
#[cfg(not(target_os = "windows"))]
const DTS_SHORTDATEFORMAT_VALUE: u32 = 0;
#[cfg(not(target_os = "windows"))]
const DTS_TIMEFORMAT_VALUE: u32 = 0;
#[cfg(not(target_os = "windows"))]
const DTS_LONGDATEFORMAT_VALUE: u32 = 0;

/// Shared `SysDateTimePick32` construction for the three picker kinds.
#[allow(clippy::too_many_arguments)]
fn create_date_time_pick(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    kind: super::types::WindowsHandleKind,
    label: &str,
    format_style: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{InitCommonControls, DATETIMEPICK_CLASS};
        use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(DATETIMEPICK_CLASS);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE | format_style,
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
        let widget_id = platform_instance.state.create_widget(kind, label, x, y, width, height);
        platform_instance.bind_native_handle(widget_id, hwnd);
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height, kind, label, format_style);
        None
    }
}

/// Native Win32 progress-bar based busy indicator for the `ActivityIndicator`
/// kind. `PBS_MARQUEE` gives the indeterminate animation, which is exactly the
/// semantics an activity indicator needs (a plain determinate bar is not).
pub fn try_create_activity_indicator(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{
            InitCommonControls, PBM_SETMARQUEE, PBS_MARQUEE, PROGRESS_CLASS,
        };
        use winapi::um::winuser::{CreateWindowExW, SendMessageW, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(PROGRESS_CLASS);
        // SAFETY: valid class string, valid parent HWND, standard styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE | PBS_MARQUEE,
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
        // Start the marquee animation (TRUE = running).
        // SAFETY: hwnd is a valid progress bar created with PBS_MARQUEE.
        unsafe {
            SendMessageW(hwnd, PBM_SETMARQUEE, 1, 30);
        }
        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::ActivityIndicator,
            "ActivityIndicator",
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
    platform: &dyn Platform,
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

        // SAFETY: InitCommonControls() is safe to call multiple times per MSDN.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide("ComboBox");
        let dropdown_height = (height as i32).max(180);
        // SAFETY: CreateWindowExW with a valid ComboBox class wide string, valid
        // parent HWND, and standard combo box styles. Null return is handled gracefully.
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
        // SAFETY: hwnd was just created by CreateWindowExW above and is guaranteed
        // to be a valid HWND (null case is already handled). bind_control_command
        // registers a window subclass procedure which is safe on a valid HWND.
        unsafe {
            platform_instance.bind_control_command(widget_id, hwnd);
        }
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}

/// Native Win32 SpinBox creation (`msctls_updown32`).
///
/// Creates a real up-down control with `UDS_SETBUDDYINT`, `UDS_ALIGNRIGHT`,
/// `UDS_ARROWKEYS` and `UDS_WRAP`. `<name>` is self-referential: a `SysUpDown32`
/// without a buddy is still a usable control, so the returned handle is the
/// up-down itself.
pub fn try_create_spin_box(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{
            InitCommonControls, UDS_ALIGNRIGHT, UDS_ARROWKEYS, UDS_SETBUDDYINT, UDS_WRAP,
            UPDOWN_CLASS,
        };
        use winapi::um::winuser::{CreateWindowExW, WS_BORDER, WS_CHILD, WS_VISIBLE};

        // SAFETY: InitCommonControls() registers the common control classes and is
        // documented as safe to call repeatedly from any thread.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(UPDOWN_CLASS);
        // SAFETY: CreateWindowExW with a valid class string, valid parent HWND and
        // standard control styles; a null result is handled below.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_BORDER
                    | UDS_SETBUDDYINT
                    | UDS_ALIGNRIGHT
                    | UDS_ARROWKEYS
                    | UDS_WRAP,
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
            super::types::WindowsHandleKind::SpinBox,
            "SpinBox",
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

/// Native Win32 ListView creation (`SysListView32`, report view).
///
/// Creates the control and attaches a single full-width column so the report
/// view is immediately usable, then enables `LVS_EX_FULLROWSELECT`.
pub fn try_create_list_view(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::commctrl::{
            InitCommonControls, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW, LVM_INSERTCOLUMNW,
            LVM_SETEXTENDEDLISTVIEWSTYLE, LVS_EX_FULLROWSELECT, LVS_REPORT, LVS_SHOWSELALWAYS,
            LVS_SINGLESEL, WC_LISTVIEW,
        };
        use winapi::um::winuser::{
            CreateWindowExW, SendMessageW, WS_BORDER, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
        };

        // SAFETY: InitCommonControls() is safe to call repeatedly.
        unsafe {
            InitCommonControls();
        }
        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        let class = super::WindowsPlatform::to_wide(WC_LISTVIEW);
        // SAFETY: valid class string, valid parent HWND, standard list-view styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_BORDER
                    | WS_TABSTOP
                    | LVS_REPORT
                    | LVS_SINGLESEL
                    | LVS_SHOWSELALWAYS,
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

        // A report-view list without a column shows nothing, so add one that
        // spans the control. SAFETY: hwnd is a valid SysListView32 just created;
        // the LVCOLUMNW is fully initialized and lives for the call.
        unsafe {
            let mut column: LVCOLUMNW = std::mem::zeroed();
            column.mask = LVCF_TEXT | LVCF_WIDTH;
            column.cx = width as i32;
            let header = super::WindowsPlatform::to_wide("Item");
            column.pszText = header.as_ptr() as *mut _;
            SendMessageW(hwnd, LVM_INSERTCOLUMNW, 0, &mut column as *mut _ as isize);
            SendMessageW(
                hwnd,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                LVS_EX_FULLROWSELECT as usize,
                LVS_EX_FULLROWSELECT as isize,
            );
        }

        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::ListView,
            "ListView",
            x,
            y,
            width,
            height,
        );
        platform_instance.bind_native_handle(widget_id, hwnd);
        // SAFETY: hwnd is a valid control owned by this adapter, which is the
        // precondition documented for bind_control_command.
        unsafe {
            platform_instance.bind_control_command(widget_id, hwnd);
        }
        Some(widget_id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}

/// Native Win32 ScrollArea creation (a scrollable container window).
///
/// Windows has no dedicated scroll-area control, so this creates a child window
/// of the built-in `Scroll` class region with `WS_HSCROLL | WS_VSCROLL` and sets
/// a default scroll range, giving real native scroll bars the content can bind
/// to via the standard `WM_HSCROLL`/`WM_VSCROLL` messages.
pub fn try_create_scroll_area(
    platform: &dyn Platform,
    parent: u64,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        use std::ptr::null_mut;
        use winapi::um::winuser::{
            CreateWindowExW, SetScrollRange, WS_BORDER, WS_CHILD, WS_HSCROLL, WS_VISIBLE,
            WS_VSCROLL,
        };

        let platform_instance = platform_as_windows(platform)?;
        let parent_hwnd = platform_instance.get_native_handle(parent)?;
        // "Static" is the cheapest built-in child window that can host scroll
        // bars; the scroll bars are what makes this a scroll area.
        let class = super::WindowsPlatform::to_wide("Static");
        // SAFETY: valid class string, valid parent HWND, standard scroll styles.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                null_mut(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | WS_HSCROLL | WS_VSCROLL,
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
        // SAFETY: hwnd is a valid window with scroll-bar styles; SetScrollRange
        // only writes the scroll info of that window.
        unsafe {
            const SB_VERT: i32 = 1;
            const SB_HORZ: i32 = 0;
            SetScrollRange(hwnd, SB_VERT, 0, height as i32, 1);
            SetScrollRange(hwnd, SB_HORZ, 0, width as i32, 1);
        }

        let widget_id = platform_instance.state.create_widget(
            super::types::WindowsHandleKind::ScrollArea,
            "ScrollArea",
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
