//! Wayland backend platform types.
//!
//! This module defines the Wayland platform's widget handle kind enum,
//! menu/list runtime state structures, and the main `WaylandPlatform` adapter.
//!
//! Architecture follows the same pattern as `LinuxPlatform` / `HarmonyPlatform`:
//! - `BackendState<WaylandHandleKind>` for widget state management
//! - Thread-safe interior mutability via `Mutex`
//! - Atomic runtime lifecycle flags
//! - Separate list data storage for ComboBox/ListBox

use crate::platform::state::BackendState;
use crate::platform::WidgetTriggerEvent;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

/// Platform-specific widget handle kind for Wayland backend.
///
/// Each variant maps to a logical widget type. State-only variants are used
/// for widgets that do not have a direct Wayland protocol counterpart and
/// are managed entirely through `BackendState`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WaylandHandleKind {
    /// Top-level window (wl_shell_surface / xdg_toplevel).
    Window,
    /// Push button.
    Button,
    /// Check box.
    CheckBox,
    /// Single-line text input.
    LineEdit,
    /// Static text label.
    Label,
    /// Radio button.
    RadioButton,
    /// Horizontal/vertical slider.
    Slider,
    /// Progress indicator bar.
    ProgressBar,
    /// Drop-down selection combo box.
    ComboBox,
    /// Multi-item list box.
    ListBox,
    /// Generic panel / container surface.
    Panel,
    /// Menu bar surface region.
    MenuBar,
    /// Popup menu.
    Menu,
    /// Menu item entry.
    MenuItem,
    /// Tool bar region.
    ToolBar,
    /// Status bar region.
    StatusBar,
    /// Message box dialog.
    MessageBox,
    /// File picker dialog.
    FileDialog,
    /// Color picker dialog.
    ColorDialog,
    /// Font picker dialog.
    FontDialog,
    /// Spin box (numeric up-down).
    SpinBox,
    /// List / tree view widget.
    ListView,
    /// Scrollable content area.
    ScrollArea,
}

/// Runtime state for menu tracking in the Wayland backend.
#[derive(Default)]
pub(crate) struct WaylandMenuState {
    /// Maps window id to attached menu bar id.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Maps parent menu id to child menu item ids.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO queue for menu trigger events.
    pub(crate) pending_menu_events: VecDeque<u64>,
    /// FIFO queue for typed widget trigger events.
    pub(crate) pending_widget_events: VecDeque<WidgetTriggerEvent>,
}

/// Shared storage for ComboBox and ListBox widget data.
#[derive(Default)]
pub(crate) struct ListData {
    /// Ordered item text entries.
    pub(crate) items: Vec<String>,
    /// Currently selected index, if any.
    pub(crate) current_index: Option<usize>,
}

/// Runtime lifecycle state for the Wayland backend.
pub(crate) struct WaylandRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}

impl WaylandRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }
}

/// Wayland desktop platform adapter.
///
/// Provides a full `Platform` trait implementation backed by `BackendState<WaylandHandleKind>`.
/// Widget operations are state-only until the native Wayland protocol integration
/// is wired through `wayland-client` and `wayland-protocols`.
pub struct WaylandPlatform {
    pub(crate) state: BackendState<WaylandHandleKind>,
    pub(crate) menus: Mutex<WaylandMenuState>,
    pub(crate) runtime: WaylandRuntimeState,
    /// Shared list storage for ComboBox and ListBox widgets.
    pub(crate) list_data: Mutex<HashMap<u64, ListData>>,
}

impl WaylandPlatform {
    /// Creates a new Wayland platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(WaylandMenuState::default()),
            runtime: WaylandRuntimeState::new(),
            list_data: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for WaylandPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WaylandPlatform {
    /// Insert widget state record and return allocated logical id.
    pub(crate) fn insert_widget(
        &self,
        kind: WaylandHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
}
