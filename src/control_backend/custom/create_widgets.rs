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

    /// Creates a widget by factory name, reaching every registered control.
    ///
    /// # Why this override exists
    ///
    /// The trait's default returns `0`, and the typed `create_*` methods only
    /// cover the kinds somebody wrote a method for. A caller holding a *name* — a
    /// binding layer, a config file, the capability matrix — had no way to reach
    /// the other 130-odd controls. Resolving through `WidgetFactory` makes the name
    /// list the single source of truth: registration is the only step needed to
    /// make a control reachable, which is what
    /// `tools/check_widget_registration_fidelity.sh` enforces.
    ///
    /// The name is normalized by the factory (case- and separator-insensitive), so
    /// `"TreeView"`, `"tree_view"` and `"treeview"` all resolve, and aliases such
    /// as `"nav_breadcrumb"` work without a table here.
    fn create_widget(
        &self,
        kind: &str,
        parent: ObjectId,
        text: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> ObjectId {
        #[cfg(full_widgets)]
        {
            let factory = crate::widget::WidgetFactory::new_with_defaults();
            let Some(capability) = factory.capability(kind) else {
                log::warn!(
                    "custom backend: no control is registered under the name {kind:?}; returning 0"
                );
                return 0;
            };
            // Mount by the *matched name*, not by the kind. Several controls share
            // a kind (`chart` / `timeline_widget` / `gantt_widget` all report
            // `WidgetKind::Chart`), and `mount_widget_of_kind` resolves a kind back
            // to a single canonical name — so going through the kind would build
            // whichever of the siblings registered first, and the caller would get a
            // different control than the one it named.
            self.mount_named_widget(capability.canonical_name, parent, text, x, y, width, height)
        }
        #[cfg(not(full_widgets))]
        {
            let _ = (kind, parent, text, x, y, width, height);
            log::warn!(
                "custom backend: name-based creation is unavailable in this profile (no \
                 constructor registry is compiled in); returning 0"
            );
            0
        }
    }
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
    /// `widget::runtime::unregister`, which owns it; what remains here is
    /// the IME policy, the accessible-name override and the last reported client size,
    /// none of which a widget holds.
    ///
    /// The client size **must** be dropped here, and that is why it is named explicitly:
    /// it is keyed by widget id, and a destroyed id can be handed out again (the registry
    /// counts ids per thread from a fixed start). Leaving the entry behind would make a
    /// brand-new window open at the size of a *different*, long-gone window, because
    /// `window_client_size` answers from this map before falling back to geometry.
    fn destroy_widget(&self, widget_id: ObjectId) -> bool {
        // `widget::runtime` is compiled out of the alloc-frugal profile, which
        // holds no widget objects by design; only the host-side maps remain there.
        #[cfg(not(alloc_frugal))]
        let released = crate::widget::runtime::unregister(widget_id);
        #[cfg(alloc_frugal)]
        let released = false;

        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let had_host_state = state.ime_enabled.remove(&widget_id).is_some()
            || state.accessibility_names.remove(&widget_id).is_some()
            || state.window_client_sizes.remove(&widget_id).is_some();
        released || had_host_state
    }
    impl_helpers!();
}
