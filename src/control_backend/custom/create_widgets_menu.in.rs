macro_rules! impl_menu_widgets {
    () => {
        #[cfg(not(feature = "mini"))]
        fn create_menu_bar(
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
            state.accessibility_names.insert(widget_id, "MenuBar".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::MenuBar,
                },
            );
            widget_id
        }
        fn create_menu(
            &self,
            parent: ObjectId,
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
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    #[cfg(not(feature = "mini"))]
                    widget_kind: WidgetKind::Menu,
                    #[cfg(feature = "mini")]
                    widget_kind: WidgetKind::Panel,
                },
            );
            widget_id
        }
        fn attach_menu_bar_to_window(&self, _window: ObjectId, _menu_bar: ObjectId) -> bool {
            true
        }
        fn menu_add_item(
            &self,
            parent_menu: ObjectId,
            text: &str,
            _shortcut: Option<&str>,
        ) -> ObjectId {
            let widget_id = self.alloc_widget_id();
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.texts.insert(widget_id, text.to_string());
            state.enabled.insert(widget_id, true);
            state.visible.insert(widget_id, true);
            state.ime_enabled.insert(widget_id, false);
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent_menu),
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 0,
                    #[cfg(not(feature = "mini"))]
                    widget_kind: WidgetKind::MenuItem,
                    #[cfg(feature = "mini")]
                    widget_kind: WidgetKind::Panel,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_tool_bar(
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
            state.accessibility_names.insert(widget_id, "ToolBar".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ToolBar,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_status_bar(
            &self,
            parent: ObjectId,
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
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::StatusBar,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_action(
            &self,
            parent: ObjectId,
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
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Action,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_tool_button(
            &self,
            parent: ObjectId,
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
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ToolButton,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_tool_box(
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
            state.accessibility_names.insert(widget_id, "ToolBox".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Toolbox,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_context_menu(
            &self,
            parent: ObjectId,
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
            state.accessibility_names.insert(widget_id, text.to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ContextMenu,
                },
            );
            widget_id
        }

        fn poll_menu_triggered(&self) -> Option<ObjectId> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .menu_trigger_queue
                .pop_front()
        }
        fn inject_menu_trigger(&self, menu_item_id: ObjectId) -> bool {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.menu_trigger_queue.push_back(menu_item_id);
            true
        }
    };
}
