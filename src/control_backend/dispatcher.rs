#[cfg(feature = "controls-custom")]
use std::sync::OnceLock;
#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
#[cfg(feature = "controls-native")]
use crate::control_backend::native::NativeControlBackend;
use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::{ControlBackendKind, ControlRoutePreference};
use crate::control_backend::routing::route_preference_for_widget_kind;
use crate::widget::WidgetKind;
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
