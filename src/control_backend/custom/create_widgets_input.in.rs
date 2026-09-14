// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_input_widgets {
    () => {
        fn create_line_edit(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::LineEdit, parent, text, x, y, width, height)
        }
        fn create_slider(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Slider, parent, "", x, y, width, height)
        }
        fn create_progress_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ProgressBar, parent, "", x, y, width, height)
        }
        fn create_combo_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ComboBox, parent, "", x, y, width, height)
        }
        fn create_list_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ListBox, parent, "", x, y, width, height)
        }
        fn create_scroll_bar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::ScrollBar, parent, "", x, y, width, height)
        }
        fn create_spin_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::SpinBox, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_text_edit(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TextEdit, parent, text, x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_rich_edit(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RichEdit, parent, text, x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_check_list_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CheckListBox, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_double_spin_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DoubleSpinBox, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_dial(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Dial, parent, "", x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_command_link(
            &self,
            parent: ObjectId,
            text: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::CommandLink, parent, text, x, y, width, height)
        }
        #[cfg(widgets_unstripped)]
        fn create_font_combo_box(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::FontComboBox, parent, "", x, y, width, height)
        }
    };
}
