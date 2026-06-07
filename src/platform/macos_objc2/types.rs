//! macOS objc2 migration preview backend.
//!
//! This backend provides a state-driven implementation behind the `objc2-macos`
//! feature flag so migration can proceed incrementally without changing default
//! runtime behavior.
use crate::platform::state::BackendState;
use crate::platform::WidgetTriggerEvent;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Internal list data storage for ComboBox and ListBox widgets.
#[derive(Default)]
pub(crate) struct ListData {
    /// Ordered item text entries.
    pub(crate) items: Vec<String>,
    /// Currently selected index, if any.
    pub(crate) current_index: Option<usize>,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum MacObjc2HandleKind {
    /// Top-level native window surrogate.
    Window,
    /// Push button control.
    Button,
    /// Toggleable checkbox control.
    CheckBox,
    /// Single-line editable text input.
    LineEdit,
    /// Static text label.
    Label,
    /// Exclusive selection radio button.
    RadioButton,
    /// Range slider.
    Slider,
    /// Determinate/indeterminate progress indicator.
    ProgressBar,
    /// Drop-down selection control.
    ComboBox,
    /// List selection control.
    ListBox,
    /// Generic container panel.
    Panel,
    /// Root menu bar container.
    MenuBar,
    /// Hierarchical menu node.
    Menu,
    /// Actionable menu leaf item.
    MenuItem,
    /// Window toolbar region.
    ToolBar,
    /// Window status bar region.
    StatusBar,
    /// Modal message box dialog.
    MessageBox,
    /// File open/save dialog.
    FileDialog,
    /// Color picker dialog.
    ColorDialog,
    /// Font selection dialog.
    FontDialog,
}
#[derive(Default)]
pub(crate) struct MacObjc2MenuState {
    /// Window id -> attached menu bar id mapping.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Parent menu id -> direct child menu/menu-item ids.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu item trigger ids.
    pub(crate) pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget trigger events.
    pub(crate) pending_widget_events: VecDeque<WidgetTriggerEvent>,
}
/// Runtime lifecycle markers used by the preview run loop.
pub(crate) struct MacObjc2RuntimeState {
    /// `true` after backend initialization has completed.
    pub(crate) initialized: AtomicBool,
    /// `true` while the preview loop is running.
    pub(crate) running: AtomicBool,
}
impl MacObjc2RuntimeState {
    pub(crate) fn new() -> Self {
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}
/// Preview objc2-backed macOS platform adapter.
pub struct MacOSObjc2Platform {
    /// Internal state for all widgets and handles
    pub(crate) state: BackendState<MacObjc2HandleKind>,
    /// Menu state for menu bar/menu/menu items
    pub(crate) menus: Mutex<MacObjc2MenuState>,
    /// Runtime state for init/run/quit
    pub(crate) runtime: MacObjc2RuntimeState,
    /// Shared list storage for ComboBox and ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, ListData>>,
}
impl MacOSObjc2Platform {
    /// Serialize all widget state for parity/regression testing
    pub fn serialize_state(&self) -> Result<String, serde_json::Error> {
        // Only serializes the widget state, not runtime or menu events
        serde_json::to_string(&self.state)
    }
}
impl MacOSObjc2Platform {
    /// Creates a new objc2 migration preview backend.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(MacObjc2MenuState::default()),
            runtime: MacObjc2RuntimeState::new(),
            list_data: Mutex::new(HashMap::new()),
        }
    }
    pub(crate) fn insert_widget(
        &self,
        kind: MacObjc2HandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        // Centralized state insertion keeps id allocation deterministic for parity tests.
        self.state.create_widget(kind, text, x, y, width, height)
    }
    pub(crate) fn kind_of(&self, id: u64) -> Option<MacObjc2HandleKind> {
        // Handle-kind checks gate parent/child relationships and trigger validation.
        self.state.kind_of(id)
    }
    pub(crate) fn objc2_runtime_marker(&self) -> usize {
        // Marker for objc2 migration preview backend
        0
    }
}

impl Default for MacOSObjc2Platform {
    fn default() -> Self {
        Self::new()
    }
}
