use super::super::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use super::types::*;
use crate::core::{MutexExt, ObjectId, PlatformFamily};

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

impl Platform for HarmonyPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn backend_name(&self) -> &'static str {
        "harmony-desktop"
    }
    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    /// Reads `MemTotal` from `/proc/meminfo`; HarmonyOS runs on a Linux-derived
    /// kernel that provides it.
    fn total_memory_mb(&self) -> Option<u64> {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        for line in content.lines() {
            let Some(rest) = line.strip_prefix("MemTotal:") else {
                continue;
            };
            let kb = rest.trim().trim_end_matches("kB").trim().parse::<u64>().ok()?;
            return Some(kb / 1024);
        }
        None
    }

    /// Walks `/sys/class/power_supply` for a battery reporting `Discharging`.
    fn is_on_battery(&self) -> bool {
        let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") else {
            return false;
        };
        for entry in entries.flatten() {
            let status_path = entry.path().join("status");
            if let Ok(status) = std::fs::read_to_string(&status_path) {
                if status.trim() == "Discharging" {
                    return true;
                }
            }
        }
        false
    }

    /// Samples RSS/VmSize for this process from `/proc/self/status`.
    fn process_memory_utilization(&self) -> Option<f32> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let mut vmrss_kb: u64 = 0;
        let mut vmsize_kb: u64 = 0;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                vmrss_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("VmSize:") {
                vmsize_kb = rest.trim().trim_end_matches("kB").trim().parse().unwrap_or(0);
            }
        }
        if vmsize_kb == 0 {
            return None;
        }
        Some((vmrss_kb as f32 / vmsize_kb as f32).clamp(0.0, 1.0))
    }

    /// Estimates CPU load as thread count over twice the available cores.
    fn process_cpu_utilization(&self) -> Option<f32> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            let Some(rest) = line.strip_prefix("Threads:") else {
                continue;
            };
            let threads = rest.trim().parse::<f32>().ok()?;
            let cores = std::thread::available_parallelism().map(|n| n.get() as f32).unwrap_or(4.0);
            return Some((threads / (cores * 2.0)).clamp(0.0, 1.0));
        }
        None
    }

    /// HarmonyOS printing is served by the ArkUI print service, which this state
    /// backend does not bind; it reports the gap instead of faking success.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("HarmonyOS printing requires the ArkUI print service (not bound)".to_string())
    }

    /// Capabilities published by the Harmony backend.
    ///
    /// The backend is state-driven: widget state, layout, events and menu
    /// semantics are all modelled in-process, and no ArkUI/window-server object
    /// is created. The flags therefore describe what the *state model* supports
    /// rather than a native compositor, and `native_menu` is `false` because the
    /// menu is an in-process tree served through an injectable queue, not an
    /// OS menu.
    fn capabilities(&self) -> crate::platform::types::PlatformCapabilities {
        crate::platform::types::PlatformCapabilities {
            dpi_scaling: true,
            ime: true,
            accessibility: true,
            native_menu: false,
            typed_widget_trigger: true,
        }
    }
    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
    }
    fn run(&self) {
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
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Harmony keeps two per-widget side tables beyond the authoritative
    /// `BackendState` record: the shared list storage (`list_data`, used by
    /// ComboBox/ListBox) and the menu bookkeeping (`menus`). Both plus any queued
    /// trigger that references the widget must be purged, otherwise a UI rebuilt
    /// in a create/destroy loop would leak one entry per discarded widget. Each
    /// lock is scoped to its own statement so no two guards are ever held at the
    /// same time.
    fn destroy_widget(&self, widget_id: u64) -> bool {
        self.list_data.lock_guard().remove(&widget_id);

        {
            let mut menus = self.menus.lock_guard();
            menus.attached_menu_bar.remove(&widget_id);
            // The widget may be a container in the menu tree: drop both the
            // children it owned and the child entry under its own parent.
            menus.menu_children.remove(&widget_id);
            for children in menus.menu_children.values_mut() {
                children.retain(|child| *child != widget_id);
            }
            // Drop queued triggers that reference a widget that no longer exists.
            menus.pending_menu_events.retain(|queued| *queued != widget_id);
            menus.pending_widget_events.retain(|event| event.widget_id != widget_id);
        }

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }
    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(HarmonyHandleKind::Window, title, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::Button, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::CheckBox, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::LineEdit, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::Label, text, x, y, width, height)
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
        self.insert_widget(HarmonyHandleKind::RadioButton, text, x, y, width, height)
    }
    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::Slider, "Slider", x, y, width, height)
    }
    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ProgressBar, "ProgressBar", x, y, width, height)
    }
    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(HarmonyHandleKind::ComboBox, "ComboBox", x, y, width, height);
        self.list_data.lock_guard().insert(id, ListData { items: Vec::new(), current_index: None });
        id
    }
    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(HarmonyHandleKind::ListBox, "ListBox", x, y, width, height);
        self.list_data.lock_guard().insert(id, ListData { items: Vec::new(), current_index: None });
        id
    }
    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
        let entry = data.entry(list_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
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
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
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
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock_guard();
        data.get(&list_box).and_then(|entry| entry.current_index)
    }
    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return 0;
        }
        let data = self.list_data.lock_guard();
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }
    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(HarmonyHandleKind::ListBox)) {
            return None;
        }
        let data = self.list_data.lock_guard();
        data.get(&list_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
        let entry = data.entry(combo_box).or_default();
        entry.items.push(text.to_string());
        true
    }
    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }
    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return false;
        }
        let mut data = self.list_data.lock_guard();
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
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock_guard();
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }
    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return 0;
        }
        let data = self.list_data.lock_guard();
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }
    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(HarmonyHandleKind::ComboBox)) {
            return None;
        }
        let data = self.list_data.lock_guard();
        data.get(&combo_box).and_then(|entry| entry.items.get(index)).cloned()
    }
    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::Panel, "Panel", x, y, width, height)
    }
    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::MenuBar, "MenuBar", x, y, width, height)
    }
    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(
            self.kind_of(parent),
            Some(HarmonyHandleKind::MenuBar | HarmonyHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.insert_widget(HarmonyHandleKind::Menu, text, x, y, width, height);
        self.menus.lock_guard().menu_children.entry(parent).or_default().push(id);
        id
    }
    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ToolBar, "ToolBar", x, y, width, height)
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
        if !matches!(self.kind_of(parent), Some(HarmonyHandleKind::Window)) {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::StatusBar, text, x, y, width, height)
    }
    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if matches!(self.kind_of(window), Some(HarmonyHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(HarmonyHandleKind::MenuBar))
        {
            self.menus.lock_guard().attached_menu_bar.insert(window, menu_bar);
            return true;
        }
        false
    }
    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(HarmonyHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(HarmonyHandleKind::MenuItem, text, 0, 0, 0, 0);
        let _ = shortcut;
        let mut menus = self.menus.lock_guard();
        menus.menu_children.entry(parent_menu).or_default().push(item_id);
        item_id
    }
    fn poll_menu_triggered(&self) -> Option<u64> {
        self.menus.lock_guard().pending_menu_events.pop_front()
    }
    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        // Only menu items may generate menu trigger events.
        if !matches!(self.kind_of(menu_item_id), Some(HarmonyHandleKind::MenuItem)) {
            return false;
        }
        self.menus.lock_guard().pending_menu_events.push_back(menu_item_id);
        true
    }
    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }
    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.menus.lock_guard().pending_widget_events.pop_front()
    }
    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        // Any known widget may enqueue a typed trigger event.
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        self.menus
            .lock_guard()
            .pending_widget_events
            .push_back(WidgetTriggerEvent { widget_id, kind });
        true
    }
    fn show_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, true);
    }
    fn hide_widget(&self, widget_id: u64) {
        self.state.set_visible(widget_id, false);
    }
    fn set_widget_geometry(&self, widget_id: u64, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }
    fn set_widget_text(&self, widget_id: u64, text: &str) {
        if !self.state.set_text(widget_id, text) {
            return;
        }
        // Keep line-edit behavior consistent with value-changed semantics.
        if matches!(self.kind_of(widget_id), Some(HarmonyHandleKind::LineEdit)) {
            self.menus
                .lock_guard()
                .pending_widget_events
                .push_back(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
        }
    }
    fn get_widget_text(&self, widget_id: u64) -> String {
        self.state.text(widget_id)
    }
    fn set_widget_enabled(&self, widget_id: u64, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }
    fn is_widget_enabled(&self, widget_id: u64) -> bool {
        self.state.enabled(widget_id)
    }
    fn set_widget_visible(&self, widget_id: u64, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }
    fn is_widget_visible(&self, widget_id: u64) -> bool {
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
        parent: u64,
        title: &str,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        // HarmomyOS constructs dialogs through the Ability/Window subsystem, so
        // the logical handle records the title/body the host should present.
        self.insert_widget(
            HarmonyHandleKind::MessageBox,
            &format!("{}: {}", title, text),
            x,
            y,
            width,
            height,
        )
    }
    fn create_file_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::FileDialog, "FileDialog", x, y, width, height)
    }
    fn create_color_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ColorDialog, "ColorDialog", x, y, width, height)
    }
    fn create_font_dialog(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::FontDialog, "FontDialog", x, y, width, height)
    }
    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::SpinBox, "SpinBox", x, y, width, height)
    }
    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ListView, "ListView", x, y, width, height)
    }
    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        self.insert_widget(HarmonyHandleKind::ScrollArea, "ScrollArea", x, y, width, height)
    }
    fn create_group_box(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::GroupBox, title, x, y, width, height)
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::Frame, "Frame", x, y, width, height)
    }
    fn create_tab_widget(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::TabWidget, "TabWidget", x, y, width, height)
    }
    fn create_splitter(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::Splitter, "Splitter", x, y, width, height)
    }
    fn create_toggle_button(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::ToggleButton, text, x, y, width, height)
    }
    fn create_calendar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::Calendar, "Calendar", x, y, width, height)
    }
    fn create_scroll_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::ScrollBar, "ScrollBar", x, y, width, height)
    }
    fn create_double_spin_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            HarmonyHandleKind::DoubleSpinBox,
            "DoubleSpinBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_font_combo_box(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            HarmonyHandleKind::FontComboBox,
            "FontComboBox",
            x,
            y,
            width,
            height,
        )
    }
    fn create_context_menu(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::ContextMenu, "ContextMenu", x, y, width, height)
    }
    fn create_popup_window(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::PopupWindow, title, x, y, width, height)
    }
    fn create_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::Dialog, title, x, y, width, height)
    }
    fn create_input_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::InputDialog, "Input", x, y, width, height)
    }
    fn create_progress_dialog(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::ProgressDialog, "Progress", x, y, width, height)
    }
    fn create_directory_dialog(
        &self,
        parent: ObjectId,
        title: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::DirectoryDialog, title, x, y, width, height)
    }
    fn create_date_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::DatePicker, "DatePicker", x, y, width, height)
    }
    fn create_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(HarmonyHandleKind::TimePicker, "TimePicker", x, y, width, height)
    }
    fn create_date_time_picker(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            HarmonyHandleKind::DateTimePicker,
            "DateTimePicker",
            x,
            y,
            width,
            height,
        )
    }
    fn create_activity_indicator(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(
            HarmonyHandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        )
    }
}
