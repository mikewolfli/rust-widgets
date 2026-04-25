#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use super::types::LinuxNativeState;
use super::types::{LinuxHandleKind, LinuxPlatform, ListData};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for LinuxPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "gtk"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Initialize GTK runtime when native path is enabled.
            let _ = gtk::init();
        }
        #[cfg(not(all(target_os = "linux", feature = "gtk-native")))]
        {
            // linux non-gtk-native preview mode: no-op
        }
    }
    fn run(&self) {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Enter GTK event loop for native-backed Linux runtime.
            gtk::main();
            return;
        }
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime.running.store(true, Ordering::SeqCst);
        while self.runtime.running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(16));
        }
    }
    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            gtk::main_quit();
        }
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(LinuxHandleKind::Window, title, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let window = gtk::Window::new(gtk::WindowType::Toplevel);
            window.set_title(title);
            window.set_default_size(width as i32, height as i32);
            window.move_(x, y);
            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let fixed = gtk::Fixed::new();
            root.pack_start(&fixed, true, true, 0);
            window.add(&root);
            let mut native = self.native.lock().expect("linux native lock poisoned");
            native.windows.insert(id, window.clone());
            native.root_boxes.insert(id, root);
            native.content_fixed.insert(id, fixed.clone());
            native
                .widgets
                .insert(id, window.clone().upcast::<gtk::Widget>());
        }
        id
    }
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Button, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let button = gtk::Button::with_label(text);
            button.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            button.connect_clicked(move |_| {
                // Normalize native button activation to typed trigger event.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::Clicked,
                    });
            });
            let widget = button.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&button, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_checkbox(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::CheckBox, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let checkbox = gtk::CheckButton::with_label(text);
            checkbox.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            checkbox.connect_toggled(move |_| {
                // Normalize checkbox toggles to click-like activation trigger.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::Clicked,
                    });
            });
            let widget = checkbox.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&checkbox, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_line_edit(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::LineEdit, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let entry = gtk::Entry::new();
            entry.set_text(text);
            entry.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            entry.connect_changed(move |_| {
                // Normalize text changes to value-changed trigger.
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::ValueChanged,
                    });
            });
            let widget = entry.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&entry, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_label(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Label, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = gtk::Label::new(Some(text));
            label.set_size_request(width as i32, height as i32);
            let widget = label.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&label, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_radio_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::RadioButton, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let radio = gtk::RadioButton::with_label(text);
            radio.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            radio.connect_toggled(move |_| {
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::Clicked,
                    });
            });
            let widget = radio.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&radio, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Slider, "Slider", x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let slider = gtk::Scale::new_with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
            slider.set_size_request(width as i32, height as i32);
            slider.set_draw_value(false);
            let menus = Arc::clone(&self.menus);
            slider.connect_value_changed(move |_| {
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::ValueChanged,
                    });
            });
            let widget = slider.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&slider, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(
            LinuxHandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        );
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let progress = gtk::ProgressBar::new();
            progress.set_size_request(width as i32, height as i32);
            let widget = progress.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&progress, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ComboBox, "ComboBox", x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        self.list_data
            .lock()
            .expect("linux list data lock poisoned")
            .insert(
                id,
                ListData {
                    items: Vec::new(),
                    current_index: None,
                },
            );
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let combo = gtk::ComboBoxText::new();
            combo.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            combo.connect_changed(move |_| {
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::SelectionChanged,
                    });
            });
            let widget = combo.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&combo, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ListBox, "ListBox", x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        self.list_data
            .lock()
            .expect("linux list data lock poisoned")
            .insert(
                id,
                ListData {
                    items: Vec::new(),
                    current_index: None,
                },
            );
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let list = gtk::ListBox::new();
            list.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            list.connect_row_selected(move |_, _| {
                menus
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_widget_events
                    .push_back(WidgetTriggerEvent {
                        widget_id: id,
                        kind: WidgetTriggerKind::SelectionChanged,
                    });
            });
            let widget = list.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&list, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.items.remove(index);
        if let Some(cur) = entry.current_index {
            if cur == index {
                entry.current_index = None;
            } else if cur > index {
                entry.current_index = Some(cur - 1);
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn list_box_current_index(&self, list_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&list_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        let entry = match data.get_mut(&combo_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.current_index = Some(index);
        true
    }
    fn combo_box_current_index(&self, combo_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return 0;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return None;
        }
        let data = self
            .list_data
            .lock()
            .expect("linux list data lock poisoned");
        data.get(&combo_box)
            .and_then(|entry| entry.items.get(index))
            .cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Panel, "Panel", x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let panel = gtk::Frame::new(None::<&str>);
            panel.set_size_request(width as i32, height as i32);
            let widget = panel.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&panel, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::MenuBar, "MenuBar", x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_bar = gtk::MenuBar::new();
            let widget = menu_bar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            native.menu_bars.insert(id, menu_bar);
            native.widgets.insert(id, widget);
            let _ = parent;
            let _ = (x, y, width, height);
        }
        id
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(LinuxHandleKind::MenuBar | LinuxHandleKind::Menu)
        ) {
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
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(menu_bar) = native.menu_bars.get(&parent) {
                menu_bar.append(&menu_item);
            } else if let Some(parent_menu) = native.menus.get(&parent) {
                parent_menu.append(&menu_item);
            }
            native
                .widgets
                .insert(id, menu_item.clone().upcast::<gtk::Widget>());
            native.menus.insert(id, menu);
            let _ = (x, y, width, height);
        }
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ToolBar, "ToolBar", x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            toolbar.set_size_request(width as i32, height as i32);
            let widget = toolbar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&toolbar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
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
        if !matches!(self.kind_of(parent), Some(LinuxHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::StatusBar, text, x, y, width, height);
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .widget_parent
            .insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = gtk::Label::new(Some(text));
            label.set_size_request(width as i32, height as i32);
            let widget = label.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&label, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        // Validate shape first, then attach native menu bar when available.
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
                let native = self.native.lock().expect("linux native lock poisoned");
                if let (Some(root), Some(bar)) = (
                    native.root_boxes.get(&window),
                    native.menu_bars.get(&menu_bar),
                ) {
                    root.pack_start(bar, false, false, 0);
                    root.reorder_child(bar, 0);
                    bar.show_all();
                }
            }
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(LinuxHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(LinuxHandleKind::MenuItem, text, 0, 0, 0, 0);
        let _ = shortcut;
        // Capture menu_children push first, then drop menus lock before acquiring native lock.
        {
            let mut menus = self.menus.lock().expect("linux menu lock poisoned");
            menus
                .menu_children
                .entry(parent_menu)
                .or_default()
                .push(item_id);
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu_item = gtk::MenuItem::with_label(text);
            let menus_arc = Arc::clone(&self.menus);
            menu_item.connect_activate(move |_| {
                // Native menu activation is forwarded to platform queue.
                menus_arc
                    .lock()
                    .expect("linux menu lock poisoned")
                    .pending_menu_events
                    .push_back(item_id);
            });
            let mut native = self.native.lock().expect("linux native lock poisoned");
            if let Some(parent) = native.menus.get(&parent_menu) {
                parent.append(&menu_item);
            }
            native
                .widgets
                .insert(item_id, menu_item.clone().upcast::<gtk::Widget>());
        }
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_menu_events
            .pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Keep injected events type-safe: only known menu items are accepted.
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
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event()
            .map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus
            .lock()
            .expect("linux menu lock poisoned")
            .pending_widget_events
            .pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        // Keep injected events deterministic: only known widget ids are accepted.
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
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.show_all();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.show();
            }
        }
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.hide();
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.hide();
            }
        }
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            // Capture parent_id first, then drop menus lock before acquiring native lock.
            let parent_id = {
                let menus = self.menus.lock().expect("linux menu lock poisoned");
                menus.widget_parent.get(&widget_id).copied()
            };
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.move_(x, y);
                window.resize(width as i32, height as i32);
                return;
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_size_request(width as i32, height as i32);
                if let Some(parent_id) = parent_id {
                    if let Some(container) = native.content_fixed.get(&parent_id) {
                        container.move_(widget, x, y);
                    }
                }
            }
        }
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                window.set_title(text);
            } else if let Some(widget) = native.widgets.get(&widget_id) {
                if let Ok(button) = widget.clone().downcast::<gtk::Button>() {
                    button.set_label(text);
                } else if let Ok(check) = widget.clone().downcast::<gtk::CheckButton>() {
                    check.set_label(text);
                } else if let Ok(entry) = widget.clone().downcast::<gtk::Entry>() {
                    entry.set_text(text);
                } else if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
                    label.set_text(text);
                } else if let Ok(menu_item) = widget.clone().downcast::<gtk::MenuItem>() {
                    menu_item.set_label(text);
                }
            }
        }
        if matches!(self.kind_of(widget_id), Some(LinuxHandleKind::LineEdit)) {
            self.menus
                .lock()
                .expect("linux menu lock poisoned")
                .pending_widget_events
                .push_back(WidgetTriggerEvent {
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
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(widget) = native.widgets.get(&widget_id) {
                widget.set_sensitive(enabled);
            }
        }
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_sensitive();
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
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let native = self.native.lock().expect("linux native lock poisoned");
            if let Some(window) = native.windows.get(&widget_id) {
                return window.is_visible();
            }
            if let Some(widget) = native.widgets.get(&widget_id) {
                return widget.is_visible();
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
    fn create_message_box(
        &self,
        _parent: u64,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.insert_widget(LinuxHandleKind::MessageBox, _text, x, y, width, height)
    }
    fn create_file_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            LinuxHandleKind::FileDialog,
            "FileDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_color_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            LinuxHandleKind::ColorDialog,
            "ColorDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_font_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            LinuxHandleKind::FontDialog,
            "FontDialog",
            _x,
            _y,
            width,
            height,
        )
    }
    fn create_spin_box(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(LinuxHandleKind::SpinBox, "SpinBox", _x, _y, width, height)
    }
    fn create_list_view(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(LinuxHandleKind::ListView, "ListView", _x, _y, width, height)
    }
    fn create_scroll_area(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(
            LinuxHandleKind::ScrollArea,
            "ScrollArea",
            _x,
            _y,
            width,
            height,
        )
    }
}
