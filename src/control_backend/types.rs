//! Control backend abstraction for native and custom-painted control paths.
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;
use crate::core::ObjectId;
use crate::platform::{get_platform, WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;
/// Control backend family used by runtime routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBackendKind {
    /// Native platform control implementation.
    Native,
    /// Custom-painted control implementation.
    Custom,
}
/// Compile-time control route preference for a widget kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRoutePreference {
    /// Prefer native backend when available.
    NativePreferred,
    /// Require custom-painted backend route.
    CustomRequired,
}
impl Default for CustomControlState {
    fn default() -> Self {
        Self {
            next_widget_id: 1,
            texts: HashMap::new(),
            enabled: HashMap::new(),
            visible: HashMap::new(),
            ime_enabled: HashMap::new(),
            accessibility_names: HashMap::new(),
            menu_trigger_queue: VecDeque::new(),
            widget_trigger_queue: VecDeque::new(),
            widget_properties: HashMap::new(),
        }
    }
}
pub(crate) struct CustomControlState {
    pub(crate) next_widget_id: ObjectId,
    pub(crate) texts: HashMap<ObjectId, String>,
    pub(crate) enabled: HashMap<ObjectId, bool>,
    pub(crate) visible: HashMap<ObjectId, bool>,
    pub(crate) ime_enabled: HashMap<ObjectId, bool>,
    pub(crate) accessibility_names: HashMap<ObjectId, String>,
    pub(crate) menu_trigger_queue: VecDeque<ObjectId>,
    pub(crate) widget_trigger_queue: VecDeque<WidgetTriggerEvent>,
    // Store widget properties for custom painting
    pub(crate) widget_properties: HashMap<ObjectId, CustomWidgetProperties>,
}
#[allow(dead_code)]
pub(crate) struct CustomWidgetProperties {
    pub(crate) parent: Option<ObjectId>,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) widget_kind: WidgetKind,
}
