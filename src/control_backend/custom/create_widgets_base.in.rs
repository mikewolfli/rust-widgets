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

            self.mount_widget_of_kind(WidgetKind::Window, 0, title, x, y, width, height)

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
            self.mount_widget_of_kind(WidgetKind::Panel, parent, "", x, y, width, height)
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
