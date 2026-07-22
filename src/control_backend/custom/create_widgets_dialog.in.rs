macro_rules! impl_dialog_widgets {
    () => {
        #[cfg(not(feature = "mini"))]
        fn create_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Dialog,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
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
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, text.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::MessageBox,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_file_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::FileDialog,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_color_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ColorDialog,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_font_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::FontDialog,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_popup_window(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::PopupWindow,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_wizard(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Wizard,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_directory_dialog(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, title.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, title.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::DirectoryDialog,
                },
            );
            widget_id
        }
    };
}
