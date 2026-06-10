use super::types::{LinuxHandleKind, LinuxPlatform};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use std::sync::Arc;

impl LinuxPlatform {
    pub(crate) fn create_menu_bar_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::MenuBar, "MenuBar", x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_bar = gtk::MenuBar::new();
            let widget = menu_bar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            native.menu_bars.insert(id, menu_bar);
            native.widgets.insert(id, widget);
            let _ = parent;
            let _ = (x, y, width, height);
        }
        id
    }
    pub(crate) fn create_menu_impl(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::MenuBar | LinuxHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Menu, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .menu_children
            .entry(parent)
            .or_default()
            .push(id);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu = gtk::Menu::new();
            let menu_item = gtk::MenuItem::with_label(text);
            menu_item.set_submenu(Some(&menu));
            let mut native = self.native.lock_guard();
            if let Some(menu_bar) = native.menu_bars.get(&parent) {
                menu_bar.append(&menu_item);
            } else if let Some(parent_menu) = native.menus.get(&parent) {
                parent_menu.append(&menu_item);
            }
            native.widgets.insert(id, menu_item.clone().upcast::<gtk::Widget>());
            native.menus.insert(id, menu);
            let _ = (x, y, width, height);
        }
        id
    }
    pub(crate) fn create_tool_bar_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ToolBar, "ToolBar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            toolbar.set_size_request(width as i32, height as i32);
            let widget = toolbar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&toolbar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_status_bar_impl(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::StatusBar, text, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = gtk::Label::new(Some(text));
            label.set_size_request(width as i32, height as i32);
            let widget = label.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&label, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn attach_menu_bar_to_window_impl(&self, window: u64, menu_bar: u64) -> bool {
        if matches!(self.kind_of(window), Some(LinuxHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(LinuxHandleKind::MenuBar))
        {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            {
                let native = self.native.lock_guard();
                if let (Some(root), Some(bar)) =
                    (native.root_boxes.get(&window), native.menu_bars.get(&menu_bar))
                {
                    root.pack_start(bar, false, false, 0);
                    root.reorder_child(bar, 0);
                    bar.show_all();
                }
            }
            return true;
        }
        false
    }
    pub(crate) fn menu_add_item_impl(
        &self,
        parent_menu: u64,
        text: &str,
        shortcut: Option<&str>,
    ) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(LinuxHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(LinuxHandleKind::MenuItem, text, 0, 0, 0, 0);
        let _ = shortcut;
        {
            let mut menus = self.menus.lock().expect("linux menu lock poisoned");
            menus.menu_children.entry(parent_menu).or_default().push(item_id);
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_item = gtk::MenuItem::with_label(text);
            let menus_arc = Arc::clone(&self.menus);
            menu_item.connect_activate(move |_| {
                menus_arc
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_menu_events
                    .push_back(item_id);
            });
            let mut native = self.native.lock_guard();
            if let Some(parent) = native.menus.get(&parent_menu) {
                parent.append(&menu_item);
            }
            native.widgets.insert(item_id, menu_item.clone().upcast::<gtk::Widget>());
        }
        item_id
    }
    pub(crate) fn poll_menu_triggered_impl(&self) -> Option<u64> {
        self.menus.lock().expect("linux menu lock poisoned").pending_menu_events.pop_front()
    }
    pub(crate) fn inject_menu_trigger_impl(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(LinuxHandleKind::MenuItem)) {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_menu_events
            .push_back(menu_item_id);
        true
    }
    pub(crate) fn poll_widget_triggered_impl(&self) -> Option<u64> {
        self.poll_widget_trigger_event_impl().map(|event| event.widget_id)
    }
    pub(crate) fn poll_widget_trigger_event_impl(&self) -> Option<WidgetTriggerEvent> {
        self.menus.lock().expect("linux menu lock poisoned").pending_widget_events.pop_front()
    }
    pub(crate) fn inject_widget_trigger_event_impl(
        &self,
        widget_id: u64,
        kind: WidgetTriggerKind,
    ) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
}
