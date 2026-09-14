// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_other_widgets {
    () => {
        #[cfg(not(alloc_frugal))]
        fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Canvas, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Grid, parent, "", x, y, width, height)
        }
        #[cfg(not(alloc_frugal))]
        fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            self.mount_widget_of_kind(WidgetKind::Chart, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::ActivityIndicator, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEnginePage, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineSettings, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineDownloadItem, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineCookieStore, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineWebChannel, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineFindTextResult, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineNotification, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineScriptDialog, parent, "", x, y, width, height)
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
            self.mount_widget_of_kind(WidgetKind::WebEngineContextMenuRequest, parent, "", x, y, width, height)
        }
    };
}
