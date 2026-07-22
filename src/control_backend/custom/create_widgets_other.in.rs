macro_rules! impl_other_widgets {
    () => {
        #[cfg(not(feature = "mini"))]
        fn create_canvas(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "Canvas".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Canvas,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_grid(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "Grid".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Grid,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_chart(&self, parent: ObjectId, x: i32, y: i32, width: u32, height: u32) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "Chart".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Chart,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebView".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineView,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_activity_indicator(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "ActivityIndicator".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ActivityIndicator,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_lcd_number(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, "0".to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "LCDNumber".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::LCDNumber,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_view(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineView".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineView,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_page(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEnginePage".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEnginePage,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_settings(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineSettings".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineSettings,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_download_item(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineDownloadItem".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineDownloadItem,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_cookie_store(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineCookieStore".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineCookieStore,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_web_channel(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineWebChannel".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineWebChannel,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_find_text_result(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineFindTextResult".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineFindTextResult,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_notification(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineNotification".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineNotification,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_script_dialog(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineScriptDialog".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineScriptDialog,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_web_engine_context_menu_request(
            &self,
            parent: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, "WebEngineContextMenuRequest".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::WebEngineContextMenuRequest,
                },
            );
            widget_id
        }
    };
}
