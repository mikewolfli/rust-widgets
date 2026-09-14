// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

// `LABEL_PROPERTY_NAMES` is used by the label accessors, which are only compiled
// where the property registry exists.
#[cfg(widgets_unstripped)]
use crate::control_backend::custom::LABEL_PROPERTY_NAMES;
use crate::control_backend::trait_def::ControlBackend;
use crate::control_backend::types::ControlBackendKind;
use crate::core::ObjectId;
use crate::platform::{WidgetTriggerEvent, WidgetTriggerKind};
use crate::widget::WidgetKind;

// Pull in per-category macros that define method bodies.
include!("create_widgets_base.in.rs");
include!("create_widgets_input.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_view.in.rs");
include!("create_widgets_container.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_dialog.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_menu.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_advanced.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_other.in.rs");
#[cfg(not(embedded_surface))]
include!("create_widgets_modern.in.rs");
include!("create_widgets_helpers.in.rs");
impl ControlBackend for super::CustomPaintControlBackend {
    impl_base_widgets!();
    impl_input_widgets!();
    #[cfg(not(embedded_surface))]
    impl_view_widgets!();
    impl_container_widgets!();
    #[cfg(not(embedded_surface))]
    impl_dialog_widgets!();
    #[cfg(not(embedded_surface))]
    impl_menu_widgets!();
    #[cfg(not(embedded_surface))]
    impl_advanced_widgets!();
    #[cfg(not(embedded_surface))]
    impl_other_widgets!();
    #[cfg(not(embedded_surface))]
    impl_modern_widgets!();
    // The route-matrix generator derives `create_qrcode` from `WidgetKind::QRCode`
    // (camel-to-snake of "QRCode" yields "qrcode"), whereas the canonical API name
    // is `create_qr_code`. Provide both; the alias delegates to the canonical one.
    #[cfg(widgets_unstripped)]
    fn create_qrcode(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
        self.create_qr_code(parent, x, y, width, height)
    }

    /// Drops every piece of host state this backend keeps for `widget_id`.
    ///
    /// The widget object itself is released through
    /// [`crate::widget::runtime::unregister`], which owns it; what remains here is
    /// the IME policy and accessible-name override, which no widget holds.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // `widget::runtime` is compiled out of the alloc-frugal profile, which
        // holds no widget objects by design; only the host-side maps remain there.
        #[cfg(not(alloc_frugal))]
        let released = crate::widget::runtime::unregister(widget_id);
        #[cfg(alloc_frugal)]
        let released = false;

        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let had_host_state = state.ime_enabled.remove(&widget_id).is_some()
            || state.accessibility_names.remove(&widget_id).is_some();
        released || had_host_state
    }
    impl_helpers!();
}
