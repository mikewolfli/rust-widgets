// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_other_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_canvas(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Canvas, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_grid(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Grid, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Chart, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_radar_chart(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::RadarChart, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_kanban_board(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::KanbanBoard, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_cascader(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Cascader, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_query_builder(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::QueryBuilder, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_emoji_picker(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::EmojiPicker, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_mention(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Mention, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::WebEngineView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_activity_indicator(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(
                WidgetKind::ActivityIndicator,
                parent,
                "",
                x,
                y,
                width,
                height,
            )
        }
        #[cfg(not(alloc_frugal))]
        fn create_lcd_number(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::LCDNumber, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::WebEngineView, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_page(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            // The web engine's page/settings/download/… names address one view, so
            // they all mount `web_view`. They are deliberately not `WidgetKind`
            // variants: those named nothing any code path could produce, so a kind
            // lookup resolved to the empty constructor name and returned `0`.
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_settings(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_download_item(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_cookie_store(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_web_channel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_find_text_result(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_notification(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_script_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_web_engine_context_menu_request(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            self.mount_named_widget("web_view", parent, "", x, y, width, height)
        }
    };
}
