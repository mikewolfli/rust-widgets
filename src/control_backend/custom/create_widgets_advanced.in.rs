macro_rules! impl_advanced_widgets {
    () => {
        #[cfg(not(feature = "mini"))]
        fn create_calendar(
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
            state.accessibility_names.insert(widget_id, "Calendar".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::Calendar,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_date_picker(
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
            state.accessibility_names.insert(widget_id, "DatePicker".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::DatePicker,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_time_picker(
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
            state.accessibility_names.insert(widget_id, "TimePicker".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::TimePicker,
                },
            );
            widget_id
        }
        #[cfg(not(feature = "mini"))]
        fn create_date_time_picker(
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
            state.accessibility_names.insert(widget_id, "DateTimePicker".to_string());
            state.widget_properties.insert(
                widget_id,
                CustomWidgetProperties {
                    parent: Some(parent),
                    x,
                    y,
                    width,
                    height,
                    widget_kind: WidgetKind::DateTimePicker,
                },
            );
            widget_id
        }
    };
}
