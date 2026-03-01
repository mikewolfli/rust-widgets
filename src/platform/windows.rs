//! Windows backend shell.

#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "windows")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
#[cfg(target_os = "windows")]
use std::sync::Mutex;
#[cfg(target_os = "windows")]
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use crate::core::PlatformFamily;

use super::state::BackendState;
use super::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};

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

/// Windows desktop platform adapter.
pub struct WindowsPlatform {
    state: BackendState<WindowsHandleKind>,
    runtime_initialized: AtomicBool,
    runtime_running: AtomicBool,
    #[cfg(target_os = "windows")]
    menu_state: Win32MenuState,
    #[cfg(target_os = "windows")]
    handle_state: Win32HandleState,
}

/// Logical kind used by Windows backend state validation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum WindowsHandleKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    MenuBar,
    Menu,
    MenuItem,
}

impl WindowsPlatform {
    /// Create Windows backend state and optional native integration stores.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            runtime_initialized: AtomicBool::new(false),
            runtime_running: AtomicBool::new(false),
            #[cfg(target_os = "windows")]
            menu_state: Win32MenuState::new(),
            #[cfg(target_os = "windows")]
            handle_state: Win32HandleState::new(),
        }
    }

    /// Return `true` when widget id exists and has expected kind.
    fn is_kind(&self, widget_id: u64, kind: WindowsHandleKind) -> bool {
        self.state.is_kind(widget_id, kind)
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
    fn backend_name(&self) -> &'static str { "win32" }
    fn family(&self) -> PlatformFamily { PlatformFamily::Desktop }
    fn init(&self) {
        self.runtime_initialized.store(true, Ordering::SeqCst);
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
        if !self.runtime_initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime_running.store(true, Ordering::SeqCst);
        while self.runtime_running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime_running.store(false, Ordering::SeqCst);
        #[cfg(target_os = "windows")]
        unsafe {
            PostQuitMessage(0);
        }
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let window_id = self
            .state
            .create_widget(WindowsHandleKind::Window, title, x, y, width, height);
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
        if !self.state.contains_widget(parent) {
            return 0;
        }
        let widget_id = self
            .state
            .create_widget(WindowsHandleKind::Button, text, x, y, width, height);
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
        if !self.state.contains_widget(parent) {
            return 0;
        }
        let widget_id = self
            .state
            .create_widget(WindowsHandleKind::CheckBox, text, x, y, width, height);
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
        if !self.state.contains_widget(parent) {
            return 0;
        }
        let widget_id = self
            .state
            .create_widget(WindowsHandleKind::LineEdit, text, x, y, width, height);
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
        if !self.is_kind(parent, WindowsHandleKind::Window) {
            return 0;
        }
        let menu_id = self
            .state
            .create_widget(WindowsHandleKind::MenuBar, "MenuBar", x, y, width, height);
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
        if !(self.is_kind(parent, WindowsHandleKind::MenuBar) || self.is_kind(parent, WindowsHandleKind::Menu)) {
            return 0;
        }
        let menu_id = self
            .state
            .create_widget(WindowsHandleKind::Menu, text, x, y, width, height);
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
        if !(self.is_kind(window, WindowsHandleKind::Window) && self.is_kind(menu_bar, WindowsHandleKind::MenuBar)) {
            return false;
        }
        #[cfg(target_os = "windows")]
        unsafe {
            if let (Some(hwnd), Some(menu)) = (self.get_native_handle(window), self.get_menu_handle(menu_bar)) {
                if SetMenu(hwnd, menu) != 0 {
                    DrawMenuBar(hwnd);
                    return true;
                }
            }
        }
        true
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !self.is_kind(parent_menu, WindowsHandleKind::Menu) {
            return 0;
        }
        let item_id = self
            .state
            .create_widget(WindowsHandleKind::MenuItem, text, 0, 0, 0, 0);
        #[cfg(not(target_os = "windows"))]
        {
            let _ = shortcut;
        }
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
        if let Some(item_id) = self.state.pop_menu_event() {
            return Some(item_id);
        }
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
        None
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !self.is_kind(menu_item_id, WindowsHandleKind::MenuItem) {
            return false;
        }
        self.state.push_menu_event(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        if let Some(event) = self.state.pop_widget_event() {
            return Some(event);
        }
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
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                ShowWindow(hwnd, SW_SHOW);
            }
        }
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                MoveWindow(hwnd, x, y, width as i32, height as i32, 1);
            }
        }
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        let is_line_edit = matches!(self.state.kind_of(widget_id), Some(WindowsHandleKind::LineEdit));
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                let text_w = Self::to_wide(text);
                SetWindowTextW(hwnd, text_w.as_ptr());
            }
        }
        if is_line_edit {
            self.state.push_widget_event(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::ValueChanged,
            });
        }
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
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
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
    }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
        #[cfg(target_os = "windows")]
        unsafe {
            if let Some(hwnd) = self.get_native_handle(widget_id) {
                return IsWindowVisible(hwnd) != 0;
            }
        }
        self.state.visible(widget_id)
    }

    fn set_widget_ime_enabled(&self, widget_id: u64, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: u64) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: u64, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: u64) -> String {
        self.state.accessibility_name(widget_id)
    }

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
}
