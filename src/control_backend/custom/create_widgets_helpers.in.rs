// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

macro_rules! impl_helpers {
    () => {
        fn poll_widget_trigger_event(&self) -> Option<WidgetTriggerEvent> {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .widget_trigger_queue
                .pop_front()
        }
        fn inject_widget_trigger_event(
            &self,
            widget_id: ObjectId,
            kind: WidgetTriggerKind,
        ) -> bool {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            state.widget_trigger_queue.push_back(WidgetTriggerEvent { widget_id, kind });
            true
        }

        /// Writes `text` to the mounted widget, and repaints it.
        ///
        /// Reads and writes go to the widget's own state rather than a shadow copy,
        /// so a property cannot disagree with the control it describes (BLUE15
        /// §10.3: the shadow maps were one of three duplicated truths).
        fn set_widget_text(&self, widget_id: ObjectId, text: &str) {
            let _ = crate::widget::capability::write_widget_property_by_id(
                widget_id,
                "text",
                crate::widget::capability::CapabilityValue::String(text.to_string()),
            );
        }

        /// Reads the mounted widget's `text` property.
        fn get_widget_text(&self, widget_id: ObjectId) -> String {
            match crate::widget::capability::read_widget_property_by_id(widget_id, "text") {
                Ok(crate::widget::capability::CapabilityValue::String(text)) => text,
                _ => String::new(),
            }
        }

        fn set_widget_enabled(&self, widget_id: ObjectId, enabled: bool) {
            self.with_live_widget(widget_id, |widget| widget.set_enabled(enabled));
            crate::widget::runtime::request_repaint(widget_id);
        }

        fn is_widget_enabled(&self, widget_id: ObjectId) -> bool {
            self.with_live_widget(widget_id, |widget| widget.is_enabled()).unwrap_or(false)
        }

        fn set_widget_visible(&self, widget_id: ObjectId, visible: bool) {
            self.with_live_widget(widget_id, |widget| widget.set_visible(visible));
            crate::widget::runtime::request_repaint(widget_id);
        }

        fn is_widget_visible(&self, widget_id: ObjectId) -> bool {
            self.with_live_widget(widget_id, |widget| widget.is_visible()).unwrap_or(false)
        }

        fn set_widget_geometry(
            &self,
            widget_id: ObjectId,
            x: i32,
            y: i32,
            width: u32,
            height: u32,
        ) {
            if crate::widget::runtime::set_geometry(
                widget_id,
                crate::core::Rect::new(x, y, width, height),
            ) {
                crate::widget::runtime::request_repaint(widget_id);
            }
        }

        /// Marks whether the mounted widget accepts IME input.
        ///
        /// IME enablement is not a widget property (it describes how the host
        /// routes composition events to the control), so it is kept in the
        /// backend's own state rather than on the widget.
        fn set_widget_ime_enabled(&self, widget_id: ObjectId, enabled: bool) -> bool {
            if !crate::widget::runtime::is_mounted(widget_id) {
                return false;
            }
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
            if !crate::widget::runtime::is_mounted(widget_id) {
                return false;
            }
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
