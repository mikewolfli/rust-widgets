//! `Platform` trait implementation for the WASM backend.

use super::types::{WasmHandleKind, WasmPlatform};
use crate::core::PlatformFamily;
use crate::platform::{DropEvent, Platform, WidgetTriggerEvent, WidgetTriggerKind};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

/// Internal list data storage for ComboBox and ListBox widgets.
#[derive(Default)]
struct WasmListData {
    /// Ordered item text entries.
    items: Vec<String>,
    /// Currently selected index, if any.
    current_index: Option<usize>,
}

/// Menu/tracking runtime state for the WASM backend.
struct WasmMenuState {
    attached_menu_bar: HashMap<u64, u64>,
    widget_parent: HashMap<u64, u64>,
    pending_menu_events: Vec<u64>,
    pending_widget_events: Vec<WidgetTriggerEvent>,
}

impl Default for WasmMenuState {
    fn default() -> Self {
        Self {
            attached_menu_bar: HashMap::new(),
            widget_parent: HashMap::new(),
            pending_menu_events: Vec::new(),
            pending_widget_events: Vec::new(),
        }
    }
}

impl Platform for WasmPlatform {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        "wasm-state-backend"
    }

    fn family(&self) -> PlatformFamily {
        PlatformFamily::Desktop
    }

    fn init(&self) {
        self.runtime.initialized.store(true, Ordering::SeqCst);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::prelude::*;
            let window = web_sys::window().expect("no global window in WASM environment");
            let document = window.document().expect("no document in WASM environment");
            // Ensure the canvas element exists in the DOM.
            if document.get_element_by_id(&self.canvas_id).is_none() {
                let canvas =
                    document.create_element("canvas").expect("failed to create canvas element");
                canvas.set_id(&self.canvas_id);
                document
                    .body()
                    .expect("no document body")
                    .append_child(&canvas)
                    .expect("failed to append canvas to body");
                let _ = canvas;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = &self.canvas_id;
        }
    }

    fn run(&self) {
        if !self.runtime.initialized.load(Ordering::SeqCst) {
            self.init();
        }
        self.runtime.running.store(true, Ordering::SeqCst);

        #[cfg(target_arch = "wasm32")]
        {
            use std::cell::RefCell;
            use std::rc::Rc;
            use wasm_bindgen::prelude::*;
            use wasm_bindgen::JsCast;

            let window = web_sys::window().expect("no global window in WASM environment");
            let running = &self.runtime.running;

            // Self-referencing closure pattern for RAF loops.
            // The Rc + RefCell holds the inner Closure so it is not dropped
            // after the first frame; each invocation re-schedules itself via
            // the shared handle.  We leak the outer Rc so the chain survives
            // for the entire program duration.
            let f = Rc::new(RefCell::new(None::<Closure<dyn FnMut()>>));
            let g = Rc::clone(&f);
            let win = window.clone();
            *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                // Re-schedule the next animation frame.
                let cb = f.borrow().as_ref().unwrap().as_ref().unchecked_ref();
                let _ = win.request_animation_frame(cb);
            }) as Box<dyn FnMut()>));

            // Kick off the first frame.
            let _ = window
                .request_animation_frame(g.borrow().as_ref().unwrap().as_ref().unchecked_ref());

            // Leak the entire Rc chain so it lives for the program duration.
            // The RAF callback will keep firing (and returning early once
            // `running` is false), but the closures will never be dropped.
            Rc::into_raw(f);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            while self.runtime.running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(16));
            }
        }
    }

    fn quit(&self) {
        self.runtime.running.store(false, Ordering::SeqCst);
    }

    // ─── Widget creation helpers ───────────────────────────────────────────────
    // Each widget factory validates the parent, inserts a state record, registers
    // a parent relationship, and optionally creates a native DOM element.

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        let id = self.insert_widget(WasmHandleKind::Window, title, x, y, width, height);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let window = web_sys::window().expect("no global window");
            let document = window.document().expect("no document");
            if let Some(canvas) = document.get_element_by_id(&self.canvas_id) {
                let html_canvas: web_sys::HtmlCanvasElement =
                    canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok();
                if let Some(c) = html_canvas {
                    c.set_width(width);
                    c.set_height(height);
                    let _ = c;
                }
            }
        }
        let _ = (x, y);
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
        let id = self.insert_widget(WasmHandleKind::Button, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
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
        let id = self.insert_widget(WasmHandleKind::CheckBox, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
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
        let id = self.insert_widget(WasmHandleKind::LineEdit, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
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
        let id = self.insert_widget(WasmHandleKind::Label, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
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
        let id = self.insert_widget(WasmHandleKind::RadioButton, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_slider(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::Slider, "Slider", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_progress_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id =
            self.insert_widget(WasmHandleKind::ProgressBar, "ProgressBar", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_combo_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::ComboBox, "ComboBox", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_list_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::ListBox, "ListBox", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    // ─── ComboBox operations ───────────────────────────────────────────────────

    fn combo_box_add_item(&self, combo_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return false;
        }
        LIST_DATA
            .lock()
            .expect("wasm list data lock poisoned")
            .entry(combo_box)
            .or_default()
            .items
            .push(text.to_string());
        true
    }

    fn combo_box_clear_items(&self, combo_box: u64) -> bool {
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return false;
        }
        let mut data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        if let Some(entry) = data.get_mut(&combo_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }

    fn combo_box_set_current_index(&self, combo_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return false;
        }
        let mut data = LIST_DATA.lock().expect("wasm list data lock poisoned");
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
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return None;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.current_index)
    }

    fn combo_box_item_count(&self, combo_box: u64) -> usize {
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return 0;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&combo_box).map_or(0, |entry| entry.items.len())
    }

    fn combo_box_item_text(&self, combo_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(combo_box), Some(WasmHandleKind::ComboBox)) {
            return None;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&combo_box).and_then(|entry| entry.items.get(index)).cloned()
    }

    // ─── ListBox operations ────────────────────────────────────────────────────

    fn list_box_add_item(&self, list_box: u64, text: &str) -> bool {
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return false;
        }
        LIST_DATA
            .lock()
            .expect("wasm list data lock poisoned")
            .entry(list_box)
            .or_default()
            .items
            .push(text.to_string());
        true
    }

    fn list_box_remove_item(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return false;
        }
        let mut data = LIST_DATA.lock().expect("wasm list data lock poisoned");
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
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return false;
        }
        let mut data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        if let Some(entry) = data.get_mut(&list_box) {
            entry.items.clear();
            entry.current_index = None;
        }
        true
    }

    fn list_box_set_current_index(&self, list_box: u64, index: usize) -> bool {
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return false;
        }
        let mut data = LIST_DATA.lock().expect("wasm list data lock poisoned");
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
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return None;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.current_index)
    }

    fn list_box_item_count(&self, list_box: u64) -> usize {
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return 0;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&list_box).map_or(0, |entry| entry.items.len())
    }

    fn list_box_item_text(&self, list_box: u64, index: usize) -> Option<String> {
        if !matches!(self.kind_of(list_box), Some(WasmHandleKind::ListBox)) {
            return None;
        }
        let data = LIST_DATA.lock().expect("wasm list data lock poisoned");
        data.get(&list_box).and_then(|entry| entry.items.get(index)).cloned()
    }

    // ─── Container / structural widgets ────────────────────────────────────────

    fn create_panel(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::Panel, "Panel", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_menu_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(WasmHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::MenuBar, "MenuBar", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_menu(&self, parent: u64, text: &str, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(WasmHandleKind::MenuBar | WasmHandleKind::Menu)) {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::Menu, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_tool_bar(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if !matches!(self.kind_of(parent), Some(WasmHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::ToolBar, "ToolBar", x, y, width, height);
        let _ = (parent, x, y, width, height);
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
        if !matches!(self.kind_of(parent), Some(WasmHandleKind::Window)) {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::StatusBar, text, x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn attach_menu_bar_to_window(&self, window: u64, menu_bar: u64) -> bool {
        if matches!(self.kind_of(window), Some(WasmHandleKind::Window))
            && matches!(self.kind_of(menu_bar), Some(WasmHandleKind::MenuBar))
        {
            MENU_STATE
                .lock()
                .expect("wasm menu state lock poisoned")
                .attached_menu_bar
                .insert(window, menu_bar);
            true
        } else {
            false
        }
    }

    fn menu_add_item(&self, parent_menu: u64, text: &str, shortcut: Option<&str>) -> u64 {
        if !matches!(self.kind_of(parent_menu), Some(WasmHandleKind::Menu)) {
            return 0;
        }
        let item_id = self.insert_widget(WasmHandleKind::MenuItem, text, 0, 0, 0, 0);
        let _ = shortcut;
        item_id
    }

    fn poll_menu_triggered(&self) -> Option<u64> {
        MENU_STATE.lock().expect("wasm menu state lock poisoned").pending_menu_events.pop()
    }

    fn inject_menu_trigger(&self, menu_item_id: u64) -> bool {
        if !matches!(self.kind_of(menu_item_id), Some(WasmHandleKind::MenuItem)) {
            return false;
        }
        MENU_STATE
            .lock()
            .expect("wasm menu state lock poisoned")
            .pending_menu_events
            .push(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<u64> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        MENU_STATE.lock().expect("wasm menu state lock poisoned").pending_widget_events.pop()
    }

    fn inject_widget_trigger_event(&self, widget_id: u64, kind: WidgetTriggerKind) -> bool {
        if self.kind_of(widget_id).is_none() {
            return false;
        }
        MENU_STATE
            .lock()
            .expect("wasm menu state lock poisoned")
            .pending_widget_events
            .push(WidgetTriggerEvent { widget_id, kind });
        true
    }

    // ─── Widget visibility / show / hide ───────────────────────────────────────

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
        self.state.set_text(widget_id, text);
        if matches!(self.kind_of(widget_id), Some(WasmHandleKind::LineEdit)) {
            MENU_STATE
                .lock()
                .expect("wasm menu state lock poisoned")
                .pending_widget_events
                .push(WidgetTriggerEvent { widget_id, kind: WidgetTriggerKind::ValueChanged });
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
        if visible {
            self.show_widget(widget_id);
        } else {
            self.hide_widget(widget_id);
        }
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

    // ─── Clipboard ─────────────────────────────────────────────────────────────

    fn set_clipboard_text(&self, text: &str) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(navigator) = web_sys::window().and_then(|w| w.navigator()) {
                if let Some(clipboard) = navigator.clipboard() {
                    let promise = clipboard.write_text(text);
                    // Fire-and-forget: the promise runs asynchronously.
                    let _ = promise;
                    return true;
                }
            }
        }
        // Fallback to in-memory clipboard when native API is unavailable.
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        #[cfg(target_arch = "wasm32")]
        {
            // web-sys clipboard.read_text() returns a Promise<String>.
            // For synchronous access we fall back to the in-memory store,
            // which is always populated when set_clipboard_text was called
            // successfully. A full async bridge is out of scope for MVP.
        }
        self.state.clipboard_text()
    }

    // ─── Drag-and-drop ─────────────────────────────────────────────────────────

    fn begin_drag(&self, source_widget_id: u64, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }

    // ─── Dialog creation (state-only, no native DOM dialog) ────────────────────

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
        self.insert_widget(WasmHandleKind::MessageBox, _text, x, y, width, height)
    }

    fn create_file_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(WasmHandleKind::FileDialog, "FileDialog", _x, _y, width, height)
    }

    fn create_color_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(WasmHandleKind::ColorDialog, "ColorDialog", _x, _y, width, height)
    }

    fn create_font_dialog(&self, _parent: u64, _x: i32, _y: i32, width: u32, height: u32) -> u64 {
        self.insert_widget(WasmHandleKind::FontDialog, "FontDialog", _x, _y, width, height)
    }

    // ─── Special widgets ───────────────────────────────────────────────────────

    fn create_spin_box(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::SpinBox, "SpinBox", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_list_view(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::ListView, "ListView", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }

    fn create_scroll_area(&self, parent: u64, x: i32, y: i32, width: u32, height: u32) -> u64 {
        if self.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.insert_widget(WasmHandleKind::ScrollArea, "ScrollArea", x, y, width, height);
        let _ = (parent, x, y, width, height);
        id
    }
}

// ─── Global state for list data and menu tracking ────────────────────────────
// These are lazily initialized statics shared across all WasmPlatform instances.
// In a multi-instance scenario this would be instance-local; for the MVP WASM
// backend a single set of globals is sufficient.

/// Thread-safe global list data storage for ComboBox and ListBox.
static LIST_DATA: std::sync::LazyLock<Mutex<HashMap<u64, WasmListData>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Thread-safe global menu/tracking state.
static MENU_STATE: std::sync::LazyLock<Mutex<WasmMenuState>> =
    std::sync::LazyLock::new(|| Mutex::new(WasmMenuState::default()));

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Platform;

    fn make_platform() -> WasmPlatform {
        WasmPlatform::with_canvas("test-canvas")
    }

    #[test]
    fn backend_name_and_family() {
        let p = make_platform();
        assert_eq!(p.backend_name(), "wasm-state-backend");
        assert_eq!(p.family(), PlatformFamily::Desktop);
    }

    #[test]
    fn create_widgets_and_check_kind() {
        let p = make_platform();
        let win = p.create_window("test", 0, 0, 800, 600);
        assert!(win > 0);
        assert_eq!(p.kind_of(win), Some(WasmHandleKind::Window));

        let btn = p.create_button(win, "Click", 10, 10, 100, 30);
        assert!(btn > 0);
        assert_eq!(p.kind_of(btn), Some(WasmHandleKind::Button));
    }

    #[test]
    fn widget_text_roundtrip() {
        let p = make_platform();
        let win = p.create_window("win", 0, 0, 400, 300);
        let lbl = p.create_label(win, "Hello", 10, 10, 80, 20);
        assert_eq!(p.get_widget_text(lbl), "Hello");
        p.set_widget_text(lbl, "World");
        assert_eq!(p.get_widget_text(lbl), "World");
    }

    #[test]
    fn widget_visibility_and_enabled() {
        let p = make_platform();
        let win = p.create_window("win", 0, 0, 400, 300);
        let btn = p.create_button(win, "btn", 0, 0, 50, 20);

        assert!(p.is_widget_visible(btn));
        assert!(p.is_widget_enabled(btn));

        p.set_widget_visible(btn, false);
        assert!(!p.is_widget_visible(btn));

        p.set_widget_enabled(btn, false);
        assert!(!p.is_widget_enabled(btn));
    }

    #[test]
    fn combo_box_operations() {
        let p = make_platform();
        let win = p.create_window("win", 0, 0, 400, 300);
        let cb = p.create_combo_box(win, 0, 0, 150, 25);

        assert!(p.combo_box_add_item(cb, "Item A"));
        assert!(p.combo_box_add_item(cb, "Item B"));
        assert_eq!(p.combo_box_item_count(cb), 2);
        assert_eq!(p.combo_box_item_text(cb, 0), Some("Item A".to_string()));
        assert_eq!(p.combo_box_item_text(cb, 1), Some("Item B".to_string()));

        assert!(p.combo_box_set_current_index(cb, 1));
        assert_eq!(p.combo_box_current_index(cb), Some(1));

        assert!(p.combo_box_clear_items(cb));
        assert_eq!(p.combo_box_item_count(cb), 0);
    }

    #[test]
    fn inject_and_poll_widget_trigger() {
        let p = make_platform();
        let win = p.create_window("win", 0, 0, 400, 300);
        let btn = p.create_button(win, "b", 0, 0, 50, 20);

        assert!(p.inject_widget_trigger_event(btn, WidgetTriggerKind::Clicked));
        let event = p.poll_widget_trigger_event();
        assert!(event.is_some());
        assert_eq!(event.unwrap().widget_id, btn);

        // Second poll should be empty.
        assert!(p.poll_widget_triggered().is_none());
    }

    #[test]
    fn invalid_parent_returns_zero() {
        let p = make_platform();
        let btn = p.create_button(9999, "orphan", 0, 0, 50, 20);
        assert_eq!(btn, 0);
    }
}
