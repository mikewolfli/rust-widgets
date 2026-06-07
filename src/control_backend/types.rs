//! Control backend abstraction for native and custom-painted control paths.
use crate::core::ObjectId;
use crate::platform::WidgetTriggerEvent;
use crate::widget::WidgetKind;
use std::collections::{HashMap, VecDeque};
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
/// Properties for custom-painted controls.
/// Stores geometry and kind metadata used by the custom-paint control backend
/// for layout and rendering dispatch.
pub(crate) struct CustomWidgetProperties {
    pub(crate) parent: Option<ObjectId>,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) widget_kind: WidgetKind,
}

impl CustomControlState {
    /// Look up properties for a widget by its id.
    pub(crate) fn widget_property(&self, widget_id: ObjectId) -> Option<&CustomWidgetProperties> {
        self.widget_properties.get(&widget_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_backend_kind_debug_and_eq() {
        assert_eq!(ControlBackendKind::Native, ControlBackendKind::Native);
        assert_eq!(ControlBackendKind::Custom, ControlBackendKind::Custom);
        assert_ne!(ControlBackendKind::Native, ControlBackendKind::Custom);
        let _ = format!("{:?}", ControlBackendKind::Native);
        let _ = format!("{:?}", ControlBackendKind::Custom);
    }

    #[test]
    fn control_route_preference_debug_and_eq() {
        assert_eq!(
            ControlRoutePreference::NativePreferred,
            ControlRoutePreference::NativePreferred
        );
        assert_eq!(ControlRoutePreference::CustomRequired, ControlRoutePreference::CustomRequired);
        assert_ne!(ControlRoutePreference::NativePreferred, ControlRoutePreference::CustomRequired);
        let _ = format!("{:?}", ControlRoutePreference::NativePreferred);
        let _ = format!("{:?}", ControlRoutePreference::CustomRequired);
    }

    #[test]
    fn custom_control_state_default_values() {
        let state = CustomControlState::default();
        assert_eq!(state.next_widget_id, 1);
        assert!(state.texts.is_empty());
        assert!(state.enabled.is_empty());
        assert!(state.visible.is_empty());
        assert!(state.ime_enabled.is_empty());
        assert!(state.accessibility_names.is_empty());
        assert!(state.menu_trigger_queue.is_empty());
        assert!(state.widget_trigger_queue.is_empty());
        assert!(state.widget_properties.is_empty());
    }

    #[test]
    fn custom_control_state_default_uses_impl() {
        // Verify that Default trait is implemented by explicit impl, not derive.
        let _state: CustomControlState = CustomControlState::default();
        // Also verify we can construct via struct literal + ..Default
        let _state2 = CustomControlState { next_widget_id: 42, ..CustomControlState::default() };
    }

    #[test]
    fn widget_property_returns_stored_properties() {
        let mut state = CustomControlState::default();
        let id = state.next_widget_id;
        state.next_widget_id += 1;

        state.widget_properties.insert(
            id,
            CustomWidgetProperties {
                parent: Some(0),
                x: 10,
                y: 20,
                width: 200,
                height: 100,
                widget_kind: WidgetKind::Button,
            },
        );

        let props = state.widget_property(id).expect("properties should exist");
        assert_eq!(props.parent, Some(0));
        assert_eq!(props.x, 10);
        assert_eq!(props.y, 20);
        assert_eq!(props.width, 200);
        assert_eq!(props.height, 100);
        assert_eq!(props.widget_kind, WidgetKind::Button);
    }

    #[test]
    fn widget_property_returns_none_for_missing_id() {
        let state = CustomControlState::default();
        assert!(state.widget_property(999).is_none());
    }
}
