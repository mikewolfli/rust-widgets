//! Linux backend shell.
use crate::platform::state::BackendState;
use crate::platform::WidgetTriggerEvent;
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
use gtk::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LinuxHandleKind {
    Window,
    Button,
    CheckBox,
    LineEdit,
    Label,
    RadioButton,
    Slider,
    ProgressBar,
    ComboBox,
    ListBox,
    Panel,
    MenuBar,
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
}
#[derive(Default)]
pub(crate) struct LinuxMenuState {
    /// Tracks menu bar attachment by window id.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Maintains menu tree relationships.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// Parent lookup for geometry updates in gtk-native fixed containers.
    pub(crate) widget_parent: HashMap<u64, u64>,
    /// FIFO queue for menu triggers.
    pub(crate) pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget triggers.
    pub(crate) pending_widget_events: VecDeque<WidgetTriggerEvent>,
}
/// Internal list data storage for ComboBox and ListBox widgets.
#[derive(Default)]
pub(crate) struct ListData {
    /// Ordered item text entries.
    pub(crate) items: Vec<String>,
    /// Currently selected index, if any.
    pub(crate) current_index: Option<usize>,
}

/// Runtime lifecycle state for Linux backend main loop fallback.
pub(crate) struct LinuxRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}
impl LinuxRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }
}
/// Linux desktop platform adapter.
pub struct LinuxPlatform {
    pub(crate) state: BackendState<LinuxHandleKind>,
    pub(crate) menus: Arc<Mutex<LinuxMenuState>>,
    pub(crate) runtime: LinuxRuntimeState,
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    pub(crate) native: Mutex<LinuxNativeState>,
    /// Shared list storage for ComboBox and ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, ListData>>,
}
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[derive(Default)]
pub(crate) struct LinuxNativeState {
    /// Native GTK windows indexed by logical widget id.
    windows: HashMap<u64, gtk::Window>,
    /// Root vertical containers hosting menu bar and content area.
    root_boxes: HashMap<u64, gtk::Box>,
    /// Absolute-position container for child controls.
    content_fixed: HashMap<u64, gtk::Fixed>,
    /// Generic widget registry for visibility/text/enabled operations.
    widgets: HashMap<u64, gtk::Widget>,
    menu_bars: HashMap<u64, gtk::MenuBar>,
    menus: HashMap<u64, gtk::Menu>,
}
impl LinuxPlatform {
    /// Creates a new Linux platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Arc::new(Mutex::new(LinuxMenuState::default())),
            runtime: LinuxRuntimeState::new(),
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            native: Mutex::new(LinuxNativeState::default()),
            list_data: Mutex::new(HashMap::new()),
        }
    }
}
impl Default for LinuxPlatform {
    fn default() -> Self {
        Self::new()
    }
}
impl LinuxPlatform {
    /// Insert and initialize one widget state record.
    pub(crate) fn insert_widget(
        &self,
        kind: LinuxHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
    pub(crate) fn kind_of(&self, id: u64) -> Option<LinuxHandleKind> {
        self.state.kind_of(id)
    }
}
