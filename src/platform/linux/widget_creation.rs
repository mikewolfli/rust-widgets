use super::types::{LinuxHandleKind, LinuxPlatform, ListData};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use std::sync::Arc;

impl LinuxPlatform {
    pub(crate) fn create_button_impl(
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
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let button = gtk::Button::with_label(text);
            button.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            button.connect_clicked(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::Clicked },
                );
            });
            let widget = button.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&button, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_checkbox_impl(
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
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let checkbox = gtk::CheckButton::with_label(text);
            checkbox.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            checkbox.connect_toggled(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::Clicked },
                );
            });
            let widget = checkbox.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&checkbox, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_line_edit_impl(
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
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let entry = gtk::Entry::new();
            entry.set_text(text);
            entry.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            entry.connect_changed(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::ValueChanged },
                );
            });
            let widget = entry.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&entry, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_label_impl(
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
    pub(crate) fn create_radio_button_impl(
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
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let radio = gtk::RadioButton::with_label(text);
            radio.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            radio.connect_toggled(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::Clicked },
                );
            });
            let widget = radio.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&radio, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_slider_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Slider, "Slider", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
            slider.set_size_request(width as i32, height as i32);
            slider.set_draw_value(false);
            let menus = Arc::clone(&self.menus);
            slider.connect_value_changed(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::ValueChanged },
                );
            });
            let widget = slider.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&slider, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_progress_bar_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(LinuxHandleKind::ProgressBar, "ProgressBar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let progress = gtk::ProgressBar::new();
            progress.set_size_request(width as i32, height as i32);
            let widget = progress.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&progress, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_combo_box_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ComboBox, "ComboBox", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        self.list_data
            .lock()
            .expect("linux list data lock poisoned")
            .insert(id, ListData { items: Vec::new(), current_index: None });
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let combo = gtk::ComboBoxText::new();
            combo.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            combo.connect_changed(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::SelectionChanged },
                );
            });
            let widget = combo.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&combo, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_list_box_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ListBox, "ListBox", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        self.list_data
            .lock()
            .expect("linux list data lock poisoned")
            .insert(id, ListData { items: Vec::new(), current_index: None });
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let list = gtk::ListBox::new();
            list.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            list.connect_row_selected(move |_, _| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::SelectionChanged },
                );
            });
            let widget = list.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&list, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn list_box_add_item_impl(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    pub(crate) fn list_box_remove_item_impl(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
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
    pub(crate) fn list_box_clear_items_impl(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    pub(crate) fn list_box_set_current_index_impl(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
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
    pub(crate) fn list_box_current_index_impl(&self, list_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    pub(crate) fn list_box_item_count_impl(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    pub(crate) fn list_box_item_text_impl(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(LinuxHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    pub(crate) fn combo_box_add_item_impl(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    pub(crate) fn combo_box_clear_items_impl(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    pub(crate) fn combo_box_set_current_index_impl(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("linux list data lock poisoned");
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
    pub(crate) fn combo_box_current_index_impl(&self, combo_box: u64) -> Option<usize> {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    pub(crate) fn combo_box_item_count_impl(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    pub(crate) fn combo_box_item_text_impl(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(LinuxHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("linux list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    pub(crate) fn create_panel_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::Panel, "Panel", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let panel = gtk::Frame::new(None::<&str>);
            panel.set_size_request(width as i32, height as i32);
            let widget = panel.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&panel, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_spin_box_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::SpinBox, "SpinBox", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let adjustment = gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 0.0);
            let spin = gtk::SpinButton::new(Some(&adjustment), 1.0, 0);
            spin.set_size_request(width as i32, height as i32);
            let widget = spin.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&spin, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_list_view_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ListView, "ListView", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let scrolled =
                gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
            scrolled.set_size_request(width as i32, height as i32);
            let store = gtk::ListStore::new(&[String::static_type()]);
            let tree = gtk::TreeView::new();
            tree.set_model(Some(&store));
            tree.append_column(&gtk::TreeViewColumn::new());
            scrolled.add(&tree);
            let widget = scrolled.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&scrolled, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
    pub(crate) fn create_scroll_area_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::ScrollArea, "ScrollArea", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let scrolled =
                gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
            scrolled.set_size_request(width as i32, height as i32);
            let widget = scrolled.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&scrolled, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
}
