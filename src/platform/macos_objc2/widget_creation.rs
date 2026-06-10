use super::types::{MacOSObjc2Platform, MacObjc2HandleKind};
use crate::core::ObjectId;
use crate::platform::{Platform, WidgetTriggerEvent, WidgetTriggerKind};

impl Platform for MacOSObjc2Platform {
    fn create_button(
        &self,
        parent: u64,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Mirror native constraint: child controls require a valid existing parent.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Button, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "objc2-macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let button = super::native::create_ns_button(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*button as *const _ as *mut std::ffi::c_void);
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
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::CheckBox, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "objc2-macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let checkbox = super::native::create_ns_checkbox(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*checkbox as *const _ as *mut std::ffi::c_void);
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
        // Keep creation contract identical to default backend for migration parity.
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::LineEdit, text, x, y, width, height)
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
        let id = self.insert_widget(MacObjc2HandleKind::Label, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "objc2-macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let label = super::native::create_ns_label(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*label as *const _ as *mut std::ffi::c_void);
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
        let id = self.insert_widget(MacObjc2HandleKind::RadioButton, text, x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "objc2-macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let radio = super::native::create_ns_radio(mtm, text, x, y, width, height);
            super::native::store_native_view(id, &*radio as *const _ as *mut std::ffi::c_void);
        }

        id
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(MacObjc2HandleKind::Slider, "Slider", x, y, width, height);

        #[cfg(all(target_os = "macos", feature = "objc2-macos"))]
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let slider = super::native::create_ns_slider(mtm, x, y, width, height);
            super::native::store_native_view(id, &*slider as *const _ as *mut std::ffi::c_void);
        }

        id
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ProgressBar, "ProgressBar", x, y, width, height)
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ComboBox, "ComboBox", x, y, width, height)
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::ListBox, "ListBox", x, y, width, height)
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        // Validate that widget exists and is a ListBox.
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = match data.get_mut(&list_box) {
            Some(e) => e,
            None => return false,
        };
        if index >= entry.items.len() {
            return false;
        }
        entry.items.remove(index);
        // Adjust current_index if the removed item was at or before it.
        if let Some(cur) = entry.current_index {
            if cur == index {
                // Item at the selected index was removed — clear selection.
                entry.current_index = None;
            } else if cur > index {
                // Selection shifted down by one.
                entry.current_index = Some(cur - 1);
            }
        }
        true
    }
    fn list_box_clear_items(&self, list_box: u64) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
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
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(MacObjc2HandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
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
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return 0;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(MacObjc2HandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock().expect("mac objc2 list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "Panel", x, y, width, height)
    }
    fn create_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "SpinBox", x, y, width, height)
    }
    fn create_list_view(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "ListView", x, y, width, height)
    }
    fn create_scroll_area(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(MacObjc2HandleKind::Panel, "ScrollArea", x, y, width, height)
    }
}
