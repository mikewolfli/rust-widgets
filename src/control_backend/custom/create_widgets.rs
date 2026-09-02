use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::{ControlBackendKind, CustomWidgetProperties};
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;

// Pull in per-category macros that define method bodies.
include!("create_widgets_base.in.rs");
include!("create_widgets_input.in.rs");
include!("create_widgets_view.in.rs");
include!("create_widgets_container.in.rs");
include!("create_widgets_dialog.in.rs");
include!("create_widgets_menu.in.rs");
include!("create_widgets_advanced.in.rs");
include!("create_widgets_other.in.rs");
include!("create_widgets_modern.in.rs");
include!("create_widgets_helpers.in.rs");

/// Full ControlBackend implementation for CustomPaintControlBackend.
/// Non-core methods inherit default implementations from the trait.
///
/// Method bodies are organized into per-category include files for maintainability.
impl ControlBackend for super::CustomPaintControlBackend {
    impl_base_widgets!();
    impl_input_widgets!();
    impl_view_widgets!();
    impl_container_widgets!();
    impl_dialog_widgets!();
    impl_menu_widgets!();
    impl_advanced_widgets!();
    impl_other_widgets!();
    impl_modern_widgets!();
    // The route-matrix generator derives `create_qrcode` from `WidgetKind::QRCode`
    // (camel-to-snake of "QRCode" yields "qrcode"), whereas the canonical API name
    // is `create_qr_code`. Provide both; the alias delegates to the canonical one.
    #[cfg(not(feature = "mini"))]
    fn create_qrcode(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_qr_code(parent, x, y, width, height)
    }
    impl_helpers!();
}
