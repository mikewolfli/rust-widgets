// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_base_widgets {
    () => {
        fn backend_name(&self) -> &'static str {
            "custom-paint-control-backend"
        }

        fn kind(&self) -> ControlBackendKind {
            ControlBackendKind::Custom
        }

        fn create_window(&self, title: &str, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            let id = self.mount_widget_of_kind(WidgetKind::Window, 0, title, x, y, width, height);
            if id != 0 {
                // A widget id is *reusable*: `widget::runtime` counts ids per thread from a
                // fixed start, so a window created after another was dropped (or created by a
                // different, earlier test on the same thread) can be handed the same id. The
                // recorded client size is keyed by id, so without this the new window would
                // answer `window_client_size` with the *previous* window's size — a stale
                // value that beats the correct geometry fallback and lays the new window's
                // children out at the wrong size.
                //
                // Clearing on creation makes "was this window resized?" a property of the
                // window rather than of the integer that names it.
                self.state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .window_client_sizes
                    .remove(&id);
            }
            id
        }
        fn create_button(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Button, parent, text, x, y, width, height)
        }
        fn create_checkbox(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CheckBox, parent, text, x, y, width, height)
        }
        fn create_label(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Label, parent, text, x, y, width, height)
        }
        fn create_radio_button(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RadioButton, parent, text, x, y, width, height)
        }
        fn create_panel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            // By name: `WidgetKind::Panel` is shared with `breadcrumb`, and the kind
            // lookup answers with whichever of the two is registered first — which was
            // the breadcrumb, so `create_panel` built a navigation trail.
            //
            // The name-based path only exists where the capability registry is
            // compiled in; a stripped profile has no constructors at all and reports
            // `0` from either path, so it keeps the kind-based call rather than
            // referring to a helper it does not compile.
            #[cfg(full_widgets)]
            {
                self.mount_named_widget("panel", parent, "", x, y, width, height)
            }
            #[cfg(not(full_widgets))]
            {
                self.mount_widget_of_kind(WidgetKind::Panel, parent, "", x, y, width, height)
            }
        }
        fn create_group_box(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::GroupBox, parent, title, x, y, width, height)
        }
        fn create_toggle_button(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                crate::control_backend::custom::TOGGLE_BUTTON_KIND,
                parent,
                text,
                x,
                y,
                width,
                height,
            )
        }
    };
}
