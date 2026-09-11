#[cfg(feature = "controls-custom")]
use crate::compat::OnceLock;
#[cfg(feature = "controls-custom")]
use crate::control_backend::custom::CustomPaintControlBackend;
#[cfg(all(feature = "controls-native", not(any(feature = "mini", feature = "embedded"))))]
use crate::control_backend::native::NativeControlBackend;
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    feature = "controls-custom"
))]
use crate::control_backend::routing::route_preference_for_widget_kind;
use crate::control_backend::trait_def::ControlBackend;
#[cfg(all(
    feature = "controls-native",
    feature = "controls-custom",
    not(any(feature = "mini", feature = "embedded"))
))]
use crate::control_backend::types::ControlRoutePreference;
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
use crate::core::ObjectId;
use crate::widget::WidgetKind;

#[cfg(all(feature = "controls-native", not(any(feature = "mini", feature = "embedded"))))]
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
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    feature = "controls-custom"
))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(
    not(feature = "controls-native"),
    feature = "controls-custom",
    not(any(feature = "mini", feature = "embedded"))
))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Return active control backend selected by compile-time features.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    not(feature = "controls-custom")
))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    native_control_backend()
}
/// No backend enabled at all (no native, no custom).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    no_control_backend()
}
/// Mini mode uses custom backend.
#[cfg(all(any(feature = "mini", feature = "embedded"), feature = "controls-custom"))]
pub fn get_control_backend() -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
///
/// # Create-time vs id-space semantics
///
/// In the hybrid (`controls-native` + `controls-custom`) profile this is the
/// canonical **create-time** selection entry: it consults
/// [`route_preference_for_widget_kind`] so a widget whose kind is
/// `CustomRequired` (e.g. self-drawn / native-surrogate kinds) resolves to the
/// custom backend instead of blindly going native.
///
/// Existing-widget operations (state setters, item APIs, menu/status APIs)
/// operate on a handle that was created by exactly one backend. Those callers
/// use [`get_control_backend`] (native-first id-space) — see its doc comment
/// for why the two entries do not conflict.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    feature = "controls-custom"
))]
pub fn get_control_backend_for_widget(kind: WidgetKind) -> &'static dyn ControlBackend {
    resolve_control_backend_for_kind(kind).0
}

/// Canonical hybrid resolution chain (single source of truth).
///
/// Every per-kind resolver (public [`get_control_backend_for_widget`] and any
/// future create-time caller) must route through this function so that the
/// native/custom choice for a widget kind can never drift between call sites.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    feature = "controls-custom"
))]
pub(crate) fn resolve_control_backend_for_kind(
    kind: WidgetKind,
) -> (&'static dyn ControlBackend, ControlRoutePreference) {
    let preference = route_preference_for_widget_kind(kind);
    let backend: &'static dyn ControlBackend = match preference {
        ControlRoutePreference::NativePreferred => native_control_backend(),
        ControlRoutePreference::CustomRequired => custom_control_backend(),
    };
    (backend, preference)
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(
    not(feature = "controls-native"),
    feature = "controls-custom",
    not(any(feature = "mini", feature = "embedded"))
))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    not(feature = "controls-custom")
))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    native_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind (no backend available).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    no_control_backend()
}
/// Returns control backend resolved by compile-time policy for one widget kind (mini mode).
#[cfg(all(any(feature = "mini", feature = "embedded"), feature = "controls-custom"))]
pub fn get_control_backend_for_widget(_kind: WidgetKind) -> &'static dyn ControlBackend {
    custom_control_backend()
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    feature = "controls-custom"
))]
pub fn active_control_policy() -> &'static str {
    "hybrid-native-first"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(
    not(feature = "controls-native"),
    feature = "controls-custom",
    not(any(feature = "mini", feature = "embedded"))
))]
pub fn active_control_policy() -> &'static str {
    "custom-full"
}
/// Return compile-time control policy label used by diagnostics and docs.
#[cfg(all(
    feature = "controls-native",
    not(any(feature = "mini", feature = "embedded")),
    not(feature = "controls-custom")
))]
pub fn active_control_policy() -> &'static str {
    "native-strict"
}
/// Return compile-time control policy label used by diagnostics and docs (no backend, or mini without custom).
#[cfg(all(not(feature = "controls-native"), not(feature = "controls-custom")))]
pub fn active_control_policy() -> &'static str {
    "none"
}
/// Return compile-time control policy label used by diagnostics and docs (mini mode).
#[cfg(all(any(feature = "mini", feature = "embedded"), feature = "controls-custom"))]
pub fn active_control_policy() -> &'static str {
    "mini-custom"
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    use crate::widget::WidgetKind;

    #[test]
    fn get_control_backend_returns_valid_backend() {
        let backend = get_control_backend();
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty");
        let kind = backend.kind();
        let _ = format!("{:?}", kind);
    }

    #[cfg(not(any(feature = "mini", feature = "embedded")))]
    #[test]
    fn get_control_backend_for_widget_returns_non_null() {
        let backend = get_control_backend_for_widget(WidgetKind::Button);
        let name = backend.backend_name();
        assert!(!name.is_empty(), "backend_name must not be empty for Button");
        let backend2 = get_control_backend_for_widget(WidgetKind::Canvas);
        let name2 = backend2.backend_name();
        assert!(!name2.is_empty(), "backend_name must not be empty for Canvas");
    }

    #[cfg(not(any(feature = "mini", feature = "embedded")))]
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

    #[cfg(all(
        feature = "controls-native",
        feature = "controls-custom",
        not(any(feature = "mini", feature = "embedded"))
    ))]
    #[test]
    fn resolve_control_backend_parity_with_per_widget_entry() {
        // get_control_backend_for_widget must be the canonical chain — both
        // entries resolve to the exact same backend for every sampled kind.
        use crate::control_backend::routing::route_preference_for_widget_kind;
        let sample = [
            WidgetKind::Window,
            WidgetKind::MessageBox,
            WidgetKind::Button,
            WidgetKind::TextEdit,
            WidgetKind::Canvas,
            WidgetKind::Arc,
            WidgetKind::Spinner,
            WidgetKind::Roller,
            WidgetKind::Dropdown,
            WidgetKind::TextArea,
            WidgetKind::Keyboard,
            WidgetKind::TileView,
            WidgetKind::Line,
            WidgetKind::Meter,
            WidgetKind::MiniChart,
            WidgetKind::ImageView,
            WidgetKind::MiniCanvas,
            WidgetKind::MenuBar,
            WidgetKind::StatusBar,
        ];
        for kind in &sample {
            let (resolved, _pref) = resolve_control_backend_for_kind(*kind);
            let via_entry = get_control_backend_for_widget(*kind);
            assert!(
                std::ptr::eq(resolved, via_entry),
                "canonical chain and get_control_backend_for_widget diverged for {:?}",
                kind,
            );
            let pref = route_preference_for_widget_kind(*kind);
            match pref {
                ControlRoutePreference::NativePreferred => {
                    assert_eq!(
                        resolved.kind(),
                        crate::control_backend::types::ControlBackendKind::Native,
                        "{:?} routed NativePreferred but resolved to non-native backend",
                        kind,
                    );
                }
                ControlRoutePreference::CustomRequired => {
                    assert_eq!(
                        resolved.kind(),
                        crate::control_backend::types::ControlBackendKind::Custom,
                        "{:?} routed CustomRequired but resolved to non-custom backend",
                        kind,
                    );
                }
            }
        }
    }

    #[cfg(all(
        feature = "controls-native",
        feature = "controls-custom",
        not(any(feature = "mini", feature = "embedded"))
    ))]
    #[test]
    fn custom_paint_kinds_never_resolve_native_backend() {
        // Regression guard for the historical silent-0 class: self-drawn
        // widgets (Arc/Spinner/…) must resolve to the custom backend, never to
        // a native backend that lacks a create path for them.
        let custom_paint = [
            WidgetKind::Arc,
            WidgetKind::Spinner,
            WidgetKind::Roller,
            WidgetKind::Dropdown,
            WidgetKind::TextArea,
            WidgetKind::Keyboard,
            WidgetKind::TileView,
            WidgetKind::Line,
            WidgetKind::Meter,
            WidgetKind::MiniChart,
            WidgetKind::ImageView,
            WidgetKind::MiniCanvas,
        ];
        for kind in &custom_paint {
            let backend = get_control_backend_for_widget(*kind);
            assert_eq!(
                backend.kind(),
                crate::control_backend::types::ControlBackendKind::Custom,
                "self-drawn WidgetKind::{:?} must resolve to custom backend",
                kind,
            );
        }
    }

    /// End-to-end guard for the 2026-09-11 routing change: kinds whose native
    /// path silently degrades to a different control must resolve to the custom
    /// backend at create time, otherwise creating e.g. a `DatePicker` would
    /// return a `Panel` with no date functionality.
    #[cfg(all(
        feature = "controls-native",
        feature = "controls-custom",
        not(any(feature = "mini", feature = "embedded"))
    ))]
    #[test]
    fn native_degraded_kinds_resolve_to_custom_backend() {
        let degraded = [
            WidgetKind::DatePicker,
            WidgetKind::TimePicker,
            WidgetKind::DateTimePicker,
            WidgetKind::Calendar,
            WidgetKind::ActivityIndicator,
            WidgetKind::Dial,
            WidgetKind::LCDNumber,
            WidgetKind::FontComboBox,
            WidgetKind::DoubleSpinBox,
            WidgetKind::ToggleButton,
            WidgetKind::ScrollBar,
            WidgetKind::ScrollArea,
            WidgetKind::TabWidget,
            WidgetKind::Splitter,
            WidgetKind::GroupBox,
            WidgetKind::Frame,
            WidgetKind::ContextMenu,
            WidgetKind::MenuItem,
            WidgetKind::DirectoryDialog,
            WidgetKind::Dialog,
            WidgetKind::InputDialog,
            WidgetKind::ProgressDialog,
            WidgetKind::PopupWindow,
        ];
        for kind in &degraded {
            let backend = get_control_backend_for_widget(*kind);
            assert_eq!(
                backend.kind(),
                crate::control_backend::types::ControlBackendKind::Custom,
                "WidgetKind::{:?} degrades on the native path and must resolve to \
                 the custom backend",
                kind,
            );
        }
    }
}
