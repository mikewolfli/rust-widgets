#[cfg(feature = "controls-custom")]
use crate::compat::OnceLock;
#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
#[cfg(all(feature = "controls-native", not(feature = "mini")))]
use crate::control_backend::native::NativeControlBackend;
#[cfg(all(feature = "controls-native", not(feature = "mini"), feature = "controls-custom"))]
use crate::control_backend::routing::route_preference_for_widget_kind;
use crate::control_backend::trait_def::ControlBackend;
#[cfg(all(feature = "controls-native", feature = "controls-custom", not(feature = "mini")))]
use crate::control_backend::types::ControlRoutePreference;
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
use crate::core::ObjectId;
use crate::widget::WidgetKind;

#[cfg(all(feature = "controls-native", not(feature = "mini")))]
fn native_control_backend() -> &'static NativeControlBackend {
    static BACKEND: NativeControlBackend = NativeControlBackend::new();
    &BACKEND
}
#[cfg(feature = "controls-custom")]
fn custom_control_backend() -> &'static CustomPaintControlBackend {
    static BACKEND: OnceLock<CustomPaintControlBackend> = OnceLock::new();
    BACKEND.get_or_init(CustomPaintControlBackend::new)
}
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
struct NoControlBackend;
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
impl crate::control_backend::trait_def::ControlBackend for NoControlBackend {
    fn backend_name(&self) -> &'static str {
        "no-control-backend"
    }
    fn kind(&self) -> crate::control_backend::types::ControlBackendKind {
        crate::control_backend::types::ControlBackendKind::Custom
    }
    fn create_window(&self, _title: &str, _x: i32, _y: i32, _width: u32, _height: u32) -> ObjectId {
        0
    }
    fn create_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_checkbox(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_line_edit(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_label(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_radio_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_slider(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_progress_bar(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_combo_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_list_box(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_panel(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_scroll_area(
        &self,
        _parent: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_group_box(
        &self,
        _parent: ObjectId,
        _title: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn create_toggle_button(
        &self,
        _parent: ObjectId,
        _text: &str,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) -> ObjectId {
        0
    }
    fn set_widget_text(&self, _widget_id: ObjectId, _text: &str) {}
    fn get_widget_text(&self, _widget_id: ObjectId) -> String {
        String::new()
    }
    fn set_widget_enabled(&self, _widget_id: ObjectId, _enabled: bool) {}
    fn is_widget_enabled(&self, _widget_id: ObjectId) -> bool {
        false
    }
    fn set_widget_visible(&self, _widget_id: ObjectId, _visible: bool) {}
    fn is_widget_visible(&self, _widget_id: ObjectId) -> bool {
        false
    }
    fn set_widget_geometry(
        &self,
        _widget_id: ObjectId,
        _x: i32,
        _y: i32,
        _width: u32,
        _height: u32,
    ) {
    }
    fn set_widget_ime_enabled(&self, _widget_id: ObjectId, _enabled: bool) -> bool {
        false
    }
    fn is_widget_ime_enabled(&self, _widget_id: ObjectId) -> bool {
        false
    }
    fn set_widget_accessibility_name(&self, _widget_id: ObjectId, _name: &str) -> bool {
        false
    }
    fn get_widget_accessibility_name(&self, _widget_id: ObjectId) -> String {
        String::new()
    }
}
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
fn no_control_backend() -> &'static NoControlBackend {
    static BACKEND: NoControlBackend = NoControlBackend;
    &BACKEND
}
/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", not(feature = "mini"), feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom", not(feature = "mini")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", not(feature = "mini"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// No backend enabled at all (no native, no custom).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    no_control_backend()
}
/// Mini mode uses custom backend.
#[cfg(all(feature = "mini", feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", not(feature = "mini"), feature = "controls-custom"))]
pub fn get_control_backend_for_widget(kind: WidgetKind) -> &'static dyn ControlBackend {
    match route_preference_for_widget_kind(kind) {
        ControlRoutePreference::NativePreferred => native_control_backend(),
        ControlRoutePreference::CustomRequired => custom_control_backend(),
    }
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom", not(feature = "mini")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", not(feature = "mini"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind (no backend available).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    no_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind (mini mode).
#[cfg(all(feature = "mini", feature = "controls-custom"))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", not(feature = "mini"), feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "hybrid-native-first"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom", not(feature = "mini")))]
pub fn active_control_policy() -> &'static str {
    "custom-full"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", not(feature = "mini"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}
/// Return compile-time control policy label used by diagnostics and docs (no backend, or mini without custom).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "none"
}
/// Return compile-time control policy label used by diagnostics and docs (mini mode).
#[cfg(all(feature = "mini", feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "mini-custom"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "mini"))]
    use crate::widget::WidgetKind;

    #[test]
    fn get_control_backend_returns_valid_backend() {
        let backend = get_control_backend();
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
        let kind = backend.kind();
        let _ = format!("{:?}", kind);
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn get_control_backend_for_widget_returns_non_null() {
        let backend = get_control_backend_for_widget(WidgetKind::Button);
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty for Button");
        let backend2 = get_control_backend_for_widget(WidgetKind::Canvas);
        let name2 = backend2.backend_name();
        assert!(!name2.is_empty(), "backend_name must not be empty for Canvas");
    }

    #[cfg(not(feature = "mini"))]
    #[test]
    fn get_control_backend_for_widget_various_kinds() {
        let kinds = [
            WidgetKind::Window,
            WidgetKind::Button,
            WidgetKind::Label,
            WidgetKind::Canvas,
            WidgetKind::Table,
            WidgetKind::TextEdit,
            WidgetKind::Slider,
            WidgetKind::MenuBar,
        ];
        for kind in &kinds {
            let backend = get_control_backend_for_widget(*kind);
            let name = backend.backend_name();
            assert!(!name.is_empty(), "backend_name must not be empty for {:?}", kind);
            let _ = backend.kind();
        }
    }

    #[test]
    fn active_control_policy_returns_expected_string() {
        let policy = active_control_policy();
        assert!(!policy.is_empty(), "policy must not be empty");
    }

    #[test]
    fn control_backend_is_send_sync() {
        #[cfg(all(feature = "controls-native", feature = "controls-custom"))]
        {
            let backend = get_control_backend();
            let _: &(dyn ControlBackend + Send + Sync) = backend;
        }
    }
}
