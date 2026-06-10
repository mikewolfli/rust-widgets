use super::types::{MacOSObjc2Platform, MacObjc2HandleKind};
use crate::platform::{Platform, WidgetTriggerEvent, WidgetTriggerKind};

impl Platform for MacOSObjc2Platform {
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::MenuBar, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(MacObjc2HandleKind::MenuBar | MacObjc2HandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ToolBar, "ToolBar", x, y, width, height)
    }
    fn create_status_bar(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(MacObjc2HandleKind::Window)) {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::StatusBar, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        // Reject invalid kind combinations to avoid stale menu-bar ownership mappings.
        if matches!(self.kind_of(window), Some(MacObjc2HandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(MacObjc2HandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("mac objc2 menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(MacObjc2HandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(MacObjc2HandleKind::MenuItem, text, 0, 0, 0, 0);
        // Shortcut parsing is intentionally deferred until native objc2 menu bridging is finalized.
        let _ = shortcut;
        let mut menus = self.menus.lock().expect("mac objc2 menu lock poisoned");
        menus.menu_children.entry(parent_menu).or_default().push(item_id);
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus.lock().expect("mac objc2 menu lock poisoned").pending_menu_events.pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(MacObjc2HandleKind::MenuItem)) {
            return false;
        }
        // Queue-only bridge preserves deterministic trigger order across test and native paths.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus.lock().expect("mac objc2 menu lock poisoned").pending_widget_events.pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        // Normalize native control notifications into typed cross-platform trigger events.
        self.menus
            .lock()
            .expect("mac objc2 menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
}
