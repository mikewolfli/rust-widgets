//! Windows backend shell.

#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::collections::VecDeque;
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;

use crate::core::PlatformFamily;

#[cfg(target_os = "windows")]
use super::WidgetTriggerKind;
use super::{Platform, StubPlatform, WidgetTriggerEvent};

#[cfg(target_os = "windows")]
use std::ptr::null_mut;

#[cfg(target_os = "windows")]
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};

#[cfg(target_os = "windows")]
use winapi::shared::windef::{HMENU, HWND};

#[cfg(target_os = "windows")]
use winapi::um::winuser::{
    AppendMenuW, CreateMenu, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DispatchMessageW,
    DrawMenuBar, EnableWindow, IsWindowEnabled, IsWindowVisible, MSG, MF_POPUP, MF_STRING,
    MoveWindow, PM_REMOVE, PeekMessageW, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOW,
    SetMenu, SetWindowTextW, ShowWindow, TranslateMessage, UpdateWindow,
    BS_AUTOCHECKBOX, ES_LEFT, WM_CLOSE, WM_COMMAND, WNDCLASSW, WS_BORDER, WS_CHILD,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    BN_CLICKED, EN_CHANGE,
};

#[cfg(target_os = "windows")]
struct Win32MenuState {
    /// Logical menu id -> native HMENU mapping.
    handles: Mutex<HashMap<u64, HMENU>>,
    /// Command id -> logical menu item id.
    menu_command_to_item: Mutex<HashMap<u32, u64>>,
    /// Command id -> logical widget id.
    control_command_to_widget: Mutex<HashMap<u32, u64>>,
    /// FIFO menu trigger queue.
    pending_menu_events: Mutex<VecDeque<u64>>,
    /// FIFO typed widget trigger queue.
    pending_widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
    /// Source of unique Win32 command ids.
    next_command_id: AtomicU64,
}

#[cfg(target_os = "windows")]
impl Win32MenuState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            menu_command_to_item: Mutex::new(HashMap::new()),
            control_command_to_widget: Mutex::new(HashMap::new()),
            pending_menu_events: Mutex::new(VecDeque::new()),
            pending_widget_events: Mutex::new(VecDeque::new()),
            next_command_id: AtomicU64::new(1000),
        }
    }
}

#[cfg(target_os = "windows")]
struct Win32HandleState {
    /// Logical widget id -> native HWND mapping.
    handles: Mutex<HashMap<u64, HWND>>,
}

#[cfg(target_os = "windows")]
impl Win32HandleState {
    fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }
}

pub struct WindowsPlatform {
    inner: StubPlatform,
    #[cfg(target_os = "windows")]
    menu_state: Win32MenuState,
    #[cfg(target_os = "windows")]
    handle_state: Win32HandleState,
}

impl WindowsPlatform {
    /// Create Windows backend with stub state and native state stores.
    pub fn new() -> Self {
        Self {
            inner: StubPlatform::new("win32", PlatformFamily::Desktop),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
            #[cfg(target_os = "windows")]
            handle_state: Win32HandleState::new(),
        }
    }

    #[cfg(target_os = "windows")]
    fn to_wide(text: &str) -> Vec<u16> {
        // Win32 APIs consume UTF-16 zero-terminated strings.
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[cfg(target_os = "windows")]
    fn register_window_class() -> bool {
        static REGISTERED: OnceLock<bool> = OnceLock::new();
        *REGISTERED.get_or_init(|| unsafe {
            let class_name = Self::to_wide("RustWidgetsWindowClass");
            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(widgets_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: null_mut(),
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null_mut(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&wc) != 0
        })
    }

    #[cfg(target_os = "windows")]
    fn bind_native_handle(&self, widget_id: u64, hwnd: HWND) {
        self.handle_state
            .handles
            .lock()
            .expect("windows handle lock poisoned")
            .insert(widget_id, hwnd);
    }

    #[cfg(target_os = "windows")]
    fn get_native_handle(&self, widget_id: u64) -> Option<HWND> {
        self.handle_state
            .handles
            .lock()
            .expect("windows handle lock poisoned")
            .get(&widget_id)
            .copied()
    }

    #[cfg(target_os = "windows")]
    fn create_native_control(
        &self,
        class_name: &str,
        text: &str,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        extra_style: u32,
        command_id: u32,
    ) -> Option<HWND> {
        let parent_hwnd = self.get_native_handle(parent)?;
        unsafe {
            let class_w = Self::to_wide(class_name);
            let text_w = Self::to_wide(text);
            let hwnd = CreateWindowExW(
                0,
                class_w.as_ptr(),
                text_w.as_ptr(),
                WS_CHILD | WS_VISIBLE | extra_style,
                x,
                y,
                width as i32,
                height as i32,
                parent_hwnd,
                command_id as usize as HMENU,
                null_mut(),
                null_mut(),
            );
            if hwnd.is_null() {
                None
            } else {
                Some(hwnd)
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn get_menu_handle(&self, menu_id: u64) -> Option<HMENU> {
        self.menu_state
            .handles
            .lock()
            .expect("windows menu lock poisoned")
            .get(&menu_id)
            .copied()
    }

    #[cfg(target_os = "windows")]
    fn bind_menu_handle(&self, menu_id: u64, handle: HMENU) {
        self.menu_state
            .handles
            .lock()
            .expect("windows menu lock poisoned")
            .insert(menu_id, handle);
    }

    #[cfg(target_os = "windows")]
    fn try_poll_native_menu_command(&self) -> Option<u64> {
        // Drain pending Win32 messages and map WM_COMMAND to logical events.
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_COMMAND {
                    let command_id = (msg.wParam as usize & 0xffff) as u32;
                    let notify_code = ((msg.wParam as usize >> 16) & 0xffff) as u32;
                    if let Some(item_id) = self
                        .menu_state
                        .menu_command_to_item
                        .lock()
                        .expect("windows command map lock poisoned")
                        .get(&command_id)
                        .copied()
                    {
                        self.menu_state
                            .pending_menu_events
                            .lock()
                            .expect("windows event lock poisoned")
                            .push_back(item_id);
                    }

                    if notify_code == BN_CLICKED || notify_code == EN_CHANGE || notify_code == 0 {
                        let widget_id = self
                            .menu_state
                            .control_command_to_widget
                            .lock()
                            .expect("windows command map lock poisoned")
                            .get(&command_id)
                            .copied();
                        if let Some(widget_id) = widget_id {
                            let kind = if notify_code == EN_CHANGE {
                                WidgetTriggerKind::ValueChanged
                            } else {
                                WidgetTriggerKind::Clicked
                            };
                            self.menu_state
                                .pending_widget_events
                                .lock()
                                .expect("windows event lock poisoned")
                                .push_back(WidgetTriggerEvent { widget_id, kind });
                        }
                    }
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        self.menu_state
            .pending_menu_events
            .lock()
            .expect("windows event lock poisoned")
            .pop_front()
    }

    #[cfg(target_os = "windows")]
    fn try_poll_native_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        let _ = self.try_poll_native_menu_command();
        self.menu_state
            .pending_widget_events
            .lock()
            .expect("windows event lock poisoned")
            .pop_front()
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn widgets_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLOSE => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

impl Platform for WindowsPlatform {
    fn backend_name(&self) -> &'static str { self.inner.backend_name() }
    fn family(&self) -> PlatformFamily { self.inner.family() }
    fn init(&self) {
        self.inner.init();
        #[cfg(target_os = "windows")]
        {
            let _ = Self::register_window_class();
        }
    }
    fn run(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        self.inner.run();
    }
    fn quit(&self) {
        #[cfg(target_os = "windows")]
        unsafe {
            PostQuitMessage(0);
        }
        self.inner.quit();
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let window_id = self.inner.create_window(title, x, y, width, height);
        #[cfg(target_os = "windows")]
        unsafe {
            if Self::register_window_class() {
                let class_name = Self::to_wide("RustWidgetsWindowClass");
                let title_w = Self::to_wide(title);
                let hwnd = CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    title_w.as_ptr(),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    x,
                    y,
                    width as i32,
                    height as i32,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                );
                if !hwnd.is_null() {
                    self.bind_native_handle(window_id, hwnd);
                    ShowWindow(hwnd, SW_SHOW);
                    UpdateWindow(hwnd);
                }
            }
        }
        window_id
    }
    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let widget_id = self.inner.create_button(parent, text, x, y, width, height);
        #[cfg(target_os = "windows")]
        {
            let command_id = self
                .menu_state
                .next_command_id
                .fetch_add(1, Ordering::Relaxed) as u32;
            if let Some(hwnd) = self.create_native_control("BUTTON", text, parent, x, y, width, height, 0, command_id) {
                self.bind_native_handle(widget_id, hwnd);
                self.menu_state
                    .control_command_to_widget
                    .lock()
                    .expect("windows command map lock poisoned")
                    .insert(command_id, widget_id);
            }
        }
        widget_id
    }
    fn create_checkbox(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let widget_id = self.inner.create_checkbox(parent, text, x, y, width, height);
        #[cfg(target_os = "windows")]
        {
            let command_id = self
                .menu_state
                .next_command_id
                .fetch_add(1, Ordering::Relaxed) as u32;
            if let Some(hwnd) = self.create_native_control(
                "BUTTON",
                text,
                parent,
                x,
                y,
                width,
                height,
                BS_AUTOCHECKBOX,
                command_id,
            ) {
                self.bind_native_handle(widget_id, hwnd);
                self.menu_state
                    .control_command_to_widget
                    .lock()
                    .expect("windows command map lock poisoned")
                    .insert(command_id, widget_id);
            }
        }
        widget_id
    }
    fn create_line_edit(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let widget_id = self.inner.create_line_edit(parent, text, x, y, width, height);
        #[cfg(target_os = "windows")]
        {
            let command_id = self
                .menu_state
                .next_command_id
                .fetch_add(1, Ordering::Relaxed) as u32;
            if let Some(hwnd) = self.create_native_control(
                "EDIT",
                text,
                parent,
                x,
                y,
                width,
                height,
                ES_LEFT | WS_BORDER,
                command_id,
            ) {
                self.bind_native_handle(widget_id, hwnd);
                self.menu_state
                    .control_command_to_widget
                    .lock()
                    .expect("windows command map lock poisoned")
                    .insert(command_id, widget_id);
            }
        }
        widget_id
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let menu_id = self.inner.create_menu_bar(parent, x, y, width, height);
        #[cfg(target_os = "windows")]
        unsafe {
            let menu = CreateMenu();
            if !menu.is_null() {
                self.bind_menu_handle(menu_id, menu);
            }
        }
        menu_id
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let menu_id = self.inner.create_menu(parent, text, x, y, width, height);
        #[cfg(target_os = "windows")]
        unsafe {
            let popup = CreatePopupMenu();
            if !popup.is_null() {
                self.bind_menu_handle(menu_id, popup);
                if let Some(parent_handle) = self.get_menu_handle(parent) {
                    let title = Self::to_wide(text);
                    AppendMenuW(parent_handle, MF_POPUP, popup as usize, title.as_ptr());
                }
            }
        }
        menu_id
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        let attached = self.inner.attach_menu_bar_to_window(window, menu_bar);
        #[cfg(target_os = "windows")]
        unsafe {
            if let (Some(hwnd), Some(menu)) = (self.get_native_handle(window), self.get_menu_handle(menu_bar)) {
                if SetMenu(hwnd, menu) != 0 {
                    DrawMenuBar(hwnd);
                    return true;
                }
            }
        }
        attached
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        let item_id = self.inner.menu_add_item(parent_menu, text, shortcut);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(menu_handle) = self.get_menu_handle(parent_menu) {
                let command_id = self
                    .menu_state
                    .next_command_id
                    .fetch_add(1, Ordering::Relaxed) as u32;
                let label = match shortcut {
                    Some(s) if !s.is_empty() => format!("{text}\t{s}"),
                    _ => text.to_string(),
                };
                let wide = Self::to_wide(&label);
                AppendMenuW(menu_handle, MF_STRING, command_id as usize, wide.as_ptr());
                self.menu_state
                    .menu_command_to_item
                    .lock()
                    .expect("windows command map lock poisoned")
                    .insert(command_id, item_id);
            }
        }
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        #[cfg(target_os = "windows")]
        {
            if let Some(item_id) = self
                .menu_state
                .pending_menu_events
                .lock()
                .expect("windows event lock poisoned")
                .pop_front()
            {
                return Some(item_id);
            }
            if let Some(item_id) = self.try_poll_native_menu_command() {
                return Some(item_id);
            }
        }
        self.inner.poll_menu_triggered()
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        #[cfg(target_os = "windows")]
        {
            if let Some(event) = self
                .menu_state
                .pending_widget_events
                .lock()
                .expect("windows event lock poisoned")
                .pop_front()
            {
                return Some(event);
            }
            if let Some(event) = self.try_poll_native_widget_trigger_event() {
                return Some(event);
            }
        }
        None
    }
    fn show_widget(&self, widget_id: u64) {
        self.inner.show_widget(widget_id);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                ShowWindow(hwnd, SW_SHOW);
            }
        }
    }
    fn hide_widget(&self, widget_id: u64) {
        self.inner.hide_widget(widget_id);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.inner.set_widget_geometry(widget_id, x, y, width, height);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                MoveWindow(hwnd, x, y, width as i32, height as i32, 1);
            }
        }
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.inner.set_widget_text(widget_id, text);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let text_w = Self::to_wide(text);
                SetWindowTextW(hwnd, text_w.as_ptr());
            }
        }
    }
    fn get_widget_text(&self, widget_id: u64) -> String { self.inner.get_widget_text(widget_id) }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.inner.set_widget_enabled(widget_id, enabled);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                EnableWindow(hwnd, if enabled { 1 } else { 0 });
            }
        }
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                return IsWindowEnabled(hwnd) != 0;
            }
        }
        self.inner.is_widget_enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) { self.inner.set_widget_visible(widget_id, visible); }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                return IsWindowVisible(hwnd) != 0;
            }
        }
        self.inner.is_widget_visible(widget_id)
    }
}
