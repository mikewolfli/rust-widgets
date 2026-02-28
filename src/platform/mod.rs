//! Platform abstraction for desktop/embedded/mobile families.

pub mod harmony;
pub mod linux;
pub mod macos;
pub mod windows;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::core::{ObjectId, PlatformFamily};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetTriggerKind {
    Unknown = 0,
    Clicked = 1,
    ValueChanged = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetTriggerEvent {
    pub widget_id: ObjectId,
    pub kind: WidgetTriggerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopBackend {
    Win32,
    Cocoa,
    Gtk,
    HarmonyDesktop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileBackend {
    Android,
    Ios,
    HarmonyMobile,
}

pub trait Platform: Send + Sync {
    fn backend_name(&self) -> &'static str;
    fn family(&self) -> PlatformFamily;
    fn init(&self);
    fn run(&self);
    fn quit(&self);
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    fn create_button(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId;
    fn create_checkbox(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, text, x, y, width, height)
    }
    fn create_line_edit(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, text, x, y, width, height)
    }
    fn create_menu_bar(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
        false
    }
    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, _shortcut: Option<&str>) -> ObjectId {
        self.create_menu(parent_menu, text, 0, 0, 0, 0)
    }
    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        None
    }
    fn inject_menu_trigger(&self, _menu_item_id: ObjectId) -> bool {
        false
    }
    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        None
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.poll_widget_triggered().map(|widget_id| WidgetTriggerEvent {
            widget_id,
            kind: WidgetTriggerKind::Unknown,
        })
    }
    fn inject_widget_trigger_event(&self, _widget_id: ObjectId, _kind: WidgetTriggerKind) -> bool {
        false
    }
    fn create_tool_bar(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, "ToolBar", x, y, width, height)
    }
    fn create_status_bar(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_button(parent, text, x, y, width, height)
    }
    fn show_widget(&self, widget_id: ObjectId);
    fn hide_widget(&self, widget_id: ObjectId);
    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32);
    fn set_widget_text(&self, widget_id: ObjectId, text: &str);
    fn get_widget_text(&self, widget_id: ObjectId) -> String;
    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool);
    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool;
    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool);
    fn is_widget_visible(&self, widget_id: ObjectId) -> bool;
}

pub trait MobilePlatformExtension: Send + Sync {
    fn mobile_backend(&self) -> MobileBackend;
    fn attach_to_native_view(&self, _native_handle: usize) -> bool;
}

#[derive(Default)]
struct WidgetState {
    text: String,
    visible: bool,
    enabled: bool,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

struct MenuNodeState {
    text: String,
}

pub struct StubPlatform {
    backend: &'static str,
    family: PlatformFamily,
    next_id: AtomicU64,
    widgets: Mutex<HashMap<ObjectId, WidgetState>>,
    menu_nodes: Mutex<HashMap<ObjectId, MenuNodeState>>,
    menu_events: Mutex<VecDeque<ObjectId>>,
    widget_events: Mutex<VecDeque<WidgetTriggerEvent>>,
}

impl StubPlatform {
    pub fn new(backend: &'static str, family: PlatformFamily) -> Self {
        Self {
            backend,
            family,
            next_id: AtomicU64::new(1),
            widgets: Mutex::new(HashMap::new()),
            menu_nodes: Mutex::new(HashMap::new()),
            menu_events: Mutex::new(VecDeque::new()),
            widget_events: Mutex::new(VecDeque::new()),
        }
    }

    fn new_id(&self) -> ObjectId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Platform for StubPlatform {
    fn backend_name(&self) -> &'static str { self.backend }
    fn family(&self) -> PlatformFamily { self.family }
    fn init(&self) {}
    fn run(&self) {}
    fn quit(&self) {}

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.new_id();
        self.widgets.lock().expect("platform lock poisoned").insert(id, WidgetState {
            text: title.to_string(),
            visible: true,
            enabled: true,
            x,
            y,
            width,
            height,
        });
        id
    }

    fn create_button(&self, _parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.new_id();
        self.widgets.lock().expect("platform lock poisoned").insert(id, WidgetState {
            text: text.to_string(),
            visible: true,
            enabled: true,
            x,
            y,
            width,
            height,
        });
        id
    }

    fn create_menu_bar(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.create_button(parent, "MenuBar", x, y, width, height);
        self.menu_nodes.lock().expect("platform lock poisoned").insert(id, MenuNodeState {
            text: "MenuBar".to_string(),
        });
        id
    }

    fn create_menu(&self, parent: ObjectId, text: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.create_button(parent, text, x, y, width, height);
        self.menu_nodes.lock().expect("platform lock poisoned").insert(id, MenuNodeState {
            text: text.to_string(),
        });
        id
    }

    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        let widgets = self.widgets.lock().expect("platform lock poisoned");
        widgets.contains_key(&window) && widgets.contains_key(&menu_bar)
    }

    fn menu_add_item(&self, parent_menu: ObjectId, text: &str, shortcut: Option<&str>) -> ObjectId {
        let id = self.create_menu(parent_menu, text, 0, 0, 0, 0);
        self.menu_nodes.lock().expect("platform lock poisoned").insert(id, MenuNodeState {
            text: text.to_string(),
        });
        let _ = shortcut;
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        let mut events = self.menu_events.lock().expect("platform lock poisoned");
        events.pop_front()
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if !self
            .menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&menu_item_id)
        {
            return false;
        }
        self.menu_events
            .lock()
            .expect("platform lock poisoned")
            .push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.widget_events
            .lock()
            .expect("platform lock poisoned")
            .pop_front()
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        if !self
            .widgets
            .lock()
            .expect("platform lock poisoned")
            .contains_key(&widget_id)
        {
            return false;
        }
        self.widget_events
            .lock()
            .expect("platform lock poisoned")
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn show_widget(&self, widget_id: ObjectId) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.visible = true;
        }
    }

    fn hide_widget(&self, widget_id: ObjectId) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.visible = false;
        }
    }

    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.x = x;
            widget.y = y;
            widget.width = width;
            widget.height = height;
        }
    }

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.text = text.to_string();
        }
        if let Some(node) = self.menu_nodes.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            node.text = text.to_string();
        }
    }

    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.text.clone())
            .unwrap_or_default()
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.enabled = enabled;
        }
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.enabled)
            .unwrap_or(false)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        if let Some(widget) = self.widgets.lock().expect("platform lock poisoned").get_mut(&widget_id) {
            widget.visible = visible;
        }
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.widgets
            .lock()
            .expect("platform lock poisoned")
            .get(&widget_id)
            .map(|widget| widget.visible)
            .unwrap_or(false)
    }
}

#[cfg(target_os = "windows")]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(windows::WindowsPlatform::new())
}

#[cfg(target_os = "macos")]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(macos::MacOSPlatform::new())
}

#[cfg(target_os = "linux")]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(linux::LinuxPlatform::new())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn create_native_platform() -> Box<dyn Platform> {
    Box::new(harmony::HarmonyPlatform::new())
}

static PLATFORM: OnceLock<Box<dyn Platform>> = OnceLock::new();

pub fn get_platform() -> &'static dyn Platform {
    PLATFORM.get_or_init(create_native_platform).as_ref()
}

pub fn init() { get_platform().init(); }
pub fn run() { get_platform().run(); }
pub fn quit() { get_platform().quit(); }
