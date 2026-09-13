// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Stub platform implementation for testing and demonstrations.
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::core::{ObjectId, PlatformFamily};
use crate::platform::state::{BackendState, WindowStateRecord};
use crate::platform::types::*;
#[cfg(all(feature = "serde", not(any(feature = "mini", feature = "embedded"))))]
use serde::{Deserialize, Serialize};

/// Handle kind discriminator for stub widget records.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    all(feature = "serde", not(any(feature = "mini", feature = "embedded"))),
    derive(Serialize, Deserialize)
)]
pub(crate) enum StubHandleKind {
    Window,
    Button,
    MenuBar,
    CheckBox,
    LineEdit,
    Label,
    RadioButton,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    Menu,
    MenuItem,
    ToolBar,
    StatusBar,
    MessageBox,
    FileDialog,
    ColorDialog,
    FontDialog,
    SpinBox,
    ListView,
    ScrollArea,
    GroupBox,
    Frame,
    TabWidget,
    Splitter,
    ToggleButton,
    Calendar,
    ScrollBar,
    DoubleSpinBox,
    FontComboBox,
    ContextMenu,
    PopupWindow,
    Dialog,
    InputDialog,
    ProgressDialog,
    DirectoryDialog,
    DatePicker,
    TimePicker,
    DateTimePicker,
    ActivityIndicator,
}

pub struct StubPlatform {
    backend: &'static str,
    family: PlatformFamily,
    state: BackendState<StubHandleKind>,
    menu_nodes: Mutex<HashMap<ObjectId, MenuNodeState>>,
    /// In-memory combo-box item storage by logical combo widget id.
    combo_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// In-memory combo-box selected index by logical combo widget id.
    combo_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// In-memory list-box item storage by logical list widget id.
    list_box_items: Mutex<HashMap<ObjectId, Vec<String>>>,
    /// In-memory list-box selected index by logical list widget id.
    list_box_selection: Mutex<HashMap<ObjectId, Option<usize>>>,
    /// Platform IME bridge for testing.
    pub(crate) ime_bridge: crate::platform::ime::MockImeBridge,
}

impl StubPlatform {
    /// Creates a new in-memory stub backend for tests and demos.
    pub fn new(backend: &'static str, family: PlatformFamily) -> Self {
        Self {
            backend,
            family,
            state: BackendState::new(),
            menu_nodes: Mutex::new(HashMap::new()),
            combo_box_items: Mutex::new(HashMap::new()),
            combo_box_selection: Mutex::new(HashMap::new()),
            list_box_items: Mutex::new(HashMap::new()),
            list_box_selection: Mutex::new(HashMap::new()),
            ime_bridge: crate::platform::ime::MockImeBridge::new(),
        }
    }

    fn is_embedded_profile(&self) -> bool {
        matches!(self.family, PlatformFamily::Embedded)
    }

    fn embedded_unsupported_id(&self, _name: &str) -> ObjectId {
        // Return a dummy id for unsupported features in embedded profile
        0
    }

    fn embedded_unsupported_bool(&self, _name: &str) -> bool {
        // Return false for unsupported features in embedded profile
        false
    }
}

impl Platform for StubPlatform {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn backend_name(&self) -> &'static str {
        self.backend
    }

    fn family(&self) -> PlatformFamily {
        self.family
    }

    /// The stub has no OS to interrogate, so it reports an explicit `None` rather
    /// than a made-up figure. Callers fall back to their conservative default.
    fn total_memory_mb(&self) -> Option<u64> {
        None
    }

    /// A stub host is treated as mains-powered, which selects the non-throttled
    /// defaults.
    fn is_on_battery(&self) -> bool {
        false
    }

    /// No process accounting exists without an OS; `None` keeps the adaptive
    /// monitor on its default instead of pretending the process is at 0%.
    fn process_memory_utilization(&self) -> Option<f32> {
        None
    }

    /// Same reasoning as [`Platform::process_memory_utilization`].
    fn process_cpu_utilization(&self) -> Option<f32> {
        None
    }

    /// The stub cannot reach a real spooler, and saying so is the point: demos
    /// exercising the system print backend must see a truthful failure.
    fn spawn_print_job(&self, _job_file: &std::path::Path) -> Result<(), String> {
        Err("stub platform has no print spooler".to_string())
    }

    fn init(&self) {
        log::info!("[stub] StubPlatform init (testing backend)");
    }

    fn run(&self) {
        log::info!("[stub] StubPlatform run (testing backend)");
    }

    fn quit(&self) {
        log::info!("[stub] StubPlatform quit");
    }

    /// Release every registry entry the backend holds for `widget_id`.
    ///
    /// Besides the authoritative `BackendState` record the stub keeps five
    /// in-memory side tables: `menu_nodes`, the combo-box item/selection maps and
    /// the list-box item/selection maps. All of them must be purged, otherwise a
    /// UI rebuilt in a create/destroy loop would leak one entry per discarded
    /// widget. Each lock is scoped to its own statement so no two guards are ever
    /// held at the same time.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        self.menu_nodes.lock().expect("platform lock poisoned").remove(&widget_id);

        self.combo_box_items.lock().expect("platform lock poisoned").remove(&widget_id);
        self.combo_box_selection.lock().expect("platform lock poisoned").remove(&widget_id);

        self.list_box_items.lock().expect("platform lock poisoned").remove(&widget_id);
        self.list_box_selection.lock().expect("platform lock poisoned").remove(&widget_id);

        // The state record is the authority on whether the widget existed.
        self.state.destroy_widget(widget_id)
    }

    fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        let id = self.state.create_widget(StubHandleKind::Window, title, x, y, width, height);
        // Mark this record as a window and seed the state a fresh OS window has:
        // restored, visible, windowed, resizable and decorated.
        self.state.init_window_state(id, WindowStateRecord::new_window());
        id
    }

    fn create_button(
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
        self.state.create_widget(StubHandleKind::Button, text, x, y, width, height)
    }

    fn create_menu_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_menu_bar");
        }
        if !matches!(self.state.kind_of(parent), Some(StubHandleKind::Window)) {
            return 0;
        }
        let id = self.state.create_widget(StubHandleKind::MenuBar, "MenuBar", x, y, width, height);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(id, MenuNodeState { text: "MenuBar".to_string() });
        id
    }

    fn create_checkbox(
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
        let id = self.state.create_widget(StubHandleKind::CheckBox, text, x, y, width, height);
        // A freshly created check box is unchecked: seeding the state makes
        // `is_widget_checked` answer from creation instead of reporting `None`
        // for a widget that demonstrably has a check state.
        self.state.set_checked(id, false);
        id
    }

    fn create_line_edit(
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
        let id = self.state.create_widget(StubHandleKind::LineEdit, text, x, y, width, height);
        // A text entry is editable by default; seeding read-only makes the state
        // answer `Some(false)` instead of `None` for a control that has one.
        self.state.set_read_only(id, false);
        self.state.set_max_length(id, u32::MAX);
        id
    }

    fn create_label(
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
        self.state.create_widget(StubHandleKind::Label, text, x, y, width, height)
    }

    fn create_radio_button(
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
        let id = self.state.create_widget(StubHandleKind::RadioButton, text, x, y, width, height);
        self.state.set_checked(id, false);
        id
    }

    fn create_slider(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        let id = self.state.create_widget(StubHandleKind::Slider, "Slider", x, y, width, height);
        // Sliders expose the conventional 0..=100 starting range and value, so a
        // caller can read back a concrete number immediately after creation.
        self.state.set_range(id, 0.0, 100.0);
        self.state.set_value(id, 0.0);
        self.state.set_step(id, 1.0);
        id
    }

    fn create_progress_bar(
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
        let id = self.state.create_widget(
            StubHandleKind::ProgressBar,
            "ProgressBar",
            x,
            y,
            width,
            height,
        );
        self.state.set_value(id, 0.0);
        self.state.set_range(id, 0.0, 100.0);
        self.state.set_indeterminate(id, false);
        id
    }

    fn create_combo_box(
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
        let id =
            self.state.create_widget(StubHandleKind::ComboBox, "ComboBox", x, y, width, height);
        self.combo_box_items.lock().expect("platform lock poisoned").insert(id, Vec::new());
        self.combo_box_selection.lock().expect("platform lock poisoned").insert(id, None);
        id
    }

    fn combo_box_add_item(&self, combo_box: ObjectId, _text: &str) -> bool {
        let mut items = self.combo_box_items.lock().expect("platform lock poisoned");
        let list = match items.get_mut(&combo_box) {
            Some(list) => list,
            None => return false,
        };
        list.push(_text.to_string());
        true
    }

    fn combo_box_clear_items(&self, combo_box: ObjectId) -> bool {
        {
            let mut items = self.combo_box_items.lock().expect("platform lock poisoned");
            if let Some(list) = items.get_mut(&combo_box) {
                list.clear();
            } else {
                return false;
            }
        }
        self.combo_box_selection.lock().expect("platform lock poisoned").insert(combo_box, None);
        true
    }

    fn combo_box_set_current_index(&self, combo_box: ObjectId, index: usize) -> bool {
        let items = self.combo_box_items.lock().expect("platform lock poisoned");
        let len = match items.get(&combo_box) {
            Some(list) => list.len(),
            None => return false,
        };
        if index >= len {
            return false;
        }
        drop(items);
        self.combo_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(combo_box, Some(index));
        true
    }

    fn combo_box_current_index(&self, combo_box: ObjectId) -> Option<usize> {
        self.combo_box_selection
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .and_then(|index| *index)
    }

    fn combo_box_item_count(&self, combo_box: ObjectId) -> usize {
        self.combo_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .map(|items| items.len())
            .unwrap_or(0)
    }

    fn combo_box_item_text(&self, combo_box: ObjectId, index: usize) -> Option<String> {
        self.combo_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&combo_box)
            .and_then(|items| items.get(index).cloned())
    }

    fn create_list_box(
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
        let id = self.state.create_widget(StubHandleKind::ListBox, "ListBox", x, y, width, height);
        self.list_box_items.lock().expect("platform lock poisoned").insert(id, Vec::new());
        self.list_box_selection.lock().expect("platform lock poisoned").insert(id, None);
        id
    }

    fn list_box_add_item(&self, list_box: ObjectId, text: &str) -> bool {
        let mut items = self.list_box_items.lock().expect("platform lock poisoned");
        let list = match items.get_mut(&list_box) {
            Some(list) => list,
            None => return false,
        };
        list.push(text.to_string());
        true
    }

    fn list_box_remove_item(&self, list_box: ObjectId, index: usize) -> bool {
        let len;
        {
            let mut items = self.list_box_items.lock().expect("platform lock poisoned");
            let list = match items.get_mut(&list_box) {
                Some(list) => list,
                None => return false,
            };
            if index >= list.len() {
                return false;
            }
            list.remove(index);
            len = list.len();
        }
        let mut selection = self.list_box_selection.lock().expect("platform lock poisoned");
        if let Some(current) = selection.get(&list_box).and_then(|value| *value) {
            if current == index {
                selection.insert(list_box, None);
            } else if current > index && len > 0 {
                selection.insert(list_box, Some((current - 1).min(len - 1)));
            }
        }
        true
    }

    fn list_box_clear_items(&self, list_box: ObjectId) -> bool {
        {
            let mut items = self.list_box_items.lock().expect("platform lock poisoned");
            if let Some(list) = items.get_mut(&list_box) {
                list.clear();
            } else {
                return false;
            }
        }
        self.list_box_selection.lock().expect("platform lock poisoned").insert(list_box, None);
        true
    }

    fn list_box_set_current_index(&self, list_box: ObjectId, index: usize) -> bool {
        let items = self.list_box_items.lock().expect("platform lock poisoned");
        let len = match items.get(&list_box) {
            Some(list) => list.len(),
            None => return false,
        };
        if index >= len {
            return false;
        }
        drop(items);
        self.list_box_selection
            .lock()
            .expect("platform lock poisoned")
            .insert(list_box, Some(index));
        true
    }

    fn list_box_current_index(&self, list_box: ObjectId) -> Option<usize> {
        self.list_box_selection
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .and_then(|index| *index)
    }

    fn list_box_item_count(&self, list_box: ObjectId) -> usize {
        self.list_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .map(|items| items.len())
            .unwrap_or(0)
    }

    fn list_box_item_text(&self, list_box: ObjectId, index: usize) -> Option<String> {
        self.list_box_items
            .lock()
            .expect("platform lock poisoned")
            .get(&list_box)
            .and_then(|items| items.get(index).cloned())
    }

    fn create_panel(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(StubHandleKind::Panel, "Panel", x, y, width, height)
    }

    fn create_menu(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_menu");
        }
        if !matches!(
            self.state.kind_of(parent),
            Some(StubHandleKind::MenuBar | StubHandleKind::Menu)
        ) {
            return 0;
        }
        let id = self.state.create_widget(StubHandleKind::Menu, text, x, y, width, height);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(id, MenuNodeState { text: text.to_string() });
        id
    }

    fn create_tool_bar(
        &self,
        parent: ObjectId,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_tool_bar");
        }
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(StubHandleKind::ToolBar, "ToolBar", x, y, width, height)
    }

    fn create_status_bar(
        &self,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("create_status_bar");
        }
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(StubHandleKind::StatusBar, text, x, y, width, height)
    }

    fn create_message_box(
        &self,
        parent: ObjectId,
        _title: &str,
        _text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(StubHandleKind::MessageBox, "MessageBox", x, y, width, height)
    }

    fn create_file_dialog(
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
        self.state.create_widget(StubHandleKind::FileDialog, "FileDialog", x, y, width, height)
    }

    fn create_color_dialog(
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
        self.state.create_widget(StubHandleKind::ColorDialog, "ColorDialog", x, y, width, height)
    }

    fn create_font_dialog(
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
        self.state.create_widget(StubHandleKind::FontDialog, "FontDialog", x, y, width, height)
    }

    fn create_spin_box(
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
        let id = self.state.create_widget(StubHandleKind::SpinBox, "SpinBox", x, y, width, height);
        // A spin box has a value, a range and a step; seed the conventional
        // defaults so a read right after creation answers concretely.
        self.state.set_range(id, 0.0, 100.0);
        self.state.set_value(id, 0.0);
        self.state.set_step(id, 1.0);
        id
    }

    fn create_list_view(
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
        self.state.create_widget(StubHandleKind::ListView, "ListView", x, y, width, height)
    }

    fn create_scroll_area(
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
        self.state.create_widget(StubHandleKind::ScrollArea, "ScrollArea", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::GroupBox, title, x, y, width, height)
    }
    fn create_frame(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        if self.state.kind_of(parent).is_none() {
            return 0;
        }
        self.state.create_widget(StubHandleKind::Frame, "Frame", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::TabWidget, "TabWidget", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::Splitter, "Splitter", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::ToggleButton, text, x, y, width, height)
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
        self.state.create_widget(StubHandleKind::Calendar, "Calendar", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::ScrollBar, "ScrollBar", x, y, width, height)
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
        let id = self.state.create_widget(
            StubHandleKind::DoubleSpinBox,
            "DoubleSpinBox",
            x,
            y,
            width,
            height,
        );
        self.state.set_range(id, 0.0, 100.0);
        self.state.set_value(id, 0.0);
        self.state.set_step(id, 1.0);
        id
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
        self.state.create_widget(StubHandleKind::FontComboBox, "FontComboBox", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::ContextMenu, "ContextMenu", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::PopupWindow, title, x, y, width, height)
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
        self.state.create_widget(StubHandleKind::Dialog, title, x, y, width, height)
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
        self.state.create_widget(StubHandleKind::InputDialog, "Input", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::ProgressDialog, "Progress", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::DirectoryDialog, title, x, y, width, height)
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
        self.state.create_widget(StubHandleKind::DatePicker, "DatePicker", x, y, width, height)
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
        self.state.create_widget(StubHandleKind::TimePicker, "TimePicker", x, y, width, height)
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
            StubHandleKind::DateTimePicker,
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
            StubHandleKind::ActivityIndicator,
            "ActivityIndicator",
            x,
            y,
            width,
            height,
        )
    }

    fn attach_menu_bar_to_window(&self, window: ObjectId, menu_bar: ObjectId) -> bool {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_bool("attach_menu_bar_to_window");
        }
        self.state.contains_widget(window) && self.state.contains_widget(menu_bar)
    }

    fn menu_add_item(
        &self,
        _parent_menu: ObjectId,
        text: &str,
        shortcut: Option<&str>,
    ) -> ObjectId {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_id("menu_add_item");
        }
        let id = self.state.create_widget(StubHandleKind::MenuItem, text, 0, 0, 0, 0);
        self.menu_nodes
            .lock()
            .expect("platform lock poisoned")
            .insert(id, MenuNodeState { text: text.to_string() });
        let _ = shortcut;
        id
    }

    fn poll_menu_triggered(&self) -> Option<ObjectId> {
        self.state.pop_menu_event()
    }

    fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
        if self.is_embedded_profile() {
            return self.embedded_unsupported_bool("inject_menu_trigger");
        }
        // Accept only known menu ids to avoid emitting orphan events.
        if !self.menu_nodes.lock().expect("platform lock poisoned").contains_key(&menu_item_id) {
            return false;
        }
        self.state.push_menu_event(menu_item_id);
        true
    }

    fn poll_widget_triggered(&self) -> Option<ObjectId> {
        self.poll_widget_trigger_event().map(|event| event.widget_id)
    }

    fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
        self.state.pop_widget_event()
    }

    fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
        // Accept only known widget ids to keep queue semantics deterministic.
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.push_widget_event(WidgetTriggerEvent { widget_id, kind });
        true
    }

    fn show_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, true);
    }

    fn hide_widget(&self, widget_id: ObjectId) {
        self.state.set_visible(widget_id, false);
    }

    fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
        self.state.set_geometry(widget_id, x, y, width, height);
    }

    fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
        self.state.set_text(widget_id, text);
        if let Some(node) =
            self.menu_nodes.lock().expect("platform lock poisoned").get_mut(&widget_id)
        {
            node.text = text.to_string();
        }
    }

    fn get_widget_text(&self, widget_id: ObjectId) -> String {
        self.state.text(widget_id)
    }

    fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
        self.state.set_enabled(widget_id, enabled);
    }

    fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.enabled(widget_id)
    }

    fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
        self.state.set_visible(widget_id, visible);
    }

    fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
        self.state.visible(widget_id)
    }

    fn set_widget_value(&self, widget_id: ObjectId, value: f64) -> bool {
        // A record holds a numeric value only if its creator seeded one, and each
        // stub `create_*` seeds exactly the properties its control has. That is
        // the natural per-control answer — a slider accepts a value, a button
        // does not — without any global classification table.
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_value(widget_id, value)
    }

    fn widget_value(&self, widget_id: ObjectId) -> Option<f64> {
        self.state.value(widget_id)
    }

    fn set_widget_range(&self, widget_id: ObjectId, min: f64, max: f64) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_range(widget_id, min, max)
    }

    fn widget_range(&self, widget_id: ObjectId) -> Option<(f64, f64)> {
        self.state.range(widget_id)
    }

    fn set_widget_selected_index(&self, widget_id: ObjectId, index: Option<usize>) -> bool {
        match self.state.kind_of(widget_id) {
            Some(StubHandleKind::ComboBox) => match index {
                // Delegate to the specialised path so bounds checking and the
                // item table stay authoritative.
                Some(i) => self.combo_box_set_current_index(widget_id, i),
                None => {
                    self.combo_box_selection
                        .lock()
                        .expect("platform lock poisoned")
                        .insert(widget_id, None);
                    true
                }
            },
            Some(StubHandleKind::ListBox) => match index {
                Some(i) => self.list_box_set_current_index(widget_id, i),
                None => {
                    self.list_box_selection
                        .lock()
                        .expect("platform lock poisoned")
                        .insert(widget_id, None);
                    true
                }
            },
            Some(_) => self.state.set_selected_index(widget_id, index),
            None => false,
        }
    }

    fn widget_selected_index(&self, widget_id: ObjectId) -> Option<usize> {
        match self.state.kind_of(widget_id) {
            Some(StubHandleKind::ComboBox) => self.combo_box_current_index(widget_id),
            Some(StubHandleKind::ListBox) => self.list_box_current_index(widget_id),
            Some(_) => self.state.selected_index(widget_id),
            None => None,
        }
    }

    fn set_widget_checked(&self, widget_id: ObjectId, checked: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_checked(widget_id, checked)
    }

    fn is_widget_checked(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.checked(widget_id)
    }

    fn set_widget_step(&self, widget_id: ObjectId, step: f64) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_step(widget_id, step)
    }

    fn widget_step(&self, widget_id: ObjectId) -> Option<f64> {
        self.state.step(widget_id)
    }

    fn set_widget_indeterminate(&self, widget_id: ObjectId, indeterminate: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_indeterminate(widget_id, indeterminate)
    }

    fn is_widget_indeterminate(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.indeterminate(widget_id)
    }

    fn set_widget_read_only(&self, widget_id: ObjectId, read_only: bool) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_read_only(widget_id, read_only)
    }

    fn is_widget_read_only(&self, widget_id: ObjectId) -> Option<bool> {
        self.state.read_only(widget_id)
    }

    fn set_widget_max_length(&self, widget_id: ObjectId, max_length: u32) -> bool {
        if !self.state.contains_widget(widget_id) {
            return false;
        }
        self.state.set_max_length(widget_id, max_length)
    }

    fn widget_max_length(&self, widget_id: ObjectId) -> Option<u32> {
        self.state.max_length(widget_id)
    }

    fn set_window_state(&self, widget_id: ObjectId, flag: WindowStateFlag, on: bool) -> bool {
        // Only a window has window state; the state record is `None` for every
        // other widget, so a control honestly reports refusal here.
        self.state.set_window_state(widget_id, flag, on)
    }

    fn is_window_in_state(&self, widget_id: ObjectId, flag: WindowStateFlag) -> Option<bool> {
        self.state.window_state(widget_id, flag)
    }

    fn set_window_min_size(&self, widget_id: ObjectId, width: u32, height: u32) -> bool {
        self.state.set_window_min_size(widget_id, width, height)
    }

    fn window_min_size(&self, widget_id: ObjectId) -> Option<(u32, u32)> {
        self.state.window_min_size(widget_id)
    }

    fn set_window_icon(&self, widget_id: ObjectId, path: &str) -> bool {
        self.state.set_window_icon(widget_id, path)
    }

    fn window_icon(&self, widget_id: ObjectId) -> Option<String> {
        self.state.window_icon(widget_id)
    }

    fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
        self.state.set_ime_enabled(widget_id, enabled)
    }

    fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
        self.state.ime_enabled(widget_id)
    }

    fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
        self.state.set_accessibility_name(widget_id, name)
    }

    fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
        self.state.accessibility_name(widget_id)
    }

    fn set_clipboard_text(&self, text: &str) -> bool {
        self.state.set_clipboard_text(text)
    }

    fn get_clipboard_text(&self) -> String {
        self.state.clipboard_text()
    }

    fn ime_bridge(&self) -> Option<&dyn crate::platform::ime::ImeBridge> {
        Some(&self.ime_bridge)
    }

    fn begin_drag(&self, source_widget_id: ObjectId, mime: &str, payload: &[u8]) -> bool {
        self.state.begin_drag(source_widget_id, mime, payload)
    }

    fn poll_drop_event(&self) -> Option<DropEvent> {
        self.state.pop_drop_event()
    }

    fn inject_drop_event(&self, event: DropEvent) -> bool {
        self.state.inject_drop_event(event)
    }
}
