use crate::platform::{state::BackendState, DropEvent};
// use crate::platform::state::BackendState; // Not needed, remove unresolved import
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
        // WC_STATIC is not available in winapi; use "Static" directly
        unsafe {
            InitCommonControls();
            let platform_instance =
                &*(platform as *const dyn super::Platform as *const WindowsPlatform);
            let parent_hwnd = platform_instance.get_native_handle(parent)?;
            let class: Vec<u16> = OsStr::new("Static").encode_wide().chain(Some(0)).collect();
            let text_wide: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
            let hwnd = CreateWindowExW(
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
            );
            if hwnd.is_null() {
                return None;
            }
            let widget_id = platform_instance.state.create_widget(
                WindowsHandleKind::Label,
                "Label",
                x,
                y,
                width,
                height,
            );
            platform_instance.bind_native_handle(widget_id, hwnd);
            return Some(widget_id);
        }
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
    let windows =
        unsafe { (platform as *const dyn super::Platform as *const WindowsPlatform).as_ref() };
    if let Some(windows) = windows {
        <WindowsPlatform as WindowsPlatformExtSlider>::try_create_slider(
            windows, parent, x, y, width, height,
        )
    } else {
        None
    }
}

use crate::core::{ObjectId, PlatformFamily};
use crate::platform::{
    EmbeddedCapabilityContract, NativeCapabilityContract, Platform, PlatformCapabilities,
    WidgetTriggerEvent, WidgetTriggerKind,
};

impl Platform for WindowsPlatform {
    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::{ShowWindow, UpdateWindow, SW_SHOW};
                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);
            }
        }
    }
    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::{ShowWindow, SW_HIDE};
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::MoveWindow;
                MoveWindow(hwnd, x, y, width as i32, height as i32, 1);
            }
        }
    }
    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        let _ = self.state.set_text(widget_id, text);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::SetWindowTextW;
                let text_wide = Self::to_wide(text);
                SetWindowTextW(hwnd, text_wide.as_ptr());
            }
        }
    }
    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::{GetWindowTextLengthW, GetWindowTextW};
                    let len = GetWindowTextLengthW(hwnd);
                    if len > 0 {
                        let mut buffer = vec![0u16; (len + 1) as usize];
                        let copied = GetWindowTextW(hwnd, buffer.as_mut_ptr(), len + 1);
                        if copied > 0 {
                            return String::from_utf16_lossy(&buffer[..copied as usize]);
                        }
                    }
                }
            }
        }
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
        #[cfg(target_os = "windows")]
        if let Some(hwnd) = self.get_native_handle(widget_id) {
            unsafe {
                use winapi::um::winuser::EnableWindow;
                EnableWindow(hwnd, if enabled { 1 } else { 0 });
            }
        }
    }
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::IsWindowEnabled;
                    return IsWindowEnabled(hwnd) != 0;
                }
            }
        }
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
    }
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                unsafe {
                    use winapi::um::winuser::IsWindowVisible;
                    return IsWindowVisible(hwnd) != 0;
                }
            }
        }
        self.state.visible(widget_id)
    }
    fn backend_name(&self) -> &'static str {
        "WindowsPlatform"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn capabilities(&self) -> PlatformCapabilities {
        PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: true,
            typed_widget_trigger: true,
        }
    }
    fn native_capability_contract(&self) -> Option<NativeCapabilityContract> {
        Some(NativeCapabilityContract::from_platform_caps(
            self.capabilities(),
        ))
    }
    fn embedded_capability_contract(&self) -> Option<EmbeddedCapabilityContract> {
        None
    }
    fn dpi_scale_factor(&self) -> f32 {
        1.0
    }
    fn init(&self) {
        self.runtime_initialized.store(true, Ordering::SeqCst);
        #[cfg(target_os = "windows")]
        {
            let _ = ACTIVE_WINDOWS_PLATFORM.set(self as *const WindowsPlatform as usize);
        }
        #[cfg(target_os = "windows")]
        unsafe {
            use winapi::um::commctrl::InitCommonControls;
            InitCommonControls();
        }
    }
    fn run(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use std::ptr::null_mut;
            use std::thread;
            use std::time::Duration;
            use winapi::um::winuser::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE, WM_QUIT,
            };

            self.runtime_running.store(true, Ordering::SeqCst);
            while self.runtime_running.load(Ordering::SeqCst) {
                let mut msg: MSG = std::mem::zeroed();
                while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        self.runtime_running.store(false, Ordering::SeqCst);
                        break;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if self.runtime_running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            self.runtime_running.store(false, Ordering::SeqCst);
        }
    }
    fn quit(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            use winapi::um::winuser::PostQuitMessage;
            self.runtime_running.store(false, Ordering::SeqCst);
            PostQuitMessage(0);
        }
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            unsafe extern "system" {
                fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
            }
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, ShowWindow, UpdateWindow, SW_SHOW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            };

            ensure_window_class_registered();
            let class_name = Self::to_wide("RustWidgetsWindowClass");
            let title_wide = Self::to_wide(title);
            let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title_wide.as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    null_mut(),
                    null_mut(),
                    hinstance as _,
                    null_mut(),
                )
            };
            if hwnd.is_null() {
                eprintln!(
                    "[rust_widgets][windows] create_window failed for title='{}'",
                    title
                );
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::Window, title, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);
            }
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.state
                .create_widget(WindowsHandleKind::Window, title, x, y, width, height)
        }
    }
    fn create_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_PUSHBUTTON, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_PUSHBUTTON,
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
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::Button, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            self.bind_control_command(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.state
                .create_widget(WindowsHandleKind::Button, text, x, y, width, height)
        }
    }
    fn create_label(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            if let Some(id) = try_create_label(self, parent, text, x, y, width, height) {
                return id;
            }
            eprintln!("[rust_widgets][windows] create_label failed (parent={parent})");
            return 0;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_checkbox(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_AUTOCHECKBOX, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_AUTOCHECKBOX,
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
                eprintln!("[rust_widgets][windows] create_checkbox failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::CheckBox, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            self.bind_control_command(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_radio_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, BS_AUTORADIOBUTTON, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Button");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_AUTORADIOBUTTON,
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
                eprintln!("[rust_widgets][windows] create_radio_button failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::RadioButton, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            self.bind_control_command(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_line_edit(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, ES_AUTOVSCROLL, ES_LEFT, ES_MULTILINE, WS_BORDER, WS_CHILD, WS_TABSTOP,
                WS_VISIBLE,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Edit");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_TABSTOP
                        | WS_VISIBLE
                        | WS_CHILD
                        | WS_BORDER
                        | ES_LEFT
                        | ES_MULTILINE
                        | ES_AUTOVSCROLL,
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
                eprintln!("[rust_widgets][windows] create_line_edit failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::LineEdit, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            self.bind_control_command(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if let Some(widget_id) = try_create_slider(self, parent, x, y, width, height) {
            return widget_id;
        }
        eprintln!("[rust_widgets][windows] create_slider failed (parent={parent})");
        0
    }
    fn create_progress_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if let Some(widget_id) = try_create_progress_bar(self, parent, x, y, width, height) {
            return widget_id;
        }
        eprintln!("[rust_widgets][windows] create_progress_bar failed (parent={parent})");
        0
    }
    fn create_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if let Some(widget_id) = try_create_combo_box(self, parent, x, y, width, height) {
            return widget_id;
        }
        eprintln!("[rust_widgets][windows] create_combo_box failed (parent={parent})");
        0
    }
    fn combo_box_add_item(&self, combo_box: ObjectId, text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ADDSTRING, CB_ERR};

            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let text_wide = Self::to_wide(text);
            let result =
                unsafe { SendMessageW(hwnd, CB_ADDSTRING, 0, text_wide.as_ptr() as isize) };
            return result != CB_ERR as isize;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, text);
            false
        }
    }
    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_RESETCONTENT};

            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            unsafe {
                SendMessageW(hwnd, CB_RESETCONTENT, 0, 0);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            false
        }
    }
    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCURSEL, CB_SETCURSEL};

            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let previous = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
            let result = unsafe { SendMessageW(hwnd, CB_SETCURSEL, index, 0) };
            if result == CB_ERR as isize {
                return false;
            }

            if result != previous {
                let _ = self
                    .inject_widget_trigger_event(combo_box, WidgetTriggerKind::SelectionChanged);
                let _ =
                    self.inject_widget_trigger_event(combo_box, WidgetTriggerKind::ValueChanged);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, index);
            false
        }
    }
    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCURSEL};

            let hwnd = self.get_native_handle(combo_box)?;
            let result = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
            if result == CB_ERR as isize {
                None
            } else {
                Some(result as usize)
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            None
        }
    }
    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETCOUNT};

            let hwnd = match self.get_native_handle(combo_box) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let result = unsafe { SendMessageW(hwnd, CB_GETCOUNT, 0, 0) };
            if result == CB_ERR as isize || result < 0 {
                0
            } else {
                result as usize
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = combo_box;
            0
        }
    }
    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, CB_ERR, CB_GETLBTEXT, CB_GETLBTEXTLEN};

            let hwnd = self.get_native_handle(combo_box)?;
            let len = unsafe { SendMessageW(hwnd, CB_GETLBTEXTLEN, index, 0) };
            if len == CB_ERR as isize || len < 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied =
                unsafe { SendMessageW(hwnd, CB_GETLBTEXT, index, buf.as_mut_ptr() as isize) };
            if copied == CB_ERR as isize || copied < 0 {
                return None;
            }
            return Some(String::from_utf16_lossy(&buf[..copied as usize]));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (combo_box, index);
            None
        }
    }
    fn create_list_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, LBS_NOTIFY, WS_BORDER, WS_CHILD, WS_TABSTOP, WS_VISIBLE,
                WS_VSCROLL,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("ListBox");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_TABSTOP | WS_VISIBLE | WS_CHILD | WS_BORDER | WS_VSCROLL | LBS_NOTIFY,
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
                eprintln!("[rust_widgets][windows] create_list_box failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::ListBox,
                "ListBox",
                x,
                y,
                width,
                height,
            );
            self.bind_native_handle(widget_id, hwnd);
            self.bind_control_command(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ADDSTRING, LB_ERR};

            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let text_wide = Self::to_wide(text);
            let result =
                unsafe { SendMessageW(hwnd, LB_ADDSTRING, 0, text_wide.as_ptr() as isize) };
            return result != LB_ERR as isize;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, text);
            false
        }
    }
    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_DELETESTRING, LB_ERR};

            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let result = unsafe { SendMessageW(hwnd, LB_DELETESTRING, index, 0) };
            return result != LB_ERR as isize;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            false
        }
    }
    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_RESETCONTENT};

            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            unsafe {
                SendMessageW(hwnd, LB_RESETCONTENT, 0, 0);
            }
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            false
        }
    }
    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_SETCURSEL};

            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return false,
            };
            let result = unsafe { SendMessageW(hwnd, LB_SETCURSEL, index, 0) };
            if result == LB_ERR as isize {
                return false;
            }
            let _ = self.inject_widget_trigger_event(list_box, WidgetTriggerKind::SelectionChanged);
            true
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            false
        }
    }
    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETCURSEL};

            let hwnd = self.get_native_handle(list_box)?;
            let result = unsafe { SendMessageW(hwnd, LB_GETCURSEL, 0, 0) };
            if result == LB_ERR as isize {
                None
            } else {
                Some(result as usize)
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            None
        }
    }
    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETCOUNT};

            let hwnd = match self.get_native_handle(list_box) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let result = unsafe { SendMessageW(hwnd, LB_GETCOUNT, 0, 0) };
            if result == LB_ERR as isize || result < 0 {
                0
            } else {
                result as usize
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = list_box;
            0
        }
    }
    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{SendMessageW, LB_ERR, LB_GETTEXT, LB_GETTEXTLEN};

            let hwnd = self.get_native_handle(list_box)?;
            let len = unsafe { SendMessageW(hwnd, LB_GETTEXTLEN, index, 0) };
            if len == LB_ERR as isize || len < 0 {
                return None;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let copied =
                unsafe { SendMessageW(hwnd, LB_GETTEXT, index, buf.as_mut_ptr() as isize) };
            if copied == LB_ERR as isize || copied < 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buf[..copied as usize]))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (list_box, index);
            None
        }
    }
    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{CreateWindowExW, WS_BORDER, WS_CHILD, WS_VISIBLE};

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_VISIBLE | WS_CHILD | WS_BORDER,
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
                eprintln!("[rust_widgets][windows] create_panel failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::Panel, "Panel", x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::CreateMenu;

            let _ = (parent, x, y, width, height);
            let menu_bar_handle = unsafe { CreateMenu() };
            if menu_bar_handle.is_null() {
                eprintln!("[rust_widgets][windows] create_menu_bar failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::MenuBar,
                "MenuBar",
                x,
                y,
                width,
                height,
            );
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(widget_id, menu_bar_handle as usize);
            }
            if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                owners.insert(widget_id, parent);
            }
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{AppendMenuW, CreatePopupMenu, DrawMenuBar, MF_POPUP};

            let parent_menu = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(_) => return 0,
                };
                match handles.get(&parent).copied() {
                    Some(value) => value,
                    None => {
                        eprintln!(
                            "[rust_widgets][windows] create_menu failed: invalid parent={parent}"
                        );
                        return 0;
                    }
                }
            };

            let submenu_handle = unsafe { CreatePopupMenu() };
            if submenu_handle.is_null() {
                eprintln!("[rust_widgets][windows] create_menu failed (parent={parent})");
                return 0;
            }

            let text_wide = Self::to_wide(text);
            let append_ok = unsafe {
                AppendMenuW(
                    parent_menu as HMENU,
                    MF_POPUP,
                    submenu_handle as usize,
                    text_wide.as_ptr(),
                )
            };
            if append_ok == 0 {
                eprintln!(
                    "[rust_widgets][windows] create_menu failed: AppendMenuW failed (parent={parent})"
                );
                return 0;
            }

            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::Menu, text, x, y, width, height);
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(widget_id, submenu_handle as usize);
            }
            let owner_window = self
                .menu_state
                .menu_owner_window
                .lock()
                .ok()
                .and_then(|owners| owners.get(&parent).copied());
            if let Some(window_id) = owner_window {
                if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                    owners.insert(widget_id, window_id);
                }
                if let Some(hwnd) = self.get_native_handle(window_id) {
                    unsafe {
                        DrawMenuBar(hwnd);
                    }
                }
            }
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{DrawMenuBar, SetMenu};

            let hwnd = match self.get_native_handle(window) {
                Some(hwnd) => hwnd,
                None => {
                    eprintln!(
                        "[rust_widgets][windows] attach_menu_bar_to_window failed: invalid window={window}"
                    );
                    return false;
                }
            };

            let menu_handle = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(_) => return false,
                };
                match handles.get(&menu_bar).copied() {
                    Some(value) => value,
                    None => {
                        eprintln!(
                            "[rust_widgets][windows] attach_menu_bar_to_window failed: invalid menu_bar={menu_bar}"
                        );
                        return false;
                    }
                }
            };

            let set_ok = unsafe { SetMenu(hwnd, menu_handle as HMENU) };
            if set_ok == 0 {
                eprintln!(
                    "[rust_widgets][windows] attach_menu_bar_to_window failed: SetMenu returned 0"
                );
                return false;
            }
            unsafe {
                DrawMenuBar(hwnd);
            }
            return true;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, menu_bar);
            false
        }
    }
    fn menu_add_item(
        &self,
        parent_menu: ObjectId,
        text: &str,
        _shortcut: Option<&str>,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winuser::{AppendMenuW, DrawMenuBar, MF_STRING};

            let parent_handle = {
                let handles = match self.menu_state.handles.lock() {
                    Ok(handles) => handles,
                    Err(_) => return 0,
                };
                match handles.get(&parent_menu).copied() {
                    Some(value) => value,
                    None => {
                        eprintln!(
                            "[rust_widgets][windows] menu_add_item failed: invalid parent_menu={parent_menu}"
                        );
                        return 0;
                    }
                }
            };

            let command_id = self
                .menu_state
                .next_command_id
                .fetch_add(1, Ordering::SeqCst) as u32;
            let text_wide = Self::to_wide(text);
            let append_ok = unsafe {
                AppendMenuW(
                    parent_handle as HMENU,
                    MF_STRING,
                    command_id as usize,
                    text_wide.as_ptr(),
                )
            };
            if append_ok == 0 {
                eprintln!(
                    "[rust_widgets][windows] menu_add_item failed: AppendMenuW failed (parent_menu={parent_menu})"
                );
                return 0;
            }

            let item_id = self
                .state
                .create_widget(WindowsHandleKind::Menu, text, 0, 0, 0, 0);

            if let Ok(mut map) = self.menu_state.menu_command_to_item.lock() {
                map.insert(command_id, item_id);
            }

            let owner_window = self
                .menu_state
                .menu_owner_window
                .lock()
                .ok()
                .and_then(|owners| owners.get(&parent_menu).copied());
            if let Some(window_id) = owner_window {
                if let Ok(mut owners) = self.menu_state.menu_owner_window.lock() {
                    owners.insert(item_id, window_id);
                }
                if let Some(hwnd) = self.get_native_handle(window_id) {
                    unsafe {
                        DrawMenuBar(hwnd);
                    }
                }
            }

            item_id
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent_menu, text);
            0
        }
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        #[cfg(target_os = "windows")]
        {
            let mut events = self.menu_state.pending_menu_events.lock().ok()?;
            events.pop_front().map(|event| event.widget_id)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_menu_events.lock() {
                events.push_back(WidgetTriggerEvent {
                    widget_id: menu_item_id,
                    kind: WidgetTriggerKind::Clicked,
                });
                return true;
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = menu_item_id;
            false
        }
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_widget_events.lock() {
                if let Some(event) = events.pop_front() {
                    return Some(event);
                }
            }
            if let Ok(mut events) = self.menu_state.pending_menu_events.lock() {
                if let Some(event) = events.pop_front() {
                    return Some(event);
                }
            }
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut events) = self.menu_state.pending_widget_events.lock() {
                events.push_back(WidgetTriggerEvent { widget_id, kind });
                return true;
            }
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (widget_id, kind);
            false
        }
    }
    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{CreateWindowExW, WS_CHILD, WS_VISIBLE};

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    null_mut(),
                    WS_VISIBLE | WS_CHILD,
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
                eprintln!("[rust_widgets][windows] create_tool_bar failed (parent={parent})");
                return 0;
            }
            let widget_id = self.state.create_widget(
                WindowsHandleKind::ToolBar,
                "ToolBar",
                x,
                y,
                width,
                height,
            );
            self.bind_native_handle(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }
    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            use std::ptr::null_mut;
            use winapi::um::winuser::{
                CreateWindowExW, SS_LEFT, SS_NOPREFIX, WS_CHILD, WS_VISIBLE,
            };

            let parent_hwnd = match self.get_native_handle(parent) {
                Some(hwnd) => hwnd,
                None => return 0,
            };
            let class_name = Self::to_wide("Static");
            let text_wide = Self::to_wide(text);
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    text_wide.as_ptr(),
                    WS_VISIBLE | WS_CHILD | SS_LEFT | SS_NOPREFIX,
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
                eprintln!("[rust_widgets][windows] create_status_bar failed (parent={parent})");
                return 0;
            }
            let widget_id =
                self.state
                    .create_widget(WindowsHandleKind::StatusBar, text, x, y, width, height);
            self.bind_native_handle(widget_id, hwnd);
            return widget_id;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, text, x, y, width, height);
            0
        }
    }
    // ...implement other required methods as stubs...

    fn create_message_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_message_box not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, title, text, x, y, width, height);
            0
        }
    }

    fn create_file_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_file_dialog not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_color_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_color_dialog not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_font_dialog(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_font_dialog not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_spin_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_spin_box not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_list_view(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_list_view not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] create_scroll_area not implemented");
            0
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (parent, x, y, width, height);
            0
        }
    }

    fn set_clipboard_text(&self, _text: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] set_clipboard_text not implemented");
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = text;
            false
        }
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] get_clipboard_text not implemented");
            String::new()
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    }

    fn begin_drag(&self, _source_widget_id: ObjectId, _mime: &str, _payload: &[u8]) -> bool {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] begin_drag not implemented");
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (source_widget_id, mime, payload);
            false
        }
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] poll_drop_event not implemented");
            None
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }

    fn inject_drop_event(&self, _event: DropEvent) -> bool {
        #[cfg(target_os = "windows")]
        {
            eprintln!("[rust_widgets][windows] inject_drop_event not implemented");
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = event;
            false
        }
    }
}
// Core and platform types
// Stub for WindowsHandleKind enum (should be replaced with actual variants as needed)
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum WindowsHandleKind {
    Window,
    Button,
    Label,
    CheckBox,
    RadioButton,
    LineEdit,
    ListBox,
    Panel,
    MenuBar,
    Menu,
    ToolBar,
    StatusBar,
    ProgressBar,
    Slider,
    ComboBox,
    // Add other widget kinds as needed
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn rust_widgets_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use winapi::um::winuser::NMHDR;
    use winapi::um::winuser::{
        DefWindowProcW, GetDlgCtrlID, PostQuitMessage, WM_COMMAND, WM_DESTROY, WM_NOTIFY,
    };

    match msg {
        WM_COMMAND => {
            let command_id = (wparam & 0xFFFF) as u32;
            let notify_code = ((wparam >> 16) & 0xFFFF) as u32;
            if let Some(platform) = active_windows_platform() {
                if let Ok(map) = platform.menu_state.menu_command_to_item.lock() {
                    if let Some(item_id) = map.get(&command_id).copied() {
                        if let Ok(mut queue) = platform.menu_state.pending_menu_events.lock() {
                            queue.push_back(WidgetTriggerEvent {
                                widget_id: item_id,
                                kind: WidgetTriggerKind::Clicked,
                            });
                        }
                        return 0;
                    }
                }
                if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                    if let Some(widget_id) = map.get(&command_id).copied() {
                        if enqueue_control_notify_event(platform, widget_id, notify_code) {
                            return 0;
                        }
                    }
                }

                if lparam != 0 {
                    let hwnd_from = lparam as HWND;
                    let fallback_command_id = unsafe { GetDlgCtrlID(hwnd_from) } as u32;
                    if fallback_command_id != 0 {
                        if let Ok(map) = platform.menu_state.control_command_to_widget.lock() {
                            if let Some(widget_id) = map.get(&fallback_command_id).copied() {
                                if enqueue_control_notify_event(platform, widget_id, notify_code) {
                                    return 0;
                                }
                            }
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_NOTIFY => {
            if let Some(platform) = active_windows_platform() {
                let hdr = lparam as *const NMHDR;
                if !hdr.is_null() {
                    let hwnd_from = unsafe { (*hdr).hwndFrom };
                    let notify_code = unsafe { (*hdr).code };
                    if let Some(widget_id) = platform.widget_id_by_native_handle(hwnd_from) {
                        if let Some(kind) =
                            platform.state.kind_of(widget_id).and_then(|widget_kind| {
                                notify_kind_for_widget(widget_kind, notify_code)
                            })
                        {
                            if let Ok(mut events) = platform.menu_state.pending_widget_events.lock()
                            {
                                events.push_back(WidgetTriggerEvent { widget_id, kind });
                            }
                            return 0;
                        }
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(target_os = "windows")]
fn ensure_window_class_registered() {
    unsafe extern "system" {
        fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    }
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| unsafe {
        use std::ptr::null_mut;
        use winapi::um::winuser::{
            LoadCursorW, RegisterClassW, CS_HREDRAW, CS_VREDRAW, IDC_ARROW, WNDCLASSW,
        };

        let class_name = WindowsPlatform::to_wide("RustWidgetsWindowClass");
        let hinstance = GetModuleHandleW(std::ptr::null());
        let mut wnd_class: WNDCLASSW = std::mem::zeroed();
        wnd_class.style = CS_HREDRAW | CS_VREDRAW;
        wnd_class.lpfnWndProc = Some(rust_widgets_wnd_proc);
        wnd_class.hInstance = hinstance as _;
        wnd_class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wnd_class.lpszClassName = class_name.as_ptr();
        let _ = RegisterClassW(&wnd_class);
    });
}

#[cfg(target_os = "windows")]
static ACTIVE_WINDOWS_PLATFORM: OnceLock<usize> = OnceLock::new();

#[cfg(target_os = "windows")]
fn active_windows_platform() -> Option<&'static WindowsPlatform> {
    let ptr = *ACTIVE_WINDOWS_PLATFORM.get()? as *const WindowsPlatform;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &*ptr })
    }
}

#[cfg(target_os = "windows")]
fn control_notify_kind_for_widget(
    kind: WindowsHandleKind,
    notify_code: u32,
) -> Option<WidgetTriggerKind> {
    const BN_CLICKED: u32 = 0;
    const CBN_SELCHANGE: u32 = 1;
    const CBN_EDITCHANGE: u32 = 5;
    const EN_CHANGE: u32 = 0x0300;
    const LBN_SELCHANGE: u32 = 1;

    match kind {
        WindowsHandleKind::Button
        | WindowsHandleKind::CheckBox
        | WindowsHandleKind::RadioButton => {
            if notify_code == BN_CLICKED {
                Some(WidgetTriggerKind::Clicked)
            } else {
                None
            }
        }
        WindowsHandleKind::LineEdit => {
            if notify_code == EN_CHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ComboBox => {
            if notify_code == CBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else if notify_code == CBN_EDITCHANGE {
                Some(WidgetTriggerKind::ValueChanged)
            } else {
                None
            }
        }
        WindowsHandleKind::ListBox => {
            if notify_code == LBN_SELCHANGE {
                Some(WidgetTriggerKind::SelectionChanged)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn enqueue_control_notify_event(
    platform: &WindowsPlatform,
    widget_id: ObjectId,
    notify_code: u32,
) -> bool {
    let widget_kind = match platform.state.kind_of(widget_id) {
        Some(kind) => kind,
        None => return false,
    };

    let kind = match control_notify_kind_for_widget(widget_kind, notify_code) {
        Some(kind) => kind,
        None => return false,
    };

    if let Ok(mut events) = platform.menu_state.pending_widget_events.lock() {
        events.push_back(WidgetTriggerEvent { widget_id, kind });
        if widget_kind == WindowsHandleKind::ComboBox && kind == WidgetTriggerKind::SelectionChanged
        {
            events.push_back(WidgetTriggerEvent {
                widget_id,
                kind: WidgetTriggerKind::ValueChanged,
            });
        }
        return true;
    }
    false
}

#[cfg(target_os = "windows")]
fn notify_kind_for_widget(kind: WindowsHandleKind, notify_code: u32) -> Option<WidgetTriggerKind> {
    if kind == WindowsHandleKind::Slider {
        return Some(WidgetTriggerKind::ValueChanged);
    }
    if notify_code == 0 {
        return None;
    }
    None
}

// Helper to convert &str to wide string for Win32 API
impl WindowsPlatform {
    pub fn to_wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    pub fn get_native_handle(&self, id: u64) -> Option<HWND> {
        #[cfg(target_os = "windows")]
        {
            let handles = self.menu_state.handles.lock().ok()?;
            handles.get(&id).map(|&h| h as HWND)
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    }
    pub fn bind_native_handle(&self, id: u64, hwnd: HWND) {
        #[cfg(target_os = "windows")]
        {
            if let Ok(mut handles) = self.menu_state.handles.lock() {
                handles.insert(id, hwnd as usize);
            } else {
                // Handle lock error explicitly
                return;
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn bind_control_command(&self, widget_id: u64, hwnd: HWND) {
        use winapi::um::winuser::{SetWindowLongPtrW, GWLP_ID};

        let command_id = self
            .menu_state
            .next_command_id
            .fetch_add(1, Ordering::SeqCst) as u32;

        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_ID, command_id as isize);
        }

        if let Ok(mut map) = self.menu_state.control_command_to_widget.lock() {
            map.insert(command_id, widget_id);
        }
    }

    #[cfg(target_os = "windows")]
    pub fn widget_id_by_native_handle(&self, hwnd: HWND) -> Option<u64> {
        let handles = self.menu_state.handles.lock().ok()?;
        handles
            .iter()
            .find_map(|(widget_id, native)| ((*native as HWND) == hwnd).then_some(*widget_id))
    }
}

// Extension trait for downcasting Platform trait object to WindowsPlatform
pub trait PlatformDowncast {
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
}

impl PlatformDowncast for dyn super::Platform {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        // This is a stub; actual implementation may use Any or other trait object logic
        None
    }
}
// Removed unresolved import crate::state::BackendState

// Win32 API types and functions
#[cfg(target_os = "windows")]
use winapi::shared::windef::HMENU;
#[cfg(target_os = "windows")]
use winapi::shared::windef::HWND;
#[cfg(not(target_os = "windows"))]
type HWND = *mut std::ffi::c_void;
// Windows backend shell.

#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
// ...existing code...
// ...existing code...

/// Windows platform backend struct definition
pub struct WindowsPlatform {
    pub state: BackendState<WindowsHandleKind>,
    pub runtime_initialized: AtomicBool,
    pub runtime_running: AtomicBool,
    #[cfg(target_os = "windows")]
    pub menu_state: Win32MenuState,
    // Removed handle_state: Win32HandleState, as Win32HandleState is not defined in state.rs
}

#[allow(dead_code)]
pub struct Win32MenuState {
    // SAFETY: HWND is only used on the main thread, and Win32MenuState is not shared across threads in this context.
    handles: Mutex<HashMap<u64, usize>>,
    menu_owner_window: Mutex<HashMap<u64, u64>>,
    menu_command_to_item: Mutex<HashMap<u32, u64>>,
    control_command_to_widget: Mutex<HashMap<u32, u64>>,
    pending_menu_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    next_command_id: AtomicU64,
}

#[cfg(target_os = "windows")]
impl Win32MenuState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            menu_owner_window: Mutex::new(HashMap::new()),
            menu_command_to_item: Mutex::new(HashMap::new()),
            control_command_to_widget: Mutex::new(HashMap::new()),
            pending_menu_events: Mutex::new(VecDeque::new()),
            pending_widget_events: Mutex::new(VecDeque::new()),
            next_command_id: AtomicU64::new(1000),
        }
    }
}

#[cfg(target_os = "windows")]

// Extension trait for native Win32 Slider (Trackbar) integration
// Stub trait for native Win32 ProgressBar integration
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
            // Get platform instance and parent HWND
            let platform_instance =
                &*(platform as *const dyn super::Platform as *const WindowsPlatform);
            let parent_hwnd = platform_instance.get_native_handle(parent)?;
            let class: Vec<u16> = OsStr::new(PROGRESS_CLASS)
                .encode_wide()
                .chain(Some(0))
                .collect();
            let hwnd = CreateWindowExW(
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
            );
            if hwnd.is_null() {
                return None;
            }
            // Create widget and bind native handle
            let widget_id = platform_instance.state.create_widget(
                WindowsHandleKind::ProgressBar,
                "ProgressBar",
                x,
                y,
                width,
                height,
            );
            platform_instance.bind_native_handle(widget_id, hwnd);
            return Some(widget_id);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}

// Stub trait for native Win32 ComboBox integration
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
            let platform_instance =
                &*(platform as *const dyn super::Platform as *const WindowsPlatform);
            let parent_hwnd = platform_instance.get_native_handle(parent)?;
            let class = WindowsPlatform::to_wide("ComboBox");
            let dropdown_height = (height as i32).max(180);
            let hwnd = CreateWindowExW(
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
            );
            if hwnd.is_null() {
                return None;
            }
            let widget_id = platform_instance.state.create_widget(
                WindowsHandleKind::ComboBox,
                "ComboBox",
                x,
                y,
                width,
                height,
            );
            platform_instance.bind_native_handle(widget_id, hwnd);
            platform_instance.bind_control_command(widget_id, hwnd);
            return Some(widget_id);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (platform, parent, x, y, width, height);
        None
    }
}
impl WindowsPlatform {
    pub fn new() -> Self {
        WindowsPlatform {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
        }
    }
}
pub trait WindowsPlatformExtSlider {
    fn try_create_slider(
        platform: &dyn super::Platform,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<u64>;
}

impl WindowsPlatformExtSlider for WindowsPlatform {
    fn try_create_slider(
        platform: &dyn super::Platform,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Option<u64> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::commctrl::InitCommonControls;
            use winapi::um::winuser::{WS_BORDER, WS_CHILD, WS_VISIBLE};
            unsafe {
                InitCommonControls();
            }
            // Use a direct cast for demonstration; actual implementation should use Any for trait objects
            let this =
                unsafe { &*(platform as *const dyn super::Platform as *const WindowsPlatform) };
            if !this.state.contains_widget(parent) {
                return None;
            }
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;
            use std::ptr::null_mut;
            use winapi::um::commctrl::TRACKBAR_CLASS;
            use winapi::um::winuser::CreateWindowExW;

            let parent_hwnd = this.get_native_handle(parent)?;
            let class: Vec<u16> = OsStr::new(TRACKBAR_CLASS)
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
            let widget_id =
                this.state
                    .create_widget(WindowsHandleKind::Slider, "Slider", x, y, width, height);
            this.bind_native_handle(widget_id, hwnd);
            return Some(widget_id);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (platform, parent, x, y, width, height);
            None
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn control_notify_mapping_button_click_routes_clicked() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::Button, 0);
        assert_eq!(kind, Some(WidgetTriggerKind::Clicked));
    }

    #[test]
    fn control_notify_mapping_line_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::LineEdit, 0x0300);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }

    #[test]
    fn control_notify_mapping_combo_selection_routes_selection_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 1);
        assert_eq!(kind, Some(WidgetTriggerKind::SelectionChanged));
    }

    #[test]
    fn control_notify_mapping_combo_edit_change_routes_value_changed() {
        let kind = control_notify_kind_for_widget(WindowsHandleKind::ComboBox, 5);
        assert_eq!(kind, Some(WidgetTriggerKind::ValueChanged));
    }

    #[test]
    fn combo_selection_notify_enqueues_selection_and_value_events() {
        let platform = WindowsPlatform::new();
        let combo =
            platform
                .state
                .create_widget(WindowsHandleKind::ComboBox, "ComboBox", 0, 0, 120, 24);

        assert!(enqueue_control_notify_event(&platform, combo, 1));

        let mut queue = platform
            .menu_state
            .pending_widget_events
            .lock()
            .expect("windows trigger queue lock poisoned");
        assert_eq!(queue.len(), 2);
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent {
                widget_id: combo,
                kind: WidgetTriggerKind::SelectionChanged,
            })
        );
        assert_eq!(
            queue.pop_front(),
            Some(WidgetTriggerEvent {
                widget_id: combo,
                kind: WidgetTriggerKind::ValueChanged,
            })
        );
    }
}
