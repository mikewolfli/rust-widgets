//! Harmony desktop backend shell.
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use crate::platform::state::BackendState;
use super::super::WidgetTriggerEvent;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HarmonyHandleKind {
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
}
#[derive(Default)]
pub(crate) struct HarmonyMenuState {
    /// Tracks menu bar attachment by window id.
    pub(crate) attached_menu_bar: HashMap<u64, u64>,
    /// Maintains menu tree relationships for backend-side validation.
    pub(crate) menu_children: HashMap<u64, Vec<u64>>,
    /// FIFO menu trigger queue, filled by native bridge injection APIs.
    pub(crate) pending_menu_events: VecDeque<u64>,
    /// FIFO typed widget trigger queue, filled by bridge callbacks and local fallbacks.
    pub(crate) pending_widget_events: VecDeque<WidgetTriggerEvent>,
}
pub(crate) struct HarmonyRuntimeState {
    pub(crate) initialized: AtomicBool,
    pub(crate) running: AtomicBool,
}
impl HarmonyRuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }
}
/// Harmony backend platform adapter.
pub struct HarmonyPlatform {
    pub(crate) state: BackendState<HarmonyHandleKind>,
    pub(crate) menus: Mutex<HarmonyMenuState>,
    pub(crate) runtime: HarmonyRuntimeState,
}
impl HarmonyPlatform {
    /// Creates a new Harmony platform adapter.
    pub fn new() -> Self {
        Self {
            state: BackendState::new(),
            menus: Mutex::new(HarmonyMenuState::default()),
            runtime: HarmonyRuntimeState::new(),
        }
    }
}
impl Default for HarmonyPlatform {
    fn default() -> Self {
        Self::new()
    }
}
impl HarmonyPlatform {
    /// Insert widget state and return allocated logical id.
    pub(crate) fn insert_widget(
        &self,
        kind: HarmonyHandleKind,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> u64 {
        self.state.create_widget(kind, text, x, y, width, height)
    }
    pub(crate) fn kind_of(&self, id: u64) -> Option<HarmonyHandleKind> {
        self.state.kind_of(id)
    }
}
