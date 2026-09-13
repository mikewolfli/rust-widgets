// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use super::types::{LinuxHandleKind, LinuxPlatform, ListData};
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::core::MutexExt;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::platform::WidgetTriggerEvent;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use crate::platform::WidgetTriggerKind;
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

    /// Native GTK `Frame`, used for the `GroupBox` kind (a labelled frame with a
    /// visible border).
    pub(crate) fn create_group_box_impl(
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
        let id = self.insert_widget(LinuxHandleKind::GroupBox, text, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let label = if text.is_empty() { None } else { Some(text) };
            let frame = gtk::Frame::new(label);
            frame.set_size_request(width as i32, height as i32);
            let widget = frame.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&frame, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `Frame` without a label, for the `Frame` kind.
    pub(crate) fn create_frame_impl(
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
        let id = self.insert_widget(LinuxHandleKind::Frame, "Frame", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let frame = gtk::Frame::new(None::<&str>);
            frame.set_size_request(width as i32, height as i32);
            let widget = frame.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&frame, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `Notebook` for the `TabWidget` kind.
    ///
    /// GTK tabs normalise to top placement; pages are added later via the
    /// container API. A single placeholder page is created so the notebook is
    /// immediately visible rather than an empty, zero-height widget.
    pub(crate) fn create_tab_widget_impl(
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
        let id = self.insert_widget(LinuxHandleKind::TabWidget, "TabWidget", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let notebook = gtk::Notebook::new();
            notebook.set_size_request(width as i32, height as i32);
            notebook.set_tab_pos(gtk::PositionType::Top);
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let tab_label = gtk::Label::new(Some("Tab 1"));
            notebook.append_page(&page, Some(&tab_label));
            let widget = notebook.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&notebook, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `Paned` for the `Splitter` kind (horizontal divider).
    pub(crate) fn create_splitter_impl(
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
        let id = self.insert_widget(LinuxHandleKind::Splitter, "Splitter", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
            paned.set_size_request(width as i32, height as i32);
            // Give both panes content so the divider is draggable and visible.
            let left = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
            paned.pack1(&left, true, false);
            paned.pack2(&right, true, false);
            paned.set_position((width as i32) / 2);
            let widget = paned.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&paned, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `ToggleButton` for the `ToggleButton` kind.
    ///
    /// The native path previously degraded this to a `CheckBox`, losing the
    /// button semantics; a real toggle button keeps both the button appearance
    /// and the active state.
    pub(crate) fn create_toggle_button_impl(
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
        let id = self.insert_widget(LinuxHandleKind::ToggleButton, text, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let toggle = gtk::ToggleButton::with_label(text);
            toggle.set_size_request(width as i32, height as i32);
            let menus = Arc::clone(&self.menus);
            toggle.connect_toggled(move |_| {
                menus.lock().expect("linux menu lock poisoned").pending_widget_events.push_back(
                    WidgetTriggerEvent { widget_id: id, kind: WidgetTriggerKind::Clicked },
                );
            });
            let widget = toggle.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&toggle, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `Calendar` for the `Calendar` kind.
    pub(crate) fn create_calendar_impl(
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
        let id = self.insert_widget(LinuxHandleKind::Calendar, "Calendar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let calendar = gtk::Calendar::new();
            calendar.set_size_request(width as i32, height as i32);
            let widget = calendar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&calendar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `Scrollbar` for the `ScrollBar` kind.
    ///
    /// A real scrollbar (with its adjustment model) instead of a degraded
    /// slider, so scroll semantics and orientation are preserved.
    pub(crate) fn create_scroll_bar_impl(
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
        let id = self.insert_widget(LinuxHandleKind::ScrollBar, "ScrollBar", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let adjustment = gtk::Adjustment::new(0.0, 0.0, 100.0, 1.0, 10.0, 10.0);
            let scrollbar = gtk::Scrollbar::new(gtk::Orientation::Vertical, Some(&adjustment));
            scrollbar.set_size_request(width as i32, height as i32);
            let widget = scrollbar.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&scrollbar, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK `SpinButton` configured for fractional input, for the
    /// `DoubleSpinBox` kind (the plain `SpinBox` uses integer digits).
    pub(crate) fn create_double_spin_box_impl(
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
        let id = self.insert_widget(
            LinuxHandleKind::DoubleSpinBox,
            "DoubleSpinBox",
            x,
            y,
            width,
            height,
        );
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let spin = gtk::SpinButton::with_range(0.0, 100.0, 0.1);
            // Two fractional digits distinguishes this from the integer SpinBox.
            spin.set_digits(2);
            spin.set_increments(0.1, 1.0);
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

    /// Native GTK `ComboBoxText` seeded for font selection, for the
    /// `FontComboBox` kind.
    pub(crate) fn create_font_combo_box_impl(
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
            self.insert_widget(LinuxHandleKind::FontComboBox, "FontComboBox", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let combo = gtk::ComboBoxText::new();
            // Seed with a few common families so the control is usable before the
            // host populates the real font list.
            for family in ["Sans", "Serif", "Monospace"] {
                combo.append_text(family);
            }
            combo.set_active(Some(0));
            combo.set_size_request(width as i32, height as i32);
            let widget = combo.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&combo, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Native GTK popup `Menu` for the `ContextMenu` kind.
    ///
    /// A context menu is a `gtk::Menu` shown on demand rather than a menu bar
    /// entry, so it is registered as a standalone menu the host can `popup()`
    /// at a pointer location. Items are appended via the normal menu-item path.
    pub(crate) fn create_context_menu_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // A context menu attaches to a Window/Panel subtree, not to a menu bar.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        if matches!(self.kind_of(parent), Some(LinuxHandleKind::Menu)) {
            // Nested menus are created through `create_menu`, not as a context menu.
            return 0;
        }
        let id =
            self.insert_widget(LinuxHandleKind::ContextMenu, "ContextMenu", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let menu = gtk::Menu::new();
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, menu.clone().upcast::<gtk::Widget>());
            native.menus.insert(id, menu);
        }
        id
    }

    /// Native GTK `Window` (popup type) for the `PopupWindow` kind.
    ///
    /// `WindowType::Popup` is the OS-supported transient popup window, so this
    /// is a real top-level rather than a panel drawn inside the parent.
    pub(crate) fn create_popup_window_impl(
        &self,
        parent: u64,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::PopupWindow, title, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let window = gtk::Window::new(gtk::WindowType::Popup);
            window.set_default_size(width as i32, height as i32);
            window.set_transient_for(self.native_window_for(parent).as_ref());
            let fixed = gtk::Fixed::new();
            window.add(&fixed);
            window.show_all();
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, window.clone().upcast::<gtk::Widget>());
            native.windows.insert(id, window);
            native.content_fixed.insert(id, fixed);
        }
        id
    }

    /// Native GTK `Dialog` for the `Dialog`/`InputDialog`/`ProgressDialog`
    /// kinds.
    ///
    /// All three previously degraded to a `MessageBox`; a real `Dialog` keeps
    /// the transient, modal window semantics (title + content area) that the
    /// host can add widgets to.
    pub(crate) fn create_dialog_impl(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.create_dialog_kind_impl(LinuxHandleKind::Dialog, parent, text, x, y, width, height)
    }

    /// Native GTK `Dialog` for the `InputDialog` kind.
    pub(crate) fn create_input_dialog_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.create_dialog_kind_impl(
            LinuxHandleKind::InputDialog,
            parent,
            "Input",
            x,
            y,
            width,
            height,
        )
    }

    /// Native GTK `Dialog` for the `ProgressDialog` kind.
    pub(crate) fn create_progress_dialog_impl(
        &self,
        parent: u64,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.create_dialog_kind_impl(
            LinuxHandleKind::ProgressDialog,
            parent,
            "Progress",
            x,
            y,
            width,
            height,
        )
    }

    /// Shared GTK `Dialog` construction for the dialog kinds.
    fn create_dialog_kind_impl(
        &self,
        kind: LinuxHandleKind,
        parent: u64,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(kind, title, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let dialog = gtk::Dialog::with_buttons(
                Some(title),
                self.native_window_for(parent).as_ref(),
                gtk::DialogFlags::MODAL,
                &[("Close", gtk::ResponseType::Close)],
            );
            dialog.set_default_size(width as i32, height as i32);
            let content = dialog.content_area();
            let fixed = gtk::Fixed::new();
            content.add(&fixed);
            dialog.show_all();
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
            native.content_fixed.insert(id, fixed);
        }
        id
    }

    /// Resolve the native GTK window for a widget id (walking up via
    /// `widget_parent`), used to set transient parents for popups and dialogs.
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    fn native_window_for(&self, widget: u64) -> Option<gtk::Window> {
        let native = self.native.lock_guard();
        if let Some(w) = native.windows.get(&widget) {
            return Some(w.clone());
        }
        let parents = self.menus.lock().expect("linux menu lock poisoned").widget_parent.clone();
        let mut cur = widget;
        for _ in 0..16 {
            match parents.get(&cur) {
                Some(&p) => {
                    if let Some(w) = native.windows.get(&p) {
                        return Some(w.clone());
                    }
                    cur = p;
                }
                None => break,
            }
        }
        None
    }

    /// Native GTK `FileChooserDialog` in folder-select mode for the
    /// `DirectoryDialog` kind.
    ///
    /// The native path previously degraded this to `create_file_dialog`, which
    /// selects *files*; a folder chooser is a distinct action.
    pub(crate) fn create_directory_dialog_impl(
        &self,
        parent: u64,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(LinuxHandleKind::DirectoryDialog, title, x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let dialog = gtk::FileChooserDialog::with_buttons(
                Some(title),
                self.native_window_for(parent).as_ref(),
                gtk::FileChooserAction::SelectFolder,
                &[("Cancel", gtk::ResponseType::Cancel), ("Select", gtk::ResponseType::Accept)],
            );
            dialog.set_default_size(width as i32, height as i32);
            dialog.show_all();
            let mut native = self.native.lock_guard();
            native.widgets.insert(id, dialog.clone().upcast::<gtk::Widget>());
        }
        id
    }

    /// Date picker.
    ///
    /// GTK ships no dedicated date-entry widget. The OS-supported choice is the
    /// calendar popup, so this composes a `gtk::MenuButton` whose popover hosts
    /// a `gtk::Calendar` — a real GTK composite rather than a plain panel, and
    /// the value is selectable through the calendar.
    pub(crate) fn create_date_picker_impl(
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
        let id = self.insert_widget(LinuxHandleKind::DatePicker, "DatePicker", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let button = gtk::MenuButton::new();
            button.set_label("Pick date");
            button.set_size_request(width as i32, height as i32);
            let popover = gtk::Popover::new(None::<&gtk::Widget>);
            let calendar = gtk::Calendar::new();
            popover.add(&calendar);
            button.set_popover(Some(&popover));
            let widget = button.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&button, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Time picker.
    ///
    /// Composed from real GTK spin buttons (hours/minutes) inside a horizontal
    /// box: GTK has no time-entry primitive, and two bounded integer spinners
    /// give the same semantics without inventing a widget.
    pub(crate) fn create_time_picker_impl(
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
        let id = self.insert_widget(LinuxHandleKind::TimePicker, "TimePicker", x, y, width, height);
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let hours = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
            let minutes = gtk::SpinButton::with_range(0.0, 59.0, 1.0);
            hours.set_digits(0);
            minutes.set_digits(0);
            row.pack_start(&hours, false, false, 0);
            row.pack_start(&gtk::Label::new(Some(":")), false, false, 0);
            row.pack_start(&minutes, false, false, 0);
            row.set_size_request(width as i32, height as i32);
            let widget = row.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&row, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Combined date+time picker: the calendar popover plus the hour/minute
    /// spinners, composed into one horizontal box.
    pub(crate) fn create_date_time_picker_impl(
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
        let id = self.insert_widget(
            LinuxHandleKind::DateTimePicker,
            "DateTimePicker",
            x,
            y,
            width,
            height,
        );
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let button = gtk::MenuButton::new();
            button.set_label("Pick date");
            let popover = gtk::Popover::new(None::<&gtk::Widget>);
            let calendar = gtk::Calendar::new();
            popover.add(&calendar);
            button.set_popover(Some(&popover));
            let hours = gtk::SpinButton::with_range(0.0, 23.0, 1.0);
            let minutes = gtk::SpinButton::with_range(0.0, 59.0, 1.0);
            hours.set_digits(0);
            minutes.set_digits(0);
            row.pack_start(&button, false, false, 0);
            row.pack_start(&hours, false, false, 0);
            row.pack_start(&gtk::Label::new(Some(":")), false, false, 0);
            row.pack_start(&minutes, false, false, 0);
            row.set_size_request(width as i32, height as i32);
            let widget = row.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&row, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }

    /// Busy indicator.
    ///
    /// GTK 3's only real busy primitive is the `gtk::Spinner`, which runs its
    /// own animation, so this is a real spinning indicator rather than a
    /// repurposed progress bar.
    pub(crate) fn create_activity_indicator_impl(
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
        let id = self.insert_widget(
            LinuxHandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        );
        self.menus.lock().expect("linux menu lock poisoned").widget_parent.insert(id, parent);
        #[cfg(all(target_os = "linux", feature = "gtk-native"))]
        {
            let spinner = gtk::Spinner::new();
            spinner.set_size_request(width as i32, height as i32);
            spinner.start();
            let widget = spinner.clone().upcast::<gtk::Widget>();
            let mut native = self.native.lock_guard();
            if let Some(container) = native.content_fixed.get(&parent) {
                container.put(&spinner, x, y);
            }
            native.widgets.insert(id, widget);
        }
        id
    }
}
