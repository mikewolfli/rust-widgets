// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Linux backend shell.
use crate::compat::HashMap;
use crate::compat::Mutex;
use crate::platform::state::BackendState;
use crate::platform::WidgetTriggerEvent;
use alloc::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
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
    /// Display text of each menu item's accelerator, keyed by item id.
    ///
    /// Kept separately from the GTK label because a host running without a
    /// usable GTK runtime (or querying before the widget is realised) must still
    /// be able to inspect which chord was bound.
    pub(crate) menu_item_shortcuts: HashMap<u64, String>,
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
        Self { initialized: AtomicBool::new(false), running: AtomicBool::new(false) }
    }
}
/// Linux desktop platform adapter.
pub struct LinuxPlatform {
    pub(crate) state: BackendState<LinuxHandleKind>,
    pub(crate) menus: Arc<Mutex<LinuxMenuState>>,
    pub(crate) runtime: LinuxRuntimeState,
    #[cfg(all(target_os = "linux", feature = "gtk-native"))]
    pub(crate) native: Mutex<LinuxNativeState>,
    /// Platform IME bridge for text input method integration (Linux only).
    #[cfg(target_os = "linux")]
    pub(crate) ime_bridge: crate::platform::ime_linux::LinuxImeBridge,
    /// Shared list storage for ComboBox and ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, ListData>>,
}
#[cfg(all(target_os = "linux", feature = "gtk-native"))]
#[derive(Default)]
pub(crate) struct LinuxNativeState {
    /// Native GTK windows indexed by logical widget id.
    pub(crate) windows: HashMap<u64, gtk::Window>,
    /// Root vertical containers hosting menu bar and content area.
    pub(crate) root_boxes: HashMap<u64, gtk::Box>,
    /// Absolute-position container for child controls.
    pub(crate) content_fixed: HashMap<u64, gtk::Fixed>,
    /// Generic widget registry for visibility/text/enabled operations.
    pub(crate) widgets: HashMap<u64, gtk::Widget>,
    pub(crate) menu_bars: HashMap<u64, gtk::MenuBar>,
    pub(crate) menus: HashMap<u64, gtk::Menu>,
    /// Native GTK dialogs (message box / file chooser / color / font)
    /// indexed by logical widget id.
    pub(crate) dialogs: HashMap<u64, gtk::Dialog>,
    /// Native GTK color selection widgets for `create_color_dialog`.
    pub(crate) color_choosers: HashMap<u64, gtk::ColorChooser>,
    /// Native GTK font selection widgets for `create_font_dialog`.
    pub(crate) font_choosers: HashMap<u64, gtk::FontChooser>,
    /// Native `DrawingArea`s hosting self-drawn widgets, indexed by the widget
    /// registry id they paint (see `linux/canvas.rs`).
    pub(crate) canvases: HashMap<u64, gtk::DrawingArea>,
    /// Window accelerator group. Menu item accelerators are registered against
    /// it so that a bound chord actually fires the item, instead of only being
    /// printed in the label (see `menu_add_item_impl`).
    pub(crate) accel_groups: HashMap<u64, gtk::AccelGroup>,
    /// Menus whose accelerator group has already received this item.
    ///
    /// `gtk_menu_item_set_accel_path` is global per item, so re-adding the same
    /// item to a second group would be ignored by GTK; tracking it keeps the
    /// bookkeeping explicit rather than relying on that silent no-op.
    pub(crate) accel_paths: HashMap<u64, String>,
}

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
unsafe impl Send for LinuxNativeState {}

// SAFETY: `LinuxPlatform` is only ever driven from the UI thread. The GTK
// widgets in `native` (`Mutex<LinuxNativeState>`) are `!Send + !Sync`, and GTK
// itself requires all calls to happen on the thread that called `gtk::init`.
//
// `Send` is required because the platform handle is stored in the crate's
// process-global registry; it does NOT permit concurrent GTK access, because
// every native entry point re-checks `gtk::is_initialized_main_thread()` before
// touching GTK (see `platform_impl.rs`).
//
// `Sync` is deliberately NOT implemented: nothing requires it, and the `gtk`
// types are `!Sync`, so a hand-written `unsafe impl Sync` would be an
// unnecessary promise that `&LinuxPlatform` is safe to share across threads.

#[cfg(all(target_os = "linux", feature = "gtk-native"))]
unsafe impl Send for LinuxPlatform {}

impl LinuxPlatform {
    /// Creates a new Linux platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Arc::new(Mutex::new(LinuxMenuState::default())),
            runtime: LinuxRuntimeState::new(),
            #[cfg(all(target_os = "linux", feature = "gtk-native"))]
            native: Mutex::new(LinuxNativeState::default()),
            #[cfg(target_os = "linux")]
            ime_bridge: crate::platform::ime_linux::LinuxImeBridge::new(),
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
