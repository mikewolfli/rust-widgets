// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_dialog_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Dialog, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_message_box(
            &self,
            parent: ObjectId,
            title: &str,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let id = self.mount_widget_of_kind(
                WidgetKind::MessageBox,
                parent,
                title,
                x,
                y,
                width,
                height,
            );
            // A message box carries two strings; the factory seeds the title, so the
            // body is written through the property contract afterwards. Writing it
            // here — rather than leaving the parameter unused — is what makes the
            // dialog show the message the caller passed.
            if id != 0 {
                // A message box whose body write fails would otherwise open empty with
                // no trace. Report it (the same way `set_widget_text` in the sibling
                // helper module does) and keep creating the box: the title is already
                // correct, and the host can still show something.
                if let Err(error) = crate::widget::capability::write_widget_property_by_id(
                    id,
                    "text",
                    crate::widget::capability::CapabilityValue::String(text.to_string()),
                ) {
                    log::warn!("custom backend: messagebox body write failed: {error:?}");
                }

                // A message box starts **hidden**: it is a modal dialog, so creating it
                // is not the same as showing it.
                //
                // A `BaseWidget` starts visible, which is right for a control placed on
                // a form — it should appear with the window. It is wrong for a dialog:
                // the caller's next step is `show_modal()`, and between the two calls the
                // box would be sitting on top of the window, blocking nothing and looking
                // like a stray panel. Measured on the control demo, the freshly created
                // box was painted over the form before any button was pressed.
                crate::widget::runtime::with_widget_mut(id, |widget| {
                    widget.set_visible(false);
                });
            }
            id
        }
        #[cfg(not(alloc_frugal))]
        fn create_file_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FileDialog, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_color_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ColorDialog, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_font_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FontDialog, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_popup_window(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::PopupWindow, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_wizard(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Wizard, parent, title, x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_directory_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::DirectoryDialog,
                parent,
                title,
                x,
                y,
                width,
                height,
            )
        }
        #[cfg(full_widgets)]
        fn create_alert_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AlertDialog, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_confirm_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ConfirmDialog, parent, "", x, y, width, height)
        }
        #[cfg(full_widgets)]
        fn create_about_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::AboutDialog, parent, "", x, y, width, height)
        }
    };
}
