// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_advanced_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_calendar(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Calendar, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_date_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DatePicker, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_time_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::TimePicker, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_date_time_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::DateTimePicker, parent, "", x, y, width, height)
        }
    };
}
