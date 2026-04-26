#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
#[cfg(feature = "controls-native")]
use crate::control_backend::native::NativeControlBackend;
use crate::control_backend::routing::route_preference_for_widget_kind;
use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::ControlRoutePreference;
use crate::widget::WidgetKind;
#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;
fn native_control_backend() -> &'static NativeControlBackend {
    static BACKEND: NativeControlBackend = NativeControlBackend::new();
    &BACKEND
}
#[cfg(feature = "controls-custom")]
fn custom_control_backend() -> &'static CustomPaintControlBackend {
    static BACKEND: OnceLock<CustomPaintControlBackend> = OnceLock::new();
    BACKEND.get_or_init(CustomPaintControlBackend::new)
}
/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn get_control_backend_for_widget(kind: WidgetKind) -> &'static dyn ControlBackend {
    match route_preference_for_widget_kind(kind) {
        ControlRoutePreference::NativePreferred => native_control_backend(),
        ControlRoutePreference::CustomRequired => custom_control_backend(),
    }
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "hybrid-native-first"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "custom-full"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(feature = "controls-native", not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::WidgetKind;

    #[test]
    fn get_control_backend_returns_valid_backend() {
        let backend = get_control_backend();
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
        let kind = backend.kind();
        // With "desktop" feature, both native and custom are enabled.
        // Default is native in hybrid mode.
        let _ = format!("{:?}", kind);
    }

    #[test]
    fn get_control_backend_for_widget_returns_non_null() {
        let backend = get_control_backend_for_widget(WidgetKind::Button);
        let name = backend.backend_name();
        assert!(
            !name.is_empty(),
            "backend_name must not be empty for Button"
        );
        let backend2 = get_control_backend_for_widget(WidgetKind::Canvas);
        let name2 = backend2.backend_name();
        assert!(
            !name2.is_empty(),
            "backend_name must not be empty for Canvas"
        );
    }

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
            assert!(
                !name.is_empty(),
                "backend_name must not be empty for {:?}",
                kind
            );
            let _ = backend.kind();
        }
    }

    #[test]
    fn active_control_policy_returns_expected_string() {
        let policy = active_control_policy();
        assert!(!policy.is_empty(), "policy must not be empty");
        // With "desktop" feature, both native and custom are enabled -> "hybrid-native-first"
        #[cfg(all(feature = "controls-native", feature = "controls-custom"))]
        assert_eq!(policy, "hybrid-native-first");
        #[cfg(all(not(feature = "controls-native"), feature = "controls-custom"))]
        assert_eq!(policy, "custom-full");
        #[cfg(feature = "controls-native")]
        #[cfg(not(feature = "controls-custom"))]
        assert_eq!(policy, "native-strict");
        #[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
        assert_eq!(policy, "native-strict");
    }

    #[test]
    fn control_backend_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NativeControlBackend>();
        #[cfg(feature = "controls-custom")]
        assert_send_sync::<CustomPaintControlBackend>();
    }
}
