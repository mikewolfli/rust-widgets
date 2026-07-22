macro_rules! impl_container_widgets {
    () => {
        fn create_scroll_area(
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
            state.accessibility_names.insert(widget_id, "ScrollArea".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::ScrollArea,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_dock_panel(
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
            state.accessibility_names.insert(widget_id, "DockPanel".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::DockPanel,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_tab_widget(
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
            state.accessibility_names.insert(widget_id, "TabWidget".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::TabWidget,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_splitter(
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
            state.accessibility_names.insert(widget_id, "Splitter".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Splitter,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_stack_widget(
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
            state.accessibility_names.insert(widget_id, "StackWidget".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::StackedWidget,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_mdi_area(
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
            state.accessibility_names.insert(widget_id, "MdiArea".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::MdiArea,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_toolbox(
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
            state.accessibility_names.insert(widget_id, "Toolbox".to_string());
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
        fn create_collapsible_pane(
            &self,
            parent: ObjectId,
            title: &str,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) -> ObjectId {
            log::warn!("shallow implementation: CollapsiblePane is an alias for Panel");
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
                    widget_kind: WidgetKind::CollapsiblePane,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_dock_widget(
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
                    widget_kind: WidgetKind::DockWidget,
                },
            );
            widget_id
        }
    };
}
