//! Harmony desktop backend shell.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::core::PlatformFamily;

use super::{Platform, StubPlatform, WidgetTriggerEvent, WidgetTriggerKind};

#[derive(Clone, Copy, PartialEq, Eq)]
enum HarmonyHandleKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    MenuBar,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
}

#[derive(Default)]
struct HarmonyMenuState {
    /// Tracks menu bar attachment by window id.
    attached_menu_bar: HashMap<u64, u64>,
    /// Maintains menu tree relationships for backend-side validation.
    menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO menu trigger queue, filled by native bridge injection APIs.
    pending_menu_events: VecDeque<u64>,
    /// FIFO typed widget trigger queue, filled by bridge callbacks and local fallbacks.
    pending_widget_events: VecDeque<WidgetTriggerEvent>,
}

pub struct HarmonyPlatform {
    inner: StubPlatform,
    kinds: Mutex<HashMap<u64, HarmonyHandleKind>>,
    menus: Mutex<HarmonyMenuState>,
}

impl HarmonyPlatform {
    pub fn new() -> Self {
        Self {
            inner: StubPlatform::new("harmony-desktop", PlatformFamily::Desktop),
            kinds: Mutex::new(HashMap::new()),
            menus: Mutex::new(HarmonyMenuState::default()),
        }
    }

    fn set_kind(&self, id: u64, kind: HarmonyHandleKind) {
        self.kinds
            .lock()
            .expect("harmony kind lock poisoned")
            .insert(id, kind);
    }

    fn kind_of(&self, id: u64) -> Option<HarmonyHandleKind> {
        self.kinds
            .lock()
            .expect("harmony kind lock poisoned")
            .get(&id)
            .copied()
    }
}

impl Platform for HarmonyPlatform {
    fn backend_name(&self) -> &'static str { self.inner.backend_name() }
    fn family(&self) -> PlatformFamily { self.inner.family() }
    fn init(&self) { self.inner.init(); }
    fn run(&self) { self.inner.run(); }
    fn quit(&self) { self.inner.quit(); }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_window(title, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::Window);
        id
    }

    fn create_button(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_button(parent, text, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::Button);
        id
    }

    fn create_checkbox(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_checkbox(parent, text, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::CheckBox);
        id
    }

    fn create_line_edit(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_line_edit(parent, text, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::LineEdit);
        id
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_menu_bar(parent, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::MenuBar);
        id
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_menu(parent, text, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::Menu);
        self.menus
            .lock()
            .expect("harmony menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_tool_bar(parent, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::ToolBar);
        id
    }

    fn create_status_bar(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.inner.create_status_bar(parent, text, x, y, width, height);
        self.set_kind(id, HarmonyHandleKind::StatusBar);
        id
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        let attached = self.inner.attach_menu_bar_to_window(window, menu_bar);
        // Backend-side validation keeps behavior aligned with native backends.
        if matches!(self.kind_of(window), Some(HarmonyHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(HarmonyHandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("harmony menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        attached
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        let item_id = self.inner.menu_add_item(parent_menu, text, shortcut);
        self.set_kind(item_id, HarmonyHandleKind::MenuItem);
        let mut menus = self.menus.lock().expect("harmony menu lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(item_id);
        item_id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        let mut menus = self.menus.lock().expect("harmony menu lock poisoned");
        if let Some(item_id) = menus.pending_menu_events.pop_front() {
            return Some(item_id);
        }
        drop(menus);
        self.inner.poll_menu_triggered()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only menu items may generate menu trigger events.
        if !matches!(self.kind_of(menu_item_id), Some(HarmonyHandleKind::MenuItem)) {
            return false;
        }
        self.menus
            .lock()
            .expect("harmony menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        let mut menus = self.menus.lock().expect("harmony menu lock poisoned");
        if let Some(event) = menus.pending_widget_events.pop_front() {
            return Some(event);
        }
        drop(menus);
        self.inner.poll_widget_trigger_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        // Any known widget may enqueue a typed trigger event.
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock()
            .expect("harmony menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn show_widget(&self, widget_id: u64) { self.inner.show_widget(widget_id); }

    fn hide_widget(&self, widget_id: u64) { self.inner.hide_widget(widget_id); }

    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.inner.set_widget_geometry(widget_id, x, y, width, height);
    }

    fn set_widget_text(&self, widget_id: u64, text: &str) {
        self.inner.set_widget_text(widget_id, text);
        // Keep line-edit behavior consistent with value-changed semantics.
        if matches!(self.kind_of(widget_id), Some(HarmonyHandleKind::LineEdit)) {
            self.menus
                .lock()
                .expect("harmony menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent {
                    widget_id,
                    kind: WidgetTriggerKind::ValueChanged,
                });
        }
    }

    fn get_widget_text(&self, widget_id: u64) -> String { self.inner.get_widget_text(widget_id) }

    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) { self.inner.set_widget_enabled(widget_id, enabled); }

    fn is_widget_enabled(&self, widget_id: u64) -> bool { self.inner.is_widget_enabled(widget_id) }

    fn set_widget_visible(&self, widget_id: u64, visible: bool) { self.inner.set_widget_visible(widget_id, visible); }

    fn is_widget_visible(&self, widget_id: u64) -> bool { self.inner.is_widget_visible(widget_id) }
}
