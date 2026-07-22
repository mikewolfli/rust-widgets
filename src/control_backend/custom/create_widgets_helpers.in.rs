macro_rules! impl_helpers {
    () => {
        fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .widget_trigger_queue
                .pop_front()
        }
        fn inject_widget_trigger_event(&self, widget_id: ObjectId, kind: WidgetTriggerKind) -> bool {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.widget_trigger_queue.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        }
        fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .texts
                .insert(widget_id, text.to_string());
        }
        fn get_widget_text(&self, widget_id: ObjectId) -> String {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .texts
                .get(&widget_id)
                .cloned()
                .unwrap_or_default()
        }
        fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .enabled
                .insert(widget_id, enabled);
        }
        fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .enabled
                .get(&widget_id)
                .copied()
                .unwrap_or(false)
        }
        fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .visible
                .insert(widget_id, visible);
        }
        fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .visible
                .get(&widget_id)
                .copied()
                .unwrap_or(false)
        }
        fn set_widget_geometry(&self, widget_id: ObjectId, x: i32, y: i32, width: u32, height: u32) {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            // Validate the widget exists and read all fields from the getter
            let _has_props = state.widget_property(widget_id).map(|p| {
                let _ = (p.parent, p.widget_kind, p.x, p.y, p.width, p.height);
            });
            if let Some(props) = state.widget_properties.get_mut(&widget_id) {
                props.x = x;
                props.y = y;
                props.width = width;
                props.height = height;
            }
        }
        fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .ime_enabled
                .insert(widget_id, enabled);
            true
        }
        fn is_widget_ime_enabled(&self, widget_id: ObjectId) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .ime_enabled
                .get(&widget_id)
                .copied()
                .unwrap_or(false)
        }
        fn set_widget_accessibility_name(&self, widget_id: ObjectId, name: &str) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .accessibility_names
                .insert(widget_id, name.to_string());
            true
        }
        fn get_widget_accessibility_name(&self, widget_id: ObjectId) -> String {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .accessibility_names
                .get(&widget_id)
                .cloned()
                .unwrap_or_default()
        }
    };
}
